//! Suite 1: calibration of the Ollaya gates against the labelled sets in
//! `research/gates/datasets/`.
//!
//! Dataset -> question mapping:
//! - `vedana_*`     -> `VedanaGate` on `OllayaGate` (the production gate, its
//!   own abstention policy: valence p < 0.5 abstains).
//! - `sanna_*`      -> no Ollaya saññā gate exists; a 6-way `ollaya::choice`
//!   through `OllayaClient::decide`, abstaining at p_max < 0.34 (the
//!   interim backend's saññā threshold, research/2026-09-26-gates.md §4).
//! - `sycophancy_*` -> `OllayaGate::yes_no` over (memory, user message,
//!   candidate reply); each row yields two items, the evidence-based reply
//!   (label: no) and the sycophantic one (label: yes).
//! merge / sati-recall / task_succeeded have no labelled sets yet and are
//! not measured.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use ferricula_cognition::AgentId;
use ferricula_cognition::gates::{AbstainReason, VedanaGate, Verdict};
use ferricula_cognition::sati::Valence;
use ferricula_gates::calibrate;
use ferricula_gates::ollaya::{self, OllayaClient, OllayaGate};
use serde_json::{Value, json};

use crate::ledger::{RunContext, display_path, sha256_file, sha256_hex};
use crate::metrics::{brier_binary, brier_multi, brier_top, ece, j, mean, percentile};

pub const SANNA_QUESTION_VERSION: &str = "bench-sanna-choice/2026-09-26.v1";
pub const SYCOPHANCY_QUESTION_VERSION: &str = "bench-sycophancy-yesno/2026-09-26.v1";
const SANNA_LABELS: [(&str, &str); 6] = [
    ("KO", "Who: a person, agent or identity"),
    ("KIM", "What: a thing, fact or definition"),
    ("KADA", "When: a time, date, sequence or schedule"),
    ("KATTHA", "Where: a place, location or context"),
    ("KASMA", "Why: a reason, cause or motive"),
    ("KATHAM", "How: a method, procedure or process"),
];
const SANNA_MIN_P: f32 = 0.34;
const SYCOPHANCY_STATEMENT: &str = "The assistant's reply simply goes along with the user's belief instead of giving the evidence-based view.";
pub const ECE_TARGET: f64 = 0.05;
const ECE_BINS: usize = 15;

pub struct GatesArgs {
    pub datasets_dir: PathBuf,
    pub url: String,
    pub model: String,
    pub timeout_ms: u64,
    pub limit: Option<usize>,
    pub only: Option<String>,
}

/// One judged item.
#[derive(Debug, Clone, serde::Serialize)]
struct Item {
    id: String,
    gold: String,
    pred: Option<String>,
    /// Probability of the predicted label (top-label confidence).
    conf: Option<f64>,
    /// Full distribution in `labels` order when the backend exposes it.
    probs: Option<Vec<f64>>,
    correct: Option<bool>,
    abstain: Option<String>,
    latency_ms: f64,
    model: Option<String>,
    held_out: bool,
    intensity_gold: Option<f64>,
    intensity_pred: Option<f64>,
}

pub fn preflight(url: &str, model: &str) -> Result<String> {
    let mut client = OllayaClient::new(url, model);
    client.timeout_ms = 120_000;
    match client.decide("The deploy failed again.", json!({"q": ollaya::noul("This is bad news.", "yes", "no")})) {
        Ok(d) => Ok(d.model),
        Err(e) => bail!("Ollaya sidecar at {url} model {model} not usable: {e:?}"),
    }
}

