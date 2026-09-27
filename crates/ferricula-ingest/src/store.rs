//! The evidence plane: documents kept verbatim, never decayed, searchable
//! with lume's field-aware BM25. Memory records elsewhere point into it by
//! `(doc_id, section index)`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use ferricula_search::bm25::{Bm25Index, Bm25Params, SearchVariant, Section};
use serde::{Deserialize, Serialize};

use crate::extract::Extracted;
use crate::sections::{DocSection, sectionize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentMeta {
    /// Content hash (FNV-1a 64, hex) of origin + text; identical re-ingests dedupe.
    pub doc_id: String,
    pub title: String,
    pub origin: String,
    pub source_kind: String,
    pub ingested_at: u64,
    pub pages: u32,
    pub sections: u32,
    pub bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentRecord {
    pub meta: DocumentMeta,
    pub sections: Vec<DocSection>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SectionHit {
    pub doc_id: String,
    pub title: String,
    pub origin: String,
    pub section: DocSection,
    pub score: f64,
}

/// Outcome of an ingest: the stored record and whether it already existed.
pub struct Ingested<'a> {
    pub record: &'a DocumentRecord,
    pub duplicate: bool,
}

pub struct DocumentStore {
    dir: PathBuf,
    docs: Vec<DocumentRecord>,
    /// Flat position -> (doc position, section position), aligned with `index`.
    locate: Vec<(usize, usize)>,
    index: Option<Bm25Index>,
}

impl DocumentStore {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        let docs_dir = dir.join("docs");
        fs::create_dir_all(&docs_dir).with_context(|| format!("create {}", docs_dir.display()))?;
        let mut docs = Vec::new();
        for entry in fs::read_dir(&docs_dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let bytes = fs::read(&path)?;
            let record: DocumentRecord = serde_json::from_slice(&bytes)
                .with_context(|| format!("parse {}", path.display()))?;
            docs.push(record);
        }
        docs.sort_by(|a, b| (a.meta.ingested_at, &a.meta.doc_id).cmp(&(b.meta.ingested_at, &b.meta.doc_id)));
        let mut store = Self { dir, docs, locate: Vec::new(), index: None };
        store.rebuild_index();
        Ok(store)
    }

    pub fn ingest(&mut self, extracted: &Extracted, source_kind: &str) -> Result<Ingested<'_>> {
        let doc_id = content_id(extracted);
        if let Some(pos) = self.docs.iter().position(|d| d.meta.doc_id == doc_id) {
            return Ok(Ingested { record: &self.docs[pos], duplicate: true });
        }
        let sections = sectionize(extracted);
        anyhow::ensure!(!sections.is_empty(), "document has no readable text");
        let record = DocumentRecord {
            meta: DocumentMeta {
                doc_id: doc_id.clone(),
                title: extracted.title.clone(),
                origin: extracted.origin.clone(),
                source_kind: source_kind.to_string(),
                ingested_at: now(),
                pages: extracted.pages.len() as u32,
                sections: sections.len() as u32,
                bytes: extracted.pages.iter().map(String::len).sum(),
            },
            sections,
        };
        write_atomic(&self.dir.join("docs").join(format!("{doc_id}.json")), &serde_json::to_vec(&record)?)?;
        self.docs.push(record);
        self.rebuild_index();
        Ok(Ingested { record: self.docs.last().expect("just pushed"), duplicate: false })
    }

    pub fn list(&self) -> Vec<&DocumentMeta> {
        self.docs.iter().map(|d| &d.meta).collect()
    }

    pub fn document(&self, doc_id: &str) -> Option<&DocumentRecord> {
        self.docs.iter().find(|d| d.meta.doc_id == doc_id)
    }

    pub fn section(&self, doc_id: &str, index: u32) -> Option<(&DocumentMeta, &DocSection)> {
        let doc = self.document(doc_id)?;
        doc.sections.get(index as usize).map(|s| (&doc.meta, s))
    }

    /// Field-aware BM25 over every section; `doc_id` narrows to one document.
    pub fn search(&self, query: &str, k: usize, doc_id: Option<&str>) -> Vec<SectionHit> {
        let Some(index) = &self.index else { return Vec::new() };
        let hits = index.search(query, SearchVariant::Plus, &Bm25Params::default(), None);
        hits.into_iter()
            .filter_map(|hit| {
                let (d, s) = *self.locate.get(hit.section_index)?;
                let doc = &self.docs[d];
                if doc_id.is_some_and(|id| id != doc.meta.doc_id) {
                    return None;
                }
                Some(SectionHit {
                    doc_id: doc.meta.doc_id.clone(),
                    title: doc.meta.title.clone(),
                    origin: doc.meta.origin.clone(),
                    section: doc.sections[s].clone(),
                    score: hit.score,
                })
            })
            .take(k)
            .collect()
    }

    fn rebuild_index(&mut self) {
        self.locate.clear();
        let mut sections = Vec::new();
        for (d, doc) in self.docs.iter().enumerate() {
            for (s, sec) in doc.sections.iter().enumerate() {
                self.locate.push((d, s));
                sections.push(Section {
                    title: format!("{} — {}", doc.meta.title, sec.heading),
                    body: sec.text.clone(),
                    line_number: sec.index as usize,
                    filename: Some(doc.meta.doc_id.clone()),
                    entities: Vec::new(),
                });
            }
        }
        self.index = (!sections.is_empty()).then(|| Bm25Index::build(sections, None));
    }
}

fn content_id(doc: &Extracted) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            hash ^= *b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    };
    feed(doc.origin.as_bytes());
    for page in &doc.pages {
        feed(&[0xff]);
        feed(page.as_bytes());
    }
    format!("{hash:016x}")
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("rename {}", path.display()))?;
    Ok(())
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
