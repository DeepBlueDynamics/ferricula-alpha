# Plan: one `plan/` directory for research, inbox, paper and the plans

_Status: proposed 2026-09-27 (Kord asked). Nothing has moved yet. Companion plans written the same day: `inbox/UI_PLAN.md`, `inbox/JEV_PLAN.md`. All three sit in `inbox/` until `plan/` exists: plans are not docs._

## Why

The thinking behind Ferricula is spread across six places with no single entry point:

| Today | Files | What it is |
|---|---|---|
| `research/` | 174 (2.3 MB) | 36 numbered essays, arXiv notes, gate study (`gates/`: datasets, prompts, runs, harness), `nuts-news/`, `original-ferricula/`, `INDEX.md`, `REFERENCES.md` |
| `inbox/` (new, untracked) | 5 | `PLAN.md`, `PLAN_V3.md`, `SEARCH_TOOL.md`, the benchmark plan PDF, the *While he thinks* mockup zip |
| `paper/` | 3 | `BRIEF.md`, `WHITEPAPER.md`, `WHITEPAPER_V2.md` |
| `docs/` | 12 + 3 new | plans and specs (`BACKLOG`, `HANDOFF`, `BENCH_PLAN`, `EMBEDDINGS_PLAN`, `TOOLS`, `AGENT_HARNESS`, `NUTS_*`, `UI_PLAN`, `JEV_PLAN`, this file) mixed with operator guides (`OPERATOR_GUIDE`, `STEVE_RUNTIME`, `STEVE_RECOVERY`, `STEVE_INTEGRATION_ACCEPTANCE`) |
| `scan/`, `tasks/` | 3 + 3 | fleet-era scans and task cards |
| root | — | `README.md`, `LICENSE.md` (and `THIRD_PARTY_NOTICES.md`, see below) |

Goal: one directory, `plan/`, holding everything we think, decide and propose; one prompt in the root telling anyone (Kord, Claude, Steve, another agent) how to submit to it and how submissions get integrated; and all of it searchable with lume, both from the shell and by Steve.

## Found while surveying (fix first)

- **`THIRD_PARTY_NOTICES.md` is deleted from the root and is not in `inbox/`.** `Dockerfile:23` copies it (`COPY Cargo.toml Cargo.lock LICENSE.md THIRD_PARTY_NOTICES.md ./`), so the next image build fails. It also carries the BSD-3 notice for lume-derived code in `crates/ferricula-search`, which the license requires us to keep. It stays in the root: `git restore THIRD_PARTY_NOTICES.md`.
- `PLAN.md`, `PLAN_V3.md`, `SEARCH_TOOL.md` and the PDF show as deleted in git and `inbox/` is untracked; the migration below records these as moves.
- `.claude/worktrees/agent-*` holds a stale copy of the whole repo from an earlier agent; it is gitignored, but it doubles every search result. Exclude it from any index (and delete it if nothing there is unmerged).

## Proposed layout

```
PLAN_INTAKE.md            ← the prompt: how to submit, how we integrate (root)
README.md  LICENSE.md  THIRD_PARTY_NOTICES.md   (stay in root)
plan/
  README.md               ← the map: what's where, status of every plan, open decisions
  DECISIONS.md            ← dated decisions with source (Kord / Steve / Claude), extracted from PLAN_V3 §3, HANDOFF, memory
  .lumeignore             ← what lume skips (raw runs, jsonl, zips)
  .lume-index/            ← gitignored
  inbox/                  ← drop zone; emptied at triage
  current/                ← the living plan
    PLAN_V3.md  BACKLOG.md  HANDOFF.md
  specs/                  ← contracts and designs we build to
    SEARCH_TOOL.md  TOOLS.md  EMBEDDINGS_PLAN.md  AGENT_HARNESS.md
    NUTS_EVENT_CONTRACT.md  NUTS_MENTION_THREAT_MODEL.md
    UI_PLAN.md  JEV_PLAN.md
    design/while-he-thinks/   (mockup extracted + the original zip)
  bench/                  ← what we measure and how
    BENCH_PLAN.md
    benchmark-plan-2026-09.pdf   (+ benchmark-plan-2026-09.md, extracted text)
    synthetic/                   (was research/bench/synthetic; the real set stays in the gitignored private/)
  research/               ← research/ as is (essays, gates/, nuts-news/, original-ferricula/, INDEX.md, REFERENCES.md)
  paper/                  ← paper/ as is
  archive/                ← superseded, kept for history
    PLAN_V2_FUSION.md (was PLAN.md)  scan/  tasks/
docs/                     ← stays: product documentation for operators
  OPERATOR_GUIDE.md  STEVE_RUNTIME.md  STEVE_RECOVERY.md  STEVE_INTEGRATION_ACCEPTANCE.md
audit/                    ← stays: evidence written by tools and exit tests (code writes here)
```