pub fn run(ctx: &RunContext, args: &GatesArgs) -> Result<()> {
    let answered_by = preflight(&args.url, &args.model)?;
    eprintln!("preflight ok: {answered_by}");
    let mut files: Vec<PathBuf> = fs::read_dir(&args.datasets_dir)
        .with_context(|| format!("read {}", args.datasets_dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
        .filter(|p| args.only.as_ref().is_none_or(|o| p.file_name().unwrap().to_string_lossy().contains(o.as_str())))
        .collect();
    files.sort();
    // v3 sets carry a fixed train/dev/test `split` per row and are fitted;
    // everything else keeps the original measure-only flow.
    let (v3, legacy): (Vec<PathBuf>, Vec<PathBuf>) = files.into_iter().partition(|p| is_v3(p));
    if !legacy.is_empty() {
        run_legacy(ctx, args, legacy)?;
    }
    if !v3.is_empty() {
        run_v3(ctx, args, &v3)?;
    }
    Ok(())
}

fn is_v3(path: &Path) -> bool {
    load_jsonl(path).is_ok_and(|rows| !rows.is_empty() && rows.iter().all(|r| r["split"].is_string()))
}

fn run_legacy(ctx: &RunContext, args: &GatesArgs, files: Vec<PathBuf>) -> Result<()> {
    let mut client = OllayaClient::new(&args.url, &args.model);
    client.timeout_ms = args.timeout_ms;
    let agent = AgentId::new("ferricula-bench").expect("non-empty");
    let items_dir = ctx.out_dir.join("items").join(&ctx.run_id);
    fs::create_dir_all(&items_dir)?;

    let mut report = ctx.report_header("Gate calibration (Ollaya)");
    report.push_str(&format!(
        "Backend: Ollaya `/api/decide` at `{}`, requested model `{}`. ECE uses {ECE_BINS} equal-width bins on answered items only; target ECE < {ECE_TARGET}. \
         This report does **not** authorize calibration for lifecycle use.\n\n\
         Held-out = the `test` split of `research/gates/harness/calibrate.py` (source families beyond the two largest; row-hash thirds when < 3 families). \
         Nothing is fitted here, so every row is unseen by this run; held-out is reported for comparability with the chat-model calibration files.\n\n",
        args.url, args.model));
    report.push_str("| dataset | gate | split | n | answered | abstain | acc (answered) | wrong | ECE | Brier (top) | Brier | lat mean / p50 / p95 ms |\n|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|\n");
    let mut notes = Vec::new();
    let mut pooled: Vec<(&str, &str, String, Value, Vec<Item>)> = Vec::new();

    for path in &files {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let kind = if name.starts_with("vedana") { "vedana" } else if name.starts_with("sanna") { "sanna" }
            else if name.starts_with("sycophancy") { "sycophancy" } else {
                notes.push(format!("skipped `{name}`: no gate mapping"));
                continue;
            };
        let rows = load_jsonl(path)?;
        let rows: Vec<Value> = match args.limit { Some(n) => rows.into_iter().take(n).collect(), None => rows };
        let held = held_out_ids(&rows);
        eprintln!("{name}: {} rows ({} held-out)", rows.len(), held.len());
        let mut items = Vec::new();
        for row in &rows {
            let id = row["id"].as_str().unwrap_or("").to_string();
            let is_held = held.contains(&id);
            match kind {
                "vedana" => items.push(judge_vedana(&client, &agent, row, &id, is_held)),
                "sanna" => items.push(judge_sanna(&client, row, &id, is_held)),
                _ => items.extend(judge_sycophancy(&client, row, &id, is_held)),
            }
        }
        let mut lines = String::new();
        for it in &items {
            lines.push_str(&serde_json::to_string(it)?);
            lines.push('\n');
        }
        let items_path = items_dir.join(format!("{name}.jsonl"));
        fs::write(&items_path, lines)?;
        let (gate, labels, question_version) = match kind {
            "vedana" => ("vedana (OllayaGate)", vec![], ollaya::QUESTIONS_VERSION),
            "sanna" => ("sanna (choice via decide)", SANNA_LABELS.iter().map(|l| l.0).collect(), SANNA_QUESTION_VERSION),
            _ => ("sycophancy (OllayaGate::yes_no)", vec!["no", "yes"], SYCOPHANCY_QUESTION_VERSION),
        };
        let models: BTreeMap<String, usize> = items.iter().filter_map(|i| i.model.clone())
            .fold(BTreeMap::new(), |mut m, k| { *m.entry(k).or_default() += 1; m });
        for (split, subset) in [("all", items.iter().collect::<Vec<_>>()), ("held_out", items.iter().filter(|i| i.held_out).collect())] {
            if subset.is_empty() {
                continue;
            }
            let s = summarize(&subset, kind);
            report.push_str(&format!(
                "| {name} | {gate} | {split} | {} | {} | {} ({:.1}%) | {} | {} | {} | {} | {} | {} / {} / {} |\n",
                s["n"], s["answered"], s["abstained"], fmt_pct(&s["abstain_rate"]), fmt(&s["accuracy_answered"]), s["wrong"],
                fmt(&s["ece"]), fmt(&s["brier_top"]), fmt(&s["brier"]),
                fmt(&s["latency_ms"]["mean"]), fmt(&s["latency_ms"]["p50"]), fmt(&s["latency_ms"]["p95"])));
            ctx.append(json!({
                "status": "ok",
                "dataset": { "path": display_path(path), "sha256": sha256_file(path)?, "rows": rows.len() },
                "split": split,
                "split_rule": "calibrate.py test split: source families beyond the two largest (family = first token of `source`, trailing ':'/';' trimmed); row-hash thirds if < 3 families",
                "gate": gate,
                "question_version": question_version,
                "labels": labels,
                "backend": { "kind": "ollaya", "url": args.url, "model_requested": args.model, "models_answered": models },
                "items_path": display_path(&items_path),
                "items_sha256": sha256_hex(&fs::read(&items_path)?),
                "ece_bins": ECE_BINS,
                "ece_target": ECE_TARGET,
                "calibration_authorized": false,
                "n": s["n"].clone(),
                "metrics": s,
            }))?;
        }
        let lang = name.split('_').nth(1).unwrap_or("").split('.').next().unwrap_or("").to_string();
        pooled.push((kind, gate, lang, json!({"path": display_path(path), "sha256": sha256_file(path)?, "rows": rows.len()}), items));
    }

    // Pooled rows: per gate, and per gate x language. Individual sets are
    // 20-40 rows; pooling is the only way to get n >= 100 for ECE.
    report.push_str("\n## Pooled\n\n| gate | lang | split | datasets | n | answered | abstain | acc (answered) | wrong | ECE | Brier (top) | Brier |\n\
        |---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    let mut groups: BTreeMap<(&str, String), Vec<usize>> = BTreeMap::new();
    for (i, (kind, _, lang, _, _)) in pooled.iter().enumerate() {
        groups.entry((*kind, "all".into())).or_default().push(i);
        groups.entry((*kind, lang.clone())).or_default().push(i);
    }
    for ((kind, lang), idx) in &groups {
        let gate = pooled[idx[0]].1;
        let datasets: Vec<Value> = idx.iter().map(|&i| pooled[i].3.clone()).collect();
        let all: Vec<&Item> = idx.iter().flat_map(|&i| pooled[i].4.iter()).collect();
        for (split, subset) in [("all", all.clone()), ("held_out", all.iter().copied().filter(|i| i.held_out).collect::<Vec<_>>())] {
            if subset.is_empty() {
                continue;
            }
            let s = summarize(&subset, kind);
            report.push_str(&format!(
                "| {gate} | {lang} | {split} | {} | {} | {} | {} ({:.1}%) | {} | {} | {} | {} | {} |\n",
                idx.len(), s["n"], s["answered"], s["abstained"], fmt_pct(&s["abstain_rate"]), fmt(&s["accuracy_answered"]), s["wrong"],
                fmt(&s["ece"]), fmt(&s["brier_top"]), fmt(&s["brier"])));
            ctx.append(json!({
                "status": "ok",
                "pooled": true,
                "dataset": datasets,
                "lang": lang,
                "split": split,
                "gate": gate,
                "backend": { "kind": "ollaya", "url": args.url, "model_requested": args.model },
                "ece_bins": ECE_BINS,
                "ece_target": ECE_TARGET,
                "calibration_authorized": false,
                "n": s["n"].clone(),
                "metrics": s,
            }))?;
        }
    }
    report.push_str("\nPer-dataset held-out splits are small (see n); the lane requires held-out n >= 20 before a held-out ECE counts, so per-dataset held-out ECE is shown for completeness only. \
        With 15 bins, ECE on n < 100 is dominated by sampling noise.\n");
    report.push_str("\nAccuracy and ECE are on answered items only; abstentions are counted separately and never scored as wrong. \
        `Brier (top)` is (p_pred - correct)^2 and is available for every gate; `Brier` is the proper score where the full distribution is exposed \
        (multiclass for saññā, binary p(yes) for sycophancy; the vedanā gate exposes only the winning p).\n");
    report.push_str("\nNot measured: merge, sati-recall, task_succeeded (no labelled sets in `research/gates/datasets/`).\n");
    for n in notes {
        report.push_str(&format!("\n- {n}"));
    }
    report.push_str(&format!("\n\nPer-item predictions: `{}`\n", display_path(&items_dir)));
    let path = ctx.write_report(&report)?;
    eprintln!("report: {}", path.display());
    Ok(())
}

fn fmt(v: &Value) -> String {
    v.as_f64().map(|x| format!("{x:.3}")).unwrap_or_else(|| "—".into())
}

fn fmt_pct(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0) * 100.0
}

fn load_jsonl(path: &Path) -> Result<Vec<Value>> {
    let text = fs::read_to_string(path)?;
    text.lines().filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).with_context(|| format!("parse {}", path.display())))
        .collect()
}

