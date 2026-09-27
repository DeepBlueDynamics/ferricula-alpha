# Ferricula v3 — Last-Mile Plan

_2026-09-26. Supersedes the fleet's RELEASE_BRIEF (which targeted upstream `ferricula`/v2). This repo, `ferricula-alpha`, is the one working tree and is **Ferricula v3**. PLAN.md remains the v2 fusion design and is still the substrate plan._

## 0. What v3 is, in one paragraph

An agent that has a memory and a life. It takes in experience through **sense doors** (operator conversation, documents, the web via grub, feeds, entropy from the radio), processes each input through an explicit **cognitive process** modeled on the Abhidhamma vīthi, and at each decision point asks a fast, calibrated **judge** (Ollaya / JEV) before paying for an LLM. Documents are kept verbatim in **lume** (the evidence plane never forgets); experiences live in the **thermodynamic memory** (decay is priority, not deletion). When nothing is happening the agent rests in **bhavaṅga**; boredom drives it to follow its own curiosity; sleep pressure drives it to sleep; sleep runs **bhāvanā** (consolidation) and a **dream** built from the day's residue plus entropy-selected distant memories; then it wakes, or stays asleep until someone says "hey Steve."

## 1. Baseline, 2026-09-26 (historical: where we started; §1a is current)

| Area | Reality |
|---|---|
| Recovered v1 memory (3,362 records) | loads read-only ✅ |
| Operator chat | ✅ in code, ❌ **broken live**: `curator_enabled=true` + thinking model (glm-5.3) → curator truncates at 512 tokens → `curation.rs:92` bails → chat fails; real error swallowed at `chat.rs:210` |
| Recall | lexical token coverage only |
| New memory writes | none (overlay approve = 501) |
| Document ingest | none in server; lume crawl/index is a separate CLI; PDF helper script missing from repo |
| `ferricula-search` / `-semantic` / `-gates` | built, tested, **not used by the server** |
| Autonomy | state machine exists; Dreaming state unreachable (no event sends it); no boredom/sleep pressure |
| bhāvanā / sati / curator / karmic | written and tested in cognition, only curator is called |
| Senses | HN + Nuts reads land on task records only; grub reachable (:6792, no auth) but unused by server; radio (:9080) down; Ollaya not installed |
| Benchmarks | **none run.** Every LongMemEval/LoCoMo number in research is a vendor quote |
| Paper | outline only (`memory/research/PAPER_DRAFT.md`) |
| Naming | everything says v2 / `2.0.0-alpha.0` |

## 1a. Progress (2026-09-27)

| Phase | State |
|---|---|
| R0 | ✅ chat fix, v3 versioning, persona-neutral engine, research import (no PDFs), LF for Docker, CI workflow, `init` for new agents. Open: rotate keys in the old fleet workspace (operator). |
| R1 | ✅ ingest (text/URL/PDF) → evidence plane + experience store; hybrid RRF recall; `/documents` routes; `ferricula_ingest`/`_documents`/`_read_section` MCP tools; live-verified on Steve's memory with the MemGPT paper. Durability (fsync, torn-WAL recovery), incremental BM25 (11× faster ingest) and the paywall/login/thin-page screen landed (WP-3). Gap: notes on duplicate ingests dropped. |
| R2b | ◐ meaning in the loop: shivvr `POST /embed`; `ShivvrEmbedder`; `MeaningIndex` sidecar (backfilled 2,802 zero-vector memories in 12.9 s); dense + graph arms in fused recall; chat turns remembered across conversations; meaning-based novelty; outlier-preferring curiosity; far-sampled dream traces; grounding metric. Recovered recall R@5 0.133 lexical → 0.500 hybrid (0.600 with faded memories). Open: `[recall] include_faded_recovered` (1,925 recovered memories are Archived by v1 decay; operator decision); sati-recall rerank; tag proposals; outlier tagging. See docs/EMBEDDINGS_PLAN.md |
| R2 | ◐ Ollaya sidecar running; gate backends + `yes_no` judge; calibration measured (not passing; WP-4 building larger label sets). Built, not wired: `cognition/vithi.rs` (sense doors, stages, the 52 cetasikas from measured signals) and `cognition/patthana.rs` (24 conditions as typed edge labels); wiring is backlog X2 and is how R2's exit test ("every stored record carries its gate verdicts + provenance") is met. |
| R3 | ◐ drives wired into the runtime: `[life]` config, life loop (curiosity → grub search → ingest → reflection; sleep → scratch bhāvanā → dream → wake), `/life` routes, `ferricula_life` tool, journal under `state_dir/life/`. Meditation core (object, bell, radio breath, thoughts arising/returning; `cognition/meditation.rs`) and the post-wake grace fix are built. Built, not wired: `core/thermo.rs` (live thermodynamics over the read-only base; backlog X1). Open: the **72-hour** soak (the 14-minute accelerated soak is not the R3 exit); bhāvanā commits nothing yet and does not refit calibration during sleep. |
| R4 | ◐ `ferricula-bench`: docs (BM25, dense, hybrid; verbatim/partial/paraphrase), gates, recall over the recovered memory (30 queries); LongMemEval skeleton. Plan: docs/BENCH_PLAN.md. |
| R5 | ◐ `paper/WHITEPAPER_V2.md` (architecture, case study, first results; figures and bibliography pending); v3 after the next results. |

