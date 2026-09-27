use ferricula_ingest::{DocumentStore, ExtractConfig, Source, extract};
use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, Stream, dictionary};

/// A real PDF with one line of text per page.
fn pdf(pages: &[&str]) -> Vec<u8> {
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

#[test]
fn pdf_ingest_is_paged_searchable_durable_and_deduped() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = pdf(&["Impermanence is the default state of memory records", "Recall strengthens a trace and slows its decay"]);
    let source = Source::Pdf { name: "notes.pdf".into(), bytes };
    let extracted = extract(&source, &ExtractConfig::default()).unwrap();
    assert!(extracted.paged);
    assert_eq!(extracted.pages.len(), 2);

    let doc_id = {
        let mut store = DocumentStore::open(dir.path()).unwrap();
        let first = store.ingest(&extracted, "pdf").unwrap();
        assert!(!first.duplicate);
        let id = first.record.meta.doc_id.clone();
        assert!(store.ingest(&extracted, "pdf").unwrap().duplicate);
        id
    };

    // Reopen: documents persist and the index is rebuilt.
    let store = DocumentStore::open(dir.path()).unwrap();
    assert_eq!(store.list().len(), 1);
    let hits = store.search("recall decay", 3, None);
    assert!(!hits.is_empty());
    assert_eq!(hits[0].doc_id, doc_id);
    assert_eq!(hits[0].section.page, Some(2));
    assert!(hits[0].section.text.contains("Recall strengthens"));

    let (_, section) = store.section(&doc_id, hits[0].section.index).unwrap();
    assert_eq!(section, &hits[0].section);
    assert!(store.search("recall", 3, Some("0000000000000000")).is_empty());
}

#[test]
fn markdown_text_ingest_keeps_headings() {
    let dir = tempfile::tempdir().unwrap();
    let text = "# Sense doors\nSix doors admit contact with objects.\n\n# Gates\nA judge answers yes, no, or abstains.\n";
    let extracted = extract(&Source::Text { title: None, text: text.into() }, &ExtractConfig::default()).unwrap();
    let mut store = DocumentStore::open(dir.path()).unwrap();
    store.ingest(&extracted, "text").unwrap();
    let hits = store.search("judge abstains", 1, None);
    assert_eq!(hits[0].section.heading, "Gates");
    assert_eq!(hits[0].title, "Sense doors");
}

/// `FERRICULA_TEST_PDF=path cargo test -p ferricula-ingest -- --ignored`
#[test]
#[ignore]
fn real_pdf_from_env() {
    let path = std::env::var("FERRICULA_TEST_PDF").expect("set FERRICULA_TEST_PDF");
    let bytes = std::fs::read(&path).unwrap();
    let extracted = extract(&Source::Pdf { name: path.clone(), bytes }, &ExtractConfig::default()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut store = DocumentStore::open(dir.path()).unwrap();
    let meta = store.ingest(&extracted, "pdf").unwrap().record.meta.clone();
    println!("title={:?} pages={} sections={} bytes={}", meta.title, meta.pages, meta.sections, meta.bytes);
    for hit in store.search("memory", 3, None) {
        println!("[{} §{} p{:?}] {:.60}", hit.score, hit.section.index, hit.section.page, hit.section.text.replace('\n', " "));
    }
}