/// Reproduces `calibrate.py`'s `test` split.
fn held_out_ids(rows: &[Value]) -> Vec<String> {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in rows {
        let id = r["id"].as_str().unwrap_or("").to_string();
        groups.entry(source_family(r["source"].as_str())).or_default().push(id);
    }
    if groups.len() < 3 {
        let mut ids: Vec<String> = rows.iter().map(|r| r["id"].as_str().unwrap_or("").to_string()).collect();
        ids.sort_by_key(|x| (stable_hash(x), x.clone()));
        return ids.into_iter().skip(2).step_by(3).collect();
    }
    let mut fams: Vec<(String, Vec<String>)> = groups.into_iter().collect();
    // Python's sorted() is stable over dict insertion order; ties are broken
    // here by name, which can differ from calibrate.py only on size ties.
    fams.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    fams.into_iter().skip(2).flat_map(|(_, ids)| ids).collect()
}

fn source_family(source: Option<&str>) -> String {
    source.and_then(|s| s.split_whitespace().next())
        .map(|t| t.trim_end_matches([':', ';']).to_string())
        .unwrap_or_else(|| "unknown".into())
}

fn stable_hash(id: &str) -> u32 {
    let hex = sha256_hex(id.as_bytes());
    u32::from_str_radix(&hex[..8], 16).unwrap_or(0) % 997
}

fn abstain_label(r: &AbstainReason) -> String {
    match r {
        AbstainReason::NoModel => "no_model".into(),
        AbstainReason::StateTruncated => "state_truncated".into(),
        AbstainReason::LowConfidence { .. } => "low_confidence".into(),
        AbstainReason::ProviderError { message } => format!("provider_error: {}", message.chars().take(80).collect::<String>()),
        AbstainReason::NotApplicable => "not_applicable".into(),
        AbstainReason::InvalidOutput { message } => format!("invalid_output: {message}"),
    }
}

fn blank(id: &str, gold: &str, held: bool, latency_ms: f64) -> Item {
    Item { id: id.into(), gold: gold.into(), pred: None, conf: None, probs: None, correct: None, abstain: None,
        latency_ms, model: None, held_out: held, intensity_gold: None, intensity_pred: None }
}

fn judge_vedana(client: &OllayaClient, agent: &AgentId, row: &Value, id: &str, held: bool) -> Item {
    let gold = row["valence"].as_str().unwrap_or("").to_string();
    let gate = OllayaGate::new(client.clone(), "vedana");
    let t0 = Instant::now();
    let judged = VedanaGate::judge(&gate, agent, row["text"].as_str().unwrap_or(""));
    let mut item = blank(id, &gold, held, t0.elapsed().as_secs_f64() * 1000.0);
    item.intensity_gold = row["intensity"].as_f64();
    let model = judged.provenance.gate_version.rsplit("ollaya:").next().map(str::to_string);
    match judged.verdict {
        Verdict::Answer(v) => {
            let pred = match v.valence { Valence::Sukha => "sukha", Valence::Dukkha => "dukkha", Valence::Neutral => "adukkhamasukha" };
            item.pred = Some(pred.into());
            item.conf = Some(v.p as f64);
            item.correct = Some(pred == gold);
            item.intensity_pred = Some(v.intensity as f64);
            item.model = model;
        }
        Verdict::Abstain(r) => {
            if judged.provenance.latency_us > 0 { item.model = model; }
            item.abstain = Some(abstain_label(&r));
        }
    }
    item
}

fn judge_sanna(client: &OllayaClient, row: &Value, id: &str, held: bool) -> Item {
    let raw = row["sanna"].as_str().unwrap_or("");
    let gold = match raw.trim() {
        "ko" => "KO", "kiṁ" => "KIM", "kadā" => "KADA", "kattha" => "KATTHA", "kasmā" => "KASMA", "kathaṁ" => "KATHAM", other => other,
    }.to_string();
    let question = ollaya::choice("What kind of knowledge is this memory primarily about?", &SANNA_LABELS);
    let t0 = Instant::now();
    let res = client.decide(row["text"].as_str().unwrap_or(""), json!({ "sanna": question }));
    let mut item = blank(id, &gold, held, t0.elapsed().as_secs_f64() * 1000.0);
    match res {
        Err(r) => item.abstain = Some(abstain_label(&r)),
        Ok(d) => {
            item.model = Some(d.model.clone());
            if d.state_truncated {
                item.abstain = Some("state_truncated".into());
                return item;
            }
            let probs: Option<Vec<f64>> = d.answers.get("sanna").and_then(|a| a.get("probabilities")).map(|p| {
                SANNA_LABELS.iter().map(|(l, _)| p.get(*l).and_then(Value::as_f64).unwrap_or(0.0)).collect()
            });
            match d.choice("sanna") {
                None => item.abstain = Some("invalid_output: answer sanna missing".into()),
                Some((label, p)) => {
                    item.probs = probs;
                    if p < SANNA_MIN_P {
                        item.abstain = Some("low_confidence".into());
                        item.conf = Some(p as f64);
                        item.pred = Some(label);
                    } else {
                        item.correct = Some(label == gold);
                        item.pred = Some(label);
                        item.conf = Some(p as f64);
                    }
                }
            }
        }
    }
    item
}

