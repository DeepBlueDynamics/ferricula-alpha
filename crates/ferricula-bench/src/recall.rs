//! Suite 4: waking recall over a recovered memory (R2b).
//!
//! Hand-written meaning-level queries with gold recovered ids
//! (`research/bench/steve-recall-queries.json`), run against a COPY of the
//! recovered memory directory (never the live volume). Arms, each cut at 10:
//!
//! - `lexical`: the server's lexical scorer (`memory::lexical_hits_with`);
//! - `dense`: cosine top-k over the recovered set of a `MeaningIndex`
//!   (stored vectors where non-zero, the rest embedded through shivvr);
//! - `hybrid_rrf`: RRF over lexical + dense + one graph hop (weight 0.5);
//! - `hybrid`: `hybrid_rrf` plus the meaning guarantee (the server default:
//!   best 2 dense hits lifted to fused ranks 2 and 4).
//!
//! Each arm runs under two lifecycle policies: `active` (released memories
//! excluded, the v3 default) and `faded` (`[recall] include_faded_recovered`).
//! Gold ids that are Forgiven/Archived cannot be found under `active`; the
//! report says how many queries that caps.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use ferricula_core::LifecycleState;
use ferricula_semantic::text_embed::{ShivvrEmbedder, TextEmbedder};
use ferricula_server::meaning::{CatalogItem, MeaningIndex, MeaningKey, MeaningSet, SearchFilter, dot, graph_hop, unit};
use ferricula_server::memory::MemoryRuntime;
use ferricula_server::recall::{ArmList, CandidateKind, RecallCandidate, fuse_arms_all, promote_keys};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::ledger::{RunContext, display_path, sha256_file};
use crate::metrics::{j, mrr, percentile, recall_at_k};

pub struct RecallArgs {
    pub memory_dir: PathBuf,
    pub queries: PathBuf,
    pub shivvr_url: String,
    /// The server's `[recall]` fusion settings the `hybrid` arm mirrors.
    pub lexical_weight: f64,
    pub dense_guarantee: usize,
}

/// Fusion variants reported in the sweep: (lexical weight, guarantee).
const SWEEP: [(f64, usize); 9] = [(1.0, 0), (1.0, 2), (1.0, 3), (0.5, 0), (0.5, 2), (0.5, 3), (0.25, 0), (0.25, 2), (0.25, 3)];

#[derive(Deserialize)]
struct QuerySet {
    memory: String,
    queries: Vec<GoldQuery>,
}

#[derive(Deserialize, Clone)]
struct GoldQuery {
    id: String,
    query: String,
    gold: Vec<u32>,
}

const CUTOFF: usize = 10;
const ARMS: [&str; 4] = ["lexical", "dense", "hybrid_rrf", "hybrid"];

