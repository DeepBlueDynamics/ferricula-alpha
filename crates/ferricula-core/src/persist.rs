use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::engine::Engine;
use crate::graph::{Edge, EdgeKind, MemoryGraph};
use crate::memory::{MemoryRecord, MemoryStore};
use crate::model::Row;
use crate::prime_tree::{PrimeTree, PrimeTreeSnapshot};
use crate::skg::{SkgSnapshot, SkgState};

const SNAPSHOT_V1_FILE: &str = "snapshot.bin";
const SNAPSHOT_V2_FILE: &str = "snapshot_v2.bin";
const SNAPSHOT_V3_FILE: &str = "snapshot_v3.bin";
const SNAPSHOT_V4_FILE: &str = "snapshot_v4.bin";
const WAL_FILE: &str = "wal.log";
const SNAPSHOT_TMP_FILE: &str = "snapshot_v4.bin.tmp";

// Fix #34: version envelope for new snapshot writes.
//
// Layout: [b"FERR" (4)][version: u8][reserved: 3 bytes][payload: postcard bytes]
// Total header: 8 bytes.
//
// Readers that see the magic header strip it and dispatch by version byte.
// Snapshots written by ferricula < 0.10 have no header — they parse via the
// existing legacy fallback chain (SnapshotV4 strict → LegacySnapshotV4 →
// SnapshotV3 → SnapshotV2 → SnapshotV1). Both paths converge on Engine state.
const SNAPSHOT_MAGIC: &[u8; 4] = b"FERR";
const SNAPSHOT_FORMAT_VERSION: u8 = 5;
const SNAPSHOT_HEADER_LEN: usize = 8;

// --- Durability ---------------------------------------------------------
//
// An acknowledged write must survive a crash (kill -9, power loss). Rust's
// `flush()` only empties user-space buffers into the OS page cache, so every
// commit point here also calls `sync_data`/`sync_all`, and every
// write-then-rename also syncs the parent directory so the rename itself is
// durable.

/// When the WAL is fsynced. Read once per [`DurableEngine::open`] from
/// `FERRICULA_WAL_SYNC`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalSync {
    /// `sync_data` after every append (default). An `Ok` from a mutating
    /// call means the entry is on stable storage.
    Always,
    /// Appends are only flushed to the OS; durability comes at
    /// [`DurableEngine::checkpoint`] (or [`DurableEngine::sync`]). Survives a
    /// process crash, but a power loss can drop the tail since the last
    /// checkpoint. For bulk loads that checkpoint at the end.
    Checkpoint,
}

impl WalSync {
    /// `FERRICULA_WAL_SYNC=always|checkpoint`; anything else (or unset) is `Always`.
    pub fn from_env() -> Self {
        match std::env::var("FERRICULA_WAL_SYNC").map(|v| v.trim().to_ascii_lowercase()) {
            Ok(v) if v == "checkpoint" => WalSync::Checkpoint,
            _ => WalSync::Always,
        }
    }
}

/// Fsync a directory so a create/rename inside it is durable.
///
/// On Unix this opens the directory and calls `fsync`. Windows has no
/// directory fsync through std (opening a directory handle needs
/// `FILE_FLAG_BACKUP_SEMANTICS`); NTFS journals metadata operations such as
/// rename, so this is a no-op there. Filesystems that refuse directory
/// fsync (`EINVAL`/`EBADF`, some network mounts) are tolerated.
pub fn sync_dir(dir: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        match File::open(dir).and_then(|d| d.sync_all()) {
            Ok(()) => Ok(()),
            Err(e) if matches!(e.raw_os_error(), Some(22) | Some(9)) => Ok(()),
            Err(e) => Err(e).with_context(|| format!("fsync dir {}", dir.display())),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        Ok(())
    }
}

/// Write `bytes` to `path` atomically and durably: write `tmp`, fsync it,
/// rename it over `path`, fsync the parent directory.
pub fn write_atomic_durable(path: &Path, tmp: &Path, bytes: &[u8]) -> Result<()> {
    {
        let mut file = File::create(tmp).with_context(|| format!("create {}", tmp.display()))?;
        file.write_all(bytes).with_context(|| format!("write {}", tmp.display()))?;
        file.sync_all().with_context(|| format!("fsync {}", tmp.display()))?;
    }
    fs::rename(tmp, path).with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        sync_dir(parent)?;
    }
    Ok(())
}

