//! Crash durability: whatever `DurableEngine` acknowledged (returned `Ok`
//! for) must be there after the process dies without a checkpoint or any
//! cleanup.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use ferricula_core::memory::MemoryRecord;
use ferricula_core::persist::WalSync;
use ferricula_core::{DurableEngine, Row};

fn row(id: u32) -> Row {
    let mut tags = BTreeMap::new();
    tags.insert("n".to_string(), id.to_string());
    Row { id, tags, vector: vec![id as f32, 1.0, 0.5], refs: None }
}

/// Drop the engine without checkpoint (and leak it, so no destructor can
/// help), reopen, and find every acknowledged write.
#[test]
fn acknowledged_writes_survive_drop_without_close() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut db = DurableEngine::open_with(dir.path(), WalSync::Always).unwrap();
        for id in 1..=50 {
            db.remember(row(id), MemoryRecord::new(id)).unwrap();
        }
        db.connect(1, 2, "next".into(), 0.5, ferricula_core::EdgeKind::Causal).unwrap();
        db.delete(50).unwrap();
        std::mem::forget(db);
    }
    let db = DurableEngine::open(dir.path()).unwrap();
    assert_eq!(db.engine().row_count(), 49);
    for id in 1..=49 {
        assert!(db.memory_store().get(id).is_some(), "record {id} lost");
    }
    assert!(db.engine().get(50).is_none());
}

/// A crash mid-append leaves a partial entry at the WAL tail. It was never
/// acknowledged: reopen drops it, keeps everything before it, and later
/// appends replay cleanly.
#[test]
fn torn_wal_tail_is_dropped_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut db = DurableEngine::open(dir.path()).unwrap();
        for id in 1..=10 {
            db.upsert(row(id)).unwrap();
        }
    }
    // Simulate a torn append: a length prefix promising 200 bytes, then 7.
    {
        let mut wal = OpenOptions::new().append(true).open(dir.path().join("wal.log")).unwrap();
        wal.write_all(&200u32.to_le_bytes()).unwrap();
        wal.write_all(b"partial").unwrap();
    }
    {
        let mut db = DurableEngine::open(dir.path()).unwrap();
        assert_eq!(db.engine().row_count(), 10);
        db.upsert(row(11)).unwrap();
    }
    let db = DurableEngine::open(dir.path()).unwrap();
    assert_eq!(db.engine().row_count(), 11);
    assert!(db.engine().get(11).is_some());
}

/// A zero-filled tail (what some filesystems leave after a crash that
/// extended the file but lost the data) is also a torn tail.
#[test]
fn zero_filled_wal_tail_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut db = DurableEngine::open(dir.path()).unwrap();
        db.upsert(row(1)).unwrap();
    }
    {
        let mut wal = OpenOptions::new().append(true).open(dir.path().join("wal.log")).unwrap();
        wal.write_all(&[0u8; 64]).unwrap();
    }
    let db = DurableEngine::open(dir.path()).unwrap();
    assert_eq!(db.engine().row_count(), 1);
}

/// Child half of `kill_during_writes_loses_nothing_acknowledged`: writes
/// until killed, printing each id after its `remember` returned `Ok`.
#[test]
#[ignore]
fn crash_child_writer() {
    let Ok(dir) = std::env::var("FERRICULA_CRASH_DIR") else { return };
    let mut db = DurableEngine::open_with(&dir, WalSync::Always).unwrap();
    let stdout = std::io::stdout();
    for id in 1..=1_000_000u32 {
        db.remember(row(id), MemoryRecord::new(id)).unwrap();
        let mut out = stdout.lock();
        writeln!(out, "ack {id}").unwrap();
        out.flush().unwrap();
    }
}

/// kill -9 while writing: every id the child reported as acknowledged is
/// present after reopen.
#[test]
fn kill_during_writes_loses_nothing_acknowledged() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "crash_child_writer", "--ignored", "--nocapture", "--test-threads=1"])
        .env("FERRICULA_CRASH_DIR", dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut last_ack = 0u32;
    let reader = BufReader::new(child.stdout.take().unwrap());
    for line in reader.lines() {
        let line = line.unwrap();
        if let Some(n) = line.strip_prefix("ack ") {
            last_ack = n.trim().parse().unwrap();
            if last_ack >= 150 {
                child.kill().unwrap(); // SIGKILL / TerminateProcess: no unwinding
                break;
            }
        }
    }
    let _ = child.wait();
    assert!(last_ack >= 150, "child stopped early at {last_ack}");

    let db = DurableEngine::open(dir.path()).unwrap();
    for id in 1..=last_ack {
        assert!(db.engine().get(id).is_some(), "acknowledged row {id} lost");
        assert!(db.memory_store().get(id).is_some(), "acknowledged record {id} lost");
    }
}