pub fn run(ctx: &RunContext, args: &RecallArgs) -> Result<()> {
    let set: QuerySet = serde_json::from_slice(&std::fs::read(&args.queries).with_context(|| format!("read {}", args.queries.display()))?)?;
    let memory = MemoryRuntime::open(&args.memory_dir).with_context(|| format!("open {}", args.memory_dir.display()))?;
    let catalog = memory.meaning_catalog();
    let states: HashMap<u32, LifecycleState> = catalog.iter().map(|r| (r.id, r.state)).collect();
    for q in &set.queries {
        for g in &q.gold {
            if !states.contains_key(g) {
                bail!("{}: gold id {g} not in the memory", q.id);
            }
        }
    }

    let embedder = ShivvrEmbedder::new(&args.shivvr_url, "gtr-t5-base@768", Duration::from_secs(120), 64)?;
    let mut index = MeaningIndex::new(embedder.space(), embedder.dim(), None);
    index.set_catalog(
        MeaningSet::Recovered,
        catalog.iter().map(|r| CatalogItem::recovered(r.id, &r.text, r.state != LifecycleState::Active, r.stored.clone())).collect(),
    );
    let pending = index.pending(&[MeaningSet::Recovered]);
    let t_embed = Instant::now();
    for chunk in pending.chunks(64) {
        let texts: Vec<&str> = chunk.iter().map(|i| i.text.as_str()).collect();
        for (item, v) in chunk.iter().zip(embedder.embed(&texts)?) {
            index.insert(item, v)?;
        }
    }
    let backfill_ms = t_embed.elapsed().as_secs_f64() * 1000.0;
    let counts = index.counts();

    let t_q = Instant::now();
    let texts: Vec<&str> = set.queries.iter().map(|q| q.query.as_str()).collect();
    let qvecs = embedder.embed(&texts)?;
    let query_embed_ms = t_q.elapsed().as_secs_f64() * 1000.0 / texts.len().max(1) as f64;

    let mut results: serde_json::Map<String, Value> = serde_json::Map::new();
    let mut per_query: Vec<Value> = Vec::new();
    let mut table: Vec<(String, String, [f64; 3])> = Vec::new();
    let mut sweep_rows: Vec<(String, f64, usize, [f64; 2])> = Vec::new();
    for (policy, faded) in [("active", false), ("faded", true)] {
        let mut ranks: HashMap<&str, Vec<Option<usize>>> = HashMap::new();
        let mut sweep: Vec<Vec<Option<usize>>> = vec![Vec::new(); SWEEP.len()];
        let mut lat: Vec<f64> = Vec::new();
        for (q, qv) in set.queries.iter().zip(&qvecs) {
            let t0 = Instant::now();
            let arm_hits = arms(&memory, &index, &q.query, qv, faded);
            let lists = [
                ("lexical", arm_hits.lexical.clone()),
                ("dense", arm_hits.dense.clone()),
                ("hybrid_rrf", arm_hits.fuse(1.0, 0)),
                ("hybrid", arm_hits.fuse(args.lexical_weight, args.dense_guarantee)),
            ];
            lat.push(t0.elapsed().as_secs_f64() * 1000.0);
            let gold: HashSet<u32> = q.gold.iter().copied().collect();
            let rank_of = |ids: &[u32]| ids.iter().take(CUTOFF).position(|id| gold.contains(id)).map(|r| r + 1);
            let mut row = json!({ "policy": policy, "id": q.id });
            for (arm, ids) in &lists {
                let rank = rank_of(ids);
                ranks.entry(arm).or_default().push(rank);
                row[*arm] = json!(rank);
            }
            for (i, (w, g)) in SWEEP.iter().enumerate() {
                sweep[i].push(rank_of(&arm_hits.fuse(*w, *g)));
            }
            per_query.push(row);
        }
        for (i, (w, g)) in SWEEP.iter().enumerate() {
            sweep_rows.push((policy.to_string(), *w, *g, [recall_at_k(&sweep[i], 5), mrr(&sweep[i])]));
        }
        let reachable = set.queries.iter().filter(|q| faded || q.gold.iter().any(|g| states[g] == LifecycleState::Active)).count();
        let mut m = serde_json::Map::new();
        for arm in ARMS {
            let r = &ranks[arm];
            m.insert(arm.to_string(), json!({
                "recall_at_1": j(recall_at_k(r, 1)),
                "recall_at_5": j(recall_at_k(r, 5)),
                "recall_at_10": j(recall_at_k(r, 10)),
                "mrr_at_10": j(mrr(r)),
            }));
            table.push((policy.to_string(), arm.to_string(), [recall_at_k(r, 1), recall_at_k(r, 5), mrr(r)]));
        }
        m.insert("reachable_queries".into(), json!(reachable));
        m.insert("latency_ms_excl_query_embed".into(), json!({ "p50": j(percentile(&lat, 50.0)), "p95": j(percentile(&lat, 95.0)) }));
        results.insert(policy.to_string(), Value::Object(m));
    }

    let queries_sha = sha256_file(&args.queries)?;
    ctx.append(json!({
        "status": "ok",
        "dataset": {
            "queries": display_path(&args.queries),
            "queries_sha256": queries_sha,
            "n": set.queries.len(),
            "memory": set.memory,
            "memory_rows_with_text": counts.recovered_total,
        },
        "backend": {
            "embedder": format!("shivvr {}", args.shivvr_url),
            "space": embedder.space(),
            "lexical": "ferricula_server::memory::lexical_hits_with",
            "fusion": format!("RRF k=60, graph weight 0.5; hybrid_rrf: lexical weight 1, no guarantee; hybrid: lexical weight {}, dense guarantee {} (the server's [recall] settings)", args.lexical_weight, args.dense_guarantee),
        },
        "model": "gtr-t5-base",
        "n": set.queries.len(),
        "metrics": results,
        "index": {
            "recovered_total": counts.recovered_total,
            "recovered_stored": counts.recovered_stored,
            "embedded_now": pending.len(),
            "truncated_sources": counts.truncated_sources,
            "backfill_ms": j(backfill_ms),
            "query_embed_ms_mean": j(query_embed_ms),
        },
        "per_query": per_query,
        "sweep": sweep_rows.iter().map(|(p, w, g, [r5, m])| json!({ "policy": p, "lexical_weight": w, "dense_guarantee": g, "recall_at_5": j(*r5), "mrr_at_10": j(*m) })).collect::<Vec<_>>(),
    }))?;

    let mut r = ctx.report_header("Waking recall over the recovered memory");
    r.push_str(&format!(
        "Queries: `{}` ({} hand-written, meaning-level; sha256 `{}`). Memory: copy of `{}` ({} rows with text; {} stored GTR-T5 vectors, {} embedded now in {:.0} ms through shivvr, {} of them from v1 text cut at 200 chars).\n\
         Query embedding: {:.1} ms mean per query (batched). Gold = recovered ids whose text answers the query; a query counts as found at k if any gold id is in the top k.\n\n",
        display_path(&args.queries), set.queries.len(), queries_sha, set.memory, counts.recovered_total, counts.recovered_stored,
        pending.len(), backfill_ms, counts.truncated_sources, query_embed_ms));
    r.push_str("| policy | arm | recall@1 | recall@5 | MRR@10 |\n|---|---|---:|---:|---:|\n");
    for (policy, arm, [r1, r5, m]) in &table {
        r.push_str(&format!("| {policy} | {arm} | {r1:.3} | {r5:.3} | {m:.3} |\n"));
    }
    let reach_active = results["active"]["reachable_queries"].as_u64().unwrap_or(0);
    r.push_str(&format!(
        "\n`active` = released (Forgiven/Archived) memories excluded, the v3 default: only {reach_active} of {} queries have an Active gold, so recall under `active` is capped at {:.3}. `faded` = `[recall] include_faded_recovered = true`.\n\n",
        set.queries.len(), reach_active as f64 / set.queries.len().max(1) as f64));
    r.push_str(&format!(
        "`hybrid_rrf` = plain RRF (lexical weight 1, no guarantee). `hybrid` = the server's fusion with lexical weight {} and dense guarantee {}.\n\n\
         ## Fusion sweep (recall@5 / MRR@10)\n\n| lexical weight | guarantee | active | faded |\n|---:|---:|---|---|\n",
        args.lexical_weight, args.dense_guarantee));
    for (w, g) in SWEEP {
        let cell = |policy: &str| sweep_rows.iter().find(|(p, ww, gg, _)| p == policy && *ww == w && *gg == g)
            .map(|(_, _, _, [r5, m])| format!("{r5:.3} / {m:.3}")).unwrap_or_default();
        r.push_str(&format!("| {w} | {g} | {} | {} |\n", cell("active"), cell("faded")));
    }
    r.push_str("\n## Per query (rank of the first gold id, top 10; `-` = not found)\n\n| id | query | active lexical / dense / hybrid | faded lexical / dense / hybrid |\n|---|---|---|---|\n");
    let fmt = |v: &Value| v.as_u64().map(|x| x.to_string()).unwrap_or_else(|| "-".into());
    for q in &set.queries {
        let find = |policy: &str| per_query.iter().find(|p| p["policy"] == policy && p["id"] == q.id.as_str()).cloned().unwrap_or(Value::Null);
        let (a, f) = (find("active"), find("faded"));
        r.push_str(&format!("| {} | {} | {} / {} / {} | {} / {} / {} |\n", q.id, q.query.replace('|', "\\|"),
            fmt(&a["lexical"]), fmt(&a["dense"]), fmt(&a["hybrid"]), fmt(&f["lexical"]), fmt(&f["dense"]), fmt(&f["hybrid"])));
    }
    r.push_str("\n## Caveats\n\n- 30 queries, written by reading the memories: small and not blind; treat differences of one or two queries as noise.\n\
        - Recovered memories only (no experience rows or documents), so this measures the recovered-memory arms of hybrid recall.\n\
        - Most v1 texts are cut at 200 characters; a gold memory is only as findable as its surviving prefix.\n");
    let path = ctx.write_report(&r)?;
    eprintln!("report: {}", path.display());
    Ok(())
}

