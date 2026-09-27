//! `ferricula-bench gates|docs|longmem [--limit N] [--seed S] [--out audit/bench]`
//!
//! Every run appends JSON-lines rows to `<out>/ledger.jsonl` and writes a
//! markdown report `<out>/<suite>-<date>.md`.

mod docs;
mod gates;
mod ledger;
mod longmem;
mod metrics;

use std::path::PathBuf;

use anyhow::{Result, bail};

const USAGE: &str = "usage: ferricula-bench gates|docs|longmem [--limit N] [--seed S] [--out DIR]
  gates:   [--datasets research/gates/datasets] [--url http://127.0.0.1:11435] [--model laya] [--timeout-ms 30000] [--only SUBSTR]
  docs:    [--corpus research] (--limit = sampled sentences, default 200; env FERRICULA_BENCH_PDFS, BENCH_PARAPHRASE_MODEL, OLLAMA_URL)
  longmem: [--dataset data/longmemeval/longmemeval_s.json] [--full-server] (env LONGMEMEVAL_PATH, BENCH_ANSWER_MODEL, OLLAMA_URL, FERRICULA_URL)";

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(suite) = args.first().cloned() else { bail!("{USAGE}") };
    let mut flags = std::collections::HashMap::new();
    let mut switches = std::collections::HashSet::new();
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        let Some(name) = a.strip_prefix("--") else { bail!("unexpected argument {a}\n{USAGE}") };
        if matches!(name, "full-server") {
            switches.insert(name.to_string());
            i += 1;
        } else {
            let Some(v) = args.get(i + 1) else { bail!("--{name} needs a value") };
            flags.insert(name.to_string(), v.clone());
            i += 2;
        }
    }
    let get = |k: &str| flags.get(k).cloned();
    let limit: Option<usize> = get("limit").map(|v| v.parse()).transpose()?;
    let seed: u64 = get("seed").map(|v| v.parse()).transpose()?.unwrap_or(42);
    let out = PathBuf::from(get("out").unwrap_or_else(|| "audit/bench".into()));
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
    let ollama = env("OLLAMA_URL").unwrap_or_else(|| "http://127.0.0.1:11434/v1".into());

    match suite.as_str() {
        "gates" => {
            let ctx = ledger::RunContext::new("gates", &out, seed);
            gates::run(&ctx, &gates::GatesArgs {
                datasets_dir: PathBuf::from(get("datasets").unwrap_or_else(|| "research/gates/datasets".into())),
                url: get("url").or_else(|| env("OLLAYA_URL")).unwrap_or_else(|| "http://127.0.0.1:11435".into()),
                model: get("model").unwrap_or_else(|| "laya".into()),
                timeout_ms: get("timeout-ms").map(|v| v.parse()).transpose()?.unwrap_or(30_000),
                limit,
                only: get("only"),
            })
        }
        "docs" => {
            let ctx = ledger::RunContext::new("docs", &out, seed);
            docs::run(&ctx, &docs::DocsArgs {
                corpus_dir: PathBuf::from(get("corpus").unwrap_or_else(|| "research".into())),
                pdf_dir: env("FERRICULA_BENCH_PDFS").map(PathBuf::from),
                n: limit.unwrap_or(200),
                drop_frac: 0.4,
                paraphrase: env("BENCH_PARAPHRASE_MODEL").map(|m| (ollama.clone(), m)),
            })
        }
        "longmem" => {
            let ctx = ledger::RunContext::new("longmem", &out, seed);
            longmem::run(&ctx, &longmem::LongmemArgs {
                dataset: PathBuf::from(get("dataset").or_else(|| env("LONGMEMEVAL_PATH")).unwrap_or_else(|| longmem::DEFAULT_PATH.into())),
                limit,
                full_server: switches.contains("full-server"),
                answer: env("BENCH_ANSWER_MODEL").map(|m| (ollama.clone(), m)),
                server_url: env("FERRICULA_URL"),
            })
        }
        _ => bail!("unknown suite {suite}\n{USAGE}"),
    }
}
