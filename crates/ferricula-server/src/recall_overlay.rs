//! Recall counts that never touch a memory row.
//!
//! `state_dir/overlay/recall-stats.json` maps `m:<id>` (recovered) and
//! `x:<id>` (experience) to how often a completed reply cited that memory
//! and when. Ranking reads the map. The recovered store is not opened here.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Default strengthening weight (`s`), on the fusion scale: reciprocal-rank
/// scores are about 1/61, and rank 1 vs rank 10 differ by about 0.002, so one
/// citation (s * ln 2) moves a memory a few places, not to the top. 0.15 made
/// the overlay the dominant term (Steve, PR #12 review).
pub const DEFAULT_S: f64 = 0.003;
/// Default fading weight (`f`), on the same scale; it saturates at `f`.
pub const DEFAULT_F: f64 = 0.002;
/// Default half-saturation age in days (`h`).
pub const DEFAULT_H: f64 = 30.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecallStat {
    pub recalls: u64,
    pub last_recalled: u64,
}

/// Durable citation counts for one runtime's `state_dir`.
#[derive(Debug)]
pub struct RecallOverlay {
    path: PathBuf,
    inner: Mutex<BTreeMap<String, RecallStat>>,
}

impl RecallOverlay {
    /// Load `state_dir/overlay/recall-stats.json`, or an empty map when
    /// the file is not there yet. Does not create the file.
    pub fn open(state_dir: &Path) -> Result<Arc<Self>> {
        let path = state_dir.join("overlay").join("recall-stats.json");
        let map = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .with_context(|| format!("parse {}", path.display()))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", path.display()));
            }
        };
        Ok(Arc::new(Self { path, inner: Mutex::new(map) }))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn get(&self, key: &str) -> Option<RecallStat> {
        self.inner.lock().expect("recall overlay poisoned").get(key).cloned()
    }

    pub fn recalls(&self, key: &str) -> u64 {
        self.get(key).map(|stat| stat.recalls).unwrap_or(0)
    }

    /// One citation of `key` at `now` (unix seconds). The file is replaced
    /// only after the new bytes are on disk.
    pub fn record(&self, key: &str, now: u64) -> Result<()> {
        let mut guard = self.inner.lock().expect("recall overlay poisoned");
        let mut next = guard.clone();
        let stat = next.entry(key.to_string()).or_insert(RecallStat { recalls: 0, last_recalled: now });
        stat.recalls = stat.recalls.saturating_add(1);
        stat.last_recalled = now;
        write_atomic(&self.path, &serde_json::to_vec_pretty(&next)?)?;
        *guard = next;
        Ok(())
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = File::create(&tmp).with_context(|| format!("create {}", tmp.display()))?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path).with_context(|| format!("rename {} to {}", tmp.display(), path.display()))?;
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        let _ = File::open(parent).and_then(|dir| dir.sync_all());
    }
    Ok(())
}

static INSTALLED: Mutex<Option<Arc<RecallOverlay>>> = Mutex::new(None);
static STRENGTH: Mutex<(f64, f64, f64)> = Mutex::new((DEFAULT_S, DEFAULT_F, DEFAULT_H));

/// Fusion reads this overlay. One process runs one agent; the latest open wins.
pub fn install(overlay: Arc<RecallOverlay>) {
    *INSTALLED.lock().expect("recall overlay poisoned") = Some(overlay);
}

pub fn installed() -> Option<Arc<RecallOverlay>> {
    INSTALLED.lock().expect("recall overlay poisoned").clone()
}

/// Publish `[recall]` s, f, h after a successful config check.
pub fn set_strength(s: f64, f: f64, h: f64) {
    *STRENGTH.lock().expect("recall strength poisoned") = (s, f, h);
}

pub fn strength() -> (f64, f64, f64) {
    *STRENGTH.lock().expect("recall strength poisoned")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_file_round_trips() {
        let root = std::env::temp_dir().join(format!("ferricula-recall-overlay-{}", uuid::Uuid::new_v4()));
        let overlay = RecallOverlay::open(&root).unwrap();
        assert!(overlay.get("m:7").is_none());
        overlay.record("m:7", 1_700_000_000).unwrap();
        overlay.record("x:9", 1_700_000_100).unwrap();
        overlay.record("m:7", 1_700_000_200).unwrap();
        drop(overlay);
        let again = RecallOverlay::open(&root).unwrap();
        assert_eq!(again.get("m:7"), Some(RecallStat { recalls: 2, last_recalled: 1_700_000_200 }));
        assert_eq!(again.get("x:9"), Some(RecallStat { recalls: 1, last_recalled: 1_700_000_100 }));
        assert!(again.path().ends_with("overlay/recall-stats.json"));
        let _ = fs::remove_dir_all(root);
    }
}