/// Ranked recovered hits of the lexical, dense and graph arms for one query.
struct ArmHits {
    lexical: Vec<u32>,
    dense: Vec<u32>,
    lexical_hits: Vec<ferricula_server::memory::MemoryHit>,
    dense_hits: Vec<ferricula_server::memory::MemoryHit>,
    graph_hits: Vec<ferricula_server::memory::MemoryHit>,
}

impl ArmHits {
    /// The server's fusion: RRF (lexical weighted `lexical_weight`, dense
    /// 1, graph 0.5) then the meaning guarantee for the top `guarantee`
    /// dense hits.
    fn fuse(&self, lexical_weight: f64, guarantee: usize) -> Vec<u32> {
        let as_list = |arm: &'static str, weight: f64, hits: &[ferricula_server::memory::MemoryHit]| ArmList {
            arm,
            weight,
            items: hits.iter().enumerate().map(|(i, h)| RecallCandidate::from_memory(CandidateKind::Memory, h, i + 1)).collect(),
        };
        let mut fused = fuse_arms_all(vec![
            as_list("lexical", lexical_weight, &self.lexical_hits),
            as_list("dense", 1.0, &self.dense_hits),
            as_list("graph", 0.5, &self.graph_hits),
        ]);
        let promote: Vec<String> = self.dense_hits.iter().take(guarantee)
            .map(|h| RecallCandidate::from_memory(CandidateKind::Memory, h, 1).key()).collect();
        promote_keys(&mut fused, &promote);
        fused.iter().filter_map(|c| c.memory.as_ref().map(|m| m.id)).collect()
    }
}

