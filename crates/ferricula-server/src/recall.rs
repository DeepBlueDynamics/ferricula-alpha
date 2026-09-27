//! Hybrid recall: lexical hits from the recovered base, lexical hits from
//! the writable experience store, and BM25 section hits from the document
//! store, fused with reciprocal-rank fusion into one ranked list.
//!
//! Scores from the three sources live on different scales, so fusion uses
//! ranks only (RRF, k = 60). Section candidates carry their verbatim text so
//! every answer can quote the source exactly.

use std::collections::HashMap;

use ferricula_ingest::SectionHit;
use serde::Serialize;

use crate::memory::MemoryHit;

/// Standard RRF damping constant (Cormack et al. 2009).
pub const RRF_K: f64 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateKind {
    /// Recovered (read-only) base memory.
    Memory,
    /// Writable experience memory under `state_dir` (e.g. "I read X").
    Experience,
    /// Verbatim section of an ingested document.
    DocumentSection,
}

/// A document section as evidence: verbatim text plus its address.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SectionEvidence {
    pub doc_id: String,
    pub index: u32,
    pub page: Option<u32>,
    pub title: String,
    pub heading: String,
    pub origin: String,
    /// Exact section text (never paraphrased).
    pub text: String,
    /// BM25 score from the document store (0 for direct reads).
    pub score: f64,
    /// Citation handle, e.g. `[doc 1f2e…§12 p.4]`.
    pub cite: String,
}

impl SectionEvidence {
    pub fn from_hit(hit: &SectionHit) -> Self {
        Self {
            doc_id: hit.doc_id.clone(),
            index: hit.section.index,
            page: hit.section.page,
            title: hit.title.clone(),
            heading: hit.section.heading.clone(),
            origin: hit.origin.clone(),
            text: hit.section.text.clone(),
            score: hit.score,
            cite: citation(&hit.doc_id, hit.section.index, hit.section.page),
        }
    }
}