## 2. Phases

Each phase ends with an exit test that produces evidence (command, exit code, output) in `audit/`.

### R0 — Make it true and public-safe
1. Version → `3.0.0-alpha.0`; README/PLAN/docs say v3; `RELEASE_BRIEF`-era claims corrected.
2. Fix chat: curator failure must degrade to un-curated chat, never fail the turn; per-profile `reasoning` flag raises the token budget for thinking models; log provider errors (redacted) instead of swallowing them.
3. De-personalize the code: persona comes from `agent.toml` only. `SteveRuntime`→`AgentRuntime`, `steve_*` MCP tools→`ferricula_*` (keep `steve_*` as aliases for one release), default agent id `ferricula-agent`. Steve becomes an example config, not a compiled-in identity.
4. Remove personal paths (`scripts/steve_mcp_bridge.py`, VENDOR_NOTES, scan/), check PDF redistribution (replace arXiv PDFs with links), fix `config/steve.example.toml` privacy hole (cloud model marked private).
5. Import the 09-26 research (lume-ollaya-abhidhamma plan/addendum/benchmarks, gates report, PAPER_DRAFT, essays 14–36) as markdown; no PDFs.
6. CI: `cargo check/test --workspace` on glibc ≥ 2.38.
Exit: CI green; `grep` finds no personal paths/secrets; live chat answers the LeCun question through `/chat`.

### R1 — Document sense door ("read this PDF, what do you think?")
- Writable **experience store** in `state_dir` layered over the read-only recovered base (base never mutated; recall reads both).
- `ingest(source)`: text | URL (grub `/api/markdown`) | PDF (Rust extraction, grub fallback) → lume sections (verbatim, never decays) → `ferricula-semantic` chunk + embed → memory records whose refs point at `(doc_id, section_id)` → one "I read X" episode.
- Recall becomes hybrid: lume BM25 + roaring tag filter + dense cosine + graph, fused (RRF), hydrated with verbatim sections.
- MCP tools: `ferricula_ingest`, `ferricula_recall` (hybrid, returns cited source text), `ferricula_read_section`, `ferricula_remember`, `ferricula_chat`, `ferricula_walk` (graph walk, requested by Steve himself).
Exit: over MCP, ingest a PDF, ask three questions, every answer cites verbatim section text; restart; answers still cite.

