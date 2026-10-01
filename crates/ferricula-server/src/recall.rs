//! Hybrid recall: lexical hits from the recovered base, lexical hits from
//! the writable experience store, and BM25 section hits from the document
//! store, fused with reciprocal-rank fusion into one ranked list.
//!
//! Scores from the three sources live on different scales, so fusion uses
//! ranks only (RRF, k = 60). Section candidates carry their verbatim text so
//! every answer can quote the source exactly.
//!
//! With a text embedder (R2b) two more ranked lists join the fusion: the
//! **dense** arm (cosine top-k over recovered memories, experience rows and
//! document sections in one embedding space) and the **graph** arm (one hop
//! through the recovered memory graph from the top dense recovered hits,
//! weighted lower). Every candidate lists the arms that found it.

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
    /// Arms that ranked this candidate: `lexical`, `bm25`, `dense`, `graph`.
    pub arms: Vec<&'static str>,
    /// Cosine to the query when the dense arm found it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dense_score: Option<f64>,
    /// Ranking fidelity in 0..=1 from strengthening and fading. Stored
    /// `memory.fidelity` is left as recorded. None for document sections.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_fidelity: Option<f64>,
}

impl RecallCandidate {
    pub fn from_memory(kind: CandidateKind, hit: &MemoryHit, rank: usize) -> Self {
        Self {
            kind,
            score: 0.0,
            source_rank: rank,
            source_score: f64::from(hit.score),
            memory: Some(hit.clone()),
            section: None,
            arms: Vec::new(),
            dense_score: None,
            effective_fidelity: None,
        }
    }

    pub fn from_section(section: &SectionEvidence, rank: usize) -> Self {
        Self {
            kind: CandidateKind::DocumentSection,
            score: 0.0,
            source_rank: rank,
            source_score: section.score,
            memory: None,
            section: Some(section.clone()),
            arms: Vec::new(),
            dense_score: None,
            effective_fidelity: None,
        }
    }

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
    /// Dense and graph arm hits in rank order (empty without an embedder).
    pub dense_hits: Vec<DenseHitView>,
    /// 1 - max cosine of the query over recovered + experience memories
    /// (dreams excluded); None without an embedder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dense_novelty: Option<f32>,
}

/// One dense-arm or graph-arm hit.
#[derive(Debug, Clone, Serialize)]
pub struct DenseHitView {
    pub arm: &'static str,
    pub kind: CandidateKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    pub cosine: f64,
    /// For graph hits: the dense hit it was reached from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via: Option<u32>,
}

impl DenseHitView {
    pub fn of(arm: &'static str, candidate: &RecallCandidate, cosine: f64, via: Option<u32>) -> Self {
        Self {
            arm,
            kind: candidate.kind,
            id: candidate.memory.as_ref().map(|m| m.id),
            doc_id: candidate.section.as_ref().map(|s| s.doc_id.clone()),
            index: candidate.section.as_ref().map(|s| s.index),
            cosine,
            via,
        }
    }
}

/// One ranked list for [`fuse_arms`].
pub struct ArmList {
    pub arm: &'static str,
    pub weight: f64,
    pub items: Vec<RecallCandidate>,
}

/// Weighted reciprocal-rank fusion over arm lists: an item scores
/// `sum(weight / (k + rank))` over the lists it appears in. The first
/// occurrence's payload is kept; `arms` collects every arm that ranked it
/// and `dense_score` is taken from the dense arm. Ties keep first-seen order.
pub fn fuse_arms(lists: Vec<ArmList>, limit: usize) -> Vec<RecallCandidate> {
    let mut order = fuse_arms_all(lists);
    order.truncate(limit);
    order
}

/// [`fuse_arms`] without the cut.
pub fn fuse_arms_all(lists: Vec<ArmList>) -> Vec<RecallCandidate> {
    let mut order: Vec<RecallCandidate> = Vec::new();
    let mut position: HashMap<String, usize> = HashMap::new();
    for list in lists {
        for (rank0, item) in list.items.into_iter().enumerate() {
            let contribution = list.weight / (RRF_K + (rank0 + 1) as f64);
            let key = item.key();
            let dense = (list.arm == "dense").then_some(item.source_score);
            let at = match position.get(&key) {
                Some(&at) => at,
                None => {
                    position.insert(key, order.len());
                    order.push(RecallCandidate { score: 0.0, arms: Vec::new(), dense_score: None, ..item });
                    order.len() - 1
                }
            };
            let c = &mut order[at];
            c.score += contribution;
            if !c.arms.contains(&list.arm) {
                c.arms.push(list.arm);
            }
            if dense.is_some() && c.dense_score.is_none() {
                c.dense_score = dense;
            }
        }
    }
    order.sort_by(|a, b| b.score.total_cmp(&a.score));
    apply_installed_ranking(&mut order);
    order
}

