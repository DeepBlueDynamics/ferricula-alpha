use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result, bail};
use ferricula_core::memory::Provenance;
use ferricula_core::{DurableEngine, EdgeKind, LifecycleState, MemoryRecord, MemoryRef, Row};
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
    /// Record time (unix seconds) from the lifecycle envelope.
    pub created_at: u64,
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
        self.recall_candidates_with(query, limit, false)
    }

    /// Lexical recall; `include_faded` also admits Forgiven/Archived rows
    /// whose text is still present (`[recall] include_faded_recovered`).
    pub fn recall_candidates_with(&self, query: &str, limit: usize, include_faded: bool) -> Vec<MemoryHit> {
        let engine = self.engine.lock().expect("memory engine poisoned");
        lexical_hits_with(&engine, query, limit, include_faded)
    }

    /// Every recovered row that carries text, for the meaning index:
    /// `(id, text, lifecycle, stored vector if non-zero)`. Read-only.
    pub fn meaning_catalog(&self) -> Vec<RecoveredText> {
        let engine = self.engine.lock().expect("memory engine poisoned");
        let mut out: Vec<RecoveredText> = engine
            .engine()
            .rows_iter()
            .filter_map(|row| {
                let text = row.tags.get("text")?;
                if text.trim().is_empty() {
                    return None;
                }
                let state = engine.memory_store().get(row.id)?.state;
                let stored = row.vector.iter().any(|x| *x != 0.0).then(|| row.vector.clone());
                Some(RecoveredText { id: row.id, text: text.clone(), state, stored })
            })
            .collect();
        out.sort_by_key(|r| r.id);
        out
    }

    /// Hits for specific ids (dense and graph candidates), in the given
    /// order, each scored with the paired score. Rows without text or with
    /// a lifecycle excluded by `include_faded` are skipped.
    pub fn hits_for(&self, ids: &[(u32, f32)], include_faded: bool) -> Vec<MemoryHit> {
        let engine = self.engine.lock().expect("memory engine poisoned");
        ids.iter()
            .filter_map(|(id, score)| {
                let row = engine.engine().get(*id)?;
                let record = engine.memory_store().get(*id)?;
                if !state_admitted(record.state, include_faded) {
                    return None;
                }
                Some(MemoryHit {
                    id: *id,
                    score: *score,
                    state: record.state.into(),
                    fidelity: record.fidelity,
                    importance: record.importance,
                    keystone: record.keystone,
                    created_at: record.created_at,
                    tags: row.tags.clone(),
                    refs: row.refs.clone(),
                })
            })
            .collect()
    }

    /// One-hop graph neighbors of a recovered memory (recovered graph).
    pub fn neighbors(&self, id: u32) -> Vec<u32> {
        let engine = self.engine.lock().expect("memory engine poisoned");
        engine.graph().neighbors(id).iter().collect()
    }

    /// Every id present in the recovered base (rows or records).
    pub fn ids(&self) -> HashSet<u32> {
        let engine = self.engine.lock().expect("memory engine poisoned");
        engine
            .engine()
            .rows_iter()
            .map(|row| row.id)
            .chain(engine.memory_store().all_records().iter().map(|r| r.id))
            .collect()
    }

    /// Up to `n` distinct Active recovered memories that carry text, drawn
    /// by a splitmix64 stream from `seed`. Read-only.
    pub fn sample(&self, seed: u64, n: usize) -> Vec<MemoryHit> {
        let engine = self.engine.lock().expect("memory engine poisoned");
        let mut pool: Vec<&Row> = engine
            .engine()
            .rows_iter()
            .filter(|row| {
                row.tags.get("text").is_some_and(|t| !t.trim().is_empty())
                    && engine
                        .memory_store()
                        .get(row.id)
                        .is_some_and(|r| r.state == LifecycleState::Active)
            })
            .collect();
        let mut state = seed;
        let mut out = Vec::new();
        while out.len() < n && !pool.is_empty() {
            state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^= z >> 31;
            let row = pool.swap_remove((z % pool.len() as u64) as usize);
            let Some(record) = engine.memory_store().get(row.id) else { continue };
            out.push(MemoryHit {
                id: row.id,
                score: 0.0,
                state: record.state.into(),
                fidelity: record.fidelity,
                importance: record.importance,
                keystone: record.keystone,
                created_at: record.created_at,
                tags: row.tags.clone(),
                refs: row.refs.clone(),
            });
        }
        out
    }

    /// Up to `n` Active recovered memories (lowest ids first) whose text is
    /// short (< `max_chars` characters), not visibly truncated, and which
    /// carry a stored vector: `(id, text, vector)`. Used by the embedding
    /// space probe. Read-only.
    pub fn probe_samples(&self, max_chars: usize, n: usize) -> Vec<(u32, String, Vec<f32>)> {
        let engine = self.engine.lock().expect("memory engine poisoned");
        let mut out: Vec<(u32, String, Vec<f32>)> = engine
            .engine()
            .rows_iter()
            // Non-empty and non-zero: the recovered base holds some all-zero
            // placeholder vectors (rows written without an embedding).
            .filter(|row| row.vector.iter().any(|x| *x != 0.0))
            .filter(|row| {
                engine
                    .memory_store()
                    .get(row.id)
                    .is_some_and(|r| r.state == LifecycleState::Active)
            })
            .filter_map(|row| {
                // The stored text verbatim: it is what was embedded.
                let raw = row.tags.get("text")?;
                let text = raw.trim();
                let short = !text.is_empty() && raw.chars().count() < max_chars;
                let truncated = text.ends_with("...") || text.ends_with('\u{2026}');
                (short && !truncated).then(|| (row.id, raw.clone(), row.vector.clone()))
            })
            .collect();
        out.sort_by_key(|(id, _, _)| *id);
        out.truncate(n);
        out
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
    lexical_hits_with(engine, query, limit, false)
}

