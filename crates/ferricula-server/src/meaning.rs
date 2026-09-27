//! Meaning index (R2b): one in-process view over three vector sets in one
//! embedding space (`gtr-t5-base@768` for Steve):
//!
//! - **recovered** memories: the row's stored vector when it is non-zero,
//!   otherwise a sidecar vector made from the row's `text` tag (often cut at
//!   200 characters by v1; recorded as [`VectorSource::TagTextTruncated`]);
//! - **experience** rows (reading, thinking, dream, ...);
//! - **document sections** (verbatim section text).
//!
//! Vectors not stored in the recovered base live in sidecar files under
//! `state_dir/meaning/` (the recovered `memory_dir` is never written), one
//! per set, each written atomically (tmp + fsync + rename). Each entry
//! carries the FNV-1a hash of the exact text it was made from, so an edited
//! or re-sectioned text is re-embedded instead of silently reusing a stale
//! vector. Search is brute-force cosine (adequate to ~10^5 rows) with
//! optional id bitmap, evidence and lifecycle filters.
//!
//! ## Sidecar format (`<set>.fmv`, little-endian)
//!
//! ```text
//! magic   8 bytes  "FMEANV01"
//! set     u8       0 recovered, 1 experience, 2 section
//! space   u16 len + UTF-8 bytes   e.g. "gtr-t5-base@768"
//! dim     u32
//! count   u64
//! entry × count:
//!   key     u16 len + UTF-8   "<id>" or "<doc_id>#<section index>"
//!   hash    u64               FNV-1a 64 of the embedded text
//!   source  u8                VectorSource
//!   chars   u32               characters of the embedded text
//!   vector  f32 × dim         unit length
//! check   u64               FNV-1a 64 of every preceding byte
//! ```
//!
//! A file whose space differs from the configured one is ignored (vectors
//! from different spaces are never compared); a corrupt file is ignored and
//! its rows are re-embedded by the next backfill.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail, ensure};
use roaring::RoaringBitmap;
use serde::Serialize;

pub const MAGIC: &[u8; 8] = b"FMEANV01";
/// Texts longer than this (bytes) are embedded as a prefix (shivvr's
/// per-text limit is 32 KiB; GTR-T5 reads 512 tokens anyway).
pub const MAX_EMBED_BYTES: usize = 30_000;
/// v1 cut `text` tags at this many characters.
pub const V1_TAG_CHARS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MeaningSet {
    Recovered,
    Experience,
    Section,
}

impl MeaningSet {
    pub const ALL: [MeaningSet; 3] = [Self::Recovered, Self::Experience, Self::Section];

    fn code(self) -> u8 {
        match self {
            Self::Recovered => 0,
            Self::Experience => 1,
            Self::Section => 2,
        }
    }

    pub fn file_name(self) -> &'static str {
        match self {
            Self::Recovered => "recovered.fmv",
            Self::Experience => "experience.fmv",
            Self::Section => "sections.fmv",
        }
    }
}

/// What a vector was made from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VectorSource {
    /// The recovered row's own stored vector (never written to a sidecar).
    Stored,
    /// A recovered row's `text` tag, whole.
    TagText,
    /// A recovered row's `text` tag that v1 had already cut (200 chars or
    /// an ellipsis): the vector means only what survived.
    TagTextTruncated,
    /// Full text (experience row, document section).
    Text,
    /// A prefix of a longer text (> [`MAX_EMBED_BYTES`]).
    TextTruncated,
}

impl VectorSource {
    fn code(self) -> u8 {
        match self {
            Self::Stored => 0,
            Self::TagText => 1,
            Self::TagTextTruncated => 2,
            Self::Text => 3,
            Self::TextTruncated => 4,
        }
    }

