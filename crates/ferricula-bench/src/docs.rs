//! Suite 2: document-memory retrieval over `ferricula_ingest::DocumentStore`.
//!
//! Corpus: every `*.md` directly under the corpus dir (default `research/`),
//! ingested as `Source::Text` with the file stem as title, plus PDFs from
//! `FERRICULA_BENCH_PDFS` when set. The store is written to a temp dir,
//! dropped and reopened from disk before querying, so queries run against
//! what was persisted.
//!
//! Queries: sentences (8-30 words) sampled with a fixed seed from the
//! extracted text. A sentence is kept only if exactly one stored section
//! contains it byte-for-byte; that section is the gold (doc_id, index).
//! Sentences contained in no section (cut by section packing) or in several
//! (repeated boilerplate) are counted and excluded.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use ferricula_ingest::{DocumentStore, ExtractConfig, Source, extract};
use serde_json::{Value, json};

use ferricula_semantic::text_embed::{ShivvrEmbedder, TextEmbedder};
use ferricula_server::meaning::{CatalogItem, MeaningIndex, MeaningKey, MeaningSet, SearchFilter};
use ferricula_server::recall::{ArmList, RecallCandidate, SectionEvidence, fuse_arms_all, promote_keys};

use crate::ledger::{Rng, RunContext, display_path, sha256_hex};
use crate::metrics::{j, mean, mrr, percentile, recall_at_k};

pub struct DocsArgs {
    pub corpus_dir: PathBuf,
    pub pdf_dir: Option<PathBuf>,
    pub n: usize,
    pub drop_frac: f64,
    pub paraphrase: Option<(String, String)>,
    /// shivvr base URL for the dense and hybrid arms (None: BM25 only).
    pub shivvr: Option<String>,
}

const CUTOFF: usize = 10;

struct Query {
    kind: &'static str,
    text: String,
    gold_doc: String,
    gold_section: u32,
}

