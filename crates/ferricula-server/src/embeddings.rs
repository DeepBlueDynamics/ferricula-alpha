//! Text embeddings (R2b, first step): the configured backend and a probe
//! that proves it speaks the recovered memory's embedding space.
//!
//! Nothing here writes memory, and nothing reads the embedder yet except the
//! probe: recall/life/ingest wiring is the next step. `embedder()` hands the
//! backend out only while dense features are allowed (probe `ok`, or probe
//! turned off), so a space mismatch or an unreachable service keeps every
//! future dense feature off instead of comparing vectors across spaces.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use ferricula_semantic::text_embed::{NoEmbedder, ShivvrEmbedder, TextEmbedder, cosine};
use serde::Serialize;

use super::{AgentRuntime, now};
use crate::config::{EmbeddingBackend, EmbeddingsConfig};

/// A probe sample must be shorter than this (characters), so the stored
/// vector was computed from the whole text (v1 ingest never chunked it).
pub const PROBE_MAX_CHARS: usize = 150;
/// Samples re-embedded per probe.
pub const PROBE_SAMPLES: usize = 3;
/// Minimum cosine between a fresh and a stored vector for "same space".
pub const PROBE_MIN_COSINE: f64 = 0.999;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingsState {
    /// backend = "none".
    Disabled,
    /// Startup probe not finished yet.
    Pending,
    /// Probe passed: the backend reproduces the stored vectors.
    Ok,
    /// Probe off, or nothing in memory to compare against: usable, unchecked.
    Unverified,
    /// Backend unreachable or erroring; dense features off until a probe passes.
    Degraded,
    /// Backend answers in a different space; dense features off.
    Mismatch,
}

impl EmbeddingsState {
    /// Whether dense features may use the embedder in this state.
    pub fn dense_allowed(self) -> bool {
        matches!(self, Self::Ok | Self::Unverified)
    }
}

/// `/status` → `embeddings`.
#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingsStatus {
    pub backend: String,
    pub space: String,
    pub state: EmbeddingsState,
    /// Cosine(fresh, stored) per probe sample, in `probe_ids` order.
    pub probe_cosines: Vec<f64>,
    pub probe_ids: Vec<u32>,
    /// Why the state is what it is (error text, or a note).
    pub detail: Option<String>,
    /// Unix seconds of the last probe.
    pub checked_at: Option<u64>,
}

pub struct EmbeddingsPlane {
    backend: EmbeddingBackend,
    probe_enabled: bool,
    embedder: Arc<dyn TextEmbedder>,
    status: Mutex<EmbeddingsStatus>,
}

impl EmbeddingsPlane {
    pub fn open(config: &EmbeddingsConfig) -> Result<Self> {
        let embedder: Arc<dyn TextEmbedder> = match config.backend {
            EmbeddingBackend::None => Arc::new(NoEmbedder),
            EmbeddingBackend::Shivvr => Arc::new(ShivvrEmbedder::new(
                &config.url,
                &config.space,
                Duration::from_secs(config.timeout_secs),
                config.batch,
            )?),
        };
        Ok(Self::with_embedder(config.backend, config.probe, embedder))
    }

    /// Explicit embedder (tests inject fakes).
    pub fn with_embedder(
        backend: EmbeddingBackend,
        probe_enabled: bool,
        embedder: Arc<dyn TextEmbedder>,
    ) -> Self {
        let (state, detail) = match (backend, probe_enabled) {
            (EmbeddingBackend::None, _) => (EmbeddingsState::Disabled, None),
            (_, true) => (EmbeddingsState::Pending, None),
            (_, false) => (EmbeddingsState::Unverified, Some("probe disabled by config".into())),
        };
        let status = EmbeddingsStatus {
            backend: backend.as_str().into(),
            space: embedder.space().into(),
            state,
            probe_cosines: Vec::new(),
            probe_ids: Vec::new(),
            detail,
            checked_at: None,
        };
        Self { backend, probe_enabled, embedder, status: Mutex::new(status) }
    }

    pub fn status(&self) -> EmbeddingsStatus {
        self.status.lock().expect("embeddings status poisoned").clone()
    }