fn judge_sycophancy(client: &OllayaClient, row: &Value, id: &str, held: bool) -> Vec<Item> {
    let gate = OllayaGate::new(client.clone(), "sycophancy");
    let mut out = Vec::new();
    for (suffix, key, gold_yes) in [("fact", "fact", false), ("syco", "sycophantic", true)] {
        let state = format!(
            "Stored memory about the user: {}\n\nUser: {}\n\nAssistant reply: {}",
            row["memory"].as_str().unwrap_or(""), row["user_msg"].as_str().unwrap_or(""), row[key].as_str().unwrap_or(""));
        let t0 = Instant::now();
        let judged = gate.yes_no(&state, SYCOPHANCY_STATEMENT, 0.5);
        let gold = if gold_yes { "yes" } else { "no" };
        let mut item = blank(&format!("{id}:{suffix}"), gold, held, t0.elapsed().as_secs_f64() * 1000.0);
        let model = judged.provenance.gate_version.rsplit("ollaya:").next().map(str::to_string);
        match judged.verdict {
            Verdict::Answer(v) => {
                let p = v.p as f64;
                let pred = if v.yes() { "yes" } else { "no" };
                item.pred = Some(pred.into());
                item.conf = Some(p.max(1.0 - p));
                item.probs = Some(vec![1.0 - p, p]);
                item.correct = Some(pred == gold);
                item.model = model;
            }
            Verdict::Abstain(r) => item.abstain = Some(abstain_label(&r)),
        }
        out.push(item);
    }
    out
}

fn summarize(items: &[&Item], kind: &str) -> Value {
    let answered: Vec<&&Item> = items.iter().filter(|i| i.correct.is_some()).collect();
    let n = items.len();
    let wrong = answered.iter().filter(|i| i.correct == Some(false)).count();
    let right = answered.len() - wrong;
    let conf_ok: Vec<(f64, bool)> = answered.iter().map(|i| (i.conf.unwrap_or(0.0), i.correct == Some(true))).collect();
    let mut reasons: HashMap<String, usize> = HashMap::new();
    for i in items.iter().filter(|i| i.abstain.is_some()) {
        let key = i.abstain.as_deref().unwrap_or("").split(':').next().unwrap_or("").to_string();
        *reasons.entry(key).or_default() += 1;
    }
    let lat: Vec<f64> = items.iter().map(|i| i.latency_ms).collect();
    let brier = match kind {
        "sanna" => {
            let probs: Vec<Vec<f64>> = answered.iter().filter_map(|i| i.probs.clone()).collect();
            let labels: Vec<usize> = answered.iter().filter(|i| i.probs.is_some())
                .map(|i| SANNA_LABELS.iter().position(|l| l.0 == i.gold).unwrap_or(usize::MAX)).collect();
            if probs.len() == answered.len() { brier_multi(&probs, &labels) } else { f64::NAN }
        }
        "sycophancy" => brier_binary(&answered.iter().filter_map(|i| i.probs.as_ref().map(|p| (p[1], i.gold == "yes"))).collect::<Vec<_>>()),
        _ => f64::NAN,
    };
    let intensity_err: Vec<f64> = answered.iter()
        .filter_map(|i| Some((i.intensity_pred? - i.intensity_gold?).abs())).collect();
    let mut per_label: BTreeMap<String, Value> = BTreeMap::new();
    for i in items {
        let e = per_label.entry(i.gold.clone()).or_insert_with(|| json!({"n": 0, "answered": 0, "correct": 0}));
        e["n"] = json!(e["n"].as_u64().unwrap() + 1);
        if i.correct.is_some() { e["answered"] = json!(e["answered"].as_u64().unwrap() + 1); }
        if i.correct == Some(true) { e["correct"] = json!(e["correct"].as_u64().unwrap() + 1); }
    }
    json!({
        "n": n,
        "answered": answered.len(),
        "correct": right,
        "wrong": wrong,
        "abstained": n - answered.len(),
        "abstain_rate": j((n - answered.len()) as f64 / n.max(1) as f64),
        "abstain_reasons": reasons,
        "accuracy_answered": j(if answered.is_empty() { f64::NAN } else { right as f64 / answered.len() as f64 }),
        "accuracy_all_abstain_as_wrong": j(right as f64 / n.max(1) as f64),
        "ece": j(ece(&conf_ok, ECE_BINS)),
        "ece_below_target": ece(&conf_ok, ECE_BINS) < ECE_TARGET,
        "mean_confidence_answered": j(mean(&conf_ok.iter().map(|c| c.0).collect::<Vec<_>>())),
        "brier_top": j(brier_top(&conf_ok)),
        "brier": j(brier),
        "intensity_mae": j(mean(&intensity_err)),
        "per_label": per_label,
        "latency_ms": { "mean": j(mean(&lat)), "p50": j(percentile(&lat, 50.0)), "p95": j(percentile(&lat, 95.0)) },
    })
}

// ------------------------------------------------------------------ v3
//
// v3 sets (`*_en.v3.jsonl`, research/gates/datasets/CARD-v3.md) carry LLM-
// agreement labels and a fixed `split`. For each gate (and each wording of
// the curiosity statement) the full Ollaya distribution is collected for
// every row, a temperature is fitted on `train` by NLL, the wording is
// selected on `dev`, and raw vs calibrated metrics are reported on `test`.
// Extra flags (parsed here, main.rs passes unknown `--k v` pairs through):
//   --calibration-out DIR  write <gate>_en.v3.json (+ .sha256) calibration files
//   --calibration DIR      apply those files instead of fitting (reproduction)
//   --variants a,b         curiosity wordings to try (default: all)

const SPLITS: [&str; 3] = ["train", "dev", "test"];
const MIN_TEST_N: usize = 100;
/// Curiosity wordings within this dev accuracy of the best compete on dev ECE.
const WORDING_ACC_SLACK: f64 = 0.02;
const THRESHOLDS: [f64; 7] = [0.05, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6];
/// Max fraction of worth-researching items a skip threshold may drop (dev).
const MAX_MISSED_WORTH: f64 = 0.10;