/// A recovered row's text for the meaning index.
#[derive(Debug, Clone)]
pub struct RecoveredText {
    pub id: u32,
    pub text: String,
    pub state: LifecycleState,
    /// The row's stored vector when it is non-zero (v1 wrote all-zero
    /// placeholders for rows it never embedded).
    pub stored: Option<Vec<f32>>,
}

/// Whether a lifecycle state may reach ranking. Released (Forgiven or
/// Archived) text is excluded unless faded recall is explicitly enabled.
pub fn state_admitted(state: LifecycleState, include_faded: bool) -> bool {
    state == LifecycleState::Active || include_faded
}

/// [`lexical_hits`] with the faded-memory policy made explicit.
pub fn lexical_hits_with(engine: &DurableEngine, query: &str, limit: usize, include_faded: bool) -> Vec<MemoryHit> {
    let tokens = tokens(query);
    if tokens.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for row in engine.engine().rows_iter() {
        let Some(record) = engine.memory_store().get(row.id) else {
            continue;
        };
        // Deliberate release excludes plaintext before any ranking or model call
        // (unless faded recall is explicitly configured). Low-priority Active
        // memories remain eligible; decay is not deletion.
        if !state_admitted(record.state, include_faded) {
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
            created_at: record.created_at,
            tags: row.tags.clone(),
            refs: row.refs.clone(),
        });
    }
    hits.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
    hits.truncate(limit.min(50));
    hits
}

/// Experience ids are allocated upward from here. Recovered bases mix small
/// sequential ids with hash-derived ids spread over the whole u32 range (the
/// Steve recovery has ids from 501 to 4_292_676_937), so neither a reserved
/// namespace nor "recovered max + 1" is safe. Allocation instead starts at
/// this base and skips every id the (immutable) recovered base uses.
pub const EXPERIENCE_ID_BASE: u32 = 0x8000_0000;

