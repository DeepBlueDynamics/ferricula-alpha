//! Crash durability and incremental-index equivalence for `DocumentStore`.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use ferricula_ingest::{DocumentStore, ExtractConfig, Extracted, Source, extract};

fn research_corpus() -> Vec<Extracted> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../research");
    let mut files: Vec<_> = std::fs::read_dir(&root).unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no research/*.md fixtures");
    files.iter().map(|p| {
        let text = std::fs::read_to_string(p).unwrap();
        extract(&Source::Text { title: None, text, origin: None }, &ExtractConfig::default()).unwrap()
    }).collect()
}

fn note(i: u32) -> Extracted {
    let text = format!("# Note {i}\nObservation number {i} about impermanence and recall, token{i}.\n");
    extract(&Source::Text { title: None, text, origin: None }, &ExtractConfig::default()).unwrap()
}

const QUERIES: &[&str] = &[
    "memory decay recall",
    "sleep consolidation dream",
    "judge calibration abstain",
    "sense door contact",
    "prime tree graph edges",
    "entropy radio",
    "fidelity",
    "bhavana vithi patthana",
];

/// The incrementally maintained BM25 index ranks exactly like a full
/// rebuild after every ingest over the research corpus.
#[test]
fn incremental_index_matches_full_rebuild_on_research_corpus() {
    let docs = research_corpus();
    let dir = tempfile::tempdir().unwrap();
    let mut store = DocumentStore::open(dir.path()).unwrap();
    for (i, doc) in docs.iter().enumerate() {
        store.ingest(doc, "text").unwrap();
        // Checking after every ingest is O(n^2); sample the tail and the end.
        if i % 10 == 0 || i + 1 == docs.len() {
            assert!(store.search_matches_full_rebuild(QUERIES), "ranking diverged after doc {i}");
        }
    }
    // And a reopened store (full rebuild from disk) has the same hits.
    let reopened = DocumentStore::open(dir.path()).unwrap();
    for q in QUERIES {
        let mut a: Vec<_> = store.search(q, usize::MAX, None).into_iter()
            .map(|h| (h.doc_id, h.section.index, h.score.to_bits())).collect();
        let mut b: Vec<_> = reopened.search(q, usize::MAX, None).into_iter()
            .map(|h| (h.doc_id, h.section.index, h.score.to_bits())).collect();
        a.sort();
        b.sort();
        assert_eq!(a, b, "query {q:?}");
    }
}

/// Ingest, drop without any close (leaked), reopen: everything is there.
#[test]
fn ingested_documents_survive_drop_without_close() {
    let dir = tempfile::tempdir().unwrap();
    let ids: Vec<String> = {
        let mut store = DocumentStore::open(dir.path()).unwrap();
        let ids = (0..20).map(|i| store.ingest(&note(i), "text").unwrap().record.meta.doc_id.clone()).collect();
        std::mem::forget(store);
        ids
    };
    // A crash between temp write and rename leaves a stray temp file.
    std::fs::write(dir.path().join("docs").join("deadbeefdeadbeef.json.tmp"), b"{\"partial").unwrap();
    let store = DocumentStore::open(dir.path()).unwrap();
    assert_eq!(store.list().len(), 20);
    for id in &ids {
        assert!(store.document(id).is_some(), "doc {id} lost");
    }
    assert!(!store.search("token7", 1, None).is_empty());
    assert!(!dir.path().join("docs").join("deadbeefdeadbeef.json.tmp").exists());
}

/// Child half of `kill_during_ingest_loses_nothing_acknowledged`.
#[test]
#[ignore]
fn crash_child_ingester() {
    let Ok(dir) = std::env::var("FERRICULA_CRASH_DIR") else { return };
    let mut store = DocumentStore::open(&dir).unwrap();
    let stdout = std::io::stdout();
    for i in 0..1_000_000u32 {
        let id = store.ingest(&note(i), "text").unwrap().record.meta.doc_id.clone();
        let mut out = stdout.lock();
        writeln!(out, "ack {id}").unwrap();
        out.flush().unwrap();
    }
}

/// kill -9 during ingest: every document the child acknowledged is on disk
/// and searchable after reopen.
#[test]
fn kill_during_ingest_loses_nothing_acknowledged() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "crash_child_ingester", "--ignored", "--nocapture", "--test-threads=1"])
        .env("FERRICULA_CRASH_DIR", dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut acked = Vec::new();
    for line in BufReader::new(child.stdout.take().unwrap()).lines() {
        if let Some(id) = line.unwrap().strip_prefix("ack ") {
            acked.push(id.trim().to_string());
            if acked.len() >= 60 {
                child.kill().unwrap();
                break;
            }
        }
    }
    let _ = child.wait();
    assert!(acked.len() >= 60, "child stopped early after {}", acked.len());
    let store = DocumentStore::open(dir.path()).unwrap();
    for id in &acked {
        assert!(store.document(id).is_some(), "acknowledged doc {id} lost");
    }
    assert!(store.list().len() >= acked.len());
}