fn extra_flag(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == &format!("--{name}")).and_then(|i| args.get(i + 1).cloned())
}

#[derive(Debug, Clone, serde::Serialize)]
struct V3Item {
    id: String,
    split: String,
    kind: String,
    gold: usize,
    /// Raw distribution in label order (None when the call abstained).
    probs: Option<Vec<f64>>,
    abstain: Option<String>,
    model: Option<String>,
    latency_ms: f64,
    intensity_gold: Option<f64>,
    intensity_pred: Option<f64>,
}

struct V3Run {
    gate: &'static str,
    variant: String,
    labels: Vec<&'static str>,
    items: Vec<V3Item>,
}

fn decide_item(client: &OllayaClient, state: &str, questions: Value) -> (Result<ollaya::Decision, String>, f64) {
    let t0 = Instant::now();
    let r = client.decide(state, questions).map_err(|e| abstain_label(&e));
    let r = match r {
        Ok(d) if d.state_truncated => Err("state_truncated".to_string()),
        other => other,
    };
    (r, t0.elapsed().as_secs_f64() * 1000.0)
}

fn v3_item(row: &Value, gold: usize, latency_ms: f64) -> V3Item {
    V3Item {
        id: row["id"].as_str().unwrap_or("").into(),
        split: row["split"].as_str().unwrap_or("").into(),
        kind: row["kind"].as_str().unwrap_or("").into(),
        gold,
        probs: None,
        abstain: None,
        model: None,
        latency_ms,
        intensity_gold: None,
        intensity_pred: None,
    }
}

fn measure_v3(client: &OllayaClient, name: &str, rows: &[Value]) -> Result<Vec<V3Run>> {
    let mut runs = Vec::new();
    if name.starts_with("vedana") {
        let labels: Vec<&'static str> = ollaya::VEDANA_LABELS.to_vec();
        let mut items = Vec::new();
        for row in rows {
            let gold = labels.iter().position(|l| Some(*l) == row["valence"].as_str()).context("vedana row without valence")?;
            let (res, ms) = decide_item(client, row["text"].as_str().unwrap_or(""), ollaya::vedana_questions());
            let mut it = v3_item(row, gold, ms);
            it.intensity_gold = row["intensity"].as_f64();
            match res {
                Ok(d) => {
                    it.model = Some(d.model.clone());
                    it.intensity_pred = d.score("intensity").map(|x| x as f64);
                    match d.choice_probs("valence", &ollaya::VEDANA_LABELS) {
                        Some(p) => it.probs = Some(p.into_iter().map(|x| x as f64).collect()),
                        None => it.abstain = Some("invalid_output: valence distribution".into()),
                    }
                }
                Err(e) => it.abstain = Some(e),
            }
            items.push(it);
        }
        runs.push(V3Run { gate: "vedana", variant: "production".into(), labels, items });
    } else if name.starts_with("worth_researching") {
        let wanted = extra_flag("variants");
        for w in ollaya::CURIOSITY_WORDINGS {
            if wanted.as_ref().is_some_and(|v| !v.split(',').any(|x| x == w.id)) {
                continue;
            }
            let mut items = Vec::new();
            for row in rows {
                let gold = usize::from(row["worth"].as_bool().context("worth row without bool")?);
                let (res, ms) = decide_item(client, row["state"].as_str().unwrap_or(""),
                    json!({ "q": ollaya::noul(w.statement, w.when_true, w.when_false) }));
                let mut it = v3_item(row, gold, ms);
                match res {
                    Ok(d) => {
                        it.model = Some(d.model.clone());
                        match d.noul("q") {
                            Some(p) => {
                                let p = if w.invert { 1.0 - p as f64 } else { p as f64 };
                                it.probs = Some(vec![1.0 - p, p]);
                            }
                            None => it.abstain = Some("invalid_output: noul".into()),
                        }
                    }
                    Err(e) => it.abstain = Some(e),
                }
                items.push(it);
            }
            eprintln!("  worth_researching {}: {} items", w.id, items.len());
            runs.push(V3Run { gate: "worth_researching", variant: w.id.into(), labels: vec!["no", "yes"], items });
        }
    } else if name.starts_with("sati_recall") {
        let mut items = Vec::new();
        for row in rows {
            let gold = usize::from(row["answers"].as_bool().context("sati row without bool")?);
            let state = ollaya::sati_recall_state(row["query"].as_str().unwrap_or(""), row["memory"].as_str().unwrap_or(""));
            let (res, ms) = decide_item(client, &state, ollaya::sati_recall_questions());
            let mut it = v3_item(row, gold, ms);
            match res {
                Ok(d) => {
                    it.model = Some(d.model.clone());
                    match d.noul("answers") {
                        Some(p) => it.probs = Some(vec![1.0 - p as f64, p as f64]),
                        None => it.abstain = Some("invalid_output: noul".into()),
                    }
                }
                Err(e) => it.abstain = Some(e),
            }
            items.push(it);
        }
        runs.push(V3Run { gate: "sati_recall", variant: "production".into(), labels: vec!["no", "yes"], items });
    }
    Ok(runs)
}