#[derive(Debug)]
pub struct DurableEngine {
    engine: Engine,
    memory_store: MemoryStore,
    graph: MemoryGraph,
    prime_tree: PrimeTree,
    skg: SkgState,
    persistence: Persistence,
}

impl DurableEngine {
    pub fn open(base_dir: impl AsRef<Path>) -> Result<Self> {
        Self::open_with(base_dir, WalSync::from_env())
    }

    /// Open with an explicit WAL sync policy (ignores `FERRICULA_WAL_SYNC`).
    pub fn open_with(base_dir: impl AsRef<Path>, wal_sync: WalSync) -> Result<Self> {
        let persistence = Persistence::new(base_dir, wal_sync)?;
        let (engine, memory_store, graph, prime_tree, skg) = persistence.load()?;
        Ok(Self {
            engine,
            memory_store,
            graph,
            prime_tree,
            skg,
            persistence,
        })
    }

    // --- Row operations (original API) ---

    pub fn upsert(&mut self, row: Row) -> Result<()> {
        self.engine.upsert(row.clone())?;
        self.persistence.append_wal(&WalEntry::Upsert(row))?;
        self.maybe_rotate()
    }

    pub fn delete(&mut self, id: u32) -> Result<bool> {
        let deleted = self.engine.delete(id);
        if deleted {
            self.persistence.append_wal(&WalEntry::Delete { id })?;
            self.maybe_rotate()?;
        }
        Ok(deleted)
    }

    // --- Memory record operations ---

    pub fn remember(&mut self, row: Row, record: MemoryRecord) -> Result<()> {
        let r = row.clone();
        let rec = record.clone();
        self.engine.upsert(row)?;
        self.memory_store.insert(record);
        self.persistence.append_wal(&WalEntry::Remember {
            row: r,
            record: rec,
        })?;
        self.maybe_rotate()
    }

    pub fn update_record(&mut self, record: MemoryRecord) -> Result<()> {
        let rec = record.clone();
        self.memory_store.insert(record);
        self.persistence.append_wal(&WalEntry::UpdateRecord(rec))?;
        self.maybe_rotate()
    }

    pub fn remove_memory(&mut self, id: u32) -> Result<bool> {
        let deleted = self.engine.delete(id);
        self.memory_store.remove(id);
        self.graph.remove_node(id);
        self.prime_tree.remove_member(id);
        if deleted {
            self.persistence
                .append_wal(&WalEntry::RemoveMemory { id })?;
            self.maybe_rotate()?;
        }
        Ok(deleted)
    }

    // --- Graph operations ---

    pub fn connect(&mut self, a: u32, b: u32, label: String, weight: f32, kind: EdgeKind) -> Result<()> {
        self.graph.connect(a, b, label.clone(), weight, kind);
        match kind {
            EdgeKind::Semantic => {
                self.persistence.append_wal(&WalEntry::Connect {
                    a,
                    b,
                    label,
                    weight,
                })?;
            }
            EdgeKind::Causal => {
                self.persistence.append_wal(&WalEntry::ConnectCausal {
                    from: a,
                    to: b,
                    label,
                    weight,
                })?;
            }
            EdgeKind::Structural => {
                self.persistence.append_wal(&WalEntry::ConnectStructural {
                    a,
                    b,
                    label,
                    weight,
                })?;
            }
        }
        self.maybe_rotate()
    }

    pub fn disconnect(&mut self, a: u32, b: u32) -> Result<()> {
        self.graph.disconnect(a, b);
        self.persistence.append_wal(&WalEntry::Disconnect { a, b })?;
        self.maybe_rotate()
    }

    // --- Prime tree operations ---

    pub fn insert_term(&mut self, term: &str, memory_id: u32) -> Result<u64> {
        let node_id = self.prime_tree.insert(term, memory_id);
        self.persistence.append_wal(&WalEntry::InsertTerm {
            term: term.to_string(),
            memory_id,
        })?;
        self.maybe_rotate()?;
        Ok(node_id)
    }

    // --- WAL rotation ---

    /// Auto-checkpoint if WAL exceeds size threshold.
    fn maybe_rotate(&mut self) -> Result<()> {
        if self.persistence.wal_bytes >= WAL_ROTATE_BYTES {
            eprintln!(
                "[wal] auto-checkpoint at {} bytes",
                self.persistence.wal_bytes
            );
            self.checkpoint()?;
        }
        Ok(())
    }

    // --- Checkpoint & accessors ---