pub fn run(ctx: &RunContext, args: &DocsArgs) -> Result<()> {
    // ---- corpus
    let mut files: Vec<PathBuf> = fs::read_dir(&args.corpus_dir)
        .with_context(|| format!("read {}", args.corpus_dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "md"))
        .collect();
    files.sort();
    let mut pdfs = Vec::new();
    if let Some(dir) = &args.pdf_dir {
        let mut v: Vec<PathBuf> = fs::read_dir(dir)?.filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))).collect();
        v.sort();
        pdfs = v;
    }

    let tmp = tempfile::tempdir()?;
    let config = ExtractConfig::default();
    // (doc_id, pages) for sampling and the byte-exact check.
    let mut sources: Vec<(String, Vec<String>)> = Vec::new();
    let mut manifest = String::new();
    let mut ingest_errors = Vec::new();
    let mut duplicates = 0usize;
    let t_ingest = Instant::now();
    {
        let mut store = DocumentStore::open(tmp.path())?;
        for path in files.iter().chain(pdfs.iter()) {
            let bytes = fs::read(path)?;
            manifest.push_str(&format!("{}  {}\n", sha256_hex(&bytes), display_path(path)));
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            let source = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
                Source::Pdf { name: path.file_name().unwrap().to_string_lossy().to_string(), bytes }
            } else {
                Source::Text { title: Some(stem.clone()), text: String::from_utf8_lossy(&bytes).to_string() }
            };
            let extracted = match extract(&source, &config) {
                Ok(e) => e,
                Err(e) => { ingest_errors.push(format!("{}: {e}", display_path(path))); continue; }
            };
            match store.ingest(&extracted, source.kind()) {
                Ok(ing) => {
                    if ing.duplicate { duplicates += 1; continue; }
                    sources.push((ing.record.meta.doc_id.clone(), extracted.pages.clone()));
                }
                Err(e) => ingest_errors.push(format!("{}: {e}", display_path(path))),
            }
        }
    }
    let ingest_ms = t_ingest.elapsed().as_secs_f64() * 1000.0;
    let t_open = Instant::now();
    let store = DocumentStore::open(tmp.path())?;
    let reopen_ms = t_open.elapsed().as_secs_f64() * 1000.0;
    let corpus_sha = sha256_hex(manifest.as_bytes());
    let docs = store.list();
    let total_sections: usize = docs.iter().map(|d| d.sections as usize).sum();
    let total_bytes: usize = docs.iter().map(|d| d.bytes).sum();

    // ---- verbatim storage: every persisted section equals its source slice
    let mut exact_ok = 0usize;
    let mut exact_bad = Vec::new();
    let mut all_sections: Vec<(String, u32, String)> = Vec::new();
    for (doc_id, pages) in &sources {
        let record = store.document(doc_id).context("document missing after reopen")?;
        for s in &record.sections {
            let page = &pages[s.page.map(|p| p as usize - 1).unwrap_or(0)];
            if page.get(s.offset..s.offset + s.text.len()) == Some(s.text.as_str()) { exact_ok += 1; }
            else { exact_bad.push(format!("{doc_id}#{}", s.index)); }
            all_sections.push((doc_id.clone(), s.index, s.text.clone()));
        }
    }

    // ---- sample sentences
    let mut candidates: Vec<(usize, String)> = Vec::new();
    for (d, (_, pages)) in sources.iter().enumerate() {
        for page in pages {
            for s in sentences(page) {
                candidates.push((d, s));
            }
        }
    }
    let n_candidates = candidates.len();
    let mut rng = Rng::new(ctx.seed);
    rng.shuffle(&mut candidates);
    let mut queries = Vec::new();
    let (mut not_contained, mut ambiguous, mut dup_sentence) = (0usize, 0usize, 0usize);
    let mut seen = std::collections::HashSet::new();
    let mut verbatim_gold_contains = 0usize;
    for (_d, sentence) in candidates {
        if queries.len() / 2 >= args.n { break; }
        if !seen.insert(sentence.clone()) { dup_sentence += 1; continue; }
        let holders: Vec<&(String, u32, String)> = all_sections.iter().filter(|(_, _, t)| t.contains(sentence.as_str())).collect();
        match holders.len() {
            0 => { not_contained += 1; continue; }
            1 => {}
            _ => { ambiguous += 1; continue; }
        }
        let (doc, idx, _) = holders[0];
        // Exact-quote check through the public lookup path.
        if store.section(doc, *idx).is_some_and(|(_, s)| s.text.contains(sentence.as_str())) {
            verbatim_gold_contains += 1;
        }
        let partial = drop_words(&sentence, args.drop_frac, &mut rng);
        queries.push(Query { kind: "verbatim", text: sentence.clone(), gold_doc: doc.clone(), gold_section: *idx });
        queries.push(Query { kind: "partial", text: partial, gold_doc: doc.clone(), gold_section: *idx });
    }
    let n_sampled = queries.len() / 2;

    // ---- optional paraphrase
    let mut paraphrase_errors = 0usize;
    if let Some((url, model)) = &args.paraphrase {
        let verbatim: Vec<(String, String, u32)> = queries.iter().filter(|q| q.kind == "verbatim")
            .map(|q| (q.text.clone(), q.gold_doc.clone(), q.gold_section)).collect();
        for (i, (text, doc, idx)) in verbatim.iter().enumerate() {
            match paraphrase(url, model, text) {
                Ok(p) => queries.push(Query { kind: "paraphrase", text: p, gold_doc: doc.clone(), gold_section: *idx }),
                Err(e) => { paraphrase_errors += 1; if paraphrase_errors <= 3 { eprintln!("paraphrase {i}: {e}"); } }
            }
            if i % 25 == 0 { eprintln!("paraphrased {i}/{}", verbatim.len()); }
        }
    }

    // ---- dense index over every section (optional)
    let mut dense_note: Option<String> = None;
    let mut dense: Option<(ShivvrEmbedder, MeaningIndex, f64)> = None;
    if let Some(url) = &args.shivvr {
        match build_dense(url, &all_sections) {
            Ok(d) => dense = Some(d),
            Err(e) => dense_note = Some(format!("dense arms skipped: {e:#}")),
        }
    }
    let mut query_embed_ms = 0.0;
    let qvecs: Option<Vec<Vec<f32>>> = match &dense {
        Some((embedder, _, _)) => {
            let texts: Vec<&str> = queries.iter().map(|q| q.text.as_str()).collect();
            let t0 = Instant::now();
            match embedder.embed(&texts) {
                Ok(v) => {
                    query_embed_ms = t0.elapsed().as_secs_f64() * 1000.0 / texts.len().max(1) as f64;
                    Some(v)
                }
                Err(e) => {
                    dense_note = Some(format!("dense arms skipped: query embedding failed: {e:#}"));
                    None
                }
            }
        }
        None => None,
    };

    // ---- query
    // (kind, arm) -> (ranks, doc ranks, latency ms, empty results)
    type Acc = (Vec<Option<usize>>, Vec<Option<usize>>, Vec<f64>, usize);
    let mut by_kind: BTreeMap<(&str, &str), Acc> = BTreeMap::new();
    let mut examples: Vec<Value> = Vec::new();
    for (qi, q) in queries.iter().enumerate() {
        let t0 = Instant::now();
        let hits = store.search(&q.text, CUTOFF, None);
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let bm25: Vec<(String, u32)> = hits.iter().map(|h| (h.doc_id.clone(), h.section.index)).collect();
        let mut arms: Vec<(&str, Vec<(String, u32)>, f64)> = vec![("bm25", bm25.clone(), ms)];
        if let (Some((_, index, _)), Some(qvecs)) = (&dense, &qvecs) {
            let t1 = Instant::now();
            let filter = SearchFilter { recovered: false, experience: false, ..SearchFilter::evidence(false) };
            let dhits = index.search(&qvecs[qi], CUTOFF * 2, &filter);
            let dense_keys: Vec<(String, u32)> = dhits.iter().filter_map(|h| match &h.key {
                MeaningKey::Section(d, i) => Some((d.clone(), *i)),
                _ => None,
            }).collect();
            let dense_ms = t1.elapsed().as_secs_f64() * 1000.0;
            let as_list = |arm: &'static str, keys: &[(String, u32)]| ArmList {
                arm,
                weight: 1.0,
                items: keys.iter().enumerate().map(|(i, (d, x))| RecallCandidate::from_section(&SectionEvidence {
                    doc_id: d.clone(), index: *x, page: None, title: String::new(), heading: String::new(),
                    origin: String::new(), text: String::new(), score: 0.0, cite: String::new(),
                }, i + 1)).collect(),
            };
            let t2 = Instant::now();
            let rrf = fuse_arms_all(vec![as_list("bm25", &bm25), as_list("dense", &dense_keys)]);
            let mut guaranteed = rrf.clone();
            let promote: Vec<String> = dense_keys.iter().take(2).map(|(d, x)| format!("doc:{d}:{x}")).collect();
            promote_keys(&mut guaranteed, &promote);
            let fuse_ms = t2.elapsed().as_secs_f64() * 1000.0;
            let keys = |c: &[RecallCandidate]| c.iter().filter_map(|c| c.section.as_ref().map(|s| (s.doc_id.clone(), s.index))).collect::<Vec<_>>();
            arms.push(("dense", dense_keys.iter().take(CUTOFF).cloned().collect(), dense_ms));
            arms.push(("hybrid_rrf", keys(&rrf).into_iter().take(CUTOFF).collect(), ms + dense_ms + fuse_ms));
            arms.push(("hybrid", keys(&guaranteed).into_iter().take(CUTOFF).collect(), ms + dense_ms + fuse_ms));
        }
        for (arm, keys, ms) in arms {
            let rank = keys.iter().position(|(d, x)| *d == q.gold_doc && *x == q.gold_section).map(|r| r + 1);
            let doc_rank = keys.iter().position(|(d, _)| *d == q.gold_doc).map(|r| r + 1);
            let e = by_kind.entry((q.kind, arm)).or_default();
            e.0.push(rank);
            e.1.push(doc_rank);
            e.2.push(ms);
            if keys.is_empty() { e.3 += 1; }
            if arm == "bm25" && rank != Some(1) && examples.len() < 12 {
                examples.push(json!({"kind": q.kind, "query": q.text, "gold": format!("{}#{}", q.gold_doc, q.gold_section),
                    "rank": rank, "top1": hits.first().map(|h| format!("{}#{} ({})", h.doc_id, h.section.index, h.title))}));
            }
        }
    }

    // `metrics` keeps its pre-R2b meaning (BM25); dense and hybrid arms are
    // additive keys.
    let mut metrics = serde_json::Map::new();
    let mut metrics_by_arm: BTreeMap<&str, serde_json::Map<String, Value>> = BTreeMap::new();
    for ((kind, arm), (ranks, doc_ranks, lat, empty)) in &by_kind {
        let m = json!({
            "n": ranks.len(),
            "recall_at_1": j(recall_at_k(ranks, 1)),
            "recall_at_5": j(recall_at_k(ranks, 5)),
            "recall_at_10": j(recall_at_k(ranks, 10)),
            "mrr_at_10": j(mrr(ranks)),
            "doc_recall_at_1": j(recall_at_k(doc_ranks, 1)),
            "empty_results": empty,
            "latency_ms": { "mean": j(mean(lat)), "p50": j(percentile(lat, 50.0)), "p95": j(percentile(lat, 95.0)) },
        });
        if *arm == "bm25" {
            metrics.insert(kind.to_string(), m.clone());
        }
        metrics_by_arm.entry(arm).or_default().insert(kind.to_string(), m);
    }
    let dense_meta = match &dense {
        Some((embedder, index, embed_ms)) => json!({
            "embedder": format!("shivvr {}", args.shivvr.as_deref().unwrap_or("")),
            "space": embedder.space(),
            "sections_embedded": index.counts().sections_embedded,
            "sections_truncated": index.counts().truncated_sources,
            "embed_sections_ms": j(*embed_ms),
            "query_embed_ms_mean": j(query_embed_ms),
            "fusion": "hybrid_rrf = RRF k=60 over bm25 + dense; hybrid = hybrid_rrf + meaning guarantee (dense top-2 at ranks 2, 4), the server default",
        }),
        None => json!(dense_note.clone().unwrap_or_else(|| "skipped: BENCH_SHIVVR_URL=none".into())),
    };
    let storage = json!({
        "sections_total": total_sections,
        "sections_byte_exact": exact_ok,
        "sections_not_exact": exact_bad.len(),
        "sampled_gold_contains_sentence": verbatim_gold_contains,
        "sampled": n_sampled,
    });
    let sampling = json!({
        "candidates": n_candidates, "sampled": n_sampled, "excluded_not_in_any_section": not_contained,
        "excluded_in_multiple_sections": ambiguous, "excluded_duplicate_sentence": dup_sentence,
        "sentence_words": [8, 30], "partial_drop_fraction": args.drop_frac, "cutoff": CUTOFF,
    });
    let paraphrase_meta = match &args.paraphrase {
        Some((url, model)) => json!({"url": url, "model": model, "errors": paraphrase_errors}),
        None => json!("skipped: BENCH_PARAPHRASE_MODEL unset"),
    };
    ctx.append(json!({
        "status": "ok",
        "dataset": {
            "path": display_path(&args.corpus_dir),
            "pdf_dir": args.pdf_dir.as_ref().map(|p| display_path(p)),
            "sha256": corpus_sha,
            "sha256_of": "manifest of '<sha256>  <path>' lines, sorted md then pdf",
            "files": files.len() + pdfs.len(),
            "documents": docs.len(),
            "sections": total_sections,
            "bytes": total_bytes,
            "ingest_errors": ingest_errors,
            "duplicates": duplicates,
        },
        "backend": { "kind": "ferricula_ingest::DocumentStore", "ranker": "lume field-aware BM25+ (SearchVariant::Plus, default params)" },
        "model": if dense.is_some() { json!("gtr-t5-base") } else { Value::Null },
        "n": n_sampled,
        "metrics": metrics,
        "metrics_by_arm": metrics_by_arm,
        "dense": dense_meta,
        "storage": storage,
        "sampling": sampling,
        "paraphrase": paraphrase_meta,
        "ingest_ms": j(ingest_ms),
        "reopen_ms": j(reopen_ms),
    }))?;

    // ---- report
    let mut r = ctx.report_header("Document-memory retrieval (DocumentStore)");
    r.push_str(&format!(
        "Corpus: `{}` ({} markdown) + {} PDFs; {} documents, {} sections, {} bytes; corpus manifest sha256 `{}`.\n\
         Ingest {:.0} ms; store reopened from disk in {:.0} ms before querying.\n\n",
        display_path(&args.corpus_dir), files.len(), pdfs.len(), docs.len(), total_sections, total_bytes, corpus_sha, ingest_ms, reopen_ms));
    if !ingest_errors.is_empty() {
        r.push_str(&format!("Ingest errors: {}\n\n", ingest_errors.join("; ")));
    }
    r.push_str("## Retrieval\n\nGold = the single (doc_id, section index) containing the sampled sentence. Ranked list cut at 10. \
        Arms: `bm25` (the document store), `dense` (GTR-T5 cosine over every section), `hybrid_rrf` (RRF k=60 over both), \
        `hybrid` (hybrid_rrf + the server's meaning guarantee: dense top-2 lifted to ranks 2 and 4). Latency excludes the query embedding.\n\n\
        | query type | arm | n | recall@1 | recall@5 | MRR@10 | doc recall@1 | empty | latency p50 / p95 ms |\n|---|---|---:|---:|---:|---:|---:|---:|---|\n");
    let kinds: Vec<&str> = metrics.keys().map(String::as_str).collect();
    for kind in kinds {
        for arm in ["bm25", "dense", "hybrid_rrf", "hybrid"] {
            let Some(m) = metrics_by_arm.get(arm).and_then(|a| a.get(kind)) else { continue };
            r.push_str(&format!("| {kind} | {arm} | {} | {} | {} | {} | {} | {} | {} / {} |\n", m["n"], f(&m["recall_at_1"]), f(&m["recall_at_5"]),
                f(&m["mrr_at_10"]), f(&m["doc_recall_at_1"]), m["empty_results"], f(&m["latency_ms"]["p50"]), f(&m["latency_ms"]["p95"])));
        }
    }
    match &dense {
        Some((_, index, embed_ms)) => r.push_str(&format!(
            "\nDense: {} sections embedded through shivvr in {:.0} ms ({} cut to a prefix); query embedding {:.1} ms mean per query (batched).\n",
            index.counts().sections_embedded, embed_ms, index.counts().truncated_sources, query_embed_ms)),
        None => r.push_str(&format!("\n{}\n", dense_note.clone().unwrap_or_else(|| "Dense arms skipped (BENCH_SHIVVR_URL=none).".into()))),
    }
    if args.paraphrase.is_none() {
        r.push_str("\nParaphrase queries: skipped (`BENCH_PARAPHRASE_MODEL` unset).\n");
    }
    r.push_str(&format!(
        "\n## Verbatim storage\n\n- Persisted sections equal to their source byte slice (`page[offset..offset+len]`): **{exact_ok} / {total_sections}**\n\
         - Sampled sentences whose gold section (via `DocumentStore::section`) contains them byte-for-byte: **{verbatim_gold_contains} / {n_sampled}**\n\n\
         ## Sampling\n\n- Candidate sentences (8-30 words): {n_candidates}\n- Sampled: {n_sampled} (seed {})\n\
         - Excluded, not inside any single section (cut by section packing): {not_contained}\n- Excluded, found in more than one section: {ambiguous}\n\
         - Excluded, duplicate sentence text: {dup_sentence}\n- Partial queries drop each word with p = {} (at least 3 words kept)\n\n",
        ctx.seed, args.drop_frac));
    r.push_str("## Misses (first 12)\n\n");
    for e in &examples {
        r.push_str(&format!("- [{}] rank {}, top1 {}: \"{}\"\n", e["kind"].as_str().unwrap_or(""), e["rank"],
            e["top1"].as_str().unwrap_or("none"), e["query"].as_str().unwrap_or("").replace('|', "\\|")));
    }
    r.push_str("\n## Caveats\n\n- Queries are drawn from the corpus itself, so verbatim recall measures lexical addressability of stored text, not question answering.\n\
        - Partial queries are random word drops, not natural queries.\n- Dense arms use GTR-T5-base (the recovered memory's space); no reranking.\n");
    let path = ctx.write_report(&r)?;
    eprintln!("report: {}", path.display());
    Ok(())
}

