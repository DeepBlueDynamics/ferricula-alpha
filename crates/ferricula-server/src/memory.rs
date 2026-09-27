use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use ferricula_core::memory::Provenance;
use ferricula_core::{DurableEngine, LifecycleState, MemoryRecord, MemoryRef, Row};
use serde::Serialize;

/// Read access to the recovery copy. Experience commits remain serialized by
/// the agent task loop; candidate lookup itself never mutates memory.
pub struct MemoryRuntime {
    engine: Mutex<DurableEngine>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryHit {
    pub id: u32,
    pub score: f32,
    pub state: LifecycleStateView,
    pub fidelity: f32,
    pub importance: f32,
    pub keystone: bool,
    pub tags: BTreeMap<String, String>,
    pub refs: Option<ferricula_core::MemoryRef>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleStateView {
    Active,
    Forgiven,
    Archived,
}

impl From<LifecycleState> for LifecycleStateView {
    fn from(value: LifecycleState) -> Self {
        match value {
            LifecycleState::Active => Self::Active,
            LifecycleState::Forgiven => Self::Forgiven,
            LifecycleState::Archived => Self::Archived,
        }
    }
}

impl MemoryRuntime {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            engine: Mutex::new(DurableEngine::open(path)?),
        })
    }

    /// Provider-free lexical candidate retrieval keeps the agent's continuity
    /// available before an embedding service is configured. Dense/hybrid
    /// ranking can refine these candidates later.
    pub fn recall_candidates(&self, query: &str, limit: usize) -> Vec<MemoryHit> {
        let engine = self.engine.lock().expect("memory engine poisoned");
        lexical_hits(&engine, query, limit)
    }

    /// Highest id present in the recovered base (rows or records), if any.
    pub fn max_id(&self) -> Option<u32> {
        let engine = self.engine.lock().expect("memory engine poisoned");
        max_id(&engine)
    }
}

fn max_id(engine: &DurableEngine) -> Option<u32> {
    let rows = engine.engine().rows_iter().map(|row| row.id).max();
    let records = engine.memory_store().all_records().iter().map(|r| r.id).max();
    rows.max(records)
}

/// The lexical scorer shared by the recovered base and the experience store,
/// so candidates from both planes are ranked on the same scale.
pub fn lexical_hits(engine: &DurableEngine, query: &str, limit: usize) -> Vec<MemoryHit> {
    let tokens = tokens(query);
    if tokens.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for row in engine.engine().rows_iter() {
        let Some(record) = engine.memory_store().get(row.id) else {
            continue;
        };
        // Deliberate release excludes plaintext before any ranking or model call.
        // Low-priority Active memories remain eligible; decay is not deletion.
        if record.state != LifecycleState::Active {
            continue;
        }
        let haystack = row
            .tags
            .iter()
            .map(|(key, value)| format!("{key} {value}"))
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        let matched = tokens
            .iter()
            .filter(|token| haystack.contains(*token))
            .count();
        if matched == 0 {
            continue;
        }
        let coverage = matched as f32 / tokens.len() as f32;
        let state_weight = match record.state {
            LifecycleState::Active => 1.0,
            LifecycleState::Forgiven => 0.55,
            LifecycleState::Archived => 0.35,
        };
        let editorial =
            1.0 + record.importance.max(0.0) * 0.15 + f32::from(record.keystone) * 0.15;
        hits.push(MemoryHit {
            id: row.id,
            score: coverage * record.fidelity * state_weight * editorial,
            state: record.state.into(),
            fidelity: record.fidelity,
            importance: record.importance,
            keystone: record.keystone,
            tags: row.tags.clone(),
            refs: row.refs.clone(),
        });
    }
    hits.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
    hits.truncate(limit.min(50));
    hits
}

/// Experience ids live in the upper half of the u32 space. Recovered v1/v2
/// memories were allocated from small sequential ids, so the two namespaces
/// are disjoint by construction and an id alone says which plane it came
/// from. If a recovered base ever reaches this range, allocation continues
/// above its maximum instead (see [`allocate_experience_id`]).
pub const EXPERIENCE_ID_BASE: u32 = 0x8000_0000;

/// Next experience id: at or above the namespace base, above every recovered
/// id, above every id already in the experience store, and above the
/// persisted high-water mark (so an id is never reused, even if its row were
/// later removed).
pub fn allocate_experience_id(
    recovered_max: Option<u32>,
    experience_max: Option<u32>,
    high_water: Option<u32>,
) -> Result<u32> {
    let mut next = EXPERIENCE_ID_BASE;
    for used in [recovered_max, experience_max, high_water].into_iter().flatten() {
        let after = used
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("experience id space exhausted"))?;
        next = next.max(after);
    }
    Ok(next)
}