    /// Force the WAL to stable storage: redundant under [`WalSync::Always`],
    /// the commit point under [`WalSync::Checkpoint`] when a full checkpoint
    /// is not wanted.
    pub fn sync(&mut self) -> Result<()> {
        self.persistence.sync_wal()
    }

    pub fn wal_sync(&self) -> WalSync {
        self.persistence.wal_sync
    }

    pub fn checkpoint(&mut self) -> Result<()> {
        self.persistence.checkpoint(
            &self.engine,
            &self.memory_store,
            &self.graph,
            &self.prime_tree,
            &self.skg,
        )
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }

    pub fn memory_store(&self) -> &MemoryStore {
        &self.memory_store
    }

    pub fn memory_store_mut(&mut self) -> &mut MemoryStore {
        &mut self.memory_store
    }

    pub fn graph(&self) -> &MemoryGraph {
        &self.graph
    }

    pub fn graph_mut(&mut self) -> &mut MemoryGraph {
        &mut self.graph
    }

    pub fn prime_tree(&self) -> &PrimeTree {
        &self.prime_tree
    }

    pub fn prime_tree_mut(&mut self) -> &mut PrimeTree {
        &mut self.prime_tree
    }

    pub fn skg(&self) -> &SkgState {
        &self.skg
    }

    pub fn skg_mut(&mut self) -> &mut SkgState {
        &mut self.skg
    }
}

/// Max WAL size before auto-checkpoint (64 MB).
const WAL_ROTATE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug)]
struct Persistence {
    #[allow(dead_code)]
    base_dir: PathBuf,
    snapshot_v1_path: PathBuf,
    snapshot_v2_path: PathBuf,
    snapshot_v3_path: PathBuf,
    snapshot_v4_path: PathBuf,
    snapshot_tmp_path: PathBuf,
    wal_path: PathBuf,
    wal_bytes: u64,
    wal_sync: WalSync,
}

impl Persistence {
    fn new(base_dir: impl AsRef<Path>, wal_sync: WalSync) -> Result<Self> {
        let base_dir = base_dir.as_ref().to_path_buf();
        fs::create_dir_all(&base_dir)?;
        let wal_path = base_dir.join(WAL_FILE);
        let wal_bytes = fs::metadata(&wal_path).map(|m| m.len()).unwrap_or(0);
        Ok(Self {
            snapshot_v1_path: base_dir.join(SNAPSHOT_V1_FILE),
            snapshot_v2_path: base_dir.join(SNAPSHOT_V2_FILE),
            snapshot_v3_path: base_dir.join(SNAPSHOT_V3_FILE),
            snapshot_v4_path: base_dir.join(SNAPSHOT_V4_FILE),
            snapshot_tmp_path: base_dir.join(SNAPSHOT_TMP_FILE),
            wal_path,
            wal_bytes,
            wal_sync,
            base_dir,
        })
    }

