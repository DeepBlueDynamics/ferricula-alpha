# Ferricula v3 backlog

_Consolidated 2026-09-27 from the operator session, PLAN_V3, docs/EMBEDDINGS_PLAN, the soak audit and the research plans; each item was checked against the code on `v3/r0` (file:line). **(r2b)** = work on branch `v3/r2b-recall`, in progress._

**The main conflict:** the r2b recall work edits almost every server file (`runtime.rs`, `life.rs`, `chat.rs`, `recall.rs`, `memory.rs`, `documents.rs`, `api.rs`, `mcp.rs`, `config.rs`, `main.rs`) plus `cognition/src/life.rs` and `bench/src/{main,docs}.rs`. Until it merges, parallel work goes only into new files, core, ingest, search, gates, semantic, untouched cognition modules, shivvr and docs.

Sizes: S < 1 day · M 1–3 days · L > 3 days.

## Now

**N1. Dense recall + backfill + remembered turns** · L · in progress (r2b)
Steve recalls by meaning across recovered memory, experience and document sections, and remembers what was said in earlier conversations. Today recall is lexical + BM25 (`documents.rs:146-162`), chat turns are never written as experience (`chat.rs:110-293`), the curator reads only recovered memory (`chat.rs:231-233`). (r2b) adds `meaning.rs`/`meaning_plane.rs`, dense + graph arms in fused recall, `remember_turn` (hearing/thinking rows), curator on fused hits, dream-grounding prompt and metric, `is_dream_image` filtering for dream traces and the dense index. Still open inside N1: curiosity seeds and the lexical path still admit v1 `[dream image]` memories.
Exit: over MCP "Dad or Paul?" recalls memory 3802021270; a new conversation recalls "Kord names his iPhones Steve"; 30-query recovered-recall bench and docs paraphrase hybrid R@1 > 0.71 with ledger rows; shivvr stopped → `embeddings: degraded`, chat works; 0 all-zero vectors left.

**N2. Tired agent falls back asleep after an operator wake** · S
`step` sets Engaged on an operator stimulus without touching sleep pressure (`cognition/life.rs:182-187`); the same call then re-enters sleep (`:239-244`), costing another consolidation and dream. Fix: a post-wake grace window (`wake_grace_min`), keep `dreamed_this_sleep` if sleep resumes within it.
Exit: unit test (pressure 1.2, asleep, operator → only `[Wake]`); no second dream within the grace window.

**N3. Paywall / junk-page filter** · S
Nature's "Access options" page was ingested. Add a typed rejection in `ferricula-ingest/src/extract.rs` (access/subscribe/login phrases, minimum words per section); curiosity journals `rejected: paywall`.

**N4. Durability pass** · M
WAL `flush()` without fsync (`core/persist.rs:437`); document store and `ThermoLayer::save` without fsync (`ingest/store.rs:177-182`, `core/thermo.rs:168-169`); a note on a duplicate ingest is dropped (`server/documents.rs:72-84`, after N1); BM25 rebuilt per ingest (`ingest/store.rs:102,142-157`); lume prints debug lines on every search (`search/bm25.rs:557,668-676`).
Exit: kill -9 during ingest loses nothing acknowledged; duplicate ingest with a new note records it; corpus ingest ≥ 5× faster; no stderr unless `LUME_DEBUG`.

## Next (after N1 merges; server files force this order)

**X1. Live thermodynamics** · L
`ThermoLayer` (`core/thermo.rs`) has no caller; v1 `spawn_clock` is unused; bhāvanā runs on a scratch copy and reports `committed_to_store: false` (`server/life.rs:785-841`); release proposals are never decided (`bhavana.rs:476` unused). Open the layer in `state_dir/thermo`, radio-entropy tick task, `recall(ids)` for every recall and cited hit, fused scores × `weight`, bhāvanā commits decay/edges/cluster index (never delete; release stays a logged proposal).
Exit: 30-night run: store never shrinks, recalled ids gain fidelity, neglected lose it; consolidate journal `committed: true`; recovered `memory_dir` checksum unchanged.

