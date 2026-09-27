# Research gaps versus ferricula_v2

Author: Architect / Appalling Goldfish (n8-olive-vole). Inspection date: 2026-09-19.
Scope: research notes only; no crate changes, experiments, builds, or archetype designs.

**Source status: provisional pending the two local PDF reads.** Read `RESEARCH_RUN.md`, the v2 source files cited below, `ferricula_v2/PLAN.md`, and the relevant research portions of the v1 roadmap. The two PDFs named in the run exist in `ferricula/research/`, but their full contents have not yet been read: available nuts_read accepts UTF-8 text, and the session prohibits shell file reads. Read-only PDF-extraction permission or Coordinator-provided text is pending. The run's PDF extract is secondhand evidence, not a substitute for reading either PDF. Primary web sources were checked independently and are linked below.

## Assessment

**Inference from inspected code:** v2 already supplies useful memory lifecycle, persistence, graph, and retrieval components. It does not yet establish the DenseAM/LSR mathematical behavior or the conversational benchmark results described in the research agenda. There is also a concrete integration gap between components present in crates and the recall path used by the inspected server runtime.

“Verified” below means source or paper text was read; it does not mean code was executed. “Gap” means absent from the inspected path or not demonstrated by evidence found, not proof of absence throughout every branch. Paper-inspired acceptance criteria are recommendations, not requirements imposed by the papers on Ferricula.

## What exists in the inspected checkout

Paths in this section are relative to `ferricula_v2/crates/`.

| Verified source evidence | What this covers | Limit |
|---|---|---|
| `ferricula-core/src/engine.rs:137`, `vector_topk`; bitmap tag operations earlier in that file | Exact vector candidate ranking with cosine/L2 and bitmap filtering | Ranks stored rows; does not evolve a noisy query toward a learned attractor |
| `ferricula-core/src/memory.rs:75`, `MemoryRecord`; `decay_tick:130`, `on_recall:145`, `resonates:208` | Fidelity, decay, lifecycle, keystones, timestamps, reinforcement, and recall gates | Fidelity starts at 1 and decays by a rule; it is not measured factual correctness. The temporal gate uses time since recall, not fact-validity intervals |
| `ferricula-core/src/persist.rs:78`, `remember`; `checkpoint:177`; WAL replay and snapshot loading | Durable row/envelope and graph infrastructure, with legacy decoding code | Restart and migration checks were not run in this review |
| `ferricula-cognition/src/dream.rs:62`, `consolidate_group:416` | Cosine threshold 0.85, survivor selection by fidelity, consolidation provenance, graph transfer, archival | Similarity grouping and metadata consolidation do not establish truth-aware contradiction resolution or LSR energy descent |
| `ferricula-core/src/graph.rs:15`, `EdgeKind`; `Edge:32` | Semantic, directed causal, and document-structural edges | Edge fields have no fact-validity intervals. Per-edge similarity history is explicitly not snapshot-persisted (`:59`) |
| `ferricula-search/src/fusion.rs:11`, `reciprocal_rank_fusion`; `hybrid.rs:859`, `execute_hybrid_search` | Lexical/semantic fusion components, including FST intent boosting | `hybrid.rs:43` still embeds through shivvr scratch ingestion; source presence is not zero-sidecar integration |
| `ferricula-server/src/memory.rs:55`, `recall_candidates`; `runtime.rs:838` delegates to it | Server candidate recall using tag-token coverage, fidelity, lifecycle weight, and importance | This path is lexical, scans rows, and caps returned candidates at 50; it does not call the full planned dense/graph/BM25 pipeline |
| `ferricula-server/src/memory_overlay.rs`, `OverlayLog::append`, `projection`, `verify`; `runtime.rs` | Durable overlay events, projections, validation, task state, and context injection | Audit/event infrastructure is not by itself a temporal factual knowledge graph or a complete conversational memory manager |
| `ferricula-search/src/eval.rs:1`, `answer_recall:69`, `EvalAggregate:121` | Q&A parsing and answer-token-containment retrieval metrics: Hit@k, MRR, nDCG@k | Does not establish conversational answer correctness, temporal reasoning, or LoCoMo/BEAM performance |