    fn load(&self) -> Result<(Engine, MemoryStore, MemoryGraph, PrimeTree, SkgState)> {
        let mut engine = Engine::new();
        let mut memory_store = MemoryStore::new();
        let mut graph = MemoryGraph::new();
        let mut prime_tree = PrimeTree::new();
        let mut skg = SkgState::new();

        // Try V4 snapshot first (edges with EdgeKind)
        if self.snapshot_v4_path.exists() {
            let bytes = fs::read(&self.snapshot_v4_path)?;
            if !bytes.is_empty() {
                // Fix #34: detect the version envelope. If present, strip the
                // 8-byte header before parsing. If not, fall through to the
                // legacy raw-postcard path that's been in place since v0.9.x.
                let payload: &[u8] = if bytes.len() >= SNAPSHOT_HEADER_LEN
                    && &bytes[..4] == SNAPSHOT_MAGIC
                {
                    let version = bytes[4];
                    match version {
                        5 => &bytes[SNAPSHOT_HEADER_LEN..],
                        v => bail!(
                            "snapshot envelope version {v} is unsupported by this build; \
                             upgrade ferricula or run a migration"
                        ),
                    }
                } else {
                    &bytes[..]
                };
                let snap: SnapshotV4 = match postcard::from_bytes::<SnapshotV4>(payload) {
                    Ok(s) => s,
                    Err(_) => {
                        // Fall back to pre-MemoryRef layout (v0.9.2 and earlier).
                        let legacy: LegacySnapshotV4 = postcard::from_bytes(payload)
                            .context("snapshot_v4 parse failed under both current and legacy formats")?;
                        eprintln!(
                            "[persist] migrated pre-MemoryRef snapshot ({} rows) — refs default to None; \
                             next checkpoint will rewrite in current format",
                            legacy.rows.len()
                        );
                        legacy.into_v4()
                    }
                };
                for row in snap.rows {
                    engine.upsert(row)?;
                }
                memory_store.load_records(snap.records);
                graph.load_edges(snap.edges);
                prime_tree.load_snapshot(snap.tree);
                skg = SkgState::load_snapshot(snap.skg);
            }
        } else if self.snapshot_v3_path.exists() {
            // V3: legacy edges (no EdgeKind) — migrate to Semantic
            let bytes = fs::read(&self.snapshot_v3_path)?;
            if !bytes.is_empty() {
                let snap: SnapshotV3 = postcard::from_bytes(&bytes)?;
                for row in snap.rows {
                    engine.upsert(row)?;
                }
                memory_store.load_records(snap.records);
                graph.load_edges(snap.edges.into_iter().map(LegacyEdge::into_edge).collect());
                prime_tree.load_snapshot(snap.tree);
                skg = SkgState::load_snapshot(snap.skg);
            }
        } else if self.snapshot_v2_path.exists() {
            // V2: legacy edges, no SKG — migrate to Semantic
            let bytes = fs::read(&self.snapshot_v2_path)?;
            if !bytes.is_empty() {
                let snap: SnapshotV2 = postcard::from_bytes(&bytes)?;
                for row in snap.rows {
                    engine.upsert(row)?;
                }
                memory_store.load_records(snap.records);
                graph.load_edges(snap.edges.into_iter().map(LegacyEdge::into_edge).collect());
                prime_tree.load_snapshot(snap.tree);
            }
        } else if self.snapshot_v1_path.exists() {
            // Fall back to V1 (rows only)
            let bytes = fs::read(&self.snapshot_v1_path)?;
            if !bytes.is_empty() {
                let snap: SnapshotV1 = postcard::from_bytes(&bytes)?;
                for row in snap.rows {
                    engine.upsert(row)?;
                }
            }
        }

        // Replay WAL on top
        if self.wal_path.exists() {
            for entry in self.read_wal()? {
                Self::apply_wal_entry(
                    &mut engine,
                    &mut memory_store,
                    &mut graph,
                    &mut prime_tree,
                    entry,
                )?;
            }
        }

        Ok((engine, memory_store, graph, prime_tree, skg))
    }

    fn apply_wal_entry(
        engine: &mut Engine,
        memory_store: &mut MemoryStore,
        graph: &mut MemoryGraph,
        prime_tree: &mut PrimeTree,
        entry: WalEntry,
    ) -> Result<()> {
        match entry {
            WalEntry::Upsert(row) => {
                engine.upsert(row)?;
            }
            WalEntry::Delete { id } => {
                engine.delete(id);
            }
            WalEntry::Remember { row, record } => {
                engine.upsert(row)?;
                memory_store.insert(record);
            }
            WalEntry::UpdateRecord(record) => {
                memory_store.insert(record);
            }
            WalEntry::RemoveMemory { id } => {
                engine.delete(id);
                memory_store.remove(id);
                graph.remove_node(id);
                prime_tree.remove_member(id);
            }
            WalEntry::Connect {
                a,
                b,
                label,
                weight,
            } => {
                graph.connect(a, b, label, weight, EdgeKind::Semantic);
            }
            WalEntry::Disconnect { a, b } => {
                graph.disconnect(a, b);
            }
            WalEntry::ConnectCausal {
                from,
                to,
                label,
                weight,
            } => {
                graph.connect(from, to, label, weight, EdgeKind::Causal);
            }
            WalEntry::InsertTerm { term, memory_id } => {
                prime_tree.insert(&term, memory_id);
            }
            WalEntry::ConnectStructural {
                a,
                b,
                label,
                weight,
            } => {
                graph.connect(a, b, label, weight, EdgeKind::Structural);
            }
        }
        Ok(())
    }