### R2 — The cognitive process with judges
- Explicit vīthi pipeline for every input: **phassa** (contact: sense door + correlation id) → **vedanā** gate (valence/intensity → starting ojā) → **saññā** gate (who/what/when/where/why/how tags) → **santīraṇa** (roaring + cosine: novel or known?) → **voṭṭhapana** (determining: *judgment*) → **javana** (LLM, only if the judge says it's worth it) → **tadārammaṇa** (WAL commit).
- `Judge` trait with backends: `Ollaya` (`/api/decide`, local, sidecar container on :11435), `Jev` (hosted TypeSafe, same wire), `OllamaChat` (interim), `NoModel` (always abstains). Cascade: **accept when confident, escalate to the LLM when unsure** (JEV-as-a-Judge, arXiv 2609.26550). Abstain is never "no". Uncalibrated gates are advisory only until ECE < 0.05. When a gate is conflicted (confidence in the uncertain band, or two judges disagree), radio entropy drives a random recall and the gate is asked again; if it is still split, entropy decides and the record says so (after *ME* by Thomas T. Thomas; backlog X9).
- Wire `SatiMonitor` into recall (papañca → return to object) and `record_pool` into task outcomes.
Exit: every stored record carries its gate verdicts + provenance; judge latency and escalation rate reported.

### R3 — A life: bhavaṅga, boredom, curiosity, sleep, dream
Settings (`[life]`), all overridable:
- `boredom`: rises with time since novel input, falls on novelty. Over threshold → pick a **curiosity** target (open threads from recent conversation first, then entropy-sampled memories) → judge "worth researching?" → grub search/read → ingest as experience. This rebuilds the v1 Jony Ive behavior inside the system, not in a harness.
- `sleep_pressure`: accumulates with activity and tokens spent; over threshold → sleep. Daily budget caps are hard.
- **Sleep** = `bhavana_cycle` (non-destructive consolidation, decay as priority, calibration refit) with durable `BhavanaState`/`ClusterIndex`/karmic WAL.
- **Dream**: a proposal is assembled — day residue (today's episodes ranked by vedanā) + entropy-selected distant memories + unresolved outliers — the LLM writes the dream; it is stored as a `dream` channel memory, labeled as dream, never as evidence. A dream may raise a question that wakes the agent (`wake_on_dream_question`).
- **Meditation**: an explicit mode — bhavaṅga with sati on, no inputs admitted except the operator.
- **Wake**: operator message always wakes; scheduled wakes; dream questions (if allowed).
- Entropy: gnosis-radio/sdr-rand if up, OS RNG fallback, source recorded on every draw.
Exit: 72-hour soak on a copy of the memory: store never shrinks, budget never exceeded, a transcript shows talk → curiosity crawl → sleep → dream → operator wake.

### R4 — Benchmarks (numbers we produced ourselves)
`ferricula-bench` crate + ledger in `audit/bench/`:
1. **Gate calibration** — ECE / abstain rate per gate on held-out labels.
2. **Document memory** — ingest a corpus (Monte Cristo + papers), exact-quote QA: answer accuracy, citation precision, verbatim match. This is the "photographic memory" claim; **do not use that phrase publicly until this QA suite has run** (so far only findability and byte fidelity are measured).
3. **Long-term conversational memory** — LongMemEval (EN) and LoCoMo through the MCP surface, with baselines: no memory, plain RAG, full ferricula.
4. **Lifecycle invariants** — 30/365-night simulation: no deletion without decision, decay/recall dynamics, keystone stability.
5. **Drift / sati ablation** (S9–S11) — monitors on vs off.
6. **Autonomy** — curiosity yield (ingested items later recalled usefully), cost per day, wake latency.
7. **Recall of the agent's own life** (Steve recall v1, docs/BENCH_PLAN.md B4) — 30 questions in eight groups (biography in two wordings, memory-only events, time/order, false premises, abstention, dream vs evidence, multi-hop, lifecycle scripts), each run **memory on and memory off** to separate recall from pretraining.
Abstentions reported separately from wrong answers. Every number has a ledger row.

### R5 — White paper
`paper/`: thesis (impermanence as structure; letting go as an operation; recall as reconstruction; drift never unseen), architecture (vīthi pipeline, two planes, judges), the Abhidhamma mapping table with its honest limits (analogy, not physics or consciousness claims), evaluation from R4 only, limitations. Draft from `memory/research/PAPER_DRAFT.md`.

### Later
- **Body/UI**: Rust + React app hosted in Hyperia — a room, a screen wired to grub renders, live view of vīthi/gates/dreams. Not ferricula-arena's layout.
- **Dream imagery** (R3 add-on): dream → short visual prompt (no real names, no text) → ComfyUI Qwen Image 2.1 (SDXL Turbo fallback, radio seed) → SigLIP embedding → a seeing row tagged `dream`, seen on waking. Client built (`server/comfy.rs`, `semantic/image_embed.rs`); runtime wiring is backlog X4.
- **Teach Steve to code**: `code` task class with a sandboxed workspace, tests as the judge's ground truth, outcomes into `record_pool`.

## 3. Decisions (Kord, 2026-09-26)
- Engine is persona-neutral; Steve is configuration and data, not code.
- Ollama cloud models may receive private memory (operator's choice; documented, not treated as a hole).
- Judges target an Ollaya sidecar first; JEV-compatible wire keeps hosted JEV swappable.
- `ferricula-alpha` is canonical. Upstream `ferricula` PR #40 is already merged; reverting it upstream is pending Kord's call.
- Embedding space (decided in docs/EMBEDDINGS_PLAN.md): GTR-T5-base 768-d now, because the whole recovered store is already in that space; a multilingual space is added later as a second space, never mixed.
- Spelling: the registration moment is *tadārammaṇa* throughout the repo.
- Open: key rotation for `memory/.mcp.json.bak` (Kord; the file sat in the fleet workspace where agents ran with bypass permissions, so rotate, then delete); `[recall] include_faded_recovered` for the live agent.