/// Embed every section through shivvr into a sections-only meaning index.
fn build_dense(url: &str, sections: &[(String, u32, String)]) -> Result<(ShivvrEmbedder, MeaningIndex, f64)> {
    let embedder = ShivvrEmbedder::new(url, "gtr-t5-base@768", Duration::from_secs(300), 8)?;
    let mut index = MeaningIndex::new(embedder.space(), embedder.dim(), None);
    index.set_catalog(MeaningSet::Section, sections.iter()
        .filter(|(_, _, t)| !t.trim().is_empty())
        .map(|(d, i, t)| CatalogItem::section(d, *i, t)).collect());
    let pending = index.pending(&[MeaningSet::Section]);
    let t0 = Instant::now();
    for chunk in pending.chunks(8) {
        let texts: Vec<&str> = chunk.iter().map(|i| i.text.as_str()).collect();
        for (item, v) in chunk.iter().zip(embedder.embed(&texts)?) {
            index.insert(item, v)?;
        }
    }
    Ok((embedder, index, t0.elapsed().as_secs_f64() * 1000.0))
}

fn f(v: &Value) -> String {
    v.as_f64().map(|x| format!("{x:.3}")).unwrap_or_else(|| "—".into())
}

/// Sentences of 8-30 words: lines split at `. `, `? `, `! `; table rows,
/// code fences and headings skipped; leading list/quote markers stripped.
pub fn sentences(page: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_code = false;
    for line in page.lines() {
        let t = line.trim();
        if t.starts_with("```") { in_code = !in_code; continue; }
        if in_code || t.starts_with('|') || t.starts_with('#') || t.is_empty() { continue; }
        let t = t.trim_start_matches(|c: char| c == '-' || c == '*' || c == '>' || c == ' ');
        let t = strip_ordinal(t);
        let mut start = 0;
        let bytes = t.as_bytes();
        for i in 0..bytes.len() {
            let end_here = matches!(bytes[i], b'.' | b'?' | b'!') && (i + 1 == bytes.len() || bytes[i + 1] == b' ');
            if end_here {
                push_sentence(&t[start..=i], &mut out);
                start = i + 1;
            }
        }
        if start < t.len() { push_sentence(&t[start..], &mut out); }
    }
    out
}