/// Overlay key for a memory candidate. Document sections are not memories.
pub fn overlay_key(kind: CandidateKind, id: u32) -> Option<String> {
    match kind {
        CandidateKind::Memory => Some(format!("m:{id}")),
        CandidateKind::Experience => Some(format!("x:{id}")),
        CandidateKind::DocumentSection => None,
    }
}

pub fn age_days(created_at: u64, now: u64) -> f64 {
    now.saturating_sub(created_at) as f64 / 86_400.0
}

/// `(strengthening, fading)` added to a fused score.
/// Strengthening is `s * ln(1 + recalls)`. Fading is
/// `f * age_days / (age_days + h)`, which approaches `f` and never reaches a
/// point where the row would be removed.
pub fn ranking_terms(recalls: u64, age_days: f64, s: f64, f: f64, h: f64) -> (f64, f64) {
    let strengthen = s * (1.0 + recalls as f64).ln();
    let fade = if age_days.is_finite() && h > 0.0 {
        f * age_days / (age_days + h)
    } else {
        0.0
    };
    (strengthen, fade)
}

/// `1 + strengthening - fading`, clamped to 0..=1.
pub fn effective_fidelity(strengthen: f64, fade: f64) -> f64 {
    (1.0 + strengthen - fade).clamp(0.0, 1.0)
}

pub(crate) fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// After fusion: adjust each memory candidate, then re-sort. Equal adjusted
/// scores keep the RRF order. Document sections are unchanged.
pub fn apply_ranking(
    candidates: &mut [RecallCandidate],
    mut recalls_of: impl FnMut(&str) -> u64,
    now: u64,
    s: f64,
    f: f64,
    h: f64,
) {
    for candidate in candidates.iter_mut() {
        let Some(hit) = candidate.memory.as_ref() else { continue };
        let Some(key) = overlay_key(candidate.kind, hit.id) else { continue };
        let recalls = recalls_of(&key);
        let (strengthen, fade) = ranking_terms(recalls, age_days(hit.created_at, now), s, f, h);
        candidate.score += strengthen - fade;
        candidate.effective_fidelity = Some(effective_fidelity(strengthen, fade));
    }
    candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
}

fn apply_installed_ranking(candidates: &mut [RecallCandidate]) {
    let (s, f, h) = crate::recall_overlay::strength();
    let overlay = crate::recall_overlay::installed();
    apply_ranking(
        candidates,
        |key| overlay.as_ref().map(|store| store.recalls(key)).unwrap_or(0),
        unix_now(),
        s,
        f,
        h,
    );
}