**X2. Vīthi per input + Paṭṭhāna edges** · L
`VithiBuilder` (`cognition/vithi.rs`) and `LinkEvent`→`Paccaya` (`patthana.rs`) are unused; the only gate call in the server is life curiosity (`life.rs:594`). One `process.rs` recorder: operator chat → ear, ingest/grub → eye, radio/body tick → body, thought/dream → mind; stages with measured cetasikas, stored in `state_dir/vithi/*.jsonl`, `GET /vithi` + MCP; typed edges (ReadFrom, ReflectedOn, NextTurn, DreamedFrom, RecalledTogether, SimilarityHypothesis flagged).
Exit: a chat turn and a curiosity excursion each yield a complete vīthi (≥ 5 stages) whose factors point at real signals; typed edges visible via `ferricula_walk`.

**X3. Meditation proper** · M
`/life/meditate` only switches phase (`server/life.rs:406-417`). Add `{object: "breath", minutes}` with a bell that ends the sit, breath rhythm from radio entropy at the body door (no LLM), `POST /mind/thought {text}` (mind-door thought, no reply; judge whether it pulls from the object; sati returns to the object, recorded), a per-sit journal.
Exit: a 5-minute sit ends by bell; 3 injected thoughts show `pulled_away` / `returned_to_object`; zero model calls during the sit.

**X4. Dream images** · M
Templates exist (`config/comfyui/`); no ComfyUI client, image embedding or seeing row. Dream → short visual prompt (no real names, no text) → Qwen Image 2.1 via ComfyUI `:8188` (SDXL Turbo fallback, radio seed) → `state_dir/life/dreams/` → SigLIP via shivvr `/image/embed` → seeing row tagged `dream`, edge `paccaya:upanissaya` to the dream text → first context on waking: "on waking you see an image from your dream". Needs O4.
Exit: forced sleep produces a PNG, a seeing row with a SigLIP vector, the edge, and the wake line; prompt contains no persona proper nouns.

**X5. Dream grounding enforced** · S–M
(r2b) adds the constraint and a report-only metric. Set a threshold (≥ 0.6), regenerate once or tag `ungrounded`; exclude `[dream image]` from curiosity seeds and lexical chat evidence.

**X6. LongMemEval / LoCoMo end to end** · M
Skeleton only (`bench/src/longmem.rs`). Replay sessions through `/chat` (needs N1 remembered turns), judge, ledger rows for no-memory / BM25-RAG / full.

**X7. Missing R1/R2 surface** · M
MCP `ferricula_remember` and `ferricula_walk` absent (`mcp.rs:273-396`); `/memory/overlay/approve` still 501 (`api.rs:67-70,522`); `SatiMonitor` not wired into recall; `record_pool` and `TaskSucceededGate` never called; hosted JEV backend absent.

**X8. 72-hour soak (R3 exit)** · M (wall clock)
After N1, N2, N3, X1, X5 (+X4 if ready): store never shrinks, budget never exceeded, talk → curiosity → sleep → dream → operator wake, cost/day, curiosity yield, wake latency, no double dream.

## Later