The plan reports Phase A complete and places trait seams in B, deduplication in C, unified recall/server in D, and migration/parity in E. It explicitly puts DenseAM formalization and LoCoMo/LongMemEval/BEAM work after the fusion substrate. Those are planning statements, not fresh test results. Inspected `ferricula-semantic/src/lib.rs` exports implementation modules but no Embedder/Chunker/Inverter trait definitions. The dream ghost-echo seam is also concretely unfinished: `dream.rs:503` returns `None`, with a Phase B integration TODO immediately above it.

## DenseAM and LSR: algorithmic evidence still needed

**Primary claim and method:** Hoover et al., *Dense Associative Memory with Epanechnikov Energy*, introduce log-sum-ReLU energy using an Epanechnikov kernel. They report exact pattern retrieval with exponential capacity under their model and additional emergent minima. This is an associative dynamical system, not simply a vector similarity filter. [Paper, arXiv:2506.10801v2](https://arxiv.org/abs/2506.10801v2).

**Related boundary result:** Petrova et al. analyze continuous states on an N-sphere with exponential memory load. In their sharp-kernel regime, the reported maximum zero-temperature load parameter is 0.5. LSR's finite support produces a threshold below which spurious patterns do not contribute to the noise floor. These claims depend on that model and regime; they are not guarantees for correlated text embeddings. [Paper, arXiv:2604.07401v2](https://arxiv.org/abs/2604.07401v2).

**Covered:** stored vectors, candidate filtering, dream scheduling/intensity input, cosine consolidation, provenance, and lifecycle handling are usable surrounding infrastructure.

**Gap, inferred from engine and dream implementations:** no inspected path defines the LSR energy, its update rule, support/radius handling, or convergence conditions. A 0.85 cosine merge threshold does not implement an Epanechnikov energy landscape. No `LSR` match was returned by a case-insensitive search of v2 crates.

**Evidence needed before claiming coverage:** document the exact state space and update equations; test recovery from corrupted cues against cosine and LSE baselines; sweep pattern count, dimension, separation/correlation, noise, and iteration budget. Distinguish recovery of stored patterns from emergent outputs. Verify support-empty and convergence edge cases. Do not relabel current `decay_alpha` as the papers' load parameter α.

**Ferricula versus Lume:** this is principally a cognition/recovery research gap. Lume-derived search remains the evidence-document index; a generated attractor is not a verbatim source passage. Current v2 provenance helps, but evidence-preserving output and admission criteria remain to be demonstrated.

## Rooke et al., arXiv:2601.01253: thermodynamic accounting still needed

**Verified paper:** Spencer Rooke, Dmitry Krotov, Vijay Balasubramanian, David Wolpert; submitted January 3, 2026, revised April 5, 2026. It studies polynomial DenseAMs with binary spins, finite-temperature stochastic dynamics, and low/intermediate memory load. Dynamical mean-field analysis relates driven retrieval, work/power, and entropy production; higher-order networks can fail at nonzero temperature in ways absent at zero temperature. Its illustrative control strategy is not claimed optimal. [Paper, §§II–IV, arXiv:2601.01253v2](https://arxiv.org/html/2601.01253v2).

**Covered:** the inspected memory/dream code exposes decay, heat gates, intensity, and random-byte selection inputs.

**Gap:** those mechanisms do not establish the paper's transition dynamics, thermodynamic consistency, heat/work bookkeeping, or entropy-production measurements. The inspected crates returned no `entropy_production` match. No physical energy measurement was run.

**Research criterion, inferred:** first define the mapping between algorithmic state and the paper's variables, then reproduce a bounded retrieval-accuracy versus time versus entropy-production experiment under its assumptions. Measure wall time and hardware energy separately if later authorized. Randomness consumption, software “heat,” LSR objective values, and physical dissipation are different quantities. Neither this paper nor Landauer terminology supplies a measured joules-per-forgotten-row value for the current Rust engine.

Keep this polynomial/binary-spin study distinct from the continuous LSR papers; transferring results between them requires an argument.

## Mem0, Letta/MemGPT, Zep/Graphiti: partial components, incomplete system equivalence

| System / primary evidence | What the reference asks us to demonstrate | v2 coverage and inferred gap |
|---|---|---|
| Mem0, [paper §2 and appendix B](https://arxiv.org/html/2504.19413v1) | Extract salient facts, compare with stored facts, choose add/update/delete/no-op, then retrieve; graph variant captures relations | Core durable mutation operations, provenance, and consolidation exist. The inspected cosine consolidation path does not establish semantic update/no-op/contradiction decisions. Need conversational ingestion evidence showing changed facts and duplicate facts produce appropriate memory changes |
| Letta/MemGPT, [MemGPT paper](https://arxiv.org/abs/2310.08560v2), [Letta stateful-agent docs](https://docs.letta.com/v1-sdk/concepts/stateful-agents) | Manage bounded in-context memory alongside retrievable external memory and durable interaction history | Runtime task state, overlay records, and injected retrieved candidates exist. They do not demonstrate MemGPT-style context paging, editable pinned factual blocks, or recovery of evicted conversation history. Need overflow, restart, and cross-session continuity evidence |
| Zep/Graphiti, [paper §§2–3](https://arxiv.org/html/2501.13956v1) | Link raw episodes to resolved entities/facts; track ingestion time and fact validity; invalidate contradictory facts while retaining history; retrieve across graph/lexical/semantic representations | Row references, graph edges, provenance, and overlays provide partial building blocks. Core `Edge` has no validity timestamps; `created_at`/`last_recalled` and a directed edge do not establish bitemporal facts. Need dated contradiction and historical “as of” retrieval cases |

**Published numbers are reference results only:** Mem0 reports 26% relative improvement in its judge metric over its OpenAI-memory baseline, plus 91% lower p95 latency and over 90% token savings against its full-context comparison. These are different comparisons, not a v2 target score or a universal current ranking. Zep reports DMR 94.8% versus MemGPT 93.4% in its paper. No equivalent v2 experiment was run. The relevant paper links above support these reported figures.

**Ferricula versus Lume inference:** lifecycle and consolidation belong to memory; lexical/document retrieval and exact source hydration belong to the document plane. Neither replaces semantic fact maintenance. The fusion can combine them, but system equivalence must be shown through the actual server ingestion/recall path, not inferred from crate names.

## LoCoMo and BEAM: evaluation gap

**LoCoMo:** the original paper describes conversations averaging 300 turns and 9K tokens, extending to 35 sessions. It evaluates QA, event summarization, and multimodal dialogue generation. A QA-only experiment must identify itself as such. [Maharana et al., arXiv:2402.17753](https://arxiv.org/abs/2402.17753).

**BEAM:** the paper describes 100 conversations and 2,000 validated questions, with histories up to 10 million tokens. Its LIGHT framework combines episodic memory, working memory, and a scratchpad; architectural naming alone does not reproduce the reported gains. [Tavakoli et al., arXiv:2510.27246v2](https://arxiv.org/abs/2510.27246v2).

**Covered:** v2's document eval supplies useful retrieval diagnostics, and memory persistence/lifecycle enable longitudinal experiments.

**Gap:** no LoCoMo implementation was found by the crate search, and no LoCoMo or BEAM result was produced in this review. The inspected Q&A parser retains question/answer only, not a conversational session/timestamp schema. Answer-token containment can mark a passage relevant without establishing that the generated answer is temporally or causally correct.

**Proposed evidence package:**

1. Pin dataset version/subset, conversation split, question categories, reader/embedder/judge versions, prompts, context budget, and memory policy.
2. Ingest histories in order, preserve speaker and reference time, prevent evaluation-answer leakage, and record the actual memory supplied to the answerer.
3. Report end-to-end answer quality by category, alongside retrieval diagnostics, ingestion cost, query tokens, p50/p95 latency, storage growth, and failures/skipped cases.
4. Evaluate after decay/consolidation and restart, not only immediately after ingest. Track important facts lost and superseded facts incorrectly retained.
5. Compare matched no-memory, bounded full-context where feasible, lexical, dense, hybrid, and lifecycle/consolidation ablations. At BEAM 1M/10M, label truncation and incomplete runs explicitly.
6. Keep v1 migration parity separate from external benchmark quality: a migration can preserve behavior without reaching a paper baseline.

## Research priorities for Coordinator

1. Close the local-source reading gap and reconcile this provisional note against both PDFs.
2. Preserve the source-backed distinction between existing components and pending trait/unified-recall integration.
3. Establish a small, reproducible conversational evaluation baseline before claiming improvements from consolidation or new kernels.
4. Treat temporal fact maintenance and context management as separate gaps from lexical/vector retrieval.
5. Treat LSR recovery and Rooke-style thermodynamic accounting as distinct research experiments, each with explicit assumptions.

No crates, PLAN.md, or parity directory were changed. No tests, builds, benchmarks, or PDF extraction were run. Only this research note was written.
