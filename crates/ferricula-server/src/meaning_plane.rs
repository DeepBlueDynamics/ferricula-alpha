//! The meaning index on the runtime (R2b): catalogs from the three stores,
//! the background backfill, write-time embedding, and the dense recall arm.
//!
//! Nothing here blocks a write: experience rows and document sections are
//! committed first; embedding them is attempted afterwards and, on any
//! failure (embedder down, busy with a backfill), they stay `pending` for
//! the next backfill. Without an embedder the runtime behaves exactly as
//! before (no dense arm, heuristic novelty).
use super::*;
use std::sync::RwLock;

use crate::config::EmbeddingBackend;
use crate::meaning::{CatalogItem, MeaningCounts, MeaningIndex, MeaningKey, MeaningSet, SearchFilter};
use crate::recall::{ArmList, CandidateKind, DenseHitView, RecallCandidate};

/// Batches between sidecar writes during a backfill (resume granularity).
const PERSIST_EVERY_BATCHES: usize = 8;

#[derive(Debug, Clone, Default, Serialize)]
pub struct BackfillStatus {
    pub running: bool,
    pub trigger: Option<String>,
    pub started_at: Option<u64>,
    pub finished_at: Option<u64>,
    /// Vectors made by the current/last run.
    pub embedded: usize,
    pub failed: usize,
    pub elapsed_ms: Option<u64>,
    pub last_error: Option<String>,
}

/// `/status` → `meaning`.
#[derive(Debug, Clone, Serialize)]
pub struct MeaningStatus {
    /// Dense features usable right now (embedder allowed and index loaded).
    pub enabled: bool,
    #[serde(flatten)]
    pub counts: MeaningCounts,
    pub backfill: BackfillStatus,
    /// Sidecar load notes (ignored files).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

pub(super) struct MeaningPlane {
    active: bool,
    index: RwLock<MeaningIndex>,
    /// Serializes embedding work (backfill and write-time sync).
    work: Mutex<()>,
    backfill: Mutex<BackfillStatus>,
    notes: Vec<String>,
}

impl MeaningPlane {
    pub(super) fn open(config: &RuntimeConfig, memory: &MemoryRuntime) -> Result<Self> {
        let active = config.embeddings.backend != EmbeddingBackend::None;
        let (_, dim) = ferricula_semantic::text_embed::parse_space(&config.embeddings.space)?;
        let dir = active.then(|| config.state_dir.join("meaning"));
        let mut index = MeaningIndex::new(&config.embeddings.space, dim, dir);
        let notes = if active { index.load() } else { Vec::new() };
        for note in &notes {
            eprintln!("meaning: {note}");
        }
        if active {
            let catalog = memory
                .meaning_catalog()
                .into_iter()
                .map(|r| CatalogItem::recovered(r.id, &r.text, r.state != ferricula_core::LifecycleState::Active, r.stored))
                .collect();
            index.set_catalog(MeaningSet::Recovered, catalog);
        }
        Ok(Self {
            active,
            index: RwLock::new(index),
            work: Mutex::new(()),
            backfill: Mutex::new(BackfillStatus::default()),
            notes,
        })
    }
}

/// A dense arm result, ready for fusion.
pub(super) struct DenseArm {
    pub lists: Vec<ArmList>,
    pub views: Vec<DenseHitView>,
    pub novelty: Option<f32>,
}

impl AgentRuntime {
    fn meaning_read(&self) -> std::sync::RwLockReadGuard<'_, MeaningIndex> {
        self.meaning.index.read().expect("meaning index poisoned")
    }