    /// Append entry to WAL, tracking cumulative size.
    /// Returns true if WAL has exceeded rotation threshold.
    fn append_wal(&mut self, entry: &WalEntry) -> Result<bool> {
        let mut writer = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.wal_path)
            .with_context(|| format!("open WAL at {}", self.wal_path.display()))?;
        let payload = postcard::to_stdvec(entry)?;
        let len = payload.len() as u32;
        writer.write_all(&len.to_le_bytes())?;
        writer.write_all(&payload)?;
        writer.flush()?;
        if self.wal_sync == WalSync::Always {
            writer
                .sync_data()
                .with_context(|| format!("fsync WAL at {}", self.wal_path.display()))?;
        }
        if self.wal_bytes == 0 {
            // First append after create/truncate: make the directory entry
            // durable too.
            sync_dir(&self.base_dir)?;
        }
        self.wal_bytes += 4 + payload.len() as u64;
        Ok(self.wal_bytes >= WAL_ROTATE_BYTES)
    }

    fn sync_wal(&mut self) -> Result<()> {
        if self.wal_path.exists() {
            OpenOptions::new()
                .append(true)
                .open(&self.wal_path)?
                .sync_all()
                .with_context(|| format!("fsync WAL at {}", self.wal_path.display()))?;
        }
        Ok(())
    }

    fn checkpoint(
        &mut self,
        engine: &Engine,
        memory_store: &MemoryStore,
        graph: &MemoryGraph,
        prime_tree: &PrimeTree,
        skg: &SkgState,
    ) -> Result<()> {
        let snapshot = SnapshotV4 {
            rows: engine.rows_iter().cloned().collect(),
            records: memory_store.all_records(),
            edges: graph.all_edges(),
            tree: prime_tree.snapshot(),
            skg: skg.snapshot(),
        };
        let payload = postcard::to_stdvec(&snapshot)?;

        // Fix #34: prepend the version envelope.
        let mut framed = Vec::with_capacity(SNAPSHOT_HEADER_LEN + payload.len());
        framed.extend_from_slice(SNAPSHOT_MAGIC);
        framed.push(SNAPSHOT_FORMAT_VERSION);
        framed.extend_from_slice(&[0u8, 0, 0]); // reserved
        framed.extend_from_slice(&payload);

        {
            let tmp_file = File::create(&self.snapshot_tmp_path)?;
            let mut writer = BufWriter::new(tmp_file);
            writer.write_all(&framed)?;
            writer.flush()?;
            // The snapshot must be on disk before the WAL it replaces is cut.
            writer.get_ref().sync_all()?;
        }

        fs::rename(&self.snapshot_tmp_path, &self.snapshot_v4_path)?;
        sync_dir(&self.base_dir)?;
        // Clean up old snapshots if they exist
        if self.snapshot_v3_path.exists() {
            let _ = fs::remove_file(&self.snapshot_v3_path);
        }
        if self.snapshot_v2_path.exists() {
            let _ = fs::remove_file(&self.snapshot_v2_path);
        }
        if self.snapshot_v1_path.exists() {
            let _ = fs::remove_file(&self.snapshot_v1_path);
        }
        // Truncate WAL (only after the snapshot rename is durable).
        File::create(&self.wal_path)?.sync_all()?;
        sync_dir(&self.base_dir)?;
        self.wal_bytes = 0;
        Ok(())
    }

    /// Read every complete WAL entry. A torn tail -- a length prefix or
    /// payload cut short by a crash mid-append, or a zero-filled tail left
    /// by the filesystem -- was never acknowledged (appends fsync before
    /// returning), so it is dropped and the file is truncated to the last
    /// complete entry, so later appends do not land after garbage.
    fn read_wal(&self) -> Result<Vec<WalEntry>> {
        let bytes = fs::read(&self.wal_path)
            .with_context(|| format!("read WAL at {}", self.wal_path.display()))?;
        let mut out = Vec::new();
        let mut migrated = 0usize;
        let mut pos = 0usize;
        let mut torn = false;
        while pos < bytes.len() {
            if bytes.len() - pos < 4 || bytes[pos..].iter().all(|b| *b == 0) {
                torn = true;
                break;
            }
            let len = u32::from_le_bytes(bytes[pos..pos + 4].try_into().expect("4 bytes")) as usize;
            let start = pos + 4;
            if bytes.len() - start < len {
                torn = true;
                break;
            }
            let payload = &bytes[start..start + len];
            // Try the current WalEntry shape first; on failure, fall
            // back to the pre-MemoryRef layout (v0.9.2). This handles
            // mixed WALs where a v0.9.2 process wrote entries before
            // the upgrade and a v0.9.5+ process appended after.
            let entry = match postcard::from_bytes::<WalEntry>(payload) {
                Ok(e) => e,
                Err(_) => {
                    let legacy: LegacyWalEntry = postcard::from_bytes(payload)
                        .context("WAL entry not parseable under current or legacy layout")?;
                    migrated += 1;
                    legacy.into_entry()
                }
            };
            out.push(entry);
            pos = start + len;
        }
        if torn {
            eprintln!(
                "[persist] WAL torn tail: dropping {} unacknowledged bytes at offset {}",
                bytes.len() - pos,
                pos
            );
            let file = OpenOptions::new().write(true).open(&self.wal_path)?;
            file.set_len(pos as u64)?;
            file.sync_all()?;
        }
        if migrated > 0 {
            eprintln!(
                "[persist] migrated {} pre-MemoryRef WAL entries — next checkpoint will rewrite",
                migrated
            );
        }
        Ok(out)
    }
}

