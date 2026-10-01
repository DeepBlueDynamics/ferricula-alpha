//! Ingest timing over the repo's research corpus.
//!
//! `cargo test -p ferricula-ingest --release --test bench_corpus -- --ignored --nocapture`

use std::time::Instant;

use ferricula_ingest::{DocumentStore, ExtractConfig, Source, extract};

#[test]
#[ignore]
fn ingest_research_corpus_timing() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../research");
    let mut files: Vec<_> = std::fs::read_dir(&root).unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
        .collect();
    files.sort();
    let docs: Vec<_> = files.iter().map(|p| {
        let text = std::fs::read_to_string(p).unwrap();
        extract(&Source::Text { title: None, text, origin: None }, &ExtractConfig::default()).unwrap()
    }).collect();

    let dir = tempfile::tempdir().unwrap();
    let mut store = DocumentStore::open(dir.path()).unwrap();
    let start = Instant::now();
    for doc in &docs {
        store.ingest(doc, "text").unwrap();
    }
    let ingest_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let hits = store.search("memory decay recall", 5, None);
    let search_ms = start.elapsed().as_secs_f64() * 1000.0;
    let sections: usize = store.list().iter().map(|m| m.sections as usize).sum();
    println!("files={} sections={} ingest_ms={ingest_ms:.1} first_search_ms={search_ms:.2} hits={}",
        docs.len(), sections, hits.len());
}