    fn from_code(code: u8) -> Result<Self> {
        Ok(match code {
            0 => Self::Stored,
            1 => Self::TagText,
            2 => Self::TagTextTruncated,
            3 => Self::Text,
            4 => Self::TextTruncated,
            other => bail!("unknown vector source {other}"),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MeaningKey {
    Recovered(u32),
    Experience(u32),
    Section(String, u32),
}

impl MeaningKey {
    pub fn set(&self) -> MeaningSet {
        match self {
            Self::Recovered(_) => MeaningSet::Recovered,
            Self::Experience(_) => MeaningSet::Experience,
            Self::Section(..) => MeaningSet::Section,
        }
    }

    /// Memory id for recovered/experience keys.
    pub fn id(&self) -> Option<u32> {
        match self {
            Self::Recovered(id) | Self::Experience(id) => Some(*id),
            Self::Section(..) => None,
        }
    }

    fn encode(&self) -> String {
        match self {
            Self::Recovered(id) | Self::Experience(id) => id.to_string(),
            Self::Section(doc, index) => format!("{doc}#{index}"),
        }
    }

    fn decode(set: MeaningSet, text: &str) -> Result<Self> {
        Ok(match set {
            MeaningSet::Recovered => Self::Recovered(text.parse().context("recovered key")?),
            MeaningSet::Experience => Self::Experience(text.parse().context("experience key")?),
            MeaningSet::Section => {
                let (doc, index) = text.rsplit_once('#').context("section key without '#'")?;
                Self::Section(doc.to_string(), index.parse().context("section index")?)
            }
        })
    }
}

/// How a row may be used.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RowFlags {
    /// Never evidence: dreams (experience channel `dream`) and v1 dream
    /// images (recovered text starting with `[dream image]`).
    pub not_evidence: bool,
    /// Recovered row whose lifecycle is Forgiven/Archived (released text
    /// that v1 kept); admitted only under `include_faded_recovered`.
    pub faded: bool,
}

/// One row the index should hold a vector for.
#[derive(Debug, Clone)]
pub struct CatalogItem {
    pub key: MeaningKey,
    /// The exact text to embed (already cut to [`MAX_EMBED_BYTES`]).
    pub text: String,
    pub hash: u64,
    pub source: VectorSource,
    pub flags: RowFlags,
    /// A usable vector already stored with the row (recovered only).
    pub stored: Option<Vec<f32>>,
}

impl CatalogItem {
    /// A recovered row: stored vector if non-zero, else its tag text.
    pub fn recovered(id: u32, text: &str, faded: bool, stored: Option<Vec<f32>>) -> Self {
        let cut = prefix(text, MAX_EMBED_BYTES);
        let trimmed = text.trim_end();
        let truncated = text.chars().count() >= V1_TAG_CHARS
            || trimmed.ends_with("...")
            || trimmed.ends_with('\u{2026}')
            || cut.len() < text.len();
        let source = match (&stored, truncated) {
            (Some(_), _) => VectorSource::Stored,
            (None, true) => VectorSource::TagTextTruncated,
            (None, false) => VectorSource::TagText,
        };
        Self {
            key: MeaningKey::Recovered(id),
            hash: fnv1a64(cut.as_bytes()),
            text: cut.to_string(),
            source,
            flags: RowFlags { not_evidence: is_dream_image(text), faded },
            stored,
        }
    }

    /// An experience row; `dream` channel rows are never evidence.
    pub fn experience(id: u32, text: &str, channel: Option<&str>) -> Self {
        let cut = prefix(text, MAX_EMBED_BYTES);
        Self {
            key: MeaningKey::Experience(id),
            hash: fnv1a64(cut.as_bytes()),
            text: cut.to_string(),
            source: if cut.len() < text.len() { VectorSource::TextTruncated } else { VectorSource::Text },
            flags: RowFlags { not_evidence: channel == Some("dream"), faded: false },
            stored: None,
        }
    }

    pub fn section(doc_id: &str, index: u32, text: &str) -> Self {
        let cut = prefix(text, MAX_EMBED_BYTES);
        Self {
            key: MeaningKey::Section(doc_id.to_string(), index),
            hash: fnv1a64(cut.as_bytes()),
            text: cut.to_string(),
            source: if cut.len() < text.len() { VectorSource::TextTruncated } else { VectorSource::Text },
            flags: RowFlags::default(),
            stored: None,
        }
    }
}

/// v1 stored generated dream images as memories; they are not evidence.
pub fn is_dream_image(text: &str) -> bool {
    text.trim_start().starts_with("[dream image]")
}

#[derive(Debug, Clone)]
struct Embedded {
    hash: u64,
    source: VectorSource,
    chars: u32,
    vector: Arc<Vec<f32>>,
}

#[derive(Debug, Clone)]
struct LiveRow {
    key: MeaningKey,
    flags: RowFlags,
    vector: Arc<Vec<f32>>,
}

#[derive(Debug, Default)]
struct SetState {
    /// Sidecar vectors (whatever texts they were made from).
    embedded: HashMap<MeaningKey, Embedded>,
    /// Rows the set should cover right now.
    catalog: Vec<CatalogItem>,
    /// Catalog rows with a vector for their current text.
    live: Vec<LiveRow>,
    live_pos: HashMap<MeaningKey, usize>,
    /// Catalog rows whose vector is the stored one.
    stored: usize,
    dirty: bool,
}

impl SetState {
    fn rebuild_live(&mut self) {
        self.live.clear();
        self.live_pos.clear();
        self.stored = 0;
        for item in &self.catalog {
            let vector = match &item.stored {
                Some(v) => {
                    self.stored += 1;
                    Some(Arc::new(unit(v)))
                }
                None => self.embedded.get(&item.key).filter(|e| e.hash == item.hash).map(|e| e.vector.clone()),
            };
            if let Some(vector) = vector {
                self.live_pos.insert(item.key.clone(), self.live.len());
                self.live.push(LiveRow { key: item.key.clone(), flags: item.flags, vector });
            }
        }
    }
}

/// Filters for a dense search.
#[derive(Debug, Clone, Copy)]
pub struct SearchFilter<'a> {
    pub recovered: bool,
    pub experience: bool,
    pub sections: bool,
    /// Skip dreams and v1 dream images.
    pub evidence_only: bool,
    /// Admit faded (Forgiven/Archived) recovered rows.
    pub include_faded: bool,
    /// Recovered/experience ids allowed (None = all).
    pub allowed: Option<&'a RoaringBitmap>,
}

impl SearchFilter<'_> {
    /// Waking recall: all three sets, evidence only.
    pub fn evidence(include_faded: bool) -> Self {
        Self {
            recovered: true,
            experience: true,
            sections: true,
            evidence_only: true,
            include_faded,
            allowed: None,
        }
    }

    /// Memories only (recovered + experience), evidence only.
    pub fn memories(include_faded: bool) -> Self {
        Self { sections: false, ..Self::evidence(include_faded) }
    }

    fn admits(&self, row: &LiveRow) -> bool {
        let set_ok = match row.key.set() {
            MeaningSet::Recovered => self.recovered,
            MeaningSet::Experience => self.experience,
            MeaningSet::Section => self.sections,
        };
        if !set_ok || (self.evidence_only && row.flags.not_evidence) || (row.flags.faded && !self.include_faded) {
            return false;
        }
        match (self.allowed, row.key.id()) {
            (Some(bitmap), Some(id)) => bitmap.contains(id),
            _ => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DenseHit {
    pub key: MeaningKey,
    pub cosine: f64,
}

/// `/status` → `meaning`.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct MeaningCounts {
    pub space: String,
    /// Recovered rows that carry text.
    pub recovered_total: usize,
    /// Recovered rows with a vector (stored or sidecar).
    pub recovered_embedded: usize,
    /// Of those, rows using their own stored vector.
    pub recovered_stored: usize,
    pub experience_total: usize,
    pub experience_embedded: usize,
    pub sections_total: usize,
    pub sections_embedded: usize,
    /// Rows still waiting for a vector.
    pub pending: usize,
    /// Sidecar vectors whose text is truncated (v1 200-char tags or long
    /// sections): they mean only the prefix that was embedded.
    pub truncated_sources: usize,
}

pub struct MeaningIndex {
    space: String,
    dim: usize,
    dir: Option<PathBuf>,
    sets: BTreeMap<MeaningSet, SetState>,
}

impl MeaningIndex {
    /// An empty index for `space`/`dim`; `dir` holds the sidecars (None:
    /// memory only, for tests and benchmarks).
    pub fn new(space: &str, dim: usize, dir: Option<PathBuf>) -> Self {
        let sets = MeaningSet::ALL.iter().map(|s| (*s, SetState::default())).collect();
        Self { space: space.to_string(), dim, dir, sets }
    }

    pub fn space(&self) -> &str {
        &self.space
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    /// Load every sidecar present. A missing file is empty; a file in
    /// another space or failing its checksum is skipped with a note.
    pub fn load(&mut self) -> Vec<String> {
        let mut notes = Vec::new();
        let Some(dir) = self.dir.clone() else { return notes };
        for set in MeaningSet::ALL {
            let path = dir.join(set.file_name());
            if !path.exists() {
                continue;
            }
            match read_sidecar(&path, set, &self.space, self.dim) {
                Ok(entries) => {
                    let state = self.sets.get_mut(&set).expect("set");
                    state.embedded = entries;
                    state.rebuild_live();
                }
                Err(error) => notes.push(format!("{}: {error:#} (ignored; will re-embed)", path.display())),
            }
        }
        notes
    }

    /// Replace a set's catalog (the rows it should cover now).
    pub fn set_catalog(&mut self, set: MeaningSet, items: Vec<CatalogItem>) {
        let state = self.sets.get_mut(&set).expect("set");
        state.catalog = items;
        state.rebuild_live();
    }

    /// Catalog rows of `sets` with no vector for their current text.
    pub fn pending(&self, sets: &[MeaningSet]) -> Vec<CatalogItem> {
        sets.iter()
            .flat_map(|set| {
                let state = &self.sets[set];
                state.catalog.iter().filter(|item| !state.live_pos.contains_key(&item.key)).cloned()
            })
            .collect()
    }

    /// Record a freshly embedded vector for a catalog row (normalized).
    pub fn insert(&mut self, item: &CatalogItem, vector: Vec<f32>) -> Result<()> {
        ensure!(vector.len() == self.dim, "vector has {} dims, space {} expects {}", vector.len(), self.space, self.dim);
        ensure!(vector.iter().all(|x| x.is_finite()), "vector has non-finite values");
        let vector = unit(&vector);
        ensure!(vector.iter().any(|x| *x != 0.0), "zero vector");
        let state = self.sets.get_mut(&item.key.set()).expect("set");
        let vector = Arc::new(vector);
        state.embedded.insert(item.key.clone(), Embedded {
            hash: item.hash,
            source: item.source,
            chars: u32::try_from(item.text.chars().count()).unwrap_or(u32::MAX),
            vector: vector.clone(),
        });
        state.dirty = true;
        if let Some(current) = state.catalog.iter().find(|c| c.key == item.key && c.hash == item.hash) {
            let row = LiveRow { key: item.key.clone(), flags: current.flags, vector };
            match state.live_pos.get(&item.key) {
                Some(&at) => state.live[at] = row,
                None => {
                    state.live_pos.insert(item.key.clone(), state.live.len());
                    state.live.push(row);
                }
            }
        }
        Ok(())
    }

    /// Write every changed set's sidecar atomically. Only vectors for rows
    /// still in the catalog are kept (a removed row's vector is dropped).
    pub fn persist(&mut self) -> Result<()> {
        let Some(dir) = self.dir.clone() else {
            for state in self.sets.values_mut() {
                state.dirty = false;
            }
            return Ok(());
        };
        fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        for (set, state) in self.sets.iter_mut() {
            if !state.dirty {
                continue;
            }
            let keep: std::collections::HashSet<&MeaningKey> = state.catalog.iter().map(|c| &c.key).collect();
            let mut entries: Vec<(&MeaningKey, &Embedded)> =
                state.embedded.iter().filter(|(k, _)| keep.contains(k)).collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            write_sidecar(&dir.join(set.file_name()), *set, &self.space, self.dim, &entries)?;
            state.dirty = false;
        }
        Ok(())
    }

    pub fn counts(&self) -> MeaningCounts {
        let r = &self.sets[&MeaningSet::Recovered];
        let x = &self.sets[&MeaningSet::Experience];
        let s = &self.sets[&MeaningSet::Section];
        let pending = [r, x, s].iter().map(|st| st.catalog.len() - st.live.len()).sum();
        let truncated_sources = [r, x, s]
            .iter()
            .flat_map(|st| st.live.iter().filter_map(|row| st.embedded.get(&row.key)))
            .filter(|e| matches!(e.source, VectorSource::TagTextTruncated | VectorSource::TextTruncated))
            .count();
        MeaningCounts {
            space: self.space.clone(),
            recovered_total: r.catalog.len(),
            recovered_embedded: r.live.len(),
            recovered_stored: r.stored,
            experience_total: x.catalog.len(),
            experience_embedded: x.live.len(),
            sections_total: s.catalog.len(),
            sections_embedded: s.live.len(),
            pending,
            truncated_sources,
        }
    }

    /// The live vector for a row, if it has one.
    pub fn vector(&self, key: &MeaningKey) -> Option<Arc<Vec<f32>>> {
        let state = &self.sets[&key.set()];
        state.live_pos.get(key).map(|&at| state.live[at].vector.clone())
    }

    pub fn is_empty(&self) -> bool {
        self.sets.values().all(|s| s.live.is_empty())
    }

    fn rows<'a>(&'a self, filter: &'a SearchFilter<'a>) -> impl Iterator<Item = &'a LiveRow> + 'a {
        self.sets.values().flat_map(|s| s.live.iter()).filter(move |row| filter.admits(row))
    }

    /// Top-`k` rows by cosine to `query` (need not be unit length).
    pub fn search(&self, query: &[f32], k: usize, filter: &SearchFilter) -> Vec<DenseHit> {
        if query.len() != self.dim || k == 0 {
            return Vec::new();
        }
        let q = unit(query);
        let mut scored: Vec<(f64, &MeaningKey)> =
            self.rows(filter).map(|row| (dot(&q, &row.vector), &row.key)).collect();
        let k = k.min(scored.len());
        if k == 0 {
            return Vec::new();
        }
        scored.select_nth_unstable_by(k - 1, |a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        scored.truncate(k);
        scored.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        scored.into_iter().map(|(cosine, key)| DenseHit { key: key.clone(), cosine }).collect()
    }

    /// Best cosine of `query` over the filtered rows.
    pub fn max_cosine(&self, query: &[f32], filter: &SearchFilter) -> Option<f64> {
        if query.len() != self.dim {
            return None;
        }
        let q = unit(query);
        self.rows(filter).map(|row| dot(&q, &row.vector)).fold(None, |acc, c| Some(acc.map_or(c, |a: f64| a.max(c))))
    }

    /// Neighborhood density of a row: mean cosine to its `k` nearest other
    /// rows under `filter`. Low density = outlier.
    pub fn density(&self, key: &MeaningKey, k: usize, filter: &SearchFilter) -> Option<f64> {
        let v = self.vector(key)?;
        let mut sims: Vec<f64> = self.rows(filter).filter(|row| &row.key != key).map(|row| dot(&v, &row.vector)).collect();
        if sims.is_empty() || k == 0 {
            return None;
        }
        let k = k.min(sims.len());
        sims.select_nth_unstable_by(k - 1, |a, b| b.total_cmp(a));
        Some(sims[..k].iter().sum::<f64>() / k as f64)
    }

    /// Mean (unit) section vector per document, over embedded sections.
    pub fn document_means(&self) -> HashMap<String, Vec<f32>> {
        let mut groups: HashMap<String, Vec<Arc<Vec<f32>>>> = HashMap::new();
        for row in &self.sets[&MeaningSet::Section].live {
            if let MeaningKey::Section(doc, _) = &row.key {
                groups.entry(doc.clone()).or_default().push(row.vector.clone());
            }
        }
        groups
            .into_iter()
            .filter_map(|(doc, vs)| mean_unit(vs.iter().map(|v| v.as_slice())).map(|m| (doc, m)))
            .collect()
    }
}

/// Dot product in f64.
pub fn dot(a: &[f32], b: &[f32]) -> f64 {
    a.iter().zip(b).map(|(x, y)| f64::from(*x) * f64::from(*y)).sum()
}

/// `v / |v|` (a zero vector stays zero).
pub fn unit(v: &[f32]) -> Vec<f32> {
    let n = v.iter().map(|x| f64::from(*x) * f64::from(*x)).sum::<f64>().sqrt();
    if n == 0.0 { v.to_vec() } else { v.iter().map(|x| (f64::from(*x) / n) as f32).collect() }
}

/// Unit mean of unit vectors; None for an empty or cancelling set.
pub fn mean_unit<'a>(vectors: impl IntoIterator<Item = &'a [f32]>) -> Option<Vec<f32>> {
    let mut sum: Vec<f64> = Vec::new();
    let mut n = 0usize;
    for v in vectors {
        if sum.is_empty() {
            sum = vec![0.0; v.len()];
        }
        if v.len() != sum.len() {
            continue;
        }
        for (s, x) in sum.iter_mut().zip(v) {
            *s += f64::from(*x);
        }
        n += 1;
    }
    if n == 0 {
        return None;
    }
    let mean: Vec<f32> = sum.iter().map(|s| (*s / n as f64) as f32).collect();
    mean.iter().any(|x| *x != 0.0).then(|| unit(&mean))
}

/// The `keep` candidates least similar to `centroid` (farthest first).
/// Candidates are `(item, unit vector)`.
pub fn farthest_from<T: Clone>(centroid: &[f32], candidates: &[(T, Arc<Vec<f32>>)], keep: usize) -> Vec<(T, f64)> {
    let mut scored: Vec<(T, f64)> = candidates.iter().map(|(t, v)| (t.clone(), dot(centroid, v))).collect();
    scored.sort_by(|a, b| a.1.total_cmp(&b.1));
    scored.truncate(keep);
    scored
}

/// Most query segments embedded per recall.
pub const MAX_QUERY_SEGMENTS: usize = 4;

/// The parts of a message that are embedded separately: its sentences (at
/// least 3 words), at most [`MAX_QUERY_SEGMENTS`] of them; a message of one
/// sentence (or none) is embedded whole. A greeting and a question then
/// each get their own vector instead of one blurred average.
pub fn query_segments(text: &str) -> Vec<String> {
    let mut segments = sentences(text);
    segments.truncate(MAX_QUERY_SEGMENTS);
    if segments.len() <= 1 {
        return vec![text.trim().to_string()];
    }
    segments
}

/// Combine per-segment dense result lists: weighted RRF (k = 60) where
/// each segment's weight is its novelty (1 − its best cosine to memory)
/// relative to the most novel segment, so familiar small talk ("Steve,
/// it's Kord.") does not crowd out the part of the message that asks for
/// something. Each key keeps its best cosine. A single list passes through.
pub fn fuse_segments(lists: Vec<(Vec<DenseHit>, f64)>, k: usize) -> Vec<DenseHit> {
    if lists.len() == 1 {
        let mut only = lists.into_iter().next().map(|(l, _)| l).unwrap_or_default();
        only.truncate(k);
        return only;
    }
    let top = lists.iter().map(|(_, n)| *n).fold(0.0f64, f64::max).max(1e-6);
    let mut score: HashMap<MeaningKey, (f64, f64)> = HashMap::new();
    for (list, novelty) in &lists {
        let weight = (novelty / top).clamp(0.0, 1.0);
        for (rank0, hit) in list.iter().enumerate() {
            let e = score.entry(hit.key.clone()).or_insert((0.0, f64::NEG_INFINITY));
            e.0 += weight / (60.0 + (rank0 + 1) as f64);
            e.1 = e.1.max(hit.cosine);
        }
    }
    let mut out: Vec<(MeaningKey, f64, f64)> = score.into_iter().map(|(key, (s, c))| (key, s, c)).collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| b.2.total_cmp(&a.2)).then_with(|| a.0.cmp(&b.0)));
    out.truncate(k);
    out.into_iter().map(|(key, _, cosine)| DenseHit { key, cosine }).collect()
}

/// One graph hop from `seeds` (in seed order): each seed's neighbors not in
/// `exclude` and not already taken, ordered by `cosine` descending within a
/// seed. Returns `(id, cosine, via seed)`.
pub fn graph_hop(
    seeds: &[u32],
    exclude: &std::collections::HashSet<u32>,
    neighbors: impl Fn(u32) -> Vec<u32>,
    cosine: impl Fn(u32) -> f64,
) -> Vec<(u32, f64, u32)> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for &seed in seeds {
        let mut hop: Vec<(u32, f64)> = neighbors(seed)
            .into_iter()
            .filter(|id| *id != seed && !exclude.contains(id) && seen.insert(*id))
            .map(|id| (id, cosine(id)))
            .collect();
        hop.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        out.extend(hop.into_iter().map(|(id, c)| (id, c, seed)));
    }
    out
}