/// V1 snapshot: rows only (backward compat).
#[derive(Debug, Serialize, Deserialize)]
struct SnapshotV1 {
    rows: Vec<Row>,
}

/// Edge format from V2/V3 snapshots (no EdgeKind field).
#[derive(Debug, Serialize, Deserialize)]
struct LegacyEdge {
    from: u32,
    to: u32,
    label: String,
    weight: f32,
}

impl LegacyEdge {
    fn into_edge(self) -> Edge {
        Edge {
            from: self.from,
            to: self.to,
            label: self.label,
            weight: self.weight,
            kind: EdgeKind::Semantic,
        }
    }
}

/// V2 snapshot: full system state (no SKG, legacy edges).
#[derive(Debug, Serialize, Deserialize)]
struct SnapshotV2 {
    rows: Vec<Row>,
    records: Vec<MemoryRecord>,
    edges: Vec<LegacyEdge>,
    tree: PrimeTreeSnapshot,
}

/// V3 snapshot: full system state + SKG (legacy edges).
#[derive(Debug, Serialize, Deserialize)]
struct SnapshotV3 {
    rows: Vec<Row>,
    records: Vec<MemoryRecord>,
    edges: Vec<LegacyEdge>,
    tree: PrimeTreeSnapshot,
    skg: SkgSnapshot,
}

/// V4 snapshot: full system state + SKG + directed edges.
#[derive(Debug, Serialize, Deserialize)]
struct SnapshotV4 {
    rows: Vec<Row>,
    records: Vec<MemoryRecord>,
    edges: Vec<Edge>,
    tree: PrimeTreeSnapshot,
    skg: SkgSnapshot,
}

/// WAL entry — new variants appended at end for backward compat.
#[derive(Debug, Clone, Serialize, Deserialize)]
enum WalEntry {
    // V1 entries (keep these first for backward compat)
    Upsert(Row),
    Delete {
        id: u32,
    },
    // V2 entries
    Remember {
        row: Row,
        record: MemoryRecord,
    },
    UpdateRecord(MemoryRecord),
    RemoveMemory {
        id: u32,
    },
    Connect {
        a: u32,
        b: u32,
        label: String,
        weight: f32,
    },
    Disconnect {
        a: u32,
        b: u32,
    },
    InsertTerm {
        term: String,
        memory_id: u32,
    },
    // V4 entry — causal (directed) edge
    ConnectCausal {
        from: u32,
        to: u32,
        label: String,
        weight: f32,
    },
    // V5 entry — structural (document order, bidirectional, dream-immune)
    ConnectStructural {
        a: u32,
        b: u32,
        label: String,
        weight: f32,
    },
}

/// Pre-MemoryRef Row layout. Used as a deserialization fallback when
/// reading snapshots/WAL written by v0.9.2 and earlier — those binaries
/// did not include the `refs` field. Postcard is positional so the new
/// `Option<MemoryRef>` field at the tail breaks read-compatibility despite
/// `#[serde(default)]` (that attribute is JSON-only). Migrating up sets
/// `refs: None`; the next checkpoint rewrites in the new format.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyRowV2 {
    id: u32,
    tags: std::collections::BTreeMap<String, String>,
    vector: Vec<f32>,
}

impl LegacyRowV2 {
    fn into_row(self) -> Row {
        Row {
            id: self.id,
            tags: self.tags,
            vector: self.vector,
            refs: None,
        }
    }
}