- **L1. Gate calibration on real data** · L. Current: vedanā 0.444 / ECE 0.283, saññā 0.455 / 0.147, yes/no 0.778 / 0.195 (ledger `69e4b846`) on 20–40 self-authored labels; the curiosity gate only skips at p < 0.2 (`life.rs:597,602`). Label harvest, nightly temperature refit in bhāvanā (reject refits that raise ECE), `laya:multilingual` for ZH/Pāli. Exit: held-out ECE < 0.05 per gate.
- **L2. CJK/Pāli tokenization, then a multilingual space** · M/L. lume ASCII-folds (`search/src/lib.rs:535-555`); the experience tokenizer treats a CJK run as one token (`server/memory.rs:406-414`); `hybrid.rs:62` is 768-d GTR only.
- **L3. Ghost echoes (shivvr `/invert`), audio door, PDF-page SigLIP** · L.
- **L4. Forgive/seal invariants and the upekkhā/nirodha decision path** · M.
- **L5. `/meaning/projection` and the Hyperia UI** (room, screen, 3D memory space; not the ferricula-arena layout) · L.
- **L6. Teach Steve to code** (code task class, sandbox, tests as ground truth, outcomes into `record_pool`) · L.
- **L7. Research-12 sense-door ingress** (async observation envelope, affect engine) · M.

## Operator-only (Kord)

- **O1.** Rotate the keys in `memory/.mcp.json.bak`, delete the file.
- **O2.** Upstream `DeepBlueDynamics/ferricula` PR #40 (merged): revert, or point upstream at alpha.
- **O3.** Merge PR #1 (`v3/r0` → main), ideally after N1 merges and CI is green.
- **O4.** Push shivvr `feat/vision-audio-embed`; fix `build.rs` (hardcoded local VS `lib.exe` path). Blocks X4, L3.
- **O5.** Disk: C: was at 100% (2.8 GB free) on 2026-09-27; merged agent worktrees were pruned (now ~25 GB free). ~40 GB of old cargo `target/` dirs remain in the `memory/` workspace.
- **O6.** Interaction limits on ferricula-alpha, hyperia, nemesis8 and ferricula expire 2027-03-27.
- **O7.** `steve-jobs-v2-test` is stopped and kept for rollback; decide how long.
- **O8.** `ferricula-steve` (127.0.0.1:18875) runs from the gitignored `config/steve.toml`. After N1: redeploy, re-ask Dad/Paul, tell Steve that Steve English liked the video on LinkedIn.
- **O9.** Personas of real people: `config/examples/steve/agent.toml` labels Steve a simulation; add a README/WHITEPAPER section (example config only, no impersonation outside the operator, disclosure in chat).

## Research debt

- **R1. Paper results:** dense/hybrid (N1), lifecycle 30/365 nights (X1), drift ablation S11 (needs X7 SatiMonitor), drives soak (X8), LongMemEval (X6), conclusion, author's note. Brief for the paper agent: `paper/BRIEF.md`.
- **R2.** Vīthi and Paṭṭhāna tables in WHITEPAPER §4 with honest limits.
- **R3.** Open questions in the plan/Addendum A (saññā taxonomy, sealing policy, overlay weight, M11 success signal); record the embedding-space decision (GTR-768 now, multilingual later) in PLAN_V3.
- **R4.** Update PLAN_V3 §1a and README Status after each merge.

## Parallel work packages

While N1 runs (no overlap with its files):

| WP | Items | Owns | Must not touch |
|---|---|---|---|
| WP-1 (running) | N1 | server crate, `cognition/life.rs`, `bench/{main,docs,recall}.rs` | — |
| WP-2 | X4 client half | new `server/src/comfy.rs` (not in `lib.rs` yet), `ferricula-semantic` image embedder (`/image/embed`), prompt sanitizer + tests | `life.rs`, `runtime.rs` |
| WP-3 | N4 (minus duplicate notes) + N3 | `core/persist.rs`, `core/thermo.rs` (fsync only), `ferricula-ingest`, `search/bm25.rs` | server |
| WP-4 | L1 data + fit | `ferricula-gates`, `bench/src/gates.rs`, `research/gates/` | `bench/main.rs` |
| WP-5 | N2 + cognition half of X3 | new `cognition/src/meditation.rs`; the `step()` fix in `cognition/life.rs` | server |

After N1 merges: **X1** (with X6 and X5 alongside) → **X2**, then **X7** → **X3** and **X4** server wiring → **X8** → **R1** → **O3**.