/// Writable experience memory under `state_dir/experience`, same on-disk
/// format as the recovered base so recall treats both uniformly. The
/// recovered base is never opened for writing.
pub struct ExperienceStore {
    inner: Mutex<ExperienceInner>,
}

struct ExperienceInner {
    engine: DurableEngine,
    high_water_path: PathBuf,
    recovered_max: Option<u32>,
    high_water: Option<u32>,
}

/// A reading experience to record for an ingested document.
#[derive(Debug, Clone)]
pub struct ReadingEvent {
    pub doc_id: String,
    pub title: String,
    pub origin: String,
    pub source_kind: String,
    pub pages: u32,
    pub sections: u32,
    pub excerpt: String,
    pub note: Option<String>,
}

impl ExperienceStore {
    pub fn open(dir: impl AsRef<Path>, recovered_max: Option<u32>) -> Result<Self> {
        let dir = dir.as_ref();
        let engine = DurableEngine::open(dir)?;
        let high_water_path = dir.join("id-high-water.json");
        let high_water = match std::fs::read(&high_water_path) {
            Ok(bytes) => Some(
                serde_json::from_slice::<u32>(&bytes)
                    .with_context(|| format!("parse {}", high_water_path.display()))?,
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        Ok(Self {
            inner: Mutex::new(ExperienceInner { engine, high_water_path, recovered_max, high_water }),
        })
    }

    pub fn recall_candidates(&self, query: &str, limit: usize) -> Vec<MemoryHit> {
        let inner = self.inner.lock().expect("experience store poisoned");
        lexical_hits(&inner.engine, query, limit)
    }

    pub fn len(&self) -> usize {
        self.inner.lock().expect("experience store poisoned").engine.memory_store().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Existing reading memory for a document, if one was recorded.
    pub fn reading_for(&self, doc_id: &str) -> Option<u32> {
        let inner = self.inner.lock().expect("experience store poisoned");
        inner
            .engine
            .engine()
            .rows_iter()
            .filter(|row| {
                row.tags.get("channel").map(String::as_str) == Some("reading")
                    && row.tags.get("doc_id").map(String::as_str) == Some(doc_id)
            })
            .map(|row| row.id)
            .min()
    }

    /// Record "I read X" as one durable experience memory; returns its id.
    /// The WAL entry is appended and the store checkpointed before return.
    pub fn remember_reading(&self, event: &ReadingEvent) -> Result<u32> {
        let mut inner = self.inner.lock().expect("experience store poisoned");
        let id = allocate_experience_id(inner.recovered_max, max_id(&inner.engine), inner.high_water)?;
        // Reserve the id durably first: a crash after this point leaks an id
        // but can never hand the same id to two memories.
        let tmp = inner.high_water_path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(&id)?)?;
        std::fs::rename(&tmp, &inner.high_water_path)?;
        inner.high_water = Some(id);

        let mut tags = BTreeMap::new();
        tags.insert("channel".to_string(), "reading".to_string());
        tags.insert(
            "text".to_string(),
            format!("Read {} ({}): {}", event.title, event.origin, event.excerpt),
        );
        tags.insert("doc_id".to_string(), event.doc_id.clone());
        tags.insert("source_kind".to_string(), event.source_kind.clone());
        tags.insert("sections".to_string(), event.sections.to_string());
        tags.insert("pages".to_string(), event.pages.to_string());
        tags.insert("title".to_string(), event.title.clone());
        if let Some(note) = event.note.as_ref().filter(|n| !n.trim().is_empty()) {
            tags.insert("note".to_string(), note.clone());
        }
        let is_web = event.origin.starts_with("http://") || event.origin.starts_with("https://");
        let refs = MemoryRef {
            book: Some(event.title.clone()),
            url: is_web.then(|| event.origin.clone()),
            filename: (!is_web).then(|| event.origin.clone()),
            ..MemoryRef::default()
        };
        let row = Row { id, tags, vector: Vec::new(), refs: Some(refs) };
        let mut record = MemoryRecord::new(id);
        record.importance = 0.5;
        record.provenance = Provenance::Ingested;
        inner.engine.remember(row, record)?;
        inner.engine.checkpoint()?;
        Ok(id)
    }
}

fn tokens(text: &str) -> Vec<String> {
    let mut out: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| part.chars().count() >= 3)
        .map(str::to_lowercase)
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn released_text_is_filtered_after_restart_but_decayed_active_text_remains() {
        use ferricula_core::memory::MemoryRecord;
        use ferricula_core::model::Row;
        let dir = std::env::current_dir().unwrap().join("target")
            .join(format!("recall-release-{}", uuid::Uuid::new_v4()));
        let mut engine = DurableEngine::open(&dir).unwrap();
        for id in 1..=3 {
            let row = Row {
                id, vector: vec![1.0, 0.0], refs: None,
                tags: BTreeMap::from([("text".into(), format!("ÜBER marker {id}"))]),
            };
            let mut record = MemoryRecord::new(id);
            if id == 1 { record.fidelity = 0.0; }
            if id >= 2 { record.forgive(); }
            if id == 3 { record.archive(); record.keystone = true; }
            engine.remember(row, record).unwrap();
        }
        drop(engine);
        let runtime = MemoryRuntime::open(&dir).unwrap();
        let before = runtime.engine.lock().unwrap().memory_store().get(1).unwrap().recall_count;
        let hits = runtime.recall_candidates("über", 10);
        assert_eq!(hits.iter().map(|h| h.id).collect::<Vec<_>>(), vec![1]);
        assert_eq!(hits[0].fidelity, 0.0);
        assert_eq!(hits[0].tags["text"], "ÜBER marker 1");
        assert_eq!(runtime.max_id(), Some(3));
        let engine = runtime.engine.lock().unwrap();
        assert_eq!(engine.memory_store().len(), 3);
        assert_eq!(engine.memory_store().get(1).unwrap().recall_count, before);
        drop(engine);
        drop(runtime);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn experience_ids_never_collide_with_recovered_or_prior_ids() {
        assert_eq!(allocate_experience_id(None, None, None).unwrap(), EXPERIENCE_ID_BASE);
        assert_eq!(allocate_experience_id(Some(41_000), None, None).unwrap(), EXPERIENCE_ID_BASE);
        assert_eq!(
            allocate_experience_id(Some(3), Some(EXPERIENCE_ID_BASE + 4), None).unwrap(),
            EXPERIENCE_ID_BASE + 5
        );
        // The high-water mark wins even if the highest row is gone.
        assert_eq!(
            allocate_experience_id(None, Some(EXPERIENCE_ID_BASE), Some(EXPERIENCE_ID_BASE + 9))
                .unwrap(),
            EXPERIENCE_ID_BASE + 10
        );
        // A recovered base that reached the namespace pushes allocation above it.
        assert_eq!(
            allocate_experience_id(Some(EXPERIENCE_ID_BASE + 100), None, None).unwrap(),
            EXPERIENCE_ID_BASE + 101
        );
        assert!(allocate_experience_id(Some(u32::MAX), None, None).is_err());
    }

    #[test]
    fn experience_store_persists_reading_and_allocates_monotonic_ids() {
        let dir = std::env::current_dir().unwrap().join("target")
            .join(format!("experience-{}", uuid::Uuid::new_v4()));
        let event = ReadingEvent {
            doc_id: "abc".into(), title: "Paper".into(), origin: "https://x.org/p.pdf".into(),
            source_kind: "url".into(), pages: 2, sections: 3, excerpt: "hello world".into(),
            note: Some("for the memory discussion".into()),
        };
        let first = {
            let store = ExperienceStore::open(&dir, Some(77)).unwrap();
            let id = store.remember_reading(&event).unwrap();
            assert_eq!(id, EXPERIENCE_ID_BASE);
            assert_eq!(store.reading_for("abc"), Some(id));
            id
        };
        let store = ExperienceStore::open(&dir, Some(77)).unwrap();
        assert_eq!(store.len(), 1);
        let hits = store.recall_candidates("paper world", 5);
        assert_eq!(hits[0].id, first);
        assert_eq!(hits[0].tags["channel"], "reading");
        assert_eq!(hits[0].tags["note"], "for the memory discussion");
        assert_eq!(hits[0].refs.as_ref().unwrap().url.as_deref(), Some("https://x.org/p.pdf"));
        let second = store
            .remember_reading(&ReadingEvent { doc_id: "def".into(), ..event })
            .unwrap();
        assert_eq!(second, first + 1);
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn tokenization_is_stable_and_deduplicated() {
        assert_eq!(
            tokens("Design, design—great products!"),
            ["design", "great", "products"]
        );
    }
}