    fn meaning_write(&self) -> std::sync::RwLockWriteGuard<'_, MeaningIndex> {
        self.meaning.index.write().expect("meaning index poisoned")
    }

    pub fn meaning_status(&self) -> MeaningStatus {
        MeaningStatus {
            enabled: self.meaning.active && self.embedder().is_some(),
            counts: self.meaning_read().counts(),
            backfill: self.meaning.backfill.lock().expect("backfill status poisoned").clone(),
            notes: self.meaning.notes.clone(),
        }
    }

    /// Experience rows (Active, with text) as catalog items.
    fn experience_catalog(&self) -> Vec<CatalogItem> {
        self.experience()
            .rows()
            .into_iter()
            .filter(|(_, record)| record.state == ferricula_core::LifecycleState::Active)
            .filter_map(|(row, _)| {
                let text = row.tags.get("text")?;
                (!text.trim().is_empty())
                    .then(|| CatalogItem::experience(row.id, text, row.tags.get("channel").map(String::as_str)))
            })
            .collect()
    }

    /// Every document section as catalog items.
    fn section_catalog(&self) -> Vec<CatalogItem> {
        let store = self.documents_store();
        let mut out = Vec::new();
        for meta in store.list() {
            if let Some(record) = store.document(&meta.doc_id) {
                for section in &record.sections {
                    if !section.text.trim().is_empty() {
                        out.push(CatalogItem::section(&meta.doc_id, section.index, &section.text));
                    }
                }
            }
        }
        out
    }

    /// Re-read the writable stores into the index catalogs (cheap; the
    /// recovered catalog is fixed for the life of the process).
    pub(super) fn meaning_refresh_writable(&self) {
        if !self.meaning.active {
            return;
        }
        let experience = self.experience_catalog();
        let sections = self.section_catalog();
        let mut index = self.meaning_write();
        index.set_catalog(MeaningSet::Experience, experience);
        index.set_catalog(MeaningSet::Section, sections);
    }

    /// Embed `items` in batches; returns (embedded, failed, first error).
    /// Persists the sidecars every few batches and at the end.
    fn meaning_embed_items(
        &self,
        embedder: &dyn ferricula_semantic::text_embed::TextEmbedder,
        items: &[CatalogItem],
        progress: &mut dyn FnMut(usize),
        patient: bool,
    ) -> (usize, usize, Option<String>) {
        let batch = self.config.embeddings.batch.max(1);
        let (mut embedded, mut failed, mut error) = (0usize, 0usize, None);
        let mut consecutive_failures = 0usize;
        for (n, chunk) in items.chunks(batch).enumerate() {
            let texts: Vec<&str> = chunk.iter().map(|i| i.text.as_str()).collect();
            // One retry after a pause: a busy or restarting embedder should
            // not end a long backfill on its first timeout.
            let result = embedder.embed(&texts).or_else(|first| {
                if !patient {
                    return Err(first);
                }
                eprintln!("meaning: batch of {} failed ({first:#}); retrying once", texts.len());
                std::thread::sleep(std::time::Duration::from_secs(5));
                embedder.embed(&texts)
            });
            match result {
                Ok(vectors) => {
                    consecutive_failures = 0;
                    let mut index = self.meaning_write();
                    for (item, vector) in chunk.iter().zip(vectors) {
                        match index.insert(item, vector) {
                            Ok(()) => embedded += 1,
                            Err(e) => {
                                failed += 1;
                                error.get_or_insert_with(|| format!("{:?}: {e:#}", item.key));
                            }
                        }
                    }
                }
                Err(e) => {
                    failed += chunk.len();
                    error = Some(format!("{e:#}"));
                    consecutive_failures += 1;
                    // Patient (backfill): skip this batch (it stays pending)
                    // and go on, unless the embedder looks down for good.
                    if !patient || consecutive_failures >= 3 {
                        break;
                    }
                }
            }
            if (n + 1) % PERSIST_EVERY_BATCHES == 0 {
                if let Err(e) = self.meaning_write().persist() {
                    error = Some(format!("persist: {e:#}"));
                }
            }
            progress(embedded);
        }
        if let Err(e) = self.meaning_write().persist() {
            error = Some(format!("persist: {e:#}"));
        }
        (embedded, failed, error)
    }

    /// Embed every row still without a vector (experience and sections
    /// first, then recovered). Blocking; resumable (sidecars are persisted
    /// every few batches). `progress` sees the counts after each batch.
    pub fn meaning_backfill_blocking(&self, trigger: &str, progress: &mut dyn FnMut(&MeaningCounts)) -> Result<MeaningCounts> {
        if !self.meaning.active {
            bail!("embeddings backend is \"none\"");
        }
        let Some(embedder) = self.embedder() else {
            bail!("embedder not usable (state {:?})", self.embeddings_status().state);
        };
        let _work = self.meaning.work.lock().expect("meaning work poisoned");
        let started = std::time::Instant::now();
        {
            let mut status = self.meaning.backfill.lock().expect("backfill status poisoned");
            *status = BackfillStatus {
                running: true,
                trigger: Some(trigger.to_string()),
                started_at: Some(now()),
                ..BackfillStatus::default()
            };
        }
        let mut total_embedded = 0usize;
        let mut total_failed = 0usize;
        let mut last_error = None;
        // New rows may arrive while we work; a few rounds catch them. A row
        // that fails to embed is not retried in the same run.
        for _round in 0..3 {
            self.meaning_refresh_writable();
            let pending = self.meaning_read().pending(&[MeaningSet::Experience, MeaningSet::Section, MeaningSet::Recovered]);
            if pending.is_empty() {
                break;
            }
            let base = total_embedded;
            let (embedded, failed, error) = self.meaning_embed_items(embedder.as_ref(), &pending, &mut |done| {
                self.meaning.backfill.lock().expect("backfill status poisoned").embedded = base + done;
                progress(&self.meaning_read().counts());
            }, true);
            total_embedded += embedded;
            total_failed += failed;
            if error.is_some() {
                last_error = error;
            }
            if embedded == 0 {
                break;
            }
        }
        let counts = self.meaning_read().counts();
        {
            let mut status = self.meaning.backfill.lock().expect("backfill status poisoned");
            status.running = false;
            status.finished_at = Some(now());
            status.embedded = total_embedded;
            status.failed = total_failed;
            status.elapsed_ms = Some(started.elapsed().as_millis() as u64);
            status.last_error = last_error.clone();
        }
        eprintln!(
            "meaning: backfill ({trigger}) embedded {total_embedded}, failed {total_failed}, pending {}, {} ms{}",
            counts.pending,
            started.elapsed().as_millis(),
            last_error.as_deref().map(|e| format!("; last error: {e}")).unwrap_or_default()
        );
        match last_error {
            Some(error) if total_embedded == 0 && counts.pending > 0 => bail!("backfill made no progress: {error}"),
            _ => Ok(counts),
        }
    }

    /// Start a background backfill unless one is running. Returns whether
    /// one was started.
    pub fn spawn_meaning_backfill(self: &Arc<Self>, trigger: &str) -> bool {
        if !self.meaning.active || self.embedder().is_none() {
            return false;
        }
        {
            let mut status = self.meaning.backfill.lock().expect("backfill status poisoned");
            if status.running {
                return false;
            }
            status.running = true;
        }
        let runtime = self.clone();
        let trigger = trigger.to_string();
        tokio::task::spawn_blocking(move || {
            if let Err(error) = runtime.meaning_backfill_blocking(&trigger, &mut |_| {}) {
                eprintln!("meaning: backfill failed: {error:#}");
                let mut status = runtime.meaning.backfill.lock().expect("backfill status poisoned");
                status.running = false;
                status.last_error = Some(format!("{error:#}"));
            }
        });
        true
    }

    /// Write-time embedding of new experience rows and sections. Never
    /// fails the caller: without an embedder, or while a backfill holds the
    /// work lock, the rows stay pending (the backfill picks them up).
    pub fn meaning_sync_writes(&self) {
        if !self.meaning.active {
            return;
        }
        self.meaning_refresh_writable();
        let Some(embedder) = self.embedder() else { return };
        let Ok(_work) = self.meaning.work.try_lock() else { return };
        let pending = self.meaning_read().pending(&[MeaningSet::Experience, MeaningSet::Section]);
        if pending.is_empty() {
            return;
        }
        let (_, failed, error) = self.meaning_embed_items(embedder.as_ref(), &pending, &mut |_| {}, false);
        if failed > 0 {
            eprintln!(
                "meaning: {failed} new row(s) left pending for backfill: {}",
                error.unwrap_or_default()
            );
        }
    }

    /// [`Self::meaning_sync_writes`] off the async executor.
    pub async fn meaning_after_write(self: &Arc<Self>) {
        let runtime = self.clone();
        let _ = tokio::task::spawn_blocking(move || runtime.meaning_sync_writes()).await;
    }

    /// Embed one text with the usable embedder.
    pub fn embed_text(&self, text: &str) -> Option<Vec<f32>> {
        if !self.meaning.active {
            return None;
        }
        let embedder = self.embedder()?;
        let text = crate::recall::truncate_bytes(text, crate::meaning::MAX_EMBED_BYTES);
        match embedder.embed(&[text]) {
            Ok(mut v) => v.pop(),
            Err(error) => {
                eprintln!("meaning: embed failed: {error:#}");
                None
            }
        }
    }

    /// Embed several texts (one batch call per embedder batch).
    pub fn embed_texts(&self, texts: &[String]) -> Option<Vec<Vec<f32>>> {
        if !self.meaning.active || texts.is_empty() {
            return None;
        }
        let embedder = self.embedder()?;
        let cut: Vec<&str> = texts.iter().map(|t| crate::recall::truncate_bytes(t, crate::meaning::MAX_EMBED_BYTES)).collect();
        embedder.embed(&cut).map_err(|e| eprintln!("meaning: embed failed: {e:#}")).ok()
    }

    pub fn meaning_vector(&self, key: &MeaningKey) -> Option<Arc<Vec<f32>>> {
        self.meaning_read().vector(key)
    }

    /// Neighborhood density (mean cosine to the 5 nearest memories).
    pub fn meaning_density(&self, key: &MeaningKey) -> Option<f64> {
        let faded = self.config.recall.include_faded_recovered;
        self.meaning_read().density(key, 5, &SearchFilter::memories(faded))
    }

    /// Best cosine of a vector over recovered + experience evidence.
    pub fn meaning_max_cosine(&self, vector: &[f32]) -> Option<f64> {
        let faded = self.config.recall.include_faded_recovered;
        self.meaning_read().max_cosine(vector, &SearchFilter::memories(faded))
    }

    pub fn meaning_document_means(&self) -> HashMap<String, Vec<f32>> {
        self.meaning_read().document_means()
    }

    /// The dense and graph arms for `query`, or None when the embedder or
    /// the index is unavailable (degraded mode: lexical recall only).
    pub(super) fn dense_arm(&self, query: &str) -> Option<DenseArm> {
        if !self.meaning.active || self.meaning_read().is_empty() {
            return None;
        }
        // Each sentence of the message is embedded on its own (one call);
        // segment lists are fused weighted by segment novelty.
        let segments = crate::meaning::query_segments(query);
        let vectors = self.embed_texts(&segments)?;
        let cfg = &self.config.recall;
        let faded = cfg.include_faded_recovered;
        let (hits, best) = {
            let index = self.meaning_read();
            let lists: Vec<(Vec<crate::meaning::DenseHit>, f64)> = vectors.iter().map(|v| {
                let best = index.max_cosine(v, &SearchFilter::memories(faded)).unwrap_or(0.0);
                (index.search(v, cfg.dense_k, &SearchFilter::evidence(faded)), 1.0 - best)
            }).collect();
            // The message is as novel as its most novel part.
            let best = lists.iter().map(|(_, n)| 1.0 - n).fold(None, |acc: Option<f64>, b| Some(acc.map_or(b, |a| a.min(b))));
            (crate::meaning::fuse_segments(lists, cfg.dense_k), best)
        };
        // Resolve payloads, keeping dense rank order.
        let recovered_ids: Vec<(u32, f32)> = hits.iter()
            .filter_map(|h| match h.key { MeaningKey::Recovered(id) => Some((id, h.cosine as f32)), _ => None }).collect();
        let experience_ids: Vec<(u32, f32)> = hits.iter()
            .filter_map(|h| match h.key { MeaningKey::Experience(id) => Some((id, h.cosine as f32)), _ => None }).collect();
        let recovered: HashMap<u32, MemoryHit> =
            self.memory.hits_for(&recovered_ids, faded).into_iter().map(|h| (h.id, h)).collect();
        let experience: HashMap<u32, MemoryHit> =
            self.experience().hits_for(&experience_ids).into_iter().map(|h| (h.id, h)).collect();
        let mut dense: Vec<RecallCandidate> = Vec::new();
        let mut views = Vec::new();
        for hit in &hits {
            let candidate = match &hit.key {
                MeaningKey::Recovered(id) => recovered.get(id).map(|h| RecallCandidate::from_memory(CandidateKind::Memory, h, dense.len() + 1)),
                MeaningKey::Experience(id) => experience.get(id).map(|h| RecallCandidate::from_memory(CandidateKind::Experience, h, dense.len() + 1)),
                MeaningKey::Section(doc, index) => self.document_section(doc, *index).map(|mut s| {
                    s.score = hit.cosine;
                    RecallCandidate::from_section(&s, dense.len() + 1)
                }),
            };
            if let Some(mut candidate) = candidate {
                candidate.source_score = hit.cosine;
                views.push(DenseHitView::of("dense", &candidate, hit.cosine, None));
                dense.push(candidate);
            }
        }
        // One graph hop from the top dense recovered hits (scored lower).
        let seeds: Vec<u32> = dense.iter()
            .filter(|c| c.kind == CandidateKind::Memory)
            .filter_map(|c| c.memory.as_ref().map(|m| m.id))
            .take(cfg.graph_seeds)
            .collect();
        let in_dense: std::collections::HashSet<u32> = dense.iter()
            .filter(|c| c.kind == CandidateKind::Memory)
            .filter_map(|c| c.memory.as_ref().map(|m| m.id)).collect();
        let qn: Vec<Vec<f32>> = vectors.iter().map(|v| crate::meaning::unit(v)).collect();
        let hop = crate::meaning::graph_hop(
            &seeds,
            &in_dense,
            |id| self.memory.neighbors(id),
            |id| self.meaning_vector(&MeaningKey::Recovered(id))
                .map(|v| qn.iter().map(|q| crate::meaning::dot(q, &v)).fold(f64::NEG_INFINITY, f64::max))
                .unwrap_or(0.0),
        );
        let via: HashMap<u32, u32> = hop.iter().map(|(id, _, seed)| (*id, *seed)).collect();
        let pairs: Vec<(u32, f32)> = hop.iter().map(|(id, c, _)| (*id, *c as f32)).collect();
        let mut graph: Vec<RecallCandidate> = Vec::new();
        for hit in self.memory.hits_for(&pairs, faded) {
            if hit.tags.get("text").is_some_and(|t| crate::meaning::is_dream_image(t)) {
                continue;
            }
            let candidate = RecallCandidate::from_memory(CandidateKind::Memory, &hit, graph.len() + 1);
            views.push(DenseHitView::of("graph", &candidate, f64::from(hit.score), via.get(&hit.id).copied()));
            graph.push(candidate);
        }
        Some(DenseArm {
            lists: vec![
                ArmList { arm: "dense", weight: 1.0, items: dense },
                ArmList { arm: "graph", weight: cfg.graph_weight, items: graph },
            ],
            views,
            novelty: best.map(|b| (1.0 - b).clamp(0.0, 1.0) as f32),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{InferenceTransport, ProviderRequest, ProviderResponse};
    use ferricula_core::{DurableEngine, MemoryRecord, Row};
    use ferricula_semantic::text_embed::TextEmbedder;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    const DIM: usize = 64;
    const PARENTS: u32 = 900_000_001;

    /// Deterministic "meaning": words map to concept buckets (a tiny
    /// synonym table), stop words are dropped, so "father"/"dad"/"Otto"
    /// land near "raised by Otto and Ilse" without sharing words.
    fn concept_vector(text: &str) -> Vec<f32> {
        const STOP: &[&str] = &["the", "and", "you", "your", "did", "his", "her", "call", "name", "right", "what", "was", "said", "that", "with", "for", "just", "always", "mara"];
        let concept = |w: &str| -> String {
            match w {
                "father" | "dad" | "otto" | "ilse" | "parents" | "raised" | "mother" | "adopted" => "PARENT".into(),
                "bike" | "bikes" | "bicycle" | "bicycles" => "BIKE".into(),
                "names" | "naming" => "name".into(),
                other => other.into(),
            }
        };
        let mut v = vec![0f32; DIM];
        for word in text.split(|c: char| !c.is_alphanumeric()).map(str::to_lowercase).filter(|w| w.len() >= 3) {
            if STOP.contains(&word.as_str()) {
                continue;
            }
            let c = concept(&word);
            v[(crate::meaning::fnv1a64(c.as_bytes()) % DIM as u64) as usize] += 1.0;
        }
        if v.iter().all(|x| *x == 0.0) {
            v[1] = 1.0;
        }
        crate::meaning::unit(&v)
    }

    #[derive(Default)]
    struct FakeEmbedder {
        texts: AtomicUsize,
        fail: AtomicBool,
    }

    impl TextEmbedder for FakeEmbedder {
        fn space(&self) -> &str {
            "fake@64"
        }
        fn dim(&self) -> usize {
            DIM
        }
        fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
            if self.fail.load(Ordering::SeqCst) {
                bail!("connection refused");
            }
            self.texts.fetch_add(texts.len(), Ordering::SeqCst);
            Ok(texts.iter().map(|t| concept_vector(t)).collect())
        }
    }

    #[derive(Default)]
    struct FakeModel {
        prompts: Mutex<Vec<String>>,
    }

    impl InferenceTransport for FakeModel {
        fn execute(&self, request: &ProviderRequest) -> Result<ProviderResponse> {
            let ProviderRequest::OpenAiCompatible { body, .. } = request else { bail!("openai only") };
            let prompt = body.messages.iter().map(|m| m.content.as_str()).collect::<Vec<_>>().join("\n");
            self.prompts.lock().unwrap().push(prompt.clone());
            let text = if prompt.contains("Now you dream") {
                "I walk into the garage where the board hums. Otto hands me a glassblower's file. Comets fall over an unknown sea.\nQUESTION: none".to_string()
            } else {
                "Noted.".to_string()
            };
            Ok(ProviderResponse { text: text.clone(), input_tokens: 10, output_tokens: 5,
                raw: json!({ "choices": [{ "message": { "content": text } }] }) })
        }
    }

    struct Fixture {
        root: PathBuf,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    /// A recovered base shaped like a real one: the parents memory has an
    /// all-zero placeholder vector and is drowned lexically by "Voss"
    /// memories; one row keeps a real stored vector; a v1 dream image and
    /// an archived memory must never be evidence by default.
    fn recovered_base() -> Fixture {
        let root = std::env::temp_dir().join(format!("ferricula-meaning-rt-{}", Uuid::new_v4()));
        let memory = root.join("memory");
        fs::create_dir_all(&memory).unwrap();
        fs::write(memory.join("identity.json"), r#"{"agent_id":"ferricula-agent","name":"T"}"#).unwrap();
        let mut engine = DurableEngine::open(&memory).unwrap();
        let mut put = |id: u32, text: &str, vector: Vec<f32>, archived: bool| {
            let row = Row { id, vector, refs: None, tags: BTreeMap::from([
                ("channel".to_string(), "thinking".to_string()), ("text".to_string(), text.to_string())]) };
            let mut record = MemoryRecord::new(id);
            if archived {
                record.forgive();
                record.archive();
            }
            engine.remember(row, record).unwrap();
        };
        put(PARENTS, "[said] I'm Mara Voss. Born in Bergen, 1961. Raised on the coast by Otto and Ilse Voss, a glassblower and a ferryman.", vec![0.0; DIM], false);
        for i in 0..15u32 {
            put(10 + i, &format!("Voss said you did it right, his call on your name was right, did you see it {i}?"), vec![0.0; DIM], false);
        }
        put(5, "Designing the lamp with Tomas", concept_vector("Designing the lamp with Tomas"), false);
        put(7, "[dream image] my father Otto in a garage full of light", vec![0.0; DIM], false);
        put(8, "My dad Otto fixed cars in the garage; my parents raised me there.", vec![0.0; DIM], true);
        engine.checkpoint().unwrap();
        drop(engine);
        Fixture { root }
    }

    fn open(f: &Fixture, embedder: Option<Arc<FakeEmbedder>>, tweak: impl FnOnce(&mut RuntimeConfig)) -> (Arc<AgentRuntime>, Arc<FakeModel>) {
        let memory = f.root.join("memory");
        let mut config = RuntimeConfig::default();
        config.memory_dir = memory.clone();
        config.state_dir = f.root.join("state");
        config.overlay.path = config.state_dir.join("overlay.json");
        config.schedule.enabled = false;
        config.require_operator_auth = false;
        config.operator_name = "Rowan".into();
        if embedder.is_some() {
            config.embeddings.backend = EmbeddingBackend::Shivvr;
            config.embeddings.space = "fake@64".into();
            config.embeddings.probe = false;
            config.embeddings.batch = 4;
        }
        tweak(&mut config);
        let model = Arc::new(FakeModel::default());
        let inspection = crate::inspect_data_dir(&memory).unwrap();
        let embedder = embedder.map(|e| e as Arc<dyn TextEmbedder>);
        (AgentRuntime::open_with_embedder(config, inspection, model.clone(), embedder).unwrap(), model)
    }

    const QUESTION: &str = "Did you call your father Dad, or by his name, Otto, right?";

    fn memory_ids(recall: &crate::recall::HybridRecall) -> Vec<u32> {
        recall.candidates.iter().filter_map(|c| c.memory.as_ref().map(|m| m.id)).collect()
    }

    #[test]
    fn dense_arm_finds_the_parents_memory_lexical_recall_drowns() {
        let f = recovered_base();
        let fake = Arc::new(FakeEmbedder::default());
        let (runtime, _) = open(&f, Some(fake.clone()), |_| {});
        let before = runtime.meaning_status();
        assert!(before.enabled);
        // 19 rows carry text; one uses its stored vector; 18 wait for a backfill.
        assert_eq!((before.counts.recovered_total, before.counts.recovered_stored, before.counts.pending), (19, 1, 18));
        let counts = runtime.meaning_backfill_blocking("test", &mut |_| {}).unwrap();
        assert_eq!(counts.pending, 0);
        assert_eq!(counts.recovered_embedded, 19);
        assert_eq!(fake.texts.load(Ordering::SeqCst), 18, "stored vectors are not re-embedded");

        let recall = runtime.hybrid_recall(QUESTION, 8);
        assert!(!recall.hits.iter().any(|h| h.id == PARENTS), "lexical alone never reaches it");
        assert_eq!(recall.fusion, "rrf_k60+dense+graph");
        let parents = recall.candidates.iter().find(|c| c.memory.as_ref().is_some_and(|m| m.id == PARENTS))
            .expect("dense recall surfaces the parents memory");
        assert_eq!(parents.arms, ["dense"]);
        assert!(parents.dense_score.unwrap() > 0.3);
        assert_eq!(recall.dense_hits[0].id, Some(PARENTS));
        let ids = memory_ids(&recall);
        assert!(!ids.contains(&7), "v1 dream images are never evidence");
        assert!(!ids.contains(&8), "archived stays out unless faded recall is on");
        assert!(recall.dense_novelty.is_some_and(|n| (0.0..1.0).contains(&n)));

        // Sidecars persisted under state_dir; the recovered dir is untouched.
        assert!(f.root.join("state/meaning/recovered.fmv").exists());
        assert!(!f.root.join("memory/meaning").exists());
        drop(runtime);
        let again = Arc::new(FakeEmbedder::default());
        let (runtime, _) = open(&f, Some(again.clone()), |_| {});
        assert_eq!(runtime.meaning_status().counts.pending, 0, "reloaded, not re-embedded");
        assert_eq!(again.texts.load(Ordering::SeqCst), 0);
        drop(runtime);

        // Faded recall on: the archived memory joins, with its state.
        let (runtime, _) = open(&f, Some(Arc::new(FakeEmbedder::default())), |c| c.recall.include_faded_recovered = true);
        let recall = runtime.hybrid_recall(QUESTION, 8);
        let faded = recall.candidates.iter().find(|c| c.memory.as_ref().is_some_and(|m| m.id == 8)).expect("faded memory");
        assert!(matches!(faded.memory.as_ref().unwrap().state, crate::memory::LifecycleStateView::Archived));
        assert!(!memory_ids(&recall).contains(&7));
    }

    #[test]
    fn without_an_embedder_recall_is_exactly_lexical() {
        let f = recovered_base();
        let (runtime, _) = open(&f, None, |_| {});
        assert!(!runtime.meaning_status().enabled);
        let recall = runtime.hybrid_recall(QUESTION, 8);
        assert_eq!(recall.fusion, "rrf_k60");
        assert!(recall.dense_hits.is_empty() && recall.dense_novelty.is_none());
        let lexical = crate::recall::fuse(&recall.hits, &recall.experience_hits, &recall.section_hits, 8);
        let a: Vec<String> = recall.candidates.iter().map(|c| c.key()).collect();
        let b: Vec<String> = lexical.iter().map(|c| c.key()).collect();
        assert_eq!(a, b);
        assert!(!memory_ids(&recall).contains(&PARENTS));
        assert!(runtime.meaning_backfill_blocking("test", &mut |_| {}).is_err());
        assert!(!f.root.join("state/meaning").exists());
        drop(runtime);

        // A failing embedder degrades the same way and never blocks writes.
        let fake = Arc::new(FakeEmbedder::default());
        fake.fail.store(true, Ordering::SeqCst);
        let (runtime, _) = open(&f, Some(fake.clone()), |_| {});
        let recall = runtime.hybrid_recall(QUESTION, 8);
        assert_eq!(recall.fusion, "rrf_k60");
        runtime.experience().remember("thinking", "A thought about my parents.", BTreeMap::new(), None, 0.5).unwrap();
        runtime.meaning_sync_writes();
        let status = runtime.meaning_status();
        assert_eq!(status.counts.experience_total, 1);
        assert_eq!(status.counts.experience_embedded, 0, "left pending");
        assert!(runtime.meaning_backfill_blocking("test", &mut |_| {}).is_err());
        fake.fail.store(false, Ordering::SeqCst);
        let counts = runtime.meaning_backfill_blocking("test", &mut |_| {}).unwrap();
        assert_eq!((counts.pending, counts.experience_embedded), (0, 1));
    }

    #[tokio::test]
    async fn turns_are_remembered_across_conversations_and_ingest_embeds_at_write_time() {
        let f = recovered_base();
        let fake = Arc::new(FakeEmbedder::default());
        let (runtime, model) = open(&f, Some(fake.clone()), |_| {});
        runtime.meaning_backfill_blocking("test", &mut |_| {}).unwrap();
        let say = |conversation: Uuid, message: &str| ChatRequest {
            request_id: Uuid::new_v4(), conversation_id: conversation,
            message: message.into(), reported_origin: InputOrigin::Human,
        };
        let a = Uuid::new_v4();
        let turn = runtime.converse(say(a, "Mara, it's Rowan. I always name my bicycles Mara.")).await.unwrap();
        assert_eq!(turn.status, "completed");
        assert_eq!(turn.remembered_ids.len(), 2);
        let status = runtime.meaning_status().counts;
        assert_eq!((status.experience_total, status.experience_embedded), (2, 2), "embedded at write time");
        let second = runtime.converse(say(a, "And my canoe?")).await.unwrap();
        let edges = runtime.experience().edges();
        assert!(edges.iter().any(|e| e.from == turn.remembered_ids[1] && e.to == second.remembered_ids[0]
            && e.label == "paccaya:anantara"));

        // A new conversation recalls what Rowan said, by meaning and words.
        let b = Uuid::new_v4();
        let asked = runtime.converse(say(b, "What do I name my bikes?")).await.unwrap();
        let heard = turn.remembered_ids[0];
        let candidate = asked.memory_candidates.as_array().unwrap().iter()
            .find(|c| c["id"].as_u64() == Some(u64::from(heard))).expect("hearing row recalled");
        assert_eq!(candidate["kind"], "experience");
        assert!(candidate["arms"].as_array().unwrap().iter().any(|a| a == "dense"));
        let prompts = model.prompts.lock().unwrap();
        assert!(prompts.last().unwrap().contains("Rowan said: Mara, it's Rowan. I always name my bicycles Mara."));
        drop(prompts);

        // Ingest embeds sections at write time and records near duplicates.
        let text = "# Garage\nTomas built the board in the garage and we sold fifty of them.\n\n# Shop\nThe shop wanted assembled computers.";
        let first = runtime.ingest_blocking(ferricula_ingest::Source::Text { title: Some("One".into()), text: text.into() }, None).unwrap();
        assert!(first.near_duplicate_of.is_none());
        let status = runtime.meaning_status().counts;
        assert_eq!(status.sections_embedded, status.sections_total);
        assert!(status.sections_total >= 1);
        let copy = format!("{text}\n");
        let second = runtime.ingest_blocking(ferricula_ingest::Source::Text { title: Some("Two".into()), text: copy }, None).unwrap();
        assert!(!second.duplicate);
        assert_eq!(second.near_duplicate_of.as_deref(), Some(first.doc_id.as_str()));
        assert!(second.near_duplicate_cosine.unwrap() >= 0.97);
        let reading = runtime.experience().rows().into_iter().find(|(r, _)| r.id == second.memory_id).unwrap();
        assert_eq!(reading.0.tags["near_duplicate_of"], first.doc_id);
        assert_eq!(runtime.meaning_status().counts.pending, 0);
        // Dense recall reaches sections too.
        let recall = runtime.hybrid_recall("where was the computer board made", 8);
        assert!(recall.dense_hits.iter().any(|h| h.kind == CandidateKind::DocumentSection));
    }

    #[tokio::test]
    async fn operator_novelty_curiosity_outliers_and_dream_grounding() {
        let f = recovered_base();
        let fake = Arc::new(FakeEmbedder::default());
        let (runtime, _) = open(&f, Some(fake.clone()), |c| {
            c.life.enabled = true;
            c.life.ollaya_url = "http://127.0.0.1:9".into();
            c.initial_mode = "engaged".into();
        });
        runtime.meaning_backfill_blocking("test", &mut |_| {}).unwrap();
        // Novelty: a message about something held is less novel than one about nothing held.
        let known = life::operator_novelty(&runtime.hybrid_recall("Raised by Otto and Ilse, glassblower, ferryman", 8));
        let unknown = life::operator_novelty(&runtime.hybrid_recall("orbital mechanics of distant comets", 8));
        assert!(known < unknown, "{known} !< {unknown}");
        assert!(known < 0.3);

        // Outliers: the lone iMac memory and the parents memory are far from
        // the many look-alike "Voss" rows.
        let pool: Vec<ferricula_cognition::life::Trace> = [5u32, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, PARENTS].iter()
            .map(|id| ferricula_cognition::life::Trace { id: format!("m:{id}"), text: String::new(),
                valence: ferricula_cognition::sati::Valence::Neutral, intensity: 1.0 }).collect();
        let (kept, selection) = runtime.curiosity_outliers(pool).await;
        assert_eq!(selection["method"], "outlier");
        assert_eq!(kept.len(), 3);
        assert!(kept.iter().any(|t| t.id == "m:5") && kept.iter().any(|t| t.id == format!("m:{PARENTS}")));

        // Dream: grounded prompt, grounding reported, distant chosen by meaning.
        runtime.experience().remember("thinking", "Otto and Ilse raised me in the garage.", BTreeMap::new(), None, 0.5).unwrap();
        runtime.meaning_sync_writes();
        let entries = runtime.life_force(LifeUrgeRequest::Dream).await.unwrap();
        let dream = entries.iter().find(|e| e["kind"] == "dream").unwrap();
        let g = &dream["grounding"];
        assert_eq!(g["threshold"], 0.35);
        assert_eq!(g["sentences"], 3);
        let fraction = g["fraction"].as_f64().unwrap();
        assert!(fraction > 0.0 && fraction < 1.0, "{g}");
        assert_eq!(dream["distant_selection"]["method"], "far_from_residue", "{dream}");
        let dream_id = dream["memory_id"].as_u64().unwrap() as u32;
        // The dream row is embedded but never dense evidence.
        assert!(runtime.meaning_vector(&MeaningKey::Experience(dream_id)).is_some());
        let recall = runtime.hybrid_recall("garage board glassblower file comets", 12);
        assert!(!recall.dense_hits.iter().any(|h| h.id == Some(dream_id)), "dreams are never dense evidence");
    }
}