/// Pre-MemoryRef SnapshotV4 layout (rows use LegacyRowV2).
#[derive(Debug, Serialize, Deserialize)]
struct LegacySnapshotV4 {
    rows: Vec<LegacyRowV2>,
    records: Vec<MemoryRecord>,
    edges: Vec<Edge>,
    tree: PrimeTreeSnapshot,
    skg: SkgSnapshot,
}

impl LegacySnapshotV4 {
    fn into_v4(self) -> SnapshotV4 {
        SnapshotV4 {
            rows: self.rows.into_iter().map(LegacyRowV2::into_row).collect(),
            records: self.records,
            edges: self.edges,
            tree: self.tree,
            skg: self.skg,
        }
    }
}

/// Pre-MemoryRef WalEntry layout. Variant order MUST match the v0.9.2
/// `WalEntry` exactly so postcard discriminant indices line up. Variants
/// containing `Row` use `LegacyRowV2`. `ConnectStructural` is omitted —
/// it was added in the same commit as `Row.refs`, so v0.9.2 WAL bytes
/// never contain that discriminant.
#[derive(Debug, Clone, Serialize, Deserialize)]
enum LegacyWalEntry {
    Upsert(LegacyRowV2),
    Delete {
        id: u32,
    },
    Remember {
        row: LegacyRowV2,
        record: MemoryRecord,
    },
    UpdateRecord(MemoryRecord),
    RemoveMemory {
        id: u32,
    },
    Connect {
        a: u32,
        b: u32,
        label: String,
        weight: f32,
    },
    Disconnect {
        a: u32,
        b: u32,
    },
    InsertTerm {
        term: String,
        memory_id: u32,
    },
    ConnectCausal {
        from: u32,
        to: u32,
        label: String,
        weight: f32,
    },
}

