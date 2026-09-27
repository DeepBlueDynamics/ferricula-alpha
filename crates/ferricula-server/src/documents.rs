//! Document sense door on the runtime: ingest a source into the verbatim
//! document store, record the experience of reading it in the writable
//! experience store, and serve hybrid recall over both planes plus the
//! recovered base. Everything written lives under `state_dir`.
use super::*;
use ferricula_ingest::{DocumentMeta, DocumentRecord, DocumentStore, Source};

use crate::memory::{ExperienceStore, ReadingEvent};
use crate::recall::{HybridRecall, SectionEvidence, citation, fuse};

/// Maximum operator note ("why I'm giving you this") length in bytes.
pub const MAX_NOTE_BYTES: usize = 2048;
/// Bytes of document text quoted into the reading memory.
const READING_EXCERPT_CHARS: usize = 500;

pub(super) struct DocumentPlane {
    store: Mutex<DocumentStore>,
    experience: ExperienceStore,
}

impl DocumentPlane {
    pub(super) fn open(state_dir: &Path, recovered_max: Option<u32>) -> Result<Self> {
        let store = DocumentStore::open(state_dir.join("documents"))
            .context("open document store")?;
        let experience = ExperienceStore::open(state_dir.join("experience"), recovered_max)
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
        let memory_id = match self.documents.experience.reading_for(&meta.doc_id) {
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
            })?,
        };
        Ok(IngestOutcome {
            doc_id: meta.doc_id,
            title: meta.title,
            origin: meta.origin,
            source_kind: meta.source_kind,
            pages: meta.pages,
            sections: meta.sections,
            duplicate,
            memory_id,
        })
    }

    pub fn documents(&self) -> Vec<DocumentMeta> {
        let store = self.documents.store.lock().expect("document store poisoned");
        store.list().into_iter().cloned().collect()
    }

    pub fn document(&self, doc_id: &str) -> Option<DocumentRecord> {
        let store = self.documents.store.lock().expect("document store poisoned");
        store.document(doc_id).cloned()
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

    pub fn experience_len(&self) -> usize {
        self.documents.experience.len()
    }

    /// Recovered base + experience + document sections, fused with RRF.
    pub fn hybrid_recall(&self, query: &str, limit: usize) -> HybridRecall {
        let limit = limit.clamp(1, 64);
        let hits = self.recall_candidates(query, limit);
        let experience_hits = self.experience_candidates(query, limit);
        let section_hits = self.search_documents(query, limit.min(20), None);
        let candidates = fuse(&hits, &experience_hits, &section_hits, limit);
        HybridRecall {
            query: query.to_string(),
            hits,
            experience_hits,
            section_hits,
            candidates,
            fusion: "rrf_k60",
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
