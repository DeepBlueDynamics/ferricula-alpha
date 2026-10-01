//! Suite 3 (skeleton): LongMemEval through three arms.
//!
//! Dataset: LongMemEval JSON (a top-level array), e.g. `longmemeval_s.json`
//! or `longmemeval_oracle.json` from the LongMemEval release. Not downloaded
//! automatically. Expected at `data/longmemeval/longmemeval_s.json` or
//! wherever `--dataset` / `LONGMEMEVAL_PATH` points. Fields used per item:
//! `question_id`, `question_type`, `question`, `answer`, `question_date`,
//! `haystack_session_ids`, `haystack_dates`, `haystack_sessions` (a list of
//! sessions, each a list of `{role, content, has_answer?}` turns) and
//! `answer_session_ids`. Question ids ending in `_abs` are abstention items.
//!
//! Arms:
//! - `no_memory`: answer model sees only the question (needs
//!   `BENCH_ANSWER_MODEL` at `OLLAMA_URL`; skipped otherwise).
//! - `bm25_rag`: sessions ingested into a fresh `DocumentStore` per question;
//!   session recall@k against `answer_session_ids` is measured without any
//!   LLM; answers are generated only when an answer model is configured.
//! - `full_server`: behind `--full-server`; NOT RUNNABLE yet (haystack
//!   replay into the server is not wired). Recorded as `not_implemented`.
//!
//! QA scoring here is a naive normalized-substring match, NOT the official
//! LongMemEval GPT-4o judge; numbers from it are labelled as such.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use ferricula_ingest::{DocumentStore, ExtractConfig, Source, extract};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::ledger::{RunContext, display_path, sha256_file};
use crate::metrics::{j, recall_at_k};

pub const DEFAULT_PATH: &str = "data/longmemeval/longmemeval_s.json";

