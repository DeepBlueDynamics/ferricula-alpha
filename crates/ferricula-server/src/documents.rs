//! Document sense door on the runtime: ingest a source into the verbatim
//! document store, record the experience of reading it in the writable
//! experience store, and serve hybrid recall over both planes plus the
//! recovered base. Everything written lives under `state_dir`.
use super::*;
use ferricula_ingest::{DocumentMeta, DocumentRecord, DocumentStore, Source};

use crate::memory::{ExperienceStore, ReadingEvent};
use crate::recall::{HybridRecall, SectionEvidence, citation, fuse_arms, fuse_arms_all, lexical_lists, promote_keys};

/// Maximum operator note ("why I'm giving you this") length in bytes.
pub const MAX_NOTE_BYTES: usize = 2048;
/// Bytes of document text quoted into the reading memory.
const READING_EXCERPT_CHARS: usize = 500;
/// Mean-section-vector cosine at or above which a new document is recorded
/// as a near duplicate of an earlier one (recorded, never blocking).
pub const NEAR_DUPLICATE_COSINE: f64 = 0.97;

pub(super) struct DocumentPlane {
    store: Mutex<DocumentStore>,
    experience: ExperienceStore,
}

impl DocumentPlane {
    pub(super) fn open(state_dir: &Path, recovered_ids: std::collections::HashSet<u32>) -> Result<Self> {
        let store = DocumentStore::open(state_dir.join("documents"))
            .context("open document store")?;
        let experience = ExperienceStore::open(state_dir.join("experience"), recovered_ids)
            .context("open experience store")?;
        Ok(Self { store: Mutex::new(store), experience })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct IngestOutcome {
    pub doc_id: String,
    pub title: String,
    pub origin: String,
    pub source_kind: String,
    pub pages: u32,
    pub sections: u32,
    pub duplicate: bool,
    /// Experience-store id of the "I read X" memory.
    pub memory_id: u32,
    /// An earlier document whose mean section vector is within
    /// [`NEAR_DUPLICATE_COSINE`] of this one's (dense features only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub near_duplicate_of: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub near_duplicate_cosine: Option<f64>,
}

impl AgentRuntime {
    /// Ingest off the async executor: extraction may fetch over the network
    /// (grub for pages, direct fetch for PDF URLs) and parse PDFs.
    pub async fn ingest(self: &Arc<Self>, source: Source, note: Option<String>) -> Result<IngestOutcome> {
        let runtime = self.clone();
        tokio::task::spawn_blocking(move || runtime.ingest_blocking(source, note))
            .await
            .map_err(|error| anyhow::anyhow!("ingest task failed: {error}"))?
    }

    pub fn ingest_blocking(&self, source: Source, note: Option<String>) -> Result<IngestOutcome> {
        let cfg = &self.config.documents;
        if !cfg.enabled {
            bail!("document ingestion is disabled ([documents] enabled = false)");
        }
        if matches!(source, Source::Url { .. }) && !cfg.allow_url {
            bail!("url sources are disabled ([documents] allow_url = false)");
        }
        if note.as_ref().is_some_and(|n| n.len() > MAX_NOTE_BYTES) {
            bail!("note exceeds {MAX_NOTE_BYTES} bytes");
        }
        let kind = source.kind().to_string();
        let extracted = ferricula_ingest::extract(&source, &cfg.extract_config())?;
        let (meta, excerpt, duplicate) = {
            let mut store = self.documents.store.lock().expect("document store poisoned");
            let ingested = store.ingest(&extracted, &kind)?;
            (ingested.record.meta.clone(), excerpt(&ingested.record.sections), ingested.duplicate)
        };
        let existing = self.documents.experience.reading_for(&meta.doc_id);
        // Write-time embedding of the new sections (never blocks the ingest;
        // pending rows wait for the backfill), then the near-duplicate test.
        let near_duplicate_of = if existing.is_none() && !duplicate {
            self.meaning_sync_writes();
            self.near_duplicate(&meta.doc_id)
        } else {
            None
        };
        let memory_id = match existing {
            Some(id) => id,
            None => self.documents.experience.remember_reading(&ReadingEvent {
                doc_id: meta.doc_id.clone(),
                title: meta.title.clone(),
                origin: meta.origin.clone(),
                source_kind: meta.source_kind.clone(),
                pages: meta.pages,
                sections: meta.sections,
                excerpt,
                note: note.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()),
                near_duplicate_of: near_duplicate_of.clone(),
            })?,
        };
        // The reading memory itself.
        self.meaning_sync_writes();
        Ok(IngestOutcome {
            doc_id: meta.doc_id,
            title: meta.title,
            origin: meta.origin,
            source_kind: meta.source_kind,
            pages: meta.pages,
            sections: meta.sections,
            duplicate,
            memory_id,
            near_duplicate_cosine: near_duplicate_of.as_ref().map(|(_, c)| (c * 1e4).round() / 1e4),
            near_duplicate_of: near_duplicate_of.map(|(d, _)| d),
        })
    }

    /// The most similar earlier document by mean section vector, when its
    /// cosine is at least [`NEAR_DUPLICATE_COSINE`].
    fn near_duplicate(&self, doc_id: &str) -> Option<(String, f64)> {
        let means = self.meaning_document_means();
        let mine = means.get(doc_id)?;
        means
            .iter()
            .filter(|(other, _)| other.as_str() != doc_id)
            .map(|(other, v)| (other.clone(), crate::meaning::dot(mine, v)))
            .filter(|(_, c)| *c >= NEAR_DUPLICATE_COSINE)
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// The document store (catalogs for the meaning index).
    pub(super) fn documents_store(&self) -> std::sync::MutexGuard<'_, DocumentStore> {
        self.documents.store.lock().expect("document store poisoned")
    }

    pub fn documents(&self) -> Vec<DocumentMeta> {
        let store = self.documents.store.lock().expect("document store poisoned");
        store.list().into_iter().cloned().collect()
    }

    pub fn document(&self, doc_id: &str) -> Option<DocumentRecord> {
        let store = self.documents.store.lock().expect("document store poisoned");
        store.document(doc_id).cloned()
    }

    /// The "I read X" memory for a document: its id, the ingest note (the
    /// reason it was kept, when one was given) and when it was read.
    pub fn document_reading(&self, doc_id: &str) -> Option<Value> {
        let id = self.documents.experience.reading_for(doc_id)?;
        let (row, record) = self.documents.experience.rows().into_iter().find(|(r, _)| r.id == id)?;
        Some(json!({ "memory_id": id, "note": row.tags.get("note"), "read_at": record.created_at }))
    }

    pub fn document_section(&self, doc_id: &str, index: u32) -> Option<SectionEvidence> {
        let store = self.documents.store.lock().expect("document store poisoned");
        let (meta, section) = store.section(doc_id, index)?;
        Some(SectionEvidence {
            doc_id: meta.doc_id.clone(),
            index: section.index,
            page: section.page,
            title: meta.title.clone(),
            heading: section.heading.clone(),
            origin: meta.origin.clone(),
            text: section.text.clone(),
            score: 0.0,
            cite: citation(&meta.doc_id, section.index, section.page),
        })
    }

    /// BM25 over every ingested section; `doc_id` narrows to one document.
    pub fn search_documents(&self, query: &str, k: usize, doc_id: Option<&str>) -> Vec<SectionEvidence> {
        if query.trim().is_empty() || k == 0 {
            return Vec::new();
        }
        let store = self.documents.store.lock().expect("document store poisoned");
        store.search(query, k, doc_id).iter().map(SectionEvidence::from_hit).collect()
    }

    /// Experience-store lexical hits (same scorer as the recovered base).
    pub fn experience_candidates(&self, query: &str, limit: usize) -> Vec<MemoryHit> {
        self.documents.experience.recall_candidates(query, limit)
    }

    pub(crate) fn experience(&self) -> &ExperienceStore {
        &self.documents.experience
    }

    pub fn experience_len(&self) -> usize {
        self.documents.experience.len()
    }

    /// Recovered base + experience + document sections, fused with RRF.
    /// With a usable embedder the dense arm (one query embedding, cosine
    /// top-k over all three sets, dreams excluded) and a one-hop graph arm
    /// join the lexical and BM25 arms; without one this is exactly the
    /// lexical recall. Blocking (the query embedding is an HTTP call).
    pub fn hybrid_recall(&self, query: &str, limit: usize) -> HybridRecall {
        let limit = limit.clamp(1, 64);
        let faded = self.config.recall.include_faded_recovered;
        let hits = self.memory.recall_candidates_with(query, limit, faded);
        let experience_hits = self.experience_candidates(query, limit);
        let section_hits = self.search_documents(query, limit.min(20), None);
        let mut lists = lexical_lists(&hits, &experience_hits, &section_hits);
        let (fusion, dense_hits, dense_novelty, candidates) = match self.dense_arm(query) {
            Some(arm) => {
                let promote: Vec<String> = arm.lists.iter().filter(|l| l.arm == "dense")
                    .flat_map(|l| l.items.iter().take(self.config.recall.dense_guarantee).map(|c| c.key()))
                    .collect();
                for list in lists.iter_mut().filter(|l| l.arm == "lexical") {
                    list.weight = self.config.recall.lexical_weight;
                }
                lists.extend(arm.lists);
                let mut fused = fuse_arms_all(lists);
                promote_keys(&mut fused, &promote);
                fused.truncate(limit);
                ("rrf_k60+dense+graph", arm.views, arm.novelty, fused)
            }
            None => ("rrf_k60", Vec::new(), None, fuse_arms(lists, limit)),
        };
        HybridRecall {
            query: query.to_string(),
            hits,
            experience_hits,
            section_hits,
            candidates,
            fusion,
            dense_hits,
            dense_novelty,
        }
    }

    /// [`Self::hybrid_recall`] off the async executor.
    pub async fn hybrid_recall_async(self: &Arc<Self>, query: &str, limit: usize) -> HybridRecall {
        let runtime = self.clone();
        let q = query.to_string();
        match tokio::task::spawn_blocking(move || runtime.hybrid_recall(&q, limit)).await {
            Ok(recall) => recall,
            Err(error) => {
                eprintln!("recall worker failed ({error}); lexical only");
                let hits = self.memory.recall_candidates_with(query, limit, self.config.recall.include_faded_recovered);
                let experience_hits = self.experience_candidates(query, limit);
                let section_hits = self.search_documents(query, limit.min(20), None);
                let candidates = fuse_arms(lexical_lists(&hits, &experience_hits, &section_hits), limit);
                HybridRecall {
                    query: query.to_string(), hits, experience_hits, section_hits, candidates,
                    fusion: "rrf_k60", dense_hits: Vec::new(), dense_novelty: None,
                }
            }
        }
    }
}

/// First ~500 characters of the document, whitespace collapsed.
fn excerpt(sections: &[ferricula_ingest::DocSection]) -> String {
    let mut out = String::new();
    for section in sections {
        for word in section.text.split_whitespace() {
            if out.chars().count() >= READING_EXCERPT_CHARS {
                return out;
            }
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(word);
        }
    }
    out
}