/// The recovered part of the server's hybrid recall for one query:
/// lexical, dense (k = 24) and one graph hop from the top 3 dense hits.
fn arms(memory: &MemoryRuntime, index: &MeaningIndex, query: &str, qv: &[f32], faded: bool) -> ArmHits {
    let lexical = memory.recall_candidates_with(query, CUTOFF, faded);
    let filter = SearchFilter { experience: false, sections: false, ..SearchFilter::memories(faded) };
    let dense_hits = index.search(qv, 24, &filter);
    let dense_ids: Vec<(u32, f32)> = dense_hits.iter().filter_map(|h| h.key.id().map(|id| (id, h.cosine as f32))).collect();
    let dense = memory.hits_for(&dense_ids, faded);
    let seeds: Vec<u32> = dense.iter().take(3).map(|h| h.id).collect();
    let in_dense: HashSet<u32> = dense.iter().map(|h| h.id).collect();
    let qn = unit(qv);
    let hop = graph_hop(&seeds, &in_dense, |id| memory.neighbors(id), |id| {
        index.vector(&MeaningKey::Recovered(id)).map(|v| dot(&qn, &v)).unwrap_or(0.0)
    });
    let pairs: Vec<(u32, f32)> = hop.iter().map(|(id, c, _)| (*id, *c as f32)).collect();
    let graph: Vec<_> = memory.hits_for(&pairs, faded).into_iter()
        .filter(|h| !h.tags.get("text").is_some_and(|t| ferricula_server::meaning::is_dream_image(t))).collect();
    ArmHits {
        lexical: lexical.iter().map(|h| h.id).collect(),
        dense: dense.iter().map(|h| h.id).collect(),
        lexical_hits: lexical,
        dense_hits: dense,
        graph_hits: graph,
    }
}