pub struct LongmemArgs {
    pub dataset: PathBuf,
    pub limit: Option<usize>,
    pub full_server: bool,
    pub answer: Option<(String, String)>,
    pub server_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Turn {
    pub role: String,
    pub content: String,
    #[serde(default)]
    #[allow(dead_code)] // kept for evidence-turn analysis
    pub has_answer: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LmeItem {
    pub question_id: String,
    pub question_type: String,
    pub question: String,
    /// Usually a string; some items carry numbers.
    pub answer: Value,
    #[serde(default)]
    pub question_date: Option<String>,
    pub haystack_session_ids: Vec<String>,
    #[serde(default)]
    pub haystack_dates: Vec<String>,
    pub haystack_sessions: Vec<Vec<Turn>>,
    #[serde(default)]
    pub answer_session_ids: Vec<String>,
}

impl LmeItem {
    pub fn is_abstention(&self) -> bool {
        self.question_id.ends_with("_abs")
    }

    pub fn answer_text(&self) -> String {
        match &self.answer {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        }
    }
}

pub fn load(path: &PathBuf) -> Result<Vec<LmeItem>> {
    let text = fs::read_to_string(path).with_context(|| format!(
        "read {} (LongMemEval is not downloaded automatically; fetch longmemeval_s.json from the LongMemEval release and pass --dataset or LONGMEMEVAL_PATH; default {DEFAULT_PATH})",
        path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse {} as LongMemEval JSON array", path.display()))
}

fn session_text(item: &LmeItem, i: usize) -> String {
    let date = item.haystack_dates.get(i).map(String::as_str).unwrap_or("");
    let mut s = format!("# Session {} {date}\n\n", item.haystack_session_ids[i]);
    for t in &item.haystack_sessions[i] {
        s.push_str(&format!("{}: {}\n\n", t.role, t.content));
    }
    s
}

pub fn run(ctx: &RunContext, args: &LongmemArgs) -> Result<()> {
    let items = load(&args.dataset)?;
    let items: Vec<LmeItem> = match args.limit { Some(n) => items.into_iter().take(n).collect(), None => items };
    if items.is_empty() {
        bail!("dataset has no items");
    }
    let config = ExtractConfig::default();
    let mut session_ranks: Vec<Option<usize>> = Vec::new();
    let mut qa: BTreeMap<&str, (usize, usize, usize)> = BTreeMap::new(); // arm -> (n, contains-correct, abstention items)
    for (qi, item) in items.iter().enumerate() {
        // bm25_rag retrieval
        let tmp = tempfile::tempdir()?;
        let mut store = DocumentStore::open(tmp.path())?;
        let mut doc_to_session = BTreeMap::new();
        for i in 0..item.haystack_sessions.len().min(item.haystack_session_ids.len()) {
            let text = session_text(item, i);
            let extracted = extract(&Source::Text { title: Some(item.haystack_session_ids[i].clone()), text, origin: None }, &config)?;
            if let Ok(ing) = store.ingest(&extracted, "text") {
                doc_to_session.insert(ing.record.meta.doc_id.clone(), item.haystack_session_ids[i].clone());
            }
        }
        let hits = store.search(&item.question, 10, None);
        let mut sessions_ranked: Vec<String> = Vec::new();
        for h in &hits {
            if let Some(s) = doc_to_session.get(&h.doc_id) {
                if !sessions_ranked.contains(s) { sessions_ranked.push(s.clone()); }
            }
        }
        if !item.answer_session_ids.is_empty() {
            session_ranks.push(sessions_ranked.iter().position(|s| item.answer_session_ids.contains(s)).map(|r| r + 1));
        }
        if let Some((url, model)) = &args.answer {
            let context: String = hits.iter().take(5).map(|h| h.section.text.clone()).collect::<Vec<_>>().join("\n---\n");
            for (arm, prompt) in [
                ("no_memory", item.question.clone()),
                ("bm25_rag", format!("Conversation history excerpts:\n{context}\n\nQuestion (asked {}): {}",
                    item.question_date.as_deref().unwrap_or("?"), item.question)),
            ] {
                let e = qa.entry(arm).or_default();
                e.0 += 1;
                if item.is_abstention() { e.2 += 1; }
                if let Ok(reply) = ask(url, model, &prompt) {
                    if normalize(&reply).contains(&normalize(&item.answer_text())) { e.1 += 1; }
                }
            }
        }
        if qi % 10 == 0 { eprintln!("longmem {qi}/{}", items.len()); }
    }
    let by_type = items.iter().fold(BTreeMap::<String, usize>::new(), |mut m, i| { *m.entry(i.question_type.clone()).or_default() += 1; m });
    let full_server = if args.full_server {
        json!({"status": "not_implemented", "reason": "haystack replay into the server (/chat or MCP) is not wired; asking /chat without replay would measure the operator's live memory, not LongMemEval",
               "server_url": args.server_url})
    } else {
        json!({"status": "skipped", "reason": "enable with --full-server"})
    };
    let qa_json: Value = qa.iter().map(|(arm, (n, ok, abs))| (arm.to_string(), json!({
        "n": n, "contains_match": j(*ok as f64 / (*n).max(1) as f64), "abstention_items": abs,
        "scorer": "naive normalized substring; NOT the official LongMemEval judge"}))).collect::<serde_json::Map<_, _>>().into();
    ctx.append(json!({
        "status": "skeleton",
        "dataset": { "path": display_path(&args.dataset), "sha256": sha256_file(&args.dataset)?, "items": items.len(), "question_types": by_type },
        "backend": { "bm25_rag": "ferricula_ingest::DocumentStore per question", "answer_model": args.answer.as_ref().map(|a| &a.1) },
        "n": items.len(),
        "metrics": {
            "bm25_rag_session_retrieval": { "n": session_ranks.len(), "recall_at_1": j(recall_at_k(&session_ranks, 1)),
                "recall_at_5": j(recall_at_k(&session_ranks, 5)), "recall_at_10": j(recall_at_k(&session_ranks, 10)) },
            "qa": qa_json,
            "full_server": full_server,
        },
    }))?;
    let mut r = ctx.report_header("LongMemEval (skeleton)");
    r.push_str(&format!("Dataset `{}`, {} items. **Skeleton run**: session retrieval only unless an answer model is configured; the full-server arm is not implemented.\n\n",
        display_path(&args.dataset), items.len()));
    r.push_str(&format!("| arm | metric | value |\n|---|---|---|\n| bm25_rag | session recall@1 / @5 / @10 (n={}) | {:.3} / {:.3} / {:.3} |\n",
        session_ranks.len(), recall_at_k(&session_ranks, 1), recall_at_k(&session_ranks, 5), recall_at_k(&session_ranks, 10)));
    for (arm, (n, ok, _)) in &qa {
        r.push_str(&format!("| {arm} | naive contains-match QA (n={n}) | {:.3} |\n", *ok as f64 / (*n).max(1) as f64));
    }
    r.push_str(&format!("| full_server | — | {} |\n", full_server["status"].as_str().unwrap_or("")));
    ctx.write_report(&r)?;
    Ok(())
}

fn normalize(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect::<String>()
        .split_whitespace().collect::<Vec<_>>().join(" ")
}

fn ask(url: &str, model: &str, prompt: &str) -> Result<String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(180)).build();
    let body: Value = agent.post(&format!("{}/chat/completions", url.trim_end_matches('/')))
        .send_json(json!({"model": model, "temperature": 0, "messages": [
            {"role": "system", "content": "Answer briefly. If the information is not available, say you don't know."},
            {"role": "user", "content": prompt}]}))?
        .into_json()?;
    Ok(body.pointer("/choices/0/message/content").and_then(Value::as_str).unwrap_or("").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_longmemeval_shape() {
        let fixture = r#"[{"question_id":"q1_abs","question_type":"single-session-user","question":"Where did I park?",
            "answer":"Level 3","question_date":"2023/05/30 (Tue) 10:00","haystack_session_ids":["s1","s2"],
            "haystack_dates":["2023/05/01","2023/05/02"],"haystack_sessions":[[{"role":"user","content":"I parked on level 3.","has_answer":true}],
            [{"role":"assistant","content":"Hello"}]],"answer_session_ids":["s1"]}]"#;
        let items: Vec<LmeItem> = serde_json::from_str(fixture).unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].is_abstention());
        assert_eq!(items[0].answer_text(), "Level 3");
        assert!(session_text(&items[0], 0).contains("user: I parked on level 3."));
        assert_eq!(normalize("Level-3!"), "level3");
    }
}