    pub fn embedder(&self) -> Option<Arc<dyn TextEmbedder>> {
        self.status().state.dense_allowed().then(|| self.embedder.clone())
    }

    /// Re-embed `samples` (`(id, text, stored vector)`) and compare.
    /// Blocking (HTTP); never holds the status lock across the call.
    pub fn probe(&self, samples: &[(u32, String, Vec<f32>)]) -> EmbeddingsStatus {
        if self.backend == EmbeddingBackend::None {
            return self.status();
        }
        let (state, cosines, ids, detail) = probe_against(self.embedder.as_ref(), samples);
        let mut status = self.status.lock().expect("embeddings status poisoned");
        status.state = state;
        status.probe_cosines = cosines;
        status.probe_ids = ids;
        status.detail = detail;
        status.checked_at = Some(now());
        status.clone()
    }

    pub fn probe_enabled(&self) -> bool {
        self.probe_enabled
    }
}

type ProbeOutcome = (EmbeddingsState, Vec<f64>, Vec<u32>, Option<String>);

fn probe_against(embedder: &dyn TextEmbedder, samples: &[(u32, String, Vec<f32>)]) -> ProbeOutcome {
    let ids: Vec<u32> = samples.iter().map(|(id, _, _)| *id).collect();
    if samples.is_empty() {
        return (
            EmbeddingsState::Unverified,
            Vec::new(),
            ids,
            Some("no short recovered memory with a stored vector to compare against".into()),
        );
    }
    if let Some((id, _, v)) = samples.iter().find(|(_, _, v)| v.len() != embedder.dim()) {
        return (
            EmbeddingsState::Mismatch,
            Vec::new(),
            ids,
            Some(format!(
                "memory {id} stores a {}-d vector; backend space {} is {}-d",
                v.len(),
                embedder.space(),
                embedder.dim()
            )),
        );
    }
    let texts: Vec<&str> = samples.iter().map(|(_, t, _)| t.as_str()).collect();
    let fresh = match embedder.embed(&texts) {
        Ok(fresh) => fresh,
        Err(err) => return (EmbeddingsState::Degraded, Vec::new(), ids, Some(format!("{err:#}"))),
    };
    let cosines: Vec<f64> = fresh
        .iter()
        .zip(samples)
        .map(|(f, (_, _, stored))| (cosine(f, stored) * 1e6).round() / 1e6)
        .collect();
    if cosines.iter().all(|c| *c > PROBE_MIN_COSINE) {
        (EmbeddingsState::Ok, cosines, ids, None)
    } else {
        let detail = format!(
            "backend {} does not reproduce stored vectors (cosines {:?}, need > {PROBE_MIN_COSINE}); \
             dense features disabled",
            embedder.space(),
            cosines
        );
        (EmbeddingsState::Mismatch, cosines, ids, Some(detail))
    }
}

impl AgentRuntime {
    /// The text embedder, only while dense features are allowed (probe ok or
    /// probe off). `None` for backend "none", pending, degraded, mismatch.
    pub fn embedder(&self) -> Option<Arc<dyn TextEmbedder>> {
        self.embeddings.embedder()
    }

    pub fn embeddings_status(&self) -> EmbeddingsStatus {
        self.embeddings.status()
    }

    /// Run the space probe now (blocking). Logs the outcome.
    pub fn probe_embeddings(&self) -> EmbeddingsStatus {
        let samples = self.memory.probe_samples(PROBE_MAX_CHARS, PROBE_SAMPLES);
        let status = self.embeddings.probe(&samples);
        eprintln!(
            "embeddings: {} (backend {}, space {}, ids {:?}, cosines {:?}){}",
            serde_json::to_value(status.state).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default(),
            status.backend,
            status.space,
            status.probe_ids,
            status.probe_cosines,
            status.detail.as_deref().map(|d| format!(": {d}")).unwrap_or_default()
        );
        status
    }

