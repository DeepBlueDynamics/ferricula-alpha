//! Live thermodynamics over a memory base that must not be written.
//!
//! A recovered memory store is mounted read-only, but the agent's memory
//! still has to behave like memory: traces fade when neglected and hold
//! when recalled. `ThermoLayer` keeps the *live* thermodynamic state of
//! every record (fidelity, decay rate, recall count, heat of recent use)
//! in its own file, seeded from the base on first sight and never written
//! back to it.
//!
//! Rules (v3):
//! - Decay changes **retrieval priority**, never content. A tick lowers
//!   fidelity; it never deletes, forgives or archives anything. Lifecycle
//!   transitions happen only by an explicit, logged decision elsewhere.
//! - Which records decay on a tick is chosen by entropy (the radio when it
//!   is up): time in this system is noise-driven, as in v1.
//! - Recall strengthens (α shrinks); neglect across a tick weakens (α grows
//!   within bounds); keystones and non-Active records do not decay.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::memory::{LifecycleState, MemoryRecord, now_epoch};

const FORMAT: &str = "ferricula-thermo/1";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TickReport {
    pub tick: u64,
    pub considered: usize,
    pub decayed: usize,
    pub neglected: usize,
    pub mean_fidelity: f32,
    pub entropy_seed: u64,
}

#[derive(Serialize, Deserialize)]
struct Persisted {
    format: String,
    ticks: u64,
    records: Vec<MemoryRecord>,
}

pub struct ThermoLayer {
    path: PathBuf,
    records: HashMap<u32, MemoryRecord>,
    ticks: u64,
    dirty: bool,
}