/// Sentences of a dream (split on . ! ? and newlines), at least 3 words.
pub fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        current.push(c);
        if matches!(c, '.' | '!' | '?' | '\n') {
            push_sentence(&mut current, &mut out);
        }
    }
    push_sentence(&mut current, &mut out);
    out
}

fn push_sentence(current: &mut String, out: &mut Vec<String>) {
    let s = current.trim();
    if s.split_whitespace().count() >= 3 {
        out.push(s.to_string());
    }
    current.clear();
}

/// Grounding: per sentence, the best cosine to any trace; returns
/// `(fraction with best >= threshold, per-sentence best)`.
pub fn grounding(sentences: &[Vec<f32>], traces: &[Vec<f32>], threshold: f64) -> (f64, Vec<f64>) {
    let traces: Vec<Vec<f32>> = traces.iter().map(|t| unit(t)).collect();
    let best: Vec<f64> = sentences
        .iter()
        .map(|s| {
            let s = unit(s);
            traces.iter().map(|t| dot(&s, t)).fold(f64::NEG_INFINITY, f64::max)
        })
        .map(|b| if b.is_finite() { b } else { 0.0 })
        .collect();
    if best.is_empty() {
        return (0.0, best);
    }
    let grounded = best.iter().filter(|b| **b >= threshold).count();
    (grounded as f64 / best.len() as f64, best)
}