/// Next experience id: the smallest id that is >= EXPERIENCE_ID_BASE, above
/// every id already in the experience store and the persisted high-water
/// mark (ids are monotonic and never reused, even if a row were removed),
/// and not used by the recovered base. The recovered base is read-only, so
/// its id set is fixed for the life of the process and cannot collide later.
pub fn allocate_experience_id(
    recovered: &HashSet<u32>,
    experience_max: Option<u32>,
    high_water: Option<u32>,
) -> Result<u32> {
    let exhausted = || anyhow::anyhow!("experience id space exhausted");
    let mut next = EXPERIENCE_ID_BASE;
    for used in [experience_max, high_water].into_iter().flatten() {
        next = next.max(used.checked_add(1).ok_or_else(exhausted)?);
    }
    while recovered.contains(&next) {
        next = next.checked_add(1).ok_or_else(exhausted)?;
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
    recovered_ids: HashSet<u32>,
    high_water: Option<u32>,
}

impl ExperienceInner {
    /// Allocate the next id and reserve it durably first: a crash after
    /// this point leaks an id but can never hand the same id to two memories.
    fn reserve_id(&mut self) -> Result<u32> {
        let id = allocate_experience_id(&self.recovered_ids, max_id(&self.engine), self.high_water)?;
        let tmp = self.high_water_path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(&id)?)?;
        std::fs::rename(&tmp, &self.high_water_path)?;
        self.high_water = Some(id);
        Ok(id)
    }
}

/// Characters of the operator's message kept in a `hearing` row.
pub const TURN_HEARD_CHARS: usize = 1500;
/// Characters of the agent's reply kept in the `thinking` row.
pub const TURN_SAID_CHARS: usize = 600;

/// A completed operator chat turn to remember.
#[derive(Debug, Clone)]
pub struct TurnEvent {
    pub conversation_id: uuid::Uuid,
    pub request_id: uuid::Uuid,
    /// Who spoke (config `operator_name`).
    pub speaker: String,
    pub heard: String,
    pub said: String,
}

/// At most `max` characters, with an ellipsis when cut.
pub fn bounded_chars(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('\u{2026}');
    out
}

/// What a verdict says about the memory it takes as its object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictKind {
    /// A conflict flagged, no winner yet.
    Disputes,
    /// Settled by evidence: the memory's claim is replaced.
    Supersedes,
}

impl VerdictKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disputes => "disputes",
            Self::Supersedes => "supersedes",
        }
    }
}

/// A verdict to record about an earlier memory (recovered or experience).
#[derive(Debug, Clone)]
pub struct VerdictEvent {
    pub target: u32,
    pub kind: VerdictKind,
    pub reason: String,
    /// Cite handle of the evidence that settled it (required for supersedes).
    pub evidence: Option<String>,
    pub conversation_id: Option<uuid::Uuid>,
    pub request_id: Option<uuid::Uuid>,
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
    /// An earlier document whose mean section vector is >= 0.97 cosine
    /// with this one's: `(doc_id, cosine)`. Recorded, never blocking.
    pub near_duplicate_of: Option<(String, f64)>,
}

