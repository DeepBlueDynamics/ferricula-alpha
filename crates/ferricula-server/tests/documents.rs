//! R1 exit path: hand the agent a document, recall cites verbatim section
//! text, the answer survives a restart, and the recovered memory_dir is
//! byte-identical before and after.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use ferricula_core::{DurableEngine, MemoryRecord, Row};
use ferricula_ingest::Source;
use ferricula_server::config::RuntimeConfig;
use ferricula_server::inspect_data_dir;
use ferricula_server::memory::EXPERIENCE_ID_BASE;
use ferricula_server::recall::CandidateKind;
use ferricula_server::runtime::AgentRuntime;

const DOC: &str = "# Virtual context\n\
MemGPT pages information between a bounded main context and external storage, \
the way an operating system pages between RAM and disk.\n\n\
# Interrupts\n\
Events such as user messages and timers interrupt the processor and trigger inference.\n";

fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.insert(path.strip_prefix(dir).unwrap().to_path_buf(), fs::read(&path).unwrap());
            }
        }
    }
    out
}

fn fixture() -> (tempfile::TempDir, RuntimeConfig) {
    let root = tempfile::tempdir().unwrap();
    let memory = root.path().join("memory");
    fs::create_dir_all(&memory).unwrap();
    fs::write(memory.join("identity.json"), r#"{"agent_id":"ferricula-agent","name":"Test Persona"}"#).unwrap();
    fs::write(memory.join("agent.toml"), "name = \"Test Persona\"\nrole = \"a test fixture\"\n").unwrap();
    {
        // A recovered base with real memories, including one about paging.
        let mut engine = DurableEngine::open(&memory).unwrap();
        for (id, text) in [(3u32, "I once shipped a product about paging memory"), (41, "design is how it works")] {
            let row = Row {
                id, vector: vec![1.0, 0.0], refs: None,
                tags: BTreeMap::from([("text".to_string(), text.to_string())]),
            };
            engine.remember(row, MemoryRecord::new(id)).unwrap();
        }
        engine.checkpoint().unwrap();
    }
    let mut config = RuntimeConfig::default();
    config.memory_dir = memory;
    config.state_dir = root.path().join("state");
    config.overlay.path = config.state_dir.join("overlay.json");
    config.schedule.enabled = false;
    config.require_operator_auth = false;
    (root, config)
}

fn open(config: &RuntimeConfig) -> std::sync::Arc<AgentRuntime> {
    AgentRuntime::open(config.clone(), inspect_data_dir(&config.memory_dir).unwrap()).unwrap()
}

#[tokio::test]
async fn ingest_recall_restart_and_recovered_base_untouched() {
    let (_root, config) = fixture();
    let before = snapshot(&config.memory_dir);

    let (doc_id, memory_id) = {
        let runtime = open(&config);
        let outcome = runtime.ingest(
            Source::Text { title: None, text: DOC.into() },
            Some("compare this with how you remember".into()),
        ).await.unwrap();
        assert_eq!(outcome.title, "Virtual context");
        assert_eq!(outcome.sections, 2);
        assert!(!outcome.duplicate);
        // Experience ids cannot collide with recovered ids (3, 41).
        assert!(outcome.memory_id >= EXPERIENCE_ID_BASE);

        let again = runtime.ingest(Source::Text { title: None, text: DOC.into() }, None).await.unwrap();
        assert!(again.duplicate);
        assert_eq!(again.memory_id, outcome.memory_id);
        assert_eq!(runtime.experience_len(), 1);
        (outcome.doc_id, outcome.memory_id)
    };

    // Restart: everything is reopened from disk.
    let runtime = open(&config);
    let recall = runtime.hybrid_recall("paging memory between main context and disk", 10);
    let kinds: BTreeSet<_> = recall.candidates.iter().map(|c| format!("{:?}", c.kind)).collect();
    assert!(kinds.contains("Memory") && kinds.contains("Experience") && kinds.contains("DocumentSection"), "{kinds:?}");
    let section = recall.candidates.iter()
        .find(|c| c.kind == CandidateKind::DocumentSection)
        .and_then(|c| c.section.clone()).unwrap();
    assert_eq!(section.doc_id, doc_id);
    assert!(DOC.contains(&section.text), "section text must be verbatim");
    assert!(section.text.contains("pages information between a bounded main context"));
    assert_eq!(section.cite, format!("[doc {doc_id}§{}]", section.index));
    assert!(recall.experience_hits.iter().any(|h| h.id == memory_id && h.tags["channel"] == "reading"));
    // Legacy `hits` still means recovered-base hits only.
    assert!(recall.hits.iter().all(|h| h.id < EXPERIENCE_ID_BASE));
    assert!(recall.hits.iter().any(|h| h.id == 3));

    let read = runtime.document_section(&doc_id, section.index).unwrap();
    assert_eq!(read.text, section.text);
    assert_eq!(runtime.documents().len(), 1);
    let status = runtime.status();
    assert_eq!((status.documents, status.experience_records), (1, 1));

    // State lives under state_dir; the recovered base is byte-identical.
    assert!(config.state_dir.join("documents").join("docs").join(format!("{doc_id}.json")).exists());
    assert!(config.state_dir.join("experience").exists());
    drop(runtime);
    assert_eq!(snapshot(&config.memory_dir), before, "recovered memory_dir was modified");
}

#[tokio::test]
async fn pdf_ingest_is_paged_and_cited_by_page() {
    let (_root, config) = fixture();
    let runtime = open(&config);
    let bytes = pdf(&["Title page of a short paper", "Recall strengthens a trace and slows its decay"]);
    let outcome = runtime.ingest(Source::Pdf { name: "notes.pdf".into(), bytes }, None).await.unwrap();
    assert_eq!(outcome.pages, 2);
    assert_eq!(outcome.origin, "notes.pdf");
    let hits = runtime.search_documents("recall decay", 3, None);
    assert_eq!(hits[0].page, Some(2));
    assert!(hits[0].text.contains("Recall strengthens"));
    assert_eq!(hits[0].cite, format!("[doc {}§{} p.2]", outcome.doc_id, hits[0].index));
}

#[test]
fn state_dir_inside_memory_dir_is_refused_before_creation() {
    let (_root, mut config) = fixture();
    config.state_dir = config.memory_dir.join("runtime");
    let before = snapshot(&config.memory_dir);
    let result = AgentRuntime::open(config.clone(), inspect_data_dir(&config.memory_dir).unwrap());
    assert!(result.is_err());
    assert!(!config.state_dir.exists());
    assert_eq!(snapshot(&config.memory_dir), before);
}

#[tokio::test]
async fn disabled_documents_and_urls_are_refused() {
    let (_root, mut config) = fixture();
    config.documents.allow_url = false;
    let runtime = open(&config);
    let err = runtime.ingest(Source::Url { url: "https://example.org".into() }, None).await.unwrap_err();
    assert!(err.to_string().contains("allow_url"));
    drop(runtime);
    config.documents.enabled = false;
    let runtime = open(&config);
    assert!(runtime.ingest(Source::Text { title: None, text: "x".into() }, None).await.is_err());
}

/// A real PDF with one line of text per page (same approach as
/// ferricula-ingest's tests).
fn pdf(pages: &[&str]) -> Vec<u8> {
    use lopdf::content::{Content, Operation};
    use lopdf::{Document, Object, Stream, dictionary};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Courier",
    });
    let resources_id = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font_id } });
    let mut kids = Vec::new();
    for text in pages {
        let content = Content { operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 12.into()]),
            Operation::new("Td", vec![72.into(), 700.into()]),
            Operation::new("Tj", vec![Object::string_literal(*text)]),
            Operation::new("ET", vec![]),
        ]};
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "Contents" => content_id,
        });
        kids.push(page_id.into());
    }
    let count = kids.len() as i64;
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! {
        "Type" => "Pages", "Kids" => kids, "Count" => count, "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    }));
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}