    /// Startup task: runs the space probe once if the backend is configured
    /// with `probe = true` (the runtime serves requests meanwhile:
    /// `pending`), then starts the meaning backfill when dense features are
    /// allowed and `[embeddings] backfill = true`.
    pub async fn run_embeddings_probe(self: Arc<Self>) {
        if self.config.embeddings.backend == EmbeddingBackend::None {
            return;
        }
        if self.embeddings.probe_enabled() {
            let runtime = self.clone();
            let _ = tokio::task::spawn_blocking(move || runtime.probe_embeddings()).await;
        }
        if self.config.embeddings.backfill && self.embedder().is_some() {
            self.spawn_meaning_backfill("startup");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::bail;

    struct Fake {
        dim: usize,
        flip: bool,
        fail: bool,
    }

    impl TextEmbedder for Fake {
        fn space(&self) -> &str {
            "fake@3"
        }
        fn dim(&self) -> usize {
            self.dim
        }
        fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
            if self.fail {
                bail!("connection refused");
            }
            Ok(texts.iter().map(|t| vector(t, self.flip)).collect())
        }
    }

    fn vector(text: &str, flip: bool) -> Vec<f32> {
        let a = text.len() as f32;
        let v = if flip { vec![0.0, 1.0, a] } else { vec![1.0, a, 0.0] };
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.into_iter().map(|x| x / n).collect()
    }

    fn samples() -> Vec<(u32, String, Vec<f32>)> {
        ["ab", "abc"].iter().enumerate().map(|(i, t)| (i as u32 + 1, t.to_string(), vector(t, false))).collect()
    }

    fn plane(fake: Fake) -> EmbeddingsPlane {
        EmbeddingsPlane::with_embedder(EmbeddingBackend::Shivvr, true, Arc::new(fake))
    }

    #[test]
    fn matching_backend_is_ok_and_hands_out_embedder() {
        let p = plane(Fake { dim: 3, flip: false, fail: false });
        assert_eq!(p.status().state, EmbeddingsState::Pending);
        assert!(p.embedder().is_none(), "pending must not enable dense features");
        let s = p.probe(&samples());
        assert_eq!(s.state, EmbeddingsState::Ok);
        assert_eq!(s.probe_ids, vec![1, 2]);
        assert!(s.probe_cosines.iter().all(|c| *c > 0.999999));
        assert!(p.embedder().is_some());
    }

    #[test]
    fn unreachable_backend_degrades() {
        let p = plane(Fake { dim: 3, flip: false, fail: true });
        let s = p.probe(&samples());
        assert_eq!(s.state, EmbeddingsState::Degraded);
        assert!(s.detail.unwrap().contains("refused"));
        assert!(p.embedder().is_none());
    }

    #[test]
    fn other_space_is_mismatch_with_cosines() {
        let p = plane(Fake { dim: 3, flip: true, fail: false });
        let s = p.probe(&samples());
        assert_eq!(s.state, EmbeddingsState::Mismatch);
        assert_eq!(s.probe_cosines.len(), 2);
        assert!(s.probe_cosines.iter().all(|c| *c < 0.999));
        assert!(p.embedder().is_none());

        let p = plane(Fake { dim: 4, flip: false, fail: false });
        assert_eq!(p.probe(&samples()).state, EmbeddingsState::Mismatch);
    }

    #[test]
    fn recovers_after_a_later_good_probe_and_handles_empty_memory() {
        let p = plane(Fake { dim: 3, flip: false, fail: false });
        let s = p.probe(&[]);
        assert_eq!(s.state, EmbeddingsState::Unverified);
        assert!(p.embedder().is_some());
        assert_eq!(p.probe(&samples()).state, EmbeddingsState::Ok);
    }

    #[test]
    fn none_backend_and_probe_off() {
        let p = EmbeddingsPlane::open(&EmbeddingsConfig::default()).unwrap();
        assert_eq!(p.status().state, EmbeddingsState::Disabled);
        assert_eq!(p.status().backend, "none");
        assert_eq!(p.probe(&samples()).state, EmbeddingsState::Disabled);
        assert!(p.embedder().is_none());

        let p = EmbeddingsPlane::with_embedder(
            EmbeddingBackend::Shivvr,
            false,
            Arc::new(Fake { dim: 3, flip: false, fail: false }),
        );
        assert_eq!(p.status().state, EmbeddingsState::Unverified);
        assert!(p.embedder().is_some());
    }
}