fn strip_ordinal(t: &str) -> &str {
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 && t[digits..].starts_with(". ") { &t[digits + 2..] } else { t }
}

fn push_sentence(s: &str, out: &mut Vec<String>) {
    let s = s.trim();
    let words = s.split_whitespace().count();
    if (8..=30).contains(&words) {
        out.push(s.to_string());
    }
}

/// Drop each word with probability `frac`, keeping at least 3 words.
pub fn drop_words(sentence: &str, frac: f64, rng: &mut Rng) -> String {
    let words: Vec<&str> = sentence.split_whitespace().collect();
    let mut kept: Vec<&str> = words.iter().copied().filter(|_| rng.next_f64() >= frac).collect();
    if kept.len() < 3 {
        kept = words.iter().copied().take(3).collect();
    }
    kept.join(" ")
}

fn paraphrase(url: &str, model: &str, text: &str) -> Result<String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(120)).build();
    let body: Value = agent.post(&format!("{}/chat/completions", url.trim_end_matches('/')))
        .send_json(json!({
            "model": model,
            "temperature": 0,
            "messages": [
                {"role": "system", "content": "Paraphrase the user's sentence with different wording but the same meaning. Output only the paraphrase."},
                {"role": "user", "content": text},
            ],
        }))?
        .into_json()?;
    let out = body.pointer("/choices/0/message/content").and_then(Value::as_str).unwrap_or("").trim().to_string();
    anyhow::ensure!(!out.is_empty(), "empty paraphrase");
    Ok(out)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentence_splitter() {
        let page = "# Heading here is ignored even if long enough to count words\n\
            - This bullet sentence has exactly eight words in it. Short one.\n\
            | table | row | is | skipped | even | with | many | cells |\n\
            ```\ncode line one two three four five six seven eight\n```\n\
            1. Numbered item text that is long enough to be kept here.\n";
        let s = sentences(page);
        assert_eq!(s, vec![
            "This bullet sentence has exactly eight words in it.".to_string(),
            "Numbered item text that is long enough to be kept here.".to_string(),
        ]);
    }

    #[test]
    fn drop_words_is_deterministic_and_keeps_three() {
        let s = "one two three four five six seven eight nine ten";
        let a = drop_words(s, 0.4, &mut Rng::new(3));
        let b = drop_words(s, 0.4, &mut Rng::new(3));
        assert_eq!(a, b);
        assert!(a.split_whitespace().count() >= 3);
        assert_eq!(drop_words(s, 1.0, &mut Rng::new(3)), "one two three");
    }
}