/// Meaning guarantee: the i-th of `keys` (the dense arm's best hits) is
/// moved up to position `2 * i + 1` if fusion left it lower (or it is
/// promoted from beyond the cut). Pure RRF lets items that both arms rank
/// moderately outvote the single best meaning match; this keeps the best
/// meaning matches in view without dropping the lexical leaders (every
/// other item keeps its relative order).
pub fn promote_keys(fused: &mut Vec<RecallCandidate>, keys: &[String]) {
    for (i, key) in keys.iter().enumerate() {
        let target = 2 * i + 1;
        if let Some(at) = fused.iter().position(|c| &c.key() == key) {
            if at > target {
                let item = fused.remove(at);
                fused.insert(target.min(fused.len()), item);
            }
        }
    }
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

/// Build fused candidates from the three lexical source lists (the
/// degraded, embedder-free path: exactly the pre-R2b ranking).
pub fn fuse(
    recovered: &[MemoryHit],
    experience: &[MemoryHit],
    sections: &[SectionEvidence],
    limit: usize,
) -> Vec<RecallCandidate> {
    fuse_arms(lexical_lists(recovered, experience, sections), limit)
}

/// The lexical recovered, lexical experience and BM25 section lists.
pub fn lexical_lists(recovered: &[MemoryHit], experience: &[MemoryHit], sections: &[SectionEvidence]) -> Vec<ArmList> {
    let memory_list = |kind: CandidateKind, hits: &[MemoryHit]| -> Vec<RecallCandidate> {
        hits.iter().enumerate().map(|(i, hit)| RecallCandidate::from_memory(kind, hit, i + 1)).collect()
    };
    vec![
        ArmList { arm: "lexical", weight: 1.0, items: memory_list(CandidateKind::Memory, recovered) },
        ArmList { arm: "lexical", weight: 1.0, items: memory_list(CandidateKind::Experience, experience) },
        ArmList {
            arm: "bm25",
            weight: 1.0,
            items: sections.iter().enumerate().map(|(i, s)| RecallCandidate::from_section(s, i + 1)).collect(),
        },
    ]
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
            keystone: false, created_at: 0, tags: BTreeMap::new(), refs: None,
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
        // Fixture rows are dated at the epoch, so fading (about `f`) sinks
        // both memories under the undated sections. Section order is RRF order.
        assert_eq!(fused[0].kind, CandidateKind::DocumentSection);
        assert_eq!(fused[1].kind, CandidateKind::DocumentSection);
        let s = fused[0].section.as_ref().unwrap();
        assert_eq!(s.text, "verbatim 3");
        assert_eq!(s.cite, "[doc d1§3 p.2]");
        let memory = fused.iter().find(|c| c.memory.as_ref().is_some_and(|m| m.id == 1)).unwrap();
        let fade = ranking_terms(0, age_days(0, unix_now()), crate::recall_overlay::DEFAULT_S, crate::recall_overlay::DEFAULT_F, crate::recall_overlay::DEFAULT_H).1;
        assert!((memory.score - (1.0 / 61.0 - fade)).abs() < 1e-9, "{}", memory.score);
        assert!(memory.effective_fidelity.unwrap() <= 1.0 && memory.effective_fidelity.unwrap() >= 0.0);
        assert!(fused.iter().any(|c| c.kind == CandidateKind::Experience));
        assert_eq!(fuse(&recovered, &experience, &sections, 2).len(), 2);
        assert_eq!(citation("d1", 4, None), "[doc d1§4]");
    }

    #[test]
    fn dense_and_graph_arms_merge_by_key_and_record_arms() {
        let recovered = vec![hit(1, 0.9), hit(2, 0.8)];
        let mut lists = lexical_lists(&recovered, &[], &[section("d1", 0, None)]);
        // Dense finds memory 7 first (not lexical), then memory 2.
        let dense: Vec<RecallCandidate> = [(7, 0.83f32), (2, 0.61)].iter().enumerate()
            .map(|(i, (id, cos))| RecallCandidate::from_memory(CandidateKind::Memory, &hit(*id, *cos), i + 1)).collect();
        lists.push(ArmList { arm: "dense", weight: 1.0, items: dense });
        lists.push(ArmList { arm: "graph", weight: 0.5, items: vec![RecallCandidate::from_memory(CandidateKind::Memory, &hit(9, 0.2), 1)] });
        let fused = fuse_arms(lists, 10);
        // Epoch-dated memories all fade by the same amount, so their RRF
        // order is unchanged and the undated section ranks above them.
        assert_eq!(fused[0].kind, CandidateKind::DocumentSection);
        let best = fused.iter().find(|c| c.memory.is_some()).unwrap();
        assert_eq!(best.memory.as_ref().unwrap().id, 2);
        assert_eq!(best.arms, ["lexical", "dense"]);
        assert!((best.dense_score.unwrap() - 0.61).abs() < 1e-6);
        let seven = fused.iter().find(|c| c.memory.as_ref().is_some_and(|m| m.id == 7)).unwrap();
        assert_eq!(seven.arms, ["dense"]);
        let nine = fused.iter().find(|c| c.memory.as_ref().is_some_and(|m| m.id == 9)).unwrap();
        let fade = ranking_terms(0, age_days(0, unix_now()), crate::recall_overlay::DEFAULT_S, crate::recall_overlay::DEFAULT_F, crate::recall_overlay::DEFAULT_H).1;
        assert!((nine.score - (0.5 / 61.0 - fade)).abs() < 1e-9, "graph arm is weighted lower");
        assert_eq!(fused.last().unwrap().memory.as_ref().unwrap().id, 9);
        assert_eq!(fuse_arms(Vec::new(), 5).len(), 0);
    }

    #[test]
    fn promotion_keeps_the_best_meaning_matches_in_view() {
        let mk = |id: u32| RecallCandidate::from_memory(CandidateKind::Memory, &hit(id, 0.5), 1);
        let mut fused: Vec<RecallCandidate> = (1..=8).map(mk).collect();
        promote_keys(&mut fused, &[mk(7).key(), mk(2).key(), mk(99).key()]);
        let ids: Vec<u32> = fused.iter().map(|c| c.memory.as_ref().unwrap().id).collect();
        // 7 moves to slot 1; 2 is already above slot 3; unknown keys are ignored.
        assert_eq!(ids, [1, 7, 2, 3, 4, 5, 6, 8]);
    }

    #[test]
    fn truncation_respects_char_boundaries() {
        assert_eq!(truncate_bytes("héllo", 2), "h");
        assert_eq!(truncate_bytes("abc", 10), "abc");
    }

    fn at(id: u32, created_at: u64) -> RecallCandidate {
        let mut row = hit(id, 1.0);
        row.created_at = created_at;
        let mut candidate = RecallCandidate::from_memory(CandidateKind::Experience, &row, 1);
        candidate.score = 1.0 / 61.0;
        candidate
    }

    #[test]
    fn five_citations_outrank_an_uncited_twin_with_the_same_text_score() {
        let now = 1_790_846_400;
        let mut rows = vec![at(10, now), at(11, now)];
        apply_ranking(&mut rows, |key| if key == "x:10" { 5 } else { 0 }, now, 0.15, 0.10, 30.0);
        assert_eq!(rows[0].memory.as_ref().unwrap().id, 10);
        assert!(rows[0].score > rows[1].score);
        let (strengthen, fade) = ranking_terms(5, 0.0, 0.15, 0.10, 30.0);
        assert!((rows[0].score - (1.0 / 61.0 + strengthen - fade)).abs() < 1e-12);
        assert_eq!(rows[0].effective_fidelity.unwrap(), 1.0);
        assert_eq!(rows[1].effective_fidelity.unwrap(), 1.0);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn an_old_unrecalled_row_ranks_below_a_new_twin_and_is_still_returned() {
        let now = 1_790_846_400u64;
        let new = at(21, now);
        let old = at(22, now - 400 * 86_400);
        let lists = vec![
            ArmList { arm: "lexical", weight: 1.0, items: vec![new] },
            ArmList { arm: "lexical", weight: 1.0, items: vec![old] },
        ];
        // Direct ranking, with the same clock the assertion uses. Fusion's
        // installed path is covered by `fuse` above; here the ages are fixed.
        let mut rows = lists.into_iter().flat_map(|list| list.items).collect::<Vec<_>>();
        for row in &mut rows {
            row.score = 1.0 / 61.0;
        }
        apply_ranking(&mut rows, |_| 0, now, 0.15, 0.10, 30.0);
        assert_eq!(rows.len(), 2, "fading does not drop the old row");
        assert_eq!(rows[0].memory.as_ref().unwrap().id, 21);
        assert_eq!(rows[1].memory.as_ref().unwrap().id, 22);
        assert!(rows[0].score > rows[1].score);
        let old_fid = rows[1].effective_fidelity.unwrap();
        assert!(old_fid < 1.0 && old_fid > 0.0, "{old_fid}");
        assert_eq!(rows[1].memory.as_ref().unwrap().tags.get("text"), None);
    }

    #[test]
    fn a_recall_run_does_not_change_recovered_bytes() {
        use sha2::{Digest, Sha256};
        use std::fs;
        use std::path::Path;

        fn hash_dir(dir: &Path) -> String {
            let mut files = Vec::new();
            let mut stack = vec![dir.to_path_buf()];
            while let Some(current) = stack.pop() {
                for entry in fs::read_dir(&current).unwrap() {
                    let path = entry.unwrap().path();
                    if path.is_dir() { stack.push(path); } else { files.push(path); }
                }
            }
            files.sort();
            let mut hasher = Sha256::new();
            for path in files {
                let rel = path.strip_prefix(dir).unwrap();
                hasher.update(rel.to_string_lossy().as_bytes());
                hasher.update(fs::read(&path).unwrap());
            }
            format!("{:x}", hasher.finalize())
        }

        let root = std::env::temp_dir().join(format!("ferricula-l2-{}", uuid::Uuid::new_v4()));
        let memory = root.join("memory");
        fs::create_dir_all(&memory).unwrap();
        fs::write(memory.join("identity.json"), r#"{"agent_id":"ferricula-agent","name":"T"}"#).unwrap();
        fs::write(memory.join("agent.toml"), "name = \"T\"\nrole = \"t\"\n").unwrap();
        {
            let mut engine = ferricula_core::DurableEngine::open(&memory).unwrap();
            let row = ferricula_core::Row {
                id: 3,
                vector: vec![1.0, 0.0],
                refs: None,
                tags: std::collections::BTreeMap::from([("text".to_string(), "paging memory between contexts".to_string())]),
            };
            engine.remember(row, ferricula_core::MemoryRecord::new(3)).unwrap();
            engine.checkpoint().unwrap();
        }
        let before = hash_dir(&memory);
        let mut config = crate::config::RuntimeConfig::default();
        config.memory_dir = memory.clone();
        config.state_dir = root.join("state");
        config.overlay.path = config.state_dir.join("overlay.json");
        config.schedule.enabled = false;
        config.require_operator_auth = false;
        let inspection = crate::inspect_data_dir(&memory).unwrap();
        let runtime = crate::runtime::AgentRuntime::open(config, inspection).unwrap();
        let _recall = runtime.hybrid_recall("paging memory", 8);
        let overlay = crate::recall_overlay::RecallOverlay::open(&root.join("state")).unwrap();
        overlay.record("m:3", 1_790_846_400).unwrap();
        drop(runtime);
        let after = hash_dir(&memory);
        assert_eq!(before, after, "recovered store bytes changed");
        let _ = fs::remove_dir_all(root);
    }
}