/// FNV-1a 64-bit.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn prefix(text: &str, max: usize) -> &str {
    crate::recall::truncate_bytes(text, max)
}

fn write_sidecar(
    path: &Path,
    set: MeaningSet,
    space: &str,
    dim: usize,
    entries: &[(&MeaningKey, &Embedded)],
) -> Result<()> {
    let mut buf: Vec<u8> = Vec::with_capacity(64 + entries.len() * (dim * 4 + 32));
    buf.extend_from_slice(MAGIC);
    buf.push(set.code());
    buf.extend_from_slice(&u16::try_from(space.len())?.to_le_bytes());
    buf.extend_from_slice(space.as_bytes());
    buf.extend_from_slice(&u32::try_from(dim)?.to_le_bytes());
    buf.extend_from_slice(&(entries.len() as u64).to_le_bytes());
    for (key, e) in entries {
        let k = key.encode();
        buf.extend_from_slice(&u16::try_from(k.len())?.to_le_bytes());
        buf.extend_from_slice(k.as_bytes());
        buf.extend_from_slice(&e.hash.to_le_bytes());
        buf.push(e.source.code());
        buf.extend_from_slice(&e.chars.to_le_bytes());
        ensure!(e.vector.len() == dim, "vector for {k} has {} dims", e.vector.len());
        for x in e.vector.iter() {
            buf.extend_from_slice(&x.to_le_bytes());
        }
    }
    let check = fnv1a64(&buf);
    buf.extend_from_slice(&check.to_le_bytes());
    let tmp = path.with_extension("fmv.tmp");
    {
        let mut file = fs::File::create(&tmp).with_context(|| format!("create {}", tmp.display()))?;
        file.write_all(&buf)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path).with_context(|| format!("rename to {}", path.display()))?;
    Ok(())
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        ensure!(self.at + n <= self.bytes.len(), "truncated sidecar");
        let out = &self.bytes[self.at..self.at + n];
        self.at += n;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into()?))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into()?))
    }
    fn string(&mut self) -> Result<String> {
        let n = self.u16()? as usize;
        Ok(std::str::from_utf8(self.take(n)?)?.to_string())
    }
}