Rules behind the split:
- **`plan/` is what we think; `docs/` is how to run it; `audit/` is what we measured.** Plans cite audit files; they don't contain them. `audit/` stays where `ferricula-bench` writes it (`crates/ferricula-bench/src/main.rs:44`).
- **Specs vs current:** `current/` changes every session; `specs/` changes only when a contract changes, and each spec says its status at the top.
- **Nothing is deleted.** Superseded plans go to `archive/` with a line at the top pointing to what replaced them.
- **No private data in `plan/`.** Kord's conversations with Steve stay out (the WP-4 calibration branch rule). Datasets in `research/gates/datasets/` are authored, not private; keep it that way.

## Conventions

Every markdown file in `plan/` starts with a short header, so status and origin are searchable:

```
---
title: JEV backup gating
status: proposed | active | done | superseded
source: Kord | Steve | Claude | <agent> (who submitted it)
date: 2026-09-27
replaces: (optional path; the new file names what it replaced)
replaced_by: (set on the old file when it is superseded; links point both ways)
related: (optional paths)
---
```

Existing files get the header during migration (one mechanical pass; research essays already carry `Location/Date/Author/Status` blocks, which count). File names stay as they are, apart from the renames listed above.

## The root prompt: `PLAN_INTAKE.md`

A single prompt, written for a human or an agent, that covers submitting and integrating. Draft:

> **Submitting to the Ferricula plan**
>
> Put anything that should shape Ferricula (a note, a PDF, a design export, a benchmark idea, a correction, a transcript excerpt Steve wrote) into `plan/inbox/`. Add a one-line note file beside it (`<name>.note.md`) saying who it's from and why, or tell Claude "file this". Don't edit `plan/current/` directly with new ideas; the inbox keeps provenance.
>
> **Integrating (what Claude does at the start of each session, or when asked)**
> 1. Read every inbox item completely: PDFs page by page, zips extracted. Nothing is filed unread.
> 2. Classify each one: direction change → `current/`; contract or design → `specs/`; measurement → `bench/`; background → `research/`; paper material → `paper/`.
> 3. Keep the original and add a markdown extraction beside any PDF or zip, so lume and Steve can search the text.
> 4. Add the header (title, status, source, date). Source is who submitted it, never guessed.
> 5. Compare it with the current plan. If it conflicts (for example, the benchmark plan's ≥150 items per class against PLAN_V3 R4's 30 questions), don't silently overwrite: write the conflict into `plan/README.md` under *Open decisions*, and where it's clearly settled, update the plan and log it in `DECISIONS.md` with the date and source.
> 6. Turn actionable items into `current/BACKLOG.md` entries ranked by impact (no time estimates).
> 7. Update `plan/README.md` (the map), re-index (`scripts/plan-index.sh`), and commit on the working branch with a `plan:` prefix.
> 8. Report to Kord: what came in, where it went, what conflicts need his call.
>
> **Searching the plan**
> - Shell: `lume search "<query>" --db plan/.lume-index` (BM25), `lume section …` to open a hit.
> - Steve: plan documents marked for him are ingested into his document memory; he searches them with `search_documents` and cites `[doc …§…]`.
>
> **Never:** put secrets, tokens or Kord's private conversations in `plan/`; delete anything (archive instead); record a decision Kord didn't make.

## Search with lume

**Shell index (for us and for agents).** lume indexes `.md`, `.pdf`, `.txt`, `.html` and code, skips `.git`, `target`, `.venv` and its own index, and reads a `.lumeignore` (one name or relative path per line) (`crates/ferricula-search/src/main.rs:508-570`). It does not index `.json`/`.jsonl`/`.zip`, which is right for datasets and run logs.
- Build: `lume index plan --db plan/.lume-index` (add `-s` for the semantic layer through shivvr on `:8085`).
- Refresh after changes: `lume index update plan --db plan/.lume-index`.
- `plan/.lumeignore`: `research/gates/runs`, `design/while-he-thinks/vendor`, `design/while-he-thinks/support.js`.
- `scripts/plan-index.sh` wraps both and is what the intake prompt calls. `.lume-index/` is gitignored.

