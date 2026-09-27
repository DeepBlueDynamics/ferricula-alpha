---
title: Work plan — packages for parallel agents
status: proposed
source: Claude (coordinator), for Kord
date: 2026-09-27
related: docs/BACKLOG.md, inbox/COORDINATOR.md, inbox/DECISION_DAG.md, inbox/UI_PLAN.md, inbox/JEV_PLAN.md, inbox/PLAN_CONSOLIDATION.md, docs/BENCH_PLAN.md
---

# Work plan

Each package below can go to one agent. Packages in the same wave don't share files, so they can run at the same time in separate worktrees. The coordinator (Claude, `inbox/COORDINATOR.md`) hands them out over Hyperia, reviews results, merges, redeploys, and takes questions to Kord. No time or size estimates.

## Rules for every package
- **Worktree and branch:** a worktree on `v3/wp-<id>` from `v3/r0`, under `.claude/worktrees/`. Build output goes to `D:/cargo-target` (inherited from `.cargo/config.toml`). **Never build on C:.**
- **Stay in your lane:** touch only the files your package owns. If you need a change elsewhere, ask the coordinator.
- **Before handing back:** `cargo test` for the crates you touched must pass. Add tests for new behavior.
- **Commits:** end with the trailer `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Don't push to `main`; the coordinator merges.
- **Hard limits:** no secrets in the repo (credentials live in `~/.config/ferricula/`); never print tokens; no private conversations in plans or datasets.
- **The live agent:** don't touch `ferricula-steve` (:18875). Only the coordinator redeploys it or talks to Steve.
- **Reporting:** report as in `COORDINATOR.md`: what changed, tests, exit-test evidence, open questions. No time estimates.
- **Review:** Steve reviews every package on quality (verbatim diff and report, with cites). His verdict needs a disposition; only Kord overrides. Packages that change the gates, benchmarks or tools that judge Steve also need Kord's sign-off.

## Wave 0: unblock (coordinator and Kord)
| Item | Who | Notes |
|---|---|---|
| K1 `THIRD_PARTY_NOTICES.md` | Kord decides, coordinator restores | image builds fail until done |
| K6 commit today's tree | coordinator, after Kord's OK | plans, docs, token move, `inbox/` |
| K2 TypeSafe key and API choice | Kord | blocks WP-G |
| K3 nuts-auth fixes | Kord / nuts.services | U1 can build against today's nuts-auth; don't ship the login until K3 |

## Wave 1: parallel

### WP-P · Plan directory (P1)
- **Goal:** everything in `inbox/PLAN_CONSOLIDATION.md`, including Steve's review fixes (links pointing both ways, an owned open-decisions queue, an abstention path, a privacy check at intake).
- **Owns:** every `.md` move (`research/`, `paper/`, `inbox/`, plan files in `docs/`, `scan/`, `tasks/`), `plan/**`, `PLAN_INTAKE.md`, `README.md` links, `scripts/plan-index.sh`, `scripts/plan-to-steve.sh`, `.gitignore`. Also the path defaults in `crates/ferricula-bench/src/main.rs:53,79` (only those lines) and doc-comment paths.
- **Must not touch:** any Rust logic.
- **Runs:** first, alone for the move commit. **Every other package pauses markdown edits until the move lands, then rebases.**
- **Exit:** as listed in the plan (lume search finds the benchmark plan and the essays; no stale paths; `git log --follow` works).

### WP-J · Live fixes (J2)
- **Goal:** every J2 bullet in `docs/BACKLOG.md`.
- **Owns:** `crates/ferricula-server/src/life.rs` *except* `life_worth_researching`; `crates/ferricula-cognition/src/life.rs`; `crates/ferricula-ingest/src/extract.rs` (nav stripping); the dream prompt assembly.
- **Must not touch:** `life_worth_researching` and `ferricula-gates` (WP-G), `chat*.rs`.
- **Exit:**
  - A forced two-sleep session dreams in both sleeps, and never dreams while engaged.
  - The curiosity gate input stays under 512 tokens and no longer abstains with `state_truncated` on thread seeds.
  - Re-ingesting the Macfilos eulogy page gives a handful of real sections, not 29 of chrome.
  - Dream prompts contain no scores or "Never decays".

### WP-D · Decision DAG (D1–D4)
- **Goal:** `inbox/DECISION_DAG.md` up to and including the Advocate wiring.
- **Owns:** `crates/ferricula-core/src/graph.rs` (cycle check, frontier), `crates/ferricula-server/src/memory.rs` (boundary, question and document nodes), `crates/ferricula-server/src/chat_tools.rs` (a `record_boundary` path, document supersession in `mark_disputed`), `crates/ferricula-cognition/src/advocate.rs` and its wiring point, `crates/ferricula-cognition/src/patthana.rs`, `docs/TOOLS.md`.
- **Must not touch:** `chat.rs` except to pass new context into tools (coordinate with WP-U).
- **Exit:**
  - A cycle-closing edge is refused, with a test.
  - The frontier query returns the in-force verdicts about a memory.
  - An operator "no" becomes a boundary node; a bare no triggers at most one clarifying question, gated by entropy; conflicting answers give `unresolved` plus inaction plus one question.
  - A document can be superseded, and the verdict shows up in `search_documents`.
  - The Advocate records its warnings and questions with zero authority.

### WP-U1 · UI, phase 1 (U1 + login A0)
- **Goal:** `inbox/UI_PLAN.md` phase 1 plus §3a A0: a `/talk` page with the design on existing data (reply arrives whole), and nuts-auth login alongside the operator token.
- **Owns:** new `crates/ferricula-server/src/talk.html`, a new auth module, route additions in `api.rs`, config keys for the operator allowlist and nuts-auth URLs.
- **Must not touch:** `chat.rs`, `chat_tools.rs` (read-only use of `ChatTurn`), `chat.html`.
- **Exit:**
  - `/talk` matches the mockup tokens.
  - The recall card shows sources and verdicts; the tool log shows as "searches"; the bored/tired gauges come from `/life`; the disclosure line is always visible.
  - A nuts-auth login from the allowlist gets a session; anyone else gets 403.
  - The operator token still works.

### WP-B · Benchmarks
- **Goal:** the B-ids in `docs/BENCH_PLAN.md`. Can be split across several agents, one B-id each, as the plan's dependency column allows.
- **Owns:** `crates/ferricula-bench/**` (except the WP-P path lines), `audit/bench/**`, fixtures under the bench plan's dataset directory.
- **Must not touch:** server code. Benchmark mode runs on an **empty store**, never Steve's.
- **Exit:** per B-id. Every number gets a ledger row and a disclosure block (model, judge, embedder, k, tokens, $, dataset hash, date).

### WP-R · Paper (R1, R2)
- **Goal:** `paper/WHITEPAPER_V2.md` brought up to the code and today's results, following the Research-debt section of `docs/BACKLOG.md`.
- **Owns:** `paper/**` (after WP-P, `plan/paper/**`).
- **Exit:** Table 1 matches the code; §7 cites only ledger rows; privacy note; `references.bib`; figures listed.

## Wave 2: after the Wave 1 pieces they depend on

### WP-G · JEV backup tier and conflicted gates (G1–G4, X9)
- **Depends on:** K2; WP-J merged (so the file shares a clean `life.rs`).
- **Owns:** `crates/ferricula-gates/**`, `life_worth_researching` in `server/life.rs`, the merge-gate call at `life.rs:820`, `[gates.jev]` config, the gate call log.
- **Exit:** `inbox/JEV_PLAN.md` phases; X9's exit test; Ollaya and JEV disagreements are recorded as `disputes` only.

### WP-U2 · UI, phases 2 and 3
- **Depends on:** WP-U1 and WP-D merged. Stages need X2 for full fidelity; until then they're approximated and labelled as such.
- **Owns:** `chat.rs` streaming path, `POST /chat/stream`, provider streaming in `model_transport.rs`, the judge and leading-question gates (with WP-G's gate API), Stop/cancel.
- **Exit:** `inbox/UI_PLAN.md` phases 2–3.

### WP-X · Life wiring (X1, X3, X4, X5)
- **Depends on:** WP-J merged.
- **Owns:** `core/thermo.rs` wiring, `server/life.rs` sleep and meditation paths, `server/comfy.rs` wiring.
- **Exit:** per backlog items; the 30-night lifecycle invariants from `docs/BENCH_PLAN.md`.

### WP-H · Steve on Hyperia (H1–H4)
- **Depends on:** Hyperia running; WP-J merged (shared `life.rs`); coordinate with WP-D on `chat_tools.rs`.
- **Owns:** new `server/src/hyperia.rs`, the mail poller in `life.rs`, the `send_message` tool in `chat_tools.rs`, `[hyperia]` in `config.rs`, `Dockerfile:77` secret list.
- **Exit:** the H4 smoke test in `inbox/HYPERIA_COMMS.md`.

### WP-S1 · Steward reader (S1)
- **Depends on:** WP-P.
- **Owns:** `scripts/plan-to-steve.sh`, the list of files Steve holds.
- **Exit:** Steve answers "what does our plan say about conflicted gates?" with a correct cite. Plan knowledge is tagged and excluded from benchmark scoring.

## Wave 3
- **WP-V (X2 vīthi per input):** touches `chat.rs` and `life.rs` broadly, so it runs alone once WP-U2 and WP-X have merged.
- **WP-M (D5, manager and inter-agent inbox, S2–S4):** design first, reviewed by Steve; then a separate plain steward container.
- **WP-W (W1 web corpus):** after WP-G, since ingest from chat goes through the gate.
- **X8 72-hour soak:** after WP-X and WP-G. Then the paper's results section.

## Ownership at a glance
| File or area | Wave 1 owner | Later |
|---|---|---|
| `.md` moves, `plan/**`, README links | WP-P | S1 |
| `server/life.rs` | WP-J (not `life_worth_researching`) | WP-G (gate fn), WP-X, WP-V |
| `server/chat.rs` | nobody | WP-U2, then WP-V |
| `server/chat_tools.rs`, `memory.rs`, `core/graph.rs`, `advocate.rs`, `patthana.rs` | WP-D | — |
| `server/api.rs` | WP-U1 (additions only) | WP-U2 |
| `ferricula-gates` | nobody | WP-G |
| `ferricula-bench`, `audit/bench` | WP-B | — |
| `paper/` | WP-R | — |
| `ferricula-ingest/src/extract.rs` | WP-J | — |