fn read_sidecar(path: &Path, set: MeaningSet, space: &str, dim: usize) -> Result<HashMap<MeaningKey, Embedded>> {
    let bytes = fs::read(path)?;
    ensure!(bytes.len() >= 8 + 8, "file too short");
    let (body, check) = bytes.split_at(bytes.len() - 8);
    ensure!(fnv1a64(body) == u64::from_le_bytes(check.try_into()?), "checksum mismatch");
    let mut c = Cursor { bytes: body, at: 0 };
    ensure!(c.take(8)? == MAGIC, "bad magic");
    ensure!(c.u8()? == set.code(), "file belongs to another set");
    let file_space = c.string()?;
    ensure!(file_space == space, "space {file_space:?} differs from configured {space:?}");
    let file_dim = c.u32()? as usize;
    ensure!(file_dim == dim, "dim {file_dim} differs from {dim}");
    let count = c.u64()?;
    let mut out = HashMap::new();
    for _ in 0..count {
        let key = MeaningKey::decode(set, &c.string()?)?;
        let hash = c.u64()?;
        let source = VectorSource::from_code(c.u8()?)?;
        let chars = c.u32()?;
        let raw = c.take(dim * 4)?;
        let vector: Vec<f32> = raw.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().expect("4 bytes"))).collect();
        out.insert(key, Embedded { hash, source, chars, vector: Arc::new(vector) });
    }
    ensure!(c.at == body.len(), "trailing bytes");
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Deterministic fake embedding: a bag of hashed word buckets, so texts
    /// sharing words are near and unrelated texts are far.
    pub fn fake_vector(text: &str, dim: usize) -> Vec<f32> {
        let mut v = vec![0f32; dim];
        for word in text.split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() >= 3) {
            let h = fnv1a64(word.to_lowercase().as_bytes());
            v[(h % dim as u64) as usize] += 1.0;
        }
        if v.iter().all(|x| *x == 0.0) {
            v[0] = 1.0;
        }
        unit(&v)
    }

    fn dir() -> PathBuf {
        std::env::temp_dir().join(format!("ferricula-meaning-{}", uuid::Uuid::new_v4()))
    }

    fn embed_all(index: &mut MeaningIndex, dim: usize) {
        for item in index.pending(&MeaningSet::ALL) {
            index.insert(&item, fake_vector(&item.text, dim)).unwrap();
        }
    }

    #[test]
    fn catalog_stored_vectors_pending_and_search_filters() {
        let dim = 64;
        let mut index = MeaningIndex::new("fake@64", dim, None);
        index.set_catalog(MeaningSet::Recovered, vec![
            CatalogItem::recovered(1, "Raised by Paul and Clara, a machinist and a bookkeeper", true, None),
            CatalogItem::recovered(2, "Designing the iMac with Jony", false, Some(fake_vector("Designing the iMac with Jony", dim))),
            CatalogItem::recovered(3, "[dream image] a machinist in a garden", false, None),
        ]);
        index.set_catalog(MeaningSet::Experience, vec![
            CatalogItem::experience(10, "I dreamed of a machinist and a bookkeeper", Some("dream")),
            CatalogItem::experience(11, "I read about bookkeeper careers", Some("reading")),
        ]);
        index.set_catalog(MeaningSet::Section, vec![CatalogItem::section("d1", 0, "A section on machinist tools")]);
        let c = index.counts();
        assert_eq!((c.recovered_total, c.recovered_embedded, c.recovered_stored, c.pending), (3, 1, 1, 5));
        assert_eq!(index.pending(&[MeaningSet::Recovered]).len(), 2);
        embed_all(&mut index, dim);
        assert_eq!(index.counts().pending, 0);

        let q = fake_vector("machinist bookkeeper", dim);
        // Faded row 1 excluded by default; dream + dream image excluded as evidence.
        let hits = index.search(&q, 10, &SearchFilter::evidence(false));
        let keys: Vec<MeaningKey> = hits.iter().map(|h| h.key.clone()).collect();
        assert!(!keys.contains(&MeaningKey::Recovered(1)));
        assert!(!keys.contains(&MeaningKey::Recovered(3)));
        assert!(!keys.contains(&MeaningKey::Experience(10)));
        assert!(keys.contains(&MeaningKey::Experience(11)));
        let hits = index.search(&q, 10, &SearchFilter::evidence(true));
        assert_eq!(hits[0].key, MeaningKey::Recovered(1));
        assert!(hits.windows(2).all(|w| w[0].cosine >= w[1].cosine));
        // Bitmap filter.
        let allowed: RoaringBitmap = [11u32].into_iter().collect();
        let filter = SearchFilter { allowed: Some(&allowed), sections: false, ..SearchFilter::evidence(true) };
        let hits = index.search(&q, 10, &filter);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].key, MeaningKey::Experience(11));
        // Truncation is recorded.
        let long = "x".repeat(250);
        assert_eq!(CatalogItem::recovered(9, &long, false, None).source, VectorSource::TagTextTruncated);
        assert_eq!(CatalogItem::recovered(9, "short and whole", false, None).source, VectorSource::TagText);
        assert!(index.max_cosine(&q, &SearchFilter::memories(true)).unwrap() > 0.4);
    }

    #[test]
    fn sidecar_roundtrip_hash_staleness_and_space_guard() {
        let dim = 16;
        let d = dir();
        let mut index = MeaningIndex::new("fake@16", dim, Some(d.clone()));
        let items = vec![
            CatalogItem::recovered(5, "alpha beta gamma", false, None),
            CatalogItem::recovered(6, "delta epsilon", true, None),
        ];
        index.set_catalog(MeaningSet::Recovered, items.clone());
        index.set_catalog(MeaningSet::Section, vec![CatalogItem::section("doc-a", 3, "zeta eta theta")]);
        embed_all(&mut index, dim);
        index.persist().unwrap();
        assert!(d.join("recovered.fmv").exists() && d.join("sections.fmv").exists());
        assert!(!d.join("experience.fmv").exists(), "untouched sets are not written");

        let mut again = MeaningIndex::new("fake@16", dim, Some(d.clone()));
        assert!(again.load().is_empty());
        again.set_catalog(MeaningSet::Recovered, items.clone());
        again.set_catalog(MeaningSet::Section, vec![CatalogItem::section("doc-a", 3, "zeta eta theta")]);
        assert_eq!(again.counts().pending, 0);
        let v = again.vector(&MeaningKey::Section("doc-a".into(), 3)).unwrap();
        assert!((dot(&v, &fake_vector("zeta eta theta", dim)) - 1.0).abs() < 1e-6);
        // Changed text → stale vector ignored → pending again.
        again.set_catalog(MeaningSet::Recovered, vec![CatalogItem::recovered(5, "alpha beta CHANGED", false, None), items[1].clone()]);
        assert_eq!(again.pending(&[MeaningSet::Recovered]).len(), 1);

        // Another space: ignored, not compared.
        let mut other = MeaningIndex::new("other@16", dim, Some(d.clone()));
        let notes = other.load();
        assert_eq!(notes.len(), 2, "{notes:?}");
        other.set_catalog(MeaningSet::Recovered, items);
        assert_eq!(other.counts().pending, 2);

        // Corruption: checksum catches a flipped byte.
        let path = d.join("recovered.fmv");
        let mut bytes = fs::read(&path).unwrap();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0xff;
        fs::write(&path, bytes).unwrap();
        let mut broken = MeaningIndex::new("fake@16", dim, Some(d.clone()));
        let notes = broken.load();
        assert!(notes.iter().any(|n| n.contains("checksum")), "{notes:?}");
        fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn segments_and_novelty_weighted_segment_fusion() {
        assert_eq!(query_segments("What do I name my phones?"), ["What do I name my phones?"]);
        assert_eq!(query_segments("hi"), ["hi"]);
        let two = query_segments("Steve, it's Kord. Did you call your father Dad, or Paul?");
        assert_eq!(two, ["Steve, it's Kord.", "Did you call your father Dad, or Paul?"]);
        let hit = |id: u32, c: f64| DenseHit { key: MeaningKey::Recovered(id), cosine: c };
        // Greeting list (familiar: novelty 0.3) vs question list (novelty 0.6).
        let greeting = vec![hit(1, 0.70), hit(2, 0.68), hit(3, 0.66)];
        let question = vec![hit(9, 0.40), hit(8, 0.39), hit(2, 0.30)];
        let fused = fuse_segments(vec![(greeting.clone(), 0.3), (question, 0.6)], 10);
        let ids: Vec<u32> = fused.iter().filter_map(|h| h.key.id()).collect();
        // 2 is in both lists; the question's top hits outrank the greeting's.
        assert_eq!(ids[..3], [2, 9, 8]);
        assert!((fused[0].cosine - 0.68).abs() < 1e-12, "best cosine kept");
        assert_eq!(fuse_segments(vec![(greeting, 0.3)], 2).len(), 2);
    }

    #[test]
    fn density_far_sampling_means_and_grounding() {
        let dim = 64;
        let mut index = MeaningIndex::new("fake@64", dim, None);
        let texts = ["apple design iMac", "apple design iPod", "apple design iPhone", "apple design laptop", "orbital mechanics of comets"];
        index.set_catalog(
            MeaningSet::Recovered,
            texts.iter().enumerate().map(|(i, t)| CatalogItem::recovered(i as u32, t, false, None)).collect(),
        );
        index.set_catalog(MeaningSet::Section, vec![
            CatalogItem::section("a", 0, "garage computer board"),
            CatalogItem::section("a", 1, "garage computer case"),
            CatalogItem::section("b", 0, "garage computer board"),
        ]);
        embed_all(&mut index, dim);
        let f = SearchFilter::memories(false);
        let dense = index.density(&MeaningKey::Recovered(0), 3, &f).unwrap();
        let outlier = index.density(&MeaningKey::Recovered(4), 3, &f).unwrap();
        assert!(outlier < dense, "{outlier} !< {dense}");

        let centroid = mean_unit([index.vector(&MeaningKey::Recovered(0)).unwrap().as_slice()]).unwrap();
        let cands: Vec<(u32, Arc<Vec<f32>>)> =
            (1..5).map(|i| (i, index.vector(&MeaningKey::Recovered(i)).unwrap())).collect();
        let far = farthest_from(&centroid, &cands, 1);
        assert_eq!(far[0].0, 4);

        let means = index.document_means();
        assert_eq!(means.len(), 2);
        assert!(dot(&means["a"], &means["b"]) > 0.8);

        let s = sentences("I walk into the garage. A board hums!\nOk.  Comets fall over the orchard tonight?");
        assert_eq!(s, ["I walk into the garage.", "A board hums!", "Comets fall over the orchard tonight?"]);
        let sv: Vec<Vec<f32>> = s.iter().map(|t| fake_vector(t, dim)).collect();
        let tv = vec![fake_vector("the garage where we built the board", dim)];
        let (fraction, best) = grounding(&sv, &tv, 0.35);
        assert_eq!(best.len(), 3);
        assert!((fraction - 1.0 / 3.0).abs() < 1e-9 && best[0] > 0.5, "{best:?}");
        assert_eq!(grounding(&[], &tv, 0.35).0, 0.0);
    }
}