impl ExperienceStore {
    pub fn open(dir: impl AsRef<Path>, recovered_ids: HashSet<u32>) -> Result<Self> {
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
            inner: Mutex::new(ExperienceInner { engine, high_water_path, recovered_ids, high_water }),
        })
    }

    pub fn recall_candidates(&self, query: &str, limit: usize) -> Vec<MemoryHit> {
        let inner = self.inner.lock().expect("experience store poisoned");
        lexical_hits(&inner.engine, query, limit)
    }

    /// Hits for specific experience ids (dense candidates), in order,
    /// scored with the paired score. Non-Active rows are skipped.
    pub fn hits_for(&self, ids: &[(u32, f32)]) -> Vec<MemoryHit> {
        let inner = self.inner.lock().expect("experience store poisoned");
        ids.iter()
            .filter_map(|(id, score)| {
                let row = inner.engine.engine().get(*id)?;
                let record = inner.engine.memory_store().get(*id)?;
                (record.state == LifecycleState::Active).then(|| MemoryHit {
                    id: *id,
                    score: *score,
                    state: record.state.into(),
                    fidelity: record.fidelity,
                    importance: record.importance,
                    keystone: record.keystone,
                    created_at: record.created_at,
                    tags: row.tags.clone(),
                    refs: row.refs.clone(),
                })
            })
            .collect()
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

    /// Record a verdict about an earlier memory: a new keystone row (a
    /// verdict never decays) on channel `verdict`, plus a causal edge to the
    /// memory it takes as its object (`paccaya:arammana` for disputes,
    /// `paccaya:adhipati` for supersedes). The earlier memory is never
    /// changed. Returns the verdict's id.
    pub fn remember_verdict(&self, verdict: &VerdictEvent) -> Result<u32> {
        let event = match verdict.kind {
            VerdictKind::Disputes => ferricula_cognition::patthana::LinkEvent::Disputes,
            VerdictKind::Supersedes => ferricula_cognition::patthana::LinkEvent::Supersedes,
        };
        let mut tags = BTreeMap::new();
        tags.insert("channel".to_string(), "verdict".to_string());
        tags.insert("source".to_string(), "verdict".to_string());
        tags.insert("kind".to_string(), verdict.kind.as_str().to_string());
        tags.insert("target".to_string(), verdict.target.to_string());
        tags.insert("reason".to_string(), verdict.reason.clone());
        if let Some(cite) = &verdict.evidence {
            tags.insert("evidence".to_string(), cite.clone());
        }
        if let Some(id) = verdict.conversation_id {
            tags.insert("written_in_conversation".to_string(), id.to_string());
        }
        if let Some(id) = verdict.request_id {
            tags.insert("written_in_request".to_string(), id.to_string());
        }
        let text = format!(
            "My verdict: memory {} is {}. {}{}",
            verdict.target,
            match verdict.kind { VerdictKind::Disputes => "disputed", VerdictKind::Supersedes => "superseded" },
            verdict.reason,
            verdict.evidence.as_ref().map(|c| format!(" Evidence: {c}.")).unwrap_or_default(),
        );
        tags.insert("text".to_string(), text);
        let mut inner = self.inner.lock().expect("experience store poisoned");
        let id = inner.reserve_id()?;
        let row = Row { id, tags, vector: Vec::new(), refs: None };
        let mut record = MemoryRecord::new(id);
        record.keystone = true;
        record.importance = 1.0;
        record.provenance = Provenance::Ingested;
        inner.engine.remember(row, record)?;
        inner.engine.connect(id, verdict.target, event.condition().label(), 1.0, EdgeKind::Causal)?;
        inner.engine.checkpoint()?;
        Ok(id)
    }

    /// Every verdict row: `(verdict id, target id, kind, tags, created_at)`.
    pub fn verdicts(&self) -> Vec<(u32, u32, String, BTreeMap<String, String>, u64)> {
        let inner = self.inner.lock().expect("experience store poisoned");
        inner.engine.engine().rows_iter()
            .filter(|row| row.tags.get("channel").map(String::as_str) == Some("verdict"))
            .filter_map(|row| {
                let target = row.tags.get("target")?.parse().ok()?;
                let created = inner.engine.memory_store().get(row.id).map_or(0, |r| r.created_at);
                Some((row.id, target, row.tags.get("kind").cloned().unwrap_or_default(), row.tags.clone(), created))
            })
            .collect()
    }

    /// Every experience row with its lifecycle record, in id order.
    pub fn rows(&self) -> Vec<(Row, MemoryRecord)> {
        let inner = self.inner.lock().expect("experience store poisoned");
        inner
            .engine
            .engine()
            .rows_iter()
            .filter_map(|row| {
                let record = inner.engine.memory_store().get(row.id)?;
                Some((row.clone(), record.clone()))
            })
            .collect()
    }

    /// Graph edges of the experience store (for a scratch consolidation).
    pub fn edges(&self) -> Vec<ferricula_core::graph::Edge> {
        let inner = self.inner.lock().expect("experience store poisoned");
        inner.engine.graph().all_edges()
    }

    /// Record one general experience (a thought, a dream, ...) under
    /// `channel`; returns its id. `tags` may add fields; `channel` and
    /// `text` always win.
    pub fn remember(
        &self,
        channel: &str,
        text: &str,
        mut tags: BTreeMap<String, String>,
        refs: Option<MemoryRef>,
        importance: f32,
    ) -> Result<u32> {
        let mut inner = self.inner.lock().expect("experience store poisoned");
        let id = inner.reserve_id()?;
        tags.insert("channel".to_string(), channel.to_string());
        tags.insert("text".to_string(), text.to_string());
        let row = Row { id, tags, vector: Vec::new(), refs };
        let mut record = MemoryRecord::new(id);
        record.importance = importance;
        record.provenance = Provenance::Ingested;
        inner.engine.remember(row, record)?;
        inner.engine.checkpoint()?;
        Ok(id)
    }

    /// Record one completed operator chat turn as two experience rows:
    /// `hearing` ("<speaker> said: ...") and `thinking` ("I said: ..."),
    /// tagged with the conversation, request and turn index. The hearing row
    /// follows the previous turn's last row in the same conversation and the
    /// reply follows the hearing row, each by a causal `paccaya:anantara`
    /// (proximity) edge. Returns `(hearing_id, said_id)`.
    pub fn remember_turn(&self, turn: &TurnEvent) -> Result<(u32, u32)> {
        let mut inner = self.inner.lock().expect("experience store poisoned");
        let conversation = turn.conversation_id.to_string();
        let request = turn.request_id.to_string();
        let mut previous: Option<(u32, u32)> = None; // (turn index, last row id)
        for row in inner.engine.engine().rows_iter() {
            // Only conversation rows count (a verdict written during a turn
            // once carried that turn's ids).
            if row.tags.get("source").map(String::as_str) != Some("conversation") {
                continue;
            }
            if row.tags.get("conversation_id") != Some(&conversation) {
                continue;
            }
            if row.tags.get("request_id") == Some(&request) {
                bail!("turn {request} is already remembered");
            }
            let index: u32 = row.tags.get("turn").and_then(|t| t.parse().ok()).unwrap_or(0);
            if previous.is_none_or(|(i, id)| (index, row.id) > (i, id)) {
                previous = Some((index, row.id));
            }
        }
        let index = previous.map_or(0, |(i, _)| i + 1);
        let base = |role: &str| {
            let mut tags = BTreeMap::new();
            tags.insert("source".to_string(), "conversation".to_string());
            tags.insert("role".to_string(), role.to_string());
            tags.insert("conversation_id".to_string(), conversation.clone());
            tags.insert("request_id".to_string(), request.clone());
            tags.insert("turn".to_string(), index.to_string());
            tags
        };
        let heard_id = inner.reserve_id()?;
        let mut tags = base("operator");
        tags.insert("channel".to_string(), "hearing".to_string());
        tags.insert(
            "text".to_string(),
            format!("{} said: {}", turn.speaker, bounded_chars(&turn.heard, TURN_HEARD_CHARS)),
        );
        let mut record = MemoryRecord::new(heard_id);
        record.importance = 0.6;
        record.provenance = Provenance::Ingested;
        inner.engine.remember(Row { id: heard_id, tags, vector: Vec::new(), refs: None }, record)?;

        let said_id = inner.reserve_id()?;
        let mut tags = base("agent");
        tags.insert("channel".to_string(), "thinking".to_string());
        tags.insert("text".to_string(), format!("I said: {}", bounded_chars(&turn.said, TURN_SAID_CHARS)));
        let mut record = MemoryRecord::new(said_id);
        record.importance = 0.4;
        record.provenance = Provenance::Ingested;
        inner.engine.remember(Row { id: said_id, tags, vector: Vec::new(), refs: None }, record)?;

        let anantara = ferricula_cognition::patthana::LinkEvent::NextTurn.condition().label();
        if let Some((_, prev)) = previous {
            inner.engine.connect(prev, heard_id, anantara.clone(), 1.0, EdgeKind::Causal)?;
        }
        inner.engine.connect(heard_id, said_id, anantara, 1.0, EdgeKind::Causal)?;
        inner.engine.checkpoint()?;
        Ok((heard_id, said_id))
    }

    /// Record "I read X" as one durable experience memory; returns its id.
    /// The WAL entry is appended and the store checkpointed before return.
    pub fn remember_reading(&self, event: &ReadingEvent) -> Result<u32> {
        let mut inner = self.inner.lock().expect("experience store poisoned");
        let id = inner.reserve_id()?;

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
        if let Some((doc, cosine)) = &event.near_duplicate_of {
            tags.insert("near_duplicate_of".to_string(), doc.clone());
            tags.insert("near_duplicate_cosine".to_string(), format!("{cosine:.4}"));
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
    fn probe_samples_pick_short_whole_active_texts_with_vectors() {
        use ferricula_core::memory::MemoryRecord;
        use ferricula_core::model::Row;
        let dir = std::env::current_dir().unwrap().join("target")
            .join(format!("probe-samples-{}", uuid::Uuid::new_v4()));
        let mut engine = DurableEngine::open(&dir).unwrap();
        // (The engine enforces one dimension, so empty-vector rows cannot be
        // built here; probe_samples still filters them defensively.)
        let long = "long ".repeat(40);
        let cases: [(u32, &str, bool); 6] = [
            (1, "a short whole memory", true),
            (2, "cut off here...", true),          // visibly truncated
            (4, &long, true),                      // >= 150 chars
            (5, "released memory", false),         // not Active
            (6, "another short one ", true),       // kept verbatim
            (7, "zero placeholder vector", true),  // nothing to compare
        ];
        for (id, text, active) in cases {
            let row = Row {
                id, refs: None,
                vector: if id == 7 { vec![0.0, 0.0] } else { vec![0.6, 0.8] },
                tags: BTreeMap::from([("text".into(), text.to_string())]),
            };
            let mut record = MemoryRecord::new(id);
            if !active { record.forgive(); }
            engine.remember(row, record).unwrap();
        }
        drop(engine);
        let runtime = MemoryRuntime::open(&dir).unwrap();
        let picked = runtime.probe_samples(150, 3);
        assert_eq!(picked.iter().map(|s| s.0).collect::<Vec<_>>(), vec![1, 6]);
        assert_eq!(picked[1].1, "another short one ");
        assert_eq!(picked[0].2, vec![0.6, 0.8]);
        assert_eq!(runtime.probe_samples(150, 1).len(), 1);
        drop(runtime);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn experience_ids_never_collide_with_recovered_or_prior_ids() {
        let none = HashSet::new();
        assert_eq!(allocate_experience_id(&none, None, None).unwrap(), EXPERIENCE_ID_BASE);
        // Small sequential recovered ids do not matter.
        let small: HashSet<u32> = (1..=41_000).collect();
        assert_eq!(allocate_experience_id(&small, None, None).unwrap(), EXPERIENCE_ID_BASE);
        assert_eq!(
            allocate_experience_id(&small, Some(EXPERIENCE_ID_BASE + 4), None).unwrap(),
            EXPERIENCE_ID_BASE + 5
        );
        // The high-water mark wins even if the highest row is gone.
        assert_eq!(
            allocate_experience_id(&none, Some(EXPERIENCE_ID_BASE), Some(EXPERIENCE_ID_BASE + 9))
                .unwrap(),
            EXPERIENCE_ID_BASE + 10
        );
        // Hash-derived recovered ids scattered through the range are skipped,
        // and a huge recovered id does not push allocation to the top.
        let scattered: HashSet<u32> =
            [EXPERIENCE_ID_BASE, EXPERIENCE_ID_BASE + 1, EXPERIENCE_ID_BASE + 3, 4_292_676_937]
                .into_iter().collect();
        assert_eq!(allocate_experience_id(&scattered, None, None).unwrap(), EXPERIENCE_ID_BASE + 2);
        assert_eq!(
            allocate_experience_id(&scattered, None, Some(EXPERIENCE_ID_BASE + 2)).unwrap(),
            EXPERIENCE_ID_BASE + 4
        );
        assert!(allocate_experience_id(&none, None, Some(u32::MAX)).is_err());
        let top: HashSet<u32> = [u32::MAX].into_iter().collect();
        assert!(allocate_experience_id(&top, None, Some(u32::MAX - 1)).is_err());
    }

    #[test]
    fn experience_store_persists_reading_and_allocates_monotonic_ids() {
        let dir = std::env::current_dir().unwrap().join("target")
            .join(format!("experience-{}", uuid::Uuid::new_v4()));
        let event = ReadingEvent {
            doc_id: "abc".into(), title: "Paper".into(), origin: "https://x.org/p.pdf".into(),
            source_kind: "url".into(), pages: 2, sections: 3, excerpt: "hello world".into(),
            note: Some("for the memory discussion".into()),
            near_duplicate_of: None,
        };
        let first = {
            let store = ExperienceStore::open(&dir, HashSet::from([77, EXPERIENCE_ID_BASE + 1])).unwrap();
            let id = store.remember_reading(&event).unwrap();
            assert_eq!(id, EXPERIENCE_ID_BASE);
            assert_eq!(store.reading_for("abc"), Some(id));
            id
        };
        let store = ExperienceStore::open(&dir, HashSet::from([77, EXPERIENCE_ID_BASE + 1])).unwrap();
        assert_eq!(store.len(), 1);
        let hits = store.recall_candidates("paper world", 5);
        assert_eq!(hits[0].id, first);
        assert_eq!(hits[0].tags["channel"], "reading");
        assert_eq!(hits[0].tags["note"], "for the memory discussion");
        assert_eq!(hits[0].refs.as_ref().unwrap().url.as_deref(), Some("https://x.org/p.pdf"));
        let second = store
            .remember_reading(&ReadingEvent { doc_id: "def".into(), ..event })
            .unwrap();
        // BASE + 1 belongs to the recovered base, so it is skipped.
        assert_eq!(second, first + 2);
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn chat_turns_become_linked_recallable_rows() {
        let dir = std::env::current_dir().unwrap().join("target")
            .join(format!("turns-{}", uuid::Uuid::new_v4()));
        let store = ExperienceStore::open(&dir, HashSet::new()).unwrap();
        let conv = uuid::Uuid::new_v4();
        let turn = |heard: &str, said: &str| TurnEvent {
            conversation_id: conv, request_id: uuid::Uuid::new_v4(), speaker: "Kord".into(),
            heard: heard.into(), said: said.into(),
        };
        let first = turn("I always name my iPhones Steve.", "That's flattering.");
        let (h1, s1) = store.remember_turn(&first).unwrap();
        assert!(store.remember_turn(&first).is_err(), "a turn is remembered once");
        let (h2, s2) = store.remember_turn(&turn("And my iPads?", &"x".repeat(900))).unwrap();
        let rows: std::collections::HashMap<u32, Row> = store.rows().into_iter().map(|(r, _)| (r.id, r)).collect();
        assert_eq!(rows[&h1].tags["text"], "Kord said: I always name my iPhones Steve.");
        assert_eq!(rows[&h1].tags["channel"], "hearing");
        assert_eq!(rows[&s1].tags["channel"], "thinking");
        assert_eq!(rows[&h2].tags["turn"], "1");
        assert_eq!(rows[&s2].tags["text"].chars().count(), "I said: ".len() + TURN_SAID_CHARS + 1);
        let edges = store.edges();
        let has = |a: u32, b: u32| edges.iter().any(|e| e.from == a && e.to == b && e.label == "paccaya:anantara");
        assert!(has(h1, s1) && has(s1, h2) && has(h2, s2));
        assert_eq!(edges.len(), 3);
        // A later conversation finds it lexically.
        let hits = store.recall_candidates("what do I name my phones", 5);
        assert_eq!(hits[0].id, h1);
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