/// `[doc <doc_id>§<index> p.<page>]`, page omitted for unpaged sources.
pub fn citation(doc_id: &str, index: u32, page: Option<u32>) -> String {
    match page {
        Some(page) => format!("[doc {doc_id}§{index} p.{page}]"),
        None => format!("[doc {doc_id}§{index}]"),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RecallCandidate {
    pub kind: CandidateKind,
    /// Fused RRF score (sum over lists of 1 / (k + rank)).
    pub score: f64,
    /// 1-based rank within the candidate's own source list.
    pub source_rank: usize,
    /// The source's native score (lexical coverage or BM25).
    pub source_score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemoryHit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<SectionEvidence>,
}

impl RecallCandidate {
    pub fn key(&self) -> String {
        match (&self.memory, &self.section) {
            (Some(hit), _) => format!("{:?}:{}", self.kind, hit.id),
            (_, Some(section)) => format!("doc:{}:{}", section.doc_id, section.index),
            _ => String::new(),
        }
    }
}

/// Hybrid recall result. `hits` keeps the pre-R1 `/memory/recall` meaning
/// (recovered-base lexical hits) so existing clients are unaffected.
#[derive(Debug, Clone, Serialize)]
pub struct HybridRecall {
    pub query: String,
    pub hits: Vec<MemoryHit>,
    pub experience_hits: Vec<MemoryHit>,
    pub section_hits: Vec<SectionEvidence>,
    pub candidates: Vec<RecallCandidate>,
    pub fusion: &'static str,
}

/// Reciprocal-rank fusion over keyed ranked lists. An item appearing in
/// several lists sums its contributions; the first occurrence's payload is
/// kept. Ties keep first-seen order, so output is deterministic.
pub fn rrf_fuse<T: Clone>(lists: &[Vec<(String, T)>], k: f64) -> Vec<(String, T, f64)> {
    let mut order: Vec<(String, T, f64)> = Vec::new();
    let mut position: HashMap<String, usize> = HashMap::new();
    for list in lists {
        for (rank0, (key, item)) in list.iter().enumerate() {
            let contribution = 1.0 / (k + (rank0 + 1) as f64);
            match position.get(key) {
                Some(&at) => order[at].2 += contribution,
                None => {
                    position.insert(key.clone(), order.len());
                    order.push((key.clone(), item.clone(), contribution));
                }
            }
        }
    }
    // Stable sort: equal scores keep first-seen order.
    order.sort_by(|a, b| b.2.total_cmp(&a.2));
    order
}

/// Build fused candidates from the three source lists.
pub fn fuse(
    recovered: &[MemoryHit],
    experience: &[MemoryHit],
    sections: &[SectionEvidence],
    limit: usize,
) -> Vec<RecallCandidate> {
    let memory_list = |kind: CandidateKind, hits: &[MemoryHit]| -> Vec<(String, RecallCandidate)> {
        hits.iter()
            .enumerate()
            .map(|(i, hit)| {
                let candidate = RecallCandidate {
                    kind,
                    score: 0.0,
                    source_rank: i + 1,
                    source_score: f64::from(hit.score),
                    memory: Some(hit.clone()),
                    section: None,
                };
                (candidate.key(), candidate)
            })
            .collect()
    };
    let section_list: Vec<(String, RecallCandidate)> = sections
        .iter()
        .enumerate()
        .map(|(i, section)| {
            let candidate = RecallCandidate {
                kind: CandidateKind::DocumentSection,
                score: 0.0,
                source_rank: i + 1,
                source_score: section.score,
                memory: None,
                section: Some(section.clone()),
            };
            (candidate.key(), candidate)
        })
        .collect();
    let lists = [
        memory_list(CandidateKind::Memory, recovered),
        memory_list(CandidateKind::Experience, experience),
        section_list,
    ];
    rrf_fuse(&lists, RRF_K)
        .into_iter()
        .take(limit)
        .map(|(_, mut candidate, score)| {
            candidate.score = score;
            candidate
        })
        .collect()
}

/// Truncate to at most `max` bytes on a char boundary.
pub fn truncate_bytes(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut cut = max;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    &text[..cut]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::LifecycleStateView;
    use std::collections::BTreeMap;

    fn hit(id: u32, score: f32) -> MemoryHit {
        MemoryHit {
            id, score, state: LifecycleStateView::Active, fidelity: 1.0, importance: 0.0,
            keystone: false, tags: BTreeMap::new(), refs: None,
        }
    }

    fn section(doc: &str, index: u32, page: Option<u32>) -> SectionEvidence {
        SectionEvidence {
            doc_id: doc.into(), index, page, title: "T".into(), heading: "H".into(),
            origin: "inline".into(), text: format!("verbatim {index}"), score: 3.0,
            cite: citation(doc, index, page),
        }
    }

    #[test]
    fn rrf_sums_contributions_and_orders_by_fused_score() {
        let a = vec![("x".to_string(), 'x'), ("y".to_string(), 'y')];
        let b = vec![("y".to_string(), 'y'), ("z".to_string(), 'z')];
        let fused = rrf_fuse(&[a, b], 60.0);
        let keys: Vec<&str> = fused.iter().map(|(k, _, _)| k.as_str()).collect();
        assert_eq!(keys, ["y", "x", "z"]);
        let y = 1.0 / 62.0 + 1.0 / 61.0;
        assert!((fused[0].2 - y).abs() < 1e-12);
        assert!((fused[1].2 - 1.0 / 61.0).abs() < 1e-12);
        // x and a lone first-ranked item tie; first-seen order is kept.
        let tie = rrf_fuse(&[vec![("p".to_string(), 1)], vec![("q".to_string(), 2)]], 60.0);
        assert_eq!(tie[0].0, "p");
        assert_eq!(tie[1].0, "q");
        assert!(rrf_fuse::<u8>(&[], 60.0).is_empty());
    }

    #[test]
    fn fuse_interleaves_sources_and_keeps_section_text() {
        let recovered = vec![hit(1, 0.9), hit(2, 0.5)];
        let experience = vec![hit(0x8000_0000, 0.4)];
        let sections = vec![section("d1", 3, Some(2)), section("d1", 4, None)];
        let fused = fuse(&recovered, &experience, &sections, 10);
        assert_eq!(fused.len(), 5);
        // Rank-1 items of each list tie at 1/61, in list order.
        assert_eq!(fused[0].kind, CandidateKind::Memory);
        assert_eq!(fused[1].kind, CandidateKind::Experience);
        assert_eq!(fused[2].kind, CandidateKind::DocumentSection);
        let s = fused[2].section.as_ref().unwrap();
        assert_eq!(s.text, "verbatim 3");
        assert_eq!(s.cite, "[doc d1§3 p.2]");
        assert!((fused[0].score - 1.0 / 61.0).abs() < 1e-12);
        assert_eq!(fuse(&recovered, &experience, &sections, 2).len(), 2);
        assert_eq!(citation("d1", 4, None), "[doc d1§4]");
    }

    #[test]
    fn truncation_respects_char_boundaries() {
        assert_eq!(truncate_bytes("héllo", 2), "h");
        assert_eq!(truncate_bytes("abc", 10), "abc");
    }
}