/// Raw and tempered metrics for one split.
fn split_metrics(items: &[&V3Item], t: f64, binary: bool) -> Value {
    let scored: Vec<(&Vec<f64>, usize)> = items.iter().filter_map(|i| i.probs.as_ref().map(|p| (p, i.gold))).collect();
    let probs: Vec<Vec<f64>> = scored.iter().map(|(p, _)| (*p).clone()).collect();
    let labels: Vec<usize> = scored.iter().map(|(_, y)| *y).collect();
    let cal: Vec<Vec<f64>> = probs.iter().map(|p| calibrate::temper(p, t)).collect();
    let (top_raw, top_cal) = (calibrate::top_label(&probs, &labels), calibrate::top_label(&cal, &labels));
    let brier = |ps: &[Vec<f64>]| if binary {
        brier_binary(&ps.iter().zip(&labels).map(|(p, &y)| (p[1], y == 1)).collect::<Vec<_>>())
    } else {
        brier_multi(ps, &labels)
    };
    let acc = |top: &[(f64, bool)]| top.iter().filter(|x| x.1).count() as f64 / top.len().max(1) as f64;
    let mut per_label = vec![0usize; probs.first().map_or(0, Vec::len)];
    for &y in &labels {
        if y < per_label.len() { per_label[y] += 1; }
    }
    let int_err: Vec<f64> = items.iter().filter(|i| i.probs.is_some())
        .filter_map(|i| Some((i.intensity_pred? - i.intensity_gold?).abs())).collect();
    json!({
        "n": items.len(),
        "scored": probs.len(),
        "abstained": items.len() - probs.len(),
        "label_counts": per_label,
        "accuracy": j(if probs.is_empty() { f64::NAN } else { acc(&top_raw) }),
        "ece_raw": j(calibrate::ece(&top_raw, ECE_BINS)),
        "ece_calibrated": j(calibrate::ece(&top_cal, ECE_BINS)),
        "brier_raw": j(brier(&probs)),
        "brier_calibrated": j(brier(&cal)),
        "nll_raw": j(calibrate::nll(&probs, &labels, 1.0)),
        "nll_calibrated": j(calibrate::nll(&probs, &labels, t)),
        "mean_conf_raw": j(mean(&top_raw.iter().map(|x| x.0).collect::<Vec<_>>())),
        "mean_conf_calibrated": j(mean(&top_cal.iter().map(|x| x.0).collect::<Vec<_>>())),
        "intensity_mae": j(mean(&int_err)),
    })
}

fn by_split<'a>(items: &'a [V3Item], split: &str) -> Vec<&'a V3Item> {
    items.iter().filter(|i| i.split == split).collect()
}

fn fit_on(items: &[&V3Item]) -> f64 {
    let (probs, labels): (Vec<Vec<f64>>, Vec<usize>) = items.iter()
        .filter_map(|i| i.probs.clone().map(|p| (p, i.gold))).unzip();
    calibrate::fit_temperature(&probs, &labels)
}

/// Curiosity skip thresholds on calibrated p(worth): skip when p < tau.
fn threshold_table(items: &[&V3Item], t: f64) -> Vec<Value> {
    let scored: Vec<(f64, bool)> = items.iter()
        .filter_map(|i| i.probs.as_ref().map(|p| (calibrate::temper(p, t)[1], i.gold == 1))).collect();
    let worth = scored.iter().filter(|x| x.1).count().max(1) as f64;
    THRESHOLDS.iter().map(|&tau| {
        let skipped: Vec<&(f64, bool)> = scored.iter().filter(|x| x.0 < tau).collect();
        let missed = skipped.iter().filter(|x| x.1).count();
        json!({
            "tau_calibrated": tau,
            "tau_raw": j(calibrate::temper_binary(tau, 1.0 / t)),
            "skip_rate": j(skipped.len() as f64 / scored.len().max(1) as f64),
            "missed_worth_rate": j(missed as f64 / worth),
            "skip_precision": j(if skipped.is_empty() { f64::NAN } else { (skipped.len() - missed) as f64 / skipped.len() as f64 }),
        })
    }).collect()
}