**Steve's index (so he can search the plan himself).** His `search_documents` runs over his document store (also lume BM25). A script, `scripts/plan-to-steve.sh`, ingests chosen files (`POST /documents`, `kind: text`, with a note saying where it came from). Caveats:
- doc ids are content hashes, so every edit makes a new document; ingest only settled files (`status: active` or `done`), and when a file changes, ingest the new version and have Steve mark the old one superseded with `mark_disputed`… which only works on memories, not documents. Until documents get a supersedes link, re-ingest sparingly, and list what he holds in `plan/README.md`.
- Only files meant for him: plans, specs, the benchmark plan, the paper. Not raw research runs.
- He already holds the operator guide (`2b7d502539761a31`) and the benchmark plan PDF (`e1046be544adedb3`).

## Migration steps

1. **Restore** `THIRD_PARTY_NOTICES.md` in the root.
2. **Move with history** (`git mv`, one commit, no content changes): the layout above. `inbox/` items move to their homes (the mockup zip → `specs/design/while-he-thinks/`, extracted beside it).
3. **Fix references** (second commit):
   - Code defaults: `crates/ferricula-bench/src/main.rs:53` (`research/gates/datasets` → `plan/research/gates/datasets`), `:79` (`research/bench/synthetic/queries.json` → `plan/bench/steve-recall-queries.json`); check `research/gates/harness/*.py` and `*.mjs` for relative paths.
   - Doc comments naming paths: `ferricula-gates/src/ollaya.rs` (CARD-v3), `ferricula-server/src/chat_tools.rs` (`docs/TOOLS.md`), and the other files a `grep -rE '(research|paper|docs)/'` lists in `crates/` and `README.md`.
   - Cross-links inside the moved markdown (relative links between `docs/`, `research/`, `paper/`).
   - Claude's memory: `start-here` points at `docs/HANDOFF.md` → `plan/current/HANDOFF.md`.
4. **Headers and map:** add headers; write `plan/README.md` (every file, its status, its source) and `plan/DECISIONS.md`.
5. **Root prompt:** add `PLAN_INTAKE.md`; link it from `README.md`.
6. **Index:** add `scripts/plan-index.sh`, `.lumeignore`, gitignore `plan/.lume-index/`; build the index.
7. **Steve:** add `scripts/plan-to-steve.sh`; ingest `current/PLAN_V3.md`, `current/BACKLOG.md`, `specs/*` and `paper/WHITEPAPER_V2.md`, and tell him they're there.
8. **First triage run:** process whatever is in `plan/inbox/` using the prompt, as its own test.

## Exit test

- `cargo test --workspace` and `docker build` pass (the notices file and bench defaults are the two things that break otherwise).
- `git log --follow` shows history for a moved file (e.g. `plan/current/PLAN_V3.md`).
- `grep -rE '(^|[^/])(research|paper)/' crates README.md` finds no stale path; every relative link in `plan/` resolves.
- `lume search "ECE sampling noise" --db plan/.lume-index` returns the benchmark plan; `lume search "cetasika" …` returns the Abhidhamma essays.
- Steve, asked "what does our plan say about conflicted gates?", finds X9 with `search_documents` and cites it.
- A file dropped in `plan/inbox/` and triaged by the prompt ends up filed, with a header, in the map, indexed, and committed.

## Open decisions for Kord

1. Keep `docs/` for operator guides (proposed), or move everything into `plan/`?
2. `audit/` stays in the root (proposed), or moves to `plan/evidence/` with the bench default changed?
3. Name of the root prompt: `PLAN_INTAKE.md` (proposed), or something else?
4. `PLAN.md` (the v2 fusion design) → `archive/` (proposed); PLAN_V3 still calls it "the substrate plan", so it could go to `specs/` instead.
5. Which plan files Steve should hold.

## Steve's review (2026-09-27), folded in
- **Supersession points both ways:** `replaces` on the new file, `replaced_by` on the old one (header convention above), so the correction travels with the file people open.
- **The open-decisions list gets an owner and a rhythm:** the coordinator (later the manager agent) reads it at the start of every session and brings Kord anything waiting. Otherwise "it's a graveyard".
- **Abstention is a legal intake result:** "could not read this, and why" is recorded in the map instead of a guessed filing.
- **Privacy is checked mechanically at intake,** before any agent reads an item. Anything sensitive is flagged human-only and listed as "on file, not read by an agent".
- **Plan knowledge in an agent's memory is tagged and excluded from benchmark scoring,** including what Steve already holds (the operator guide, the benchmark plan, DOCUMENTS.md).