impl LegacyWalEntry {
    fn into_entry(self) -> WalEntry {
        match self {
            Self::Upsert(r) => WalEntry::Upsert(r.into_row()),
            Self::Delete { id } => WalEntry::Delete { id },
            Self::Remember { row, record } => WalEntry::Remember {
                row: row.into_row(),
                record,
            },
            Self::UpdateRecord(r) => WalEntry::UpdateRecord(r),
            Self::RemoveMemory { id } => WalEntry::RemoveMemory { id },
            Self::Connect {
                a,
                b,
                label,
                weight,
            } => WalEntry::Connect {
                a,
                b,
                label,
                weight,
            },
            Self::Disconnect { a, b } => WalEntry::Disconnect { a, b },
            Self::InsertTerm { term, memory_id } => WalEntry::InsertTerm { term, memory_id },
            Self::ConnectCausal {
                from,
                to,
                label,
                weight,
            } => WalEntry::ConnectCausal {
                from,
                to,
                label,
                weight,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tempfile::tempdir;

    use super::*;
    use crate::memory::MemoryRecord;

    fn sample_row(id: u32, region: &str) -> Row {
        let mut tags = BTreeMap::new();
        tags.insert("region".to_string(), region.to_string());
        Row {
            id,
            tags,
            vector: vec![id as f32, 0.0, 1.0],
            refs: None,
        }
    }

    #[test]
    fn wal_replay_and_checkpoint_restore() {
        let dir = tempdir().unwrap();
        {
            let mut db = DurableEngine::open(dir.path()).unwrap();
            db.upsert(sample_row(1, "us")).unwrap();
            db.upsert(sample_row(2, "eu")).unwrap();
            db.delete(2).unwrap();
            db.checkpoint().unwrap();
        }

        let db = DurableEngine::open(dir.path()).unwrap();
        assert_eq!(db.engine().row_count(), 1);
        assert!(db.engine().get(1).is_some());
    }

    #[test]
    fn wal_replay_without_checkpoint() {
        let dir = tempdir().unwrap();
        {
            let mut db = DurableEngine::open(dir.path()).unwrap();
            db.upsert(sample_row(1, "us")).unwrap();
            db.upsert(sample_row(2, "eu")).unwrap();
        }
        let db = DurableEngine::open(dir.path()).unwrap();
        assert_eq!(db.engine().row_count(), 2);
    }

    #[test]
    fn remember_persists_memory_record() {
        let dir = tempdir().unwrap();
        {
            let mut db = DurableEngine::open(dir.path()).unwrap();
            let row = sample_row(1, "us");
            let record = MemoryRecord::new(1);
            db.remember(row, record).unwrap();
            db.checkpoint().unwrap();
        }

        let db = DurableEngine::open(dir.path()).unwrap();
        assert_eq!(db.engine().row_count(), 1);
        assert!(db.memory_store().get(1).is_some());
        assert_eq!(db.memory_store().get(1).unwrap().fidelity, 1.0);
    }

    #[test]
    fn graph_edges_persist() {
        let dir = tempdir().unwrap();
        {
            let mut db = DurableEngine::open(dir.path()).unwrap();
            db.remember(sample_row(1, "us"), MemoryRecord::new(1))
                .unwrap();
            db.remember(sample_row(2, "eu"), MemoryRecord::new(2))
                .unwrap();
            db.connect(1, 2, "related".into(), 0.9, EdgeKind::Semantic).unwrap();
            db.checkpoint().unwrap();
        }

        let db = DurableEngine::open(dir.path()).unwrap();
        assert!(db.graph().neighbors(1).contains(2));
        assert_eq!(db.graph().edge_count(), 1);
    }

    #[test]
    fn prime_tree_persists() {
        let dir = tempdir().unwrap();
        {
            let mut db = DurableEngine::open(dir.path()).unwrap();
            db.remember(sample_row(1, "us"), MemoryRecord::new(1))
                .unwrap();
            db.insert_term("weather", 1).unwrap();
            db.checkpoint().unwrap();
        }

        let db = DurableEngine::open(dir.path()).unwrap();
        let result = db.prime_tree().search_exact("weather");
        assert!(result.contains(1));
    }

    #[test]
    fn wal_replay_all_entry_types() {
        let dir = tempdir().unwrap();
        {
            let mut db = DurableEngine::open(dir.path()).unwrap();
            db.remember(sample_row(1, "us"), MemoryRecord::new(1))
                .unwrap();
            db.remember(sample_row(2, "eu"), MemoryRecord::new(2))
                .unwrap();
            db.connect(1, 2, "link".into(), 1.0, EdgeKind::Semantic).unwrap();
            db.insert_term("test", 1).unwrap();
            db.disconnect(1, 2).unwrap();
            db.remove_memory(2).unwrap();
            // No checkpoint — everything should replay from WAL
        }

        let db = DurableEngine::open(dir.path()).unwrap();
        assert_eq!(db.engine().row_count(), 1);
        assert_eq!(db.memory_store().len(), 1);
        assert_eq!(db.graph().edge_count(), 0);
        assert!(db.prime_tree().search_exact("test").contains(1));
    }

    // Fix #34: new snapshots are written with the FERR magic header,
    // and reading the same file back produces the same state.
    #[test]
    fn snapshot_envelope_round_trip() {
        let dir = tempdir().unwrap();
        {
            let mut db = DurableEngine::open(dir.path()).unwrap();
            db.remember(sample_row(7, "envelope"), MemoryRecord::new(7))
                .unwrap();
            db.checkpoint().unwrap();
        }
        // The on-disk file must start with the envelope magic + version 5.
        let path = dir.path().join(SNAPSHOT_V4_FILE);
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.len() >= SNAPSHOT_HEADER_LEN);
        assert_eq!(&bytes[..4], SNAPSHOT_MAGIC);
        assert_eq!(bytes[4], SNAPSHOT_FORMAT_VERSION);

        let db = DurableEngine::open(dir.path()).unwrap();
        assert_eq!(db.engine().row_count(), 1);
        assert!(db.engine().get(7).is_some());
    }

    // Fix #34: snapshots written WITHOUT the envelope (pre-v0.10 format)
    // still load via the legacy postcard parse path.
    #[test]
    fn snapshot_legacy_unenveloped_still_loads() {
        let dir = tempdir().unwrap();
        // Build a snapshot the way the old writer did: raw postcard, no header.
        let mut engine = Engine::new();
        engine.upsert(sample_row(42, "legacy")).unwrap();
        let snapshot = SnapshotV4 {
            rows: engine.rows_iter().cloned().collect(),
            records: Vec::new(),
            edges: Vec::new(),
            tree: PrimeTree::new().snapshot(),
            skg: SkgState::new().snapshot(),
        };
        let raw_payload = postcard::to_stdvec(&snapshot).unwrap();
        let path = dir.path().join(SNAPSHOT_V4_FILE);
        std::fs::write(&path, &raw_payload).unwrap();

        let db = DurableEngine::open(dir.path()).unwrap();
        assert_eq!(db.engine().row_count(), 1);
        assert!(db.engine().get(42).is_some());
    }
}