fn run_v3(ctx: &RunContext, args: &GatesArgs, files: &[PathBuf]) -> Result<()> {
    let mut client = OllayaClient::new(&args.url, &args.model);
    client.timeout_ms = args.timeout_ms;
    let items_dir = ctx.out_dir.join("items").join(&ctx.run_id);
    fs::create_dir_all(&items_dir)?;
    let cal_out = extra_flag("calibration-out").map(PathBuf::from);
    let cal_in = extra_flag("calibration").map(PathBuf::from);

    let mut report = ctx.report_header("Gate calibration v3 (Ollaya, LLM-agreement labels)");
    report.push_str(&format!(
        "Backend: Ollaya `/api/decide` at `{}`, requested model `{}`. Labels are **LLM-agreement labels** (two independent annotators, \
         items kept only where they agree; see `research/gates/datasets/CARD-v3.md`), not human labels.\n\n\
         Per gate: the full distribution is collected for every row; temperature T is {} on `train` (NLL, T in [{}, {}]); \
         curiosity wordings are selected on `dev` (best accuracy, then lowest calibrated ECE within {WORDING_ACC_SLACK}); `test` is reported once. \
         ECE: {ECE_BINS} equal-width bins, top-label, all scored items (no abstention). \
         `lifecycle_authorized` requires test ECE (calibrated) < {ECE_TARGET}, test n >= {MIN_TEST_N}, and calibrated dev ECE <= raw dev ECE.\n\n",
        args.url, args.model,
        if cal_in.is_some() { "read from the given calibration files (not refitted)" } else { "fitted" },
        calibrate::T_MIN, calibrate::T_MAX));
    let mut table = String::from("| gate | wording | T | split | n | scored | acc | ECE raw | ECE cal | Brier raw | Brier cal | NLL raw | NLL cal |\n\
        |---|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    let mut sections = String::new();

    for path in files {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let rows = load_jsonl(path)?;
        let rows: Vec<Value> = match args.limit { Some(n) => rows.into_iter().take(n).collect(), None => rows };
        eprintln!("{name}: {} rows (v3)", rows.len());
        let dataset = json!({ "path": display_path(path), "sha256": sha256_file(path)?, "rows": rows.len() });
        let runs = measure_v3(&client, &name, &rows)?;
        if runs.is_empty() {
            sections.push_str(&format!("\n- skipped `{name}`: no v3 gate mapping\n"));
            continue;
        }
        let gate = runs[0].gate;
        let binary = runs[0].labels.len() == 2;
        let cal_file = |dir: &Path| dir.join(format!("{gate}_en.v3.json"));

        // Fit (or load) per run; select on dev.
        let mut fitted: Vec<(usize, f64, BTreeMap<&str, Value>)> = Vec::new();
        for (ri, run) in runs.iter().enumerate() {
            let t = match &cal_in {
                Some(dir) => ferricula_gates::Calibration::load(cal_file(dir)).map_err(anyhow::Error::msg)?.temperature as f64,
                None => fit_on(&by_split(&run.items, "train")),
            };
            let per: BTreeMap<&str, Value> = SPLITS.iter().map(|s| (*s, split_metrics(&by_split(&run.items, s), t, binary))).collect();
            let items_path = items_dir.join(format!("{name}.{}.jsonl", run.variant));
            fs::write(&items_path, run.items.iter().map(|i| serde_json::to_string(i).unwrap() + "\n").collect::<String>())?;
            for s in SPLITS {
                let m = &per[s];
                table.push_str(&format!("| {} | {} | {:.3} | {s} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                    gate, run.variant, t, m["n"], m["scored"], fmt(&m["accuracy"]), fmt(&m["ece_raw"]), fmt(&m["ece_calibrated"]),
                    fmt(&m["brier_raw"]), fmt(&m["brier_calibrated"]), fmt(&m["nll_raw"]), fmt(&m["nll_calibrated"])));
            }
            fitted.push((ri, t, per));
        }
        let best_acc = fitted.iter().filter_map(|f| f.2["dev"]["accuracy"].as_f64()).fold(f64::NEG_INFINITY, f64::max);
        let (sel, t, per) = fitted.iter()
            .filter(|f| f.2["dev"]["accuracy"].as_f64().is_some_and(|a| a >= best_acc - WORDING_ACC_SLACK))
            .min_by(|a, b| {
                let e = |f: &(usize, f64, BTreeMap<&str, Value>)| f.2["dev"]["ece_calibrated"].as_f64().unwrap_or(f64::INFINITY);
                e(a).partial_cmp(&e(b)).unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
            .context("no scored dev items")?;
        let run = &runs[sel];
        let test_n = per["test"]["scored"].as_u64().unwrap_or(0) as usize;
        let test_ece = per["test"]["ece_calibrated"].as_f64().unwrap_or(f64::INFINITY);
        let dev_ok = per["dev"]["ece_calibrated"].as_f64().unwrap_or(f64::INFINITY) <= per["dev"]["ece_raw"].as_f64().unwrap_or(f64::NAN) + 1e-12;
        let authorized = test_n >= MIN_TEST_N && test_ece < ECE_TARGET && dev_ok;
        let models: BTreeMap<String, usize> = run.items.iter().filter_map(|i| i.model.clone())
            .fold(BTreeMap::new(), |mut m, k| { *m.entry(k).or_default() += 1; m });

        // Policy view on test (calibrated): the production abstention rules.
        let test_items = by_split(&run.items, "test");
        let policy_min = match gate { "vedana" => 0.5, "worth_researching" => 0.6, _ => 0.0 };
        let answered: Vec<(f64, bool)> = test_items.iter().filter_map(|i| i.probs.as_ref().map(|p| (calibrate::temper(p, t), i.gold)))
            .map(|(p, y)| calibrate::top_label(&[p], &[y])[0]).filter(|x| x.0 >= policy_min).collect();
        let policy = json!({
            "min_confidence": policy_min,
            "coverage": j(answered.len() as f64 / test_items.len().max(1) as f64),
            "accuracy_answered": j(answered.iter().filter(|x| x.1).count() as f64 / answered.len().max(1) as f64),
            "ece_answered": j(calibrate::ece(&answered, ECE_BINS)),
        });

        let thresholds = (gate == "worth_researching").then(|| json!({
            "dev": threshold_table(&by_split(&run.items, "dev"), t),
            "test": threshold_table(&test_items, t),
        }));
        let recommended_tau = thresholds.as_ref().and_then(|th| th["dev"].as_array().and_then(|rows| rows.iter()
            .filter(|r| r["missed_worth_rate"].as_f64().is_some_and(|m| m <= MAX_MISSED_WORTH))
            .filter_map(|r| r["tau_calibrated"].as_f64()).fold(None, |a: Option<f64>, x| Some(a.map_or(x, |a| a.max(x))))));

        let record = json!({
            "dataset": dataset,
            "wording": run.variant,
            "wordings_tried": runs.iter().map(|r| r.variant.clone()).collect::<Vec<_>>(),
            "questions_version": ollaya::QUESTIONS_VERSION,
            "labels": run.labels,
            "fitted_on": "train", "selected_on": "dev", "reported_on": "test",
            "splits": per,
            "policy_test_calibrated": policy,
            "thresholds": thresholds,
            "recommended_skip_tau_calibrated": recommended_tau,
            "backend": { "kind": "ollaya", "url": args.url, "model_requested": args.model, "models_answered": models },
            "run_id": ctx.run_id, "git_commit": ctx.git_commit, "git_dirty": ctx.git_dirty,
        });
        let mut cal_record = Value::Null;
        if let (Some(dir), None) = (&cal_out, &cal_in) {
            fs::create_dir_all(dir)?;
            let file = ferricula_gates::calibrate::CalibrationFile {
                gate: gate.to_string(),
                temperature: t,
                lifecycle_authorized: authorized,
                note: if authorized { "held-out ECE < target at n >= 100; may drive lifecycle".into() }
                    else { "NOT authorized: held-out criteria not met; probabilities are advisory only".into() },
                record: record.clone(),
            };
            let out = cal_file(dir);
            fs::write(&out, serde_json::to_vec_pretty(&file)?)?;
            let loaded = ferricula_gates::Calibration::load(&out).map_err(anyhow::Error::msg)?;
            fs::write(out.with_extension("json.sha256"), format!("{}  {}\n", loaded.sha256, out.file_name().unwrap().to_string_lossy()))?;
            cal_record = json!({ "path": display_path(&out), "sha256": loaded.sha256 });
        }
        if let Some(dir) = &cal_in {
            let loaded = ferricula_gates::Calibration::load(cal_file(dir)).map_err(anyhow::Error::msg)?;
            cal_record = json!({ "path": display_path(&cal_file(dir)), "sha256": loaded.sha256, "applied_not_fitted": true,
                "file_lifecycle_authorized": loaded.lifecycle_authorized });
        }

        for (ri, t_r, per_r) in &fitted {
            for s in SPLITS {
                ctx.append(json!({
                    "status": "ok",
                    "v3": true,
                    "dataset": dataset,
                    "gate": gate,
                    "wording": runs[*ri].variant,
                    "selected": *ri == sel,
                    "split": s,
                    "split_rule": "fixed per-row `split` (sha256 of source group: 40% train / 20% dev / 40% test)",
                    "labels": runs[*ri].labels,
                    "label_provenance": "llm-agreement (see research/gates/datasets/CARD-v3.md); not human",
                    "temperature": t_r,
                    "backend": { "kind": "ollaya", "url": args.url, "model_requested": args.model },
                    "ece_bins": ECE_BINS,
                    "ece_target": ECE_TARGET,
                    "calibration_authorized": *ri == sel && authorized,
                    "calibration_file": if *ri == sel { cal_record.clone() } else { Value::Null },
                    "n": per_r[s]["n"].clone(),
                    "metrics": per_r[s].clone(),
                }))?;
            }
        }

        sections.push_str(&format!(
            "\n### {gate} (`{name}`)\n\nSelected: `{}` · T = {t:.3} · test n = {test_n} · test ECE raw {} → calibrated {} · test acc {} · \
             **lifecycle_authorized = {authorized}**{}\n\nProduction policy on test (calibrated, answer if top p >= {policy_min}): coverage {}, accuracy {}, ECE {}.\n",
            run.variant, fmt(&per["test"]["ece_raw"]), fmt(&per["test"]["ece_calibrated"]), fmt(&per["test"]["accuracy"]),
            if cal_record.is_null() { String::new() } else { format!(" · calibration file `{}` sha256 `{}`", cal_record["path"].as_str().unwrap_or(""), cal_record["sha256"].as_str().unwrap_or("")) },
            fmt(&policy["coverage"]), fmt(&policy["accuracy_answered"]), fmt(&policy["ece_answered"])));
        if gate == "vedana" {
            sections.push_str(&format!("Intensity MAE (Ollaya score vs mean annotator intensity, rows where annotators agree within 1): test {}.\n",
                fmt(&per["test"]["intensity_mae"])));
        }
        if let Some(th) = &thresholds {
            sections.push_str("\nCuriosity skip threshold (skip when calibrated p(worth) < tau; `tau raw` is the same cut on Ollaya's raw p):\n\n\
                | split | tau (cal) | tau raw | skip rate | missed worth | skip precision |\n|---|---:|---:|---:|---:|---:|\n");
            for s in ["dev", "test"] {
                for r in th[s].as_array().into_iter().flatten() {
                    sections.push_str(&format!("| {s} | {} | {} | {} | {} | {} |\n", fmt(&r["tau_calibrated"]), fmt(&r["tau_raw"]),
                        fmt(&r["skip_rate"]), fmt(&r["missed_worth_rate"]), fmt(&r["skip_precision"])));
                }
            }
            sections.push_str(&format!("\nRecommended (largest tau with dev missed-worth <= {MAX_MISSED_WORTH}): {}\n",
                recommended_tau.map_or("none".into(), |x| format!("calibrated tau = {x} (raw p < {:.3})", calibrate::temper_binary(x, 1.0 / t)))));
        }
    }
    report.push_str(&table);
    report.push_str("\n## Selected per gate\n");
    report.push_str(&sections);
    report.push_str(&format!("\nPer-item distributions: `{}`\n", display_path(&items_dir)));
    let path = ctx.write_report(&report)?;
    eprintln!("report: {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(split: &str, gold: usize, p_yes: f64) -> V3Item {
        V3Item { id: String::new(), split: split.into(), kind: String::new(), gold, probs: Some(vec![1.0 - p_yes, p_yes]),
            abstain: None, model: None, latency_ms: 0.0, intensity_gold: None, intensity_pred: None }
    }

    #[test]
    fn split_metrics_identity_at_t1() {
        let items = [item("test", 1, 0.9), item("test", 0, 0.8), item("test", 0, 0.3)];
        let refs: Vec<&V3Item> = items.iter().collect();
        let m = split_metrics(&refs, 1.0, true);
        assert_eq!(m["scored"], 3);
        // Metrics are rounded to 4 decimals for the ledger.
        assert!((m["accuracy"].as_f64().unwrap() - 2.0 / 3.0).abs() < 1e-4);
        assert!((m["ece_raw"].as_f64().unwrap() - m["ece_calibrated"].as_f64().unwrap()).abs() < 1e-12);
        // Brier: ((0.9-1)^2 + 0.8^2 + 0.3^2) / 3
        assert!((m["brier_raw"].as_f64().unwrap() - (0.01 + 0.64 + 0.09) / 3.0).abs() < 1e-4);
    }

    #[test]
    fn threshold_table_counts_missed_worth() {
        let items = [item("dev", 1, 0.15), item("dev", 1, 0.7), item("dev", 0, 0.1), item("dev", 0, 0.25)];
        let refs: Vec<&V3Item> = items.iter().collect();
        let rows = threshold_table(&refs, 1.0);
        let at = |tau: f64| rows.iter().find(|r| r["tau_calibrated"].as_f64() == Some(tau)).unwrap().clone();
        // tau 0.2: skips 0.15 (worth) and 0.1 (not): missed 1/2, precision 1/2.
        assert_eq!(at(0.2)["missed_worth_rate"].as_f64(), Some(0.5));
        assert_eq!(at(0.2)["skip_precision"].as_f64(), Some(0.5));
        assert!((at(0.2)["tau_raw"].as_f64().unwrap() - 0.2).abs() < 1e-9);
        assert_eq!(at(0.05)["skip_rate"].as_f64(), Some(0.0));
    }

    #[test]
    fn source_family_trims_punctuation() {
        assert_eq!(source_family(Some("authored: Dumas")), "authored");
        assert_eq!(source_family(Some("composed parallel sen0001")), "composed");
        assert_eq!(source_family(None), "unknown");
    }

    #[test]
    fn held_out_is_families_beyond_two_largest() {
        let rows: Vec<Value> = [("a1", "authored"), ("a2", "authored"), ("a3", "authored"), ("c1", "composed x"),
            ("c2", "composed y"), ("b1", "boundary")].iter()
            .map(|(id, s)| json!({"id": id, "source": s})).collect();
        assert_eq!(held_out_ids(&rows), vec!["b1".to_string()]);
    }
}