impl ThermoLayer {
    /// Open the layer at `path` (created if missing). Records present in
    /// `base` but not yet in the layer are copied in with their base state;
    /// records already in the layer keep their live state.
    pub fn open<'a>(path: impl AsRef<Path>, base: impl IntoIterator<Item = &'a MemoryRecord>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let (mut records, ticks) = if path.exists() {
            let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
            let persisted: Persisted = postcard::from_bytes(&bytes)
                .with_context(|| format!("decode {}", path.display()))?;
            anyhow::ensure!(persisted.format == FORMAT, "unknown thermo format {}", persisted.format);
            (persisted.records.into_iter().map(|r| (r.id, r)).collect::<HashMap<_, _>>(), persisted.ticks)
        } else {
            (HashMap::new(), 0)
        };
        let before = records.len();
        for record in base {
            records.entry(record.id).or_insert_with(|| record.clone());
        }
        let dirty = records.len() != before;
        Ok(Self { path, records, ticks, dirty })
    }

    /// Add a record that did not come from the base (new experience).
    pub fn admit(&mut self, record: MemoryRecord) {
        self.records.entry(record.id).or_insert(record);
        self.dirty = true;
    }

    pub fn get(&self, id: u32) -> Option<&MemoryRecord> {
        self.records.get(&id)
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Retrieval weight in (0, 1]: live fidelity for known active records,
    /// 1.0 for anything the layer does not know (never penalise the unknown).
    pub fn weight(&self, id: u32) -> f32 {
        match self.records.get(&id) {
            Some(r) if r.state == LifecycleState::Active => r.fidelity.clamp(0.05, 1.0),
            Some(_) => 0.05,
            None => 1.0,
        }
    }

    /// The agent recalled these (they were used in an answer or thought).
    pub fn recall(&mut self, ids: &[u32]) {
        for id in ids {
            if let Some(r) = self.records.get_mut(id) {
                r.on_recall();
                self.dirty = true;
            }
        }
    }

    /// One tick of time. `fraction` of the active, non-keystone records,
    /// picked by `entropy`, lose fidelity by one decay step; the same
    /// picked records that have not been recalled within `neglect_secs`
    /// have their decay rate grow (neglect). Nothing is deleted and no
    /// lifecycle state changes.
    pub fn tick(&mut self, entropy: u64, fraction: f32, neglect_secs: u64) -> TickReport {
        self.ticks += 1;
        let now = now_epoch();
        let mut ids: Vec<u32> = self.records.values()
            .filter(|r| r.state == LifecycleState::Active && !r.keystone)
            .map(|r| r.id)
            .collect();
        ids.sort_unstable();
        let considered = ids.len();
        let take = ((considered as f32) * fraction.clamp(0.0, 1.0)).round() as usize;
        let mut state = entropy;
        let (mut decayed, mut neglected) = (0, 0);
        for _ in 0..take.min(ids.len()) {
            let pick = splitmix(&mut state) as usize % ids.len();
            let id = ids.swap_remove(pick);
            let r = self.records.get_mut(&id).expect("id from records");
            if now.saturating_sub(r.last_recalled) >= neglect_secs {
                r.on_neglect();
                neglected += 1;
            }
            r.decay_tick();
            decayed += 1;
        }
        self.dirty |= decayed > 0;
        let active: Vec<f32> = self.records.values()
            .filter(|r| r.state == LifecycleState::Active)
            .map(|r| r.fidelity)
            .collect();
        let mean_fidelity = if active.is_empty() { 0.0 } else { active.iter().sum::<f32>() / active.len() as f32 };
        TickReport { tick: self.ticks, considered, decayed, neglected, mean_fidelity, entropy_seed: entropy }
    }

    /// Persist atomically if anything changed.
    pub fn save(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let mut records: Vec<MemoryRecord> = self.records.values().cloned().collect();
        records.sort_by_key(|r| r.id);
        let bytes = postcard::to_stdvec(&Persisted { format: FORMAT.into(), ticks: self.ticks, records })?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("tmp");
        // fsync the temp file, rename, fsync the directory.
        crate::persist::write_atomic_durable(&self.path, &tmp, &bytes)
            .with_context(|| format!("save {}", self.path.display()))?;
        self.dirty = false;
        Ok(())
    }
}

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(n: u32) -> Vec<MemoryRecord> {
        (1..=n).map(|id| {
            let mut r = MemoryRecord::new_at(id, 0);
            r.last_recalled = 0;
            r
        }).collect()
    }

    #[test]
    fn ticks_lower_priority_but_never_remove_or_change_state() {
        let dir = tempfile::tempdir().unwrap();
        let b = base(100);
        let mut layer = ThermoLayer::open(dir.path().join("t.bin"), &b).unwrap();
        for i in 0..50 {
            layer.tick(i * 7919, 0.2, 60);
        }
        assert_eq!(layer.len(), 100);
        assert!(layer.records.values().all(|r| r.state == LifecycleState::Active));
        assert!(layer.records.values().any(|r| r.fidelity < 1.0));
        assert!(layer.records.values().all(|r| r.fidelity > 0.0));
    }

    #[test]
    fn keystones_hold_and_recall_slows_decay() {
        let dir = tempfile::tempdir().unwrap();
        let mut b = base(2);
        b[0].keystone = true;
        let mut layer = ThermoLayer::open(dir.path().join("t.bin"), &b).unwrap();
        let alpha_before = layer.get(2).unwrap().decay_alpha;
        layer.recall(&[2]);
        assert!(layer.get(2).unwrap().decay_alpha < alpha_before);
        for i in 0..20 {
            layer.tick(i, 1.0, u64::MAX);
        }
        assert_eq!(layer.get(1).unwrap().fidelity, 1.0);
        assert!(layer.get(2).unwrap().fidelity < 1.0);
    }

    #[test]
    fn state_persists_and_base_is_only_a_seed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.bin");
        let b = base(10);
        {
            let mut layer = ThermoLayer::open(&path, &b).unwrap();
            layer.tick(42, 1.0, 60);
            layer.save().unwrap();
        }
        let faded = ThermoLayer::open(&path, &b).unwrap();
        assert_eq!(faded.ticks(), 1);
        assert!(faded.get(3).unwrap().fidelity < 1.0, "live state wins over the base seed");
        let mut more = base(11);
        more[10].id = 11;
        let grown = ThermoLayer::open(&path, &more).unwrap();
        assert_eq!(grown.len(), 11);
        assert_eq!(grown.get(11).unwrap().fidelity, 1.0);
    }

    #[test]
    fn entropy_chooses_what_decays() {
        let dir = tempfile::tempdir().unwrap();
        let b = base(50);
        let mut a = ThermoLayer::open(dir.path().join("a.bin"), &b).unwrap();
        let mut c = ThermoLayer::open(dir.path().join("c.bin"), &b).unwrap();
        a.tick(1, 0.1, 60);
        c.tick(2, 0.1, 60);
        let faded = |l: &ThermoLayer| {
            let mut v: Vec<u32> = l.records.values().filter(|r| r.fidelity < 1.0).map(|r| r.id).collect();
            v.sort();
            v
        };
        assert_eq!(faded(&a).len(), 5);
        assert_ne!(faded(&a), faded(&c));
    }

    #[test]
    fn unknown_ids_are_not_penalised() {
        let dir = tempfile::tempdir().unwrap();
        let layer = ThermoLayer::open(dir.path().join("t.bin"), &base(1)).unwrap();
        assert_eq!(layer.weight(999), 1.0);
        assert_eq!(layer.weight(1), 1.0);
    }
}
