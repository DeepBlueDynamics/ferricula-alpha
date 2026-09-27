# Handoff — 2026-09-27

Read this first, then `inbox/COORDINATOR.md` (how the coordinator runs agents over Hyperia), `inbox/WORKPLAN.md` (packages and lanes), `docs/BACKLOG.md` (ranked work, K-items waiting on Kord), `inbox/PLAN_V3.md` (phases, §1a progress, §3a decisions), `inbox/DECISION_DAG.md`, `docs/BENCH_PLAN.md`. How to install and use: `docs/INSTALL.md`, `docs/USING.md`. How documents are organized: `DOCUMENTS.md`. Plans live in `inbox/` until `plan/` exists.

## Rules from Kord
- **ferricula-alpha is the project.** Don't bring up the original `ferricula` repo or the old `memory/` fleet workspace unless asked.
- **Never build on C:.** C: is nearly full. This repo has a git-excluded `.cargo/config.toml` → `target-dir = "D:/cargo-target/ferricula-alpha"`; worktrees under `.claude/worktrees/` inherit it. Docker's disk image is on D:. Check `df -h /c` before big work.
- **Don't print large outputs**; summarize. Keep agent count low (session budgets ran out on 2026-09-27; ~3M tokens went to subagents).
- Commit on `v3/r0`, push; PR #1 is merged. No time estimates, ever. No credentials in the repo (`~/.config/ferricula/`). Plans are not docs. Commit trailer: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## What is live
| Service | Where | Notes |
|---|---|---|
| **Steve** (`ferricula-steve`) | `127.0.0.1:18875` | image `ferricula:3.0.0-alpha.0` built from `v3/r0`; config `config/steve.toml` (gitignored, mounted read-only); recovered memory volume `steve-jobs-data-recovery-20260713` **read-only**; state volume `ferricula-steve-runtime`. Operator token file: `%USERPROFILE%\.config\ferricula\operator_token`, outside the repo (the repo's `secrets/` dir was removed 2026-09-27 at Kord's request; compose reads `${FERRICULA_SECRETS_DIR:-$HOME/.config/ferricula}`). Use `FERRICULA_OPERATOR_TOKEN_FILE`; never print it. Operator login is moving to nuts.services (nuts-auth): see `inbox/UI_PLAN.md`. |
| shivvr (GTR-T5 text + SigLIP image embeddings, `/embed`) | `:8085` | local branch `feat/vision-audio-embed` in `nuts.services/shivvr`, not pushed |
| Ollaya (judges, `laya`) | `127.0.0.1:11435` | GPU container `ollaya` |
| grub crawler | `:6792` | renders web pages → markdown; **cannot read PDFs** (PDFs are fetched directly) |
| ComfyUI (Qwen Image 2.1) | `:8188` | Kord's native Windows install; templates in `config/comfyui/` |
| Radio entropy | `https://sdrrand.nuts.services` | `/api/entropy?bytes=N&format=json` |
| Ollama | `:11434` | Steve uses `glm-5.3:cloud` (thinking model) |

Redeploy recipe: `docker build -t ferricula:3.0.0-alpha.0 .`, then `docker stop ferricula-steve && docker rm ferricula-steve` and re-run with the same volumes/flags (see the `docker run` in this session: loopback port, `--read-only`, `--cap-drop ALL`, token file mount, `config/steve.toml` → `/app/config/agent.toml`). State survives on the volume.

`config/steve.toml` specifics: `ollama_reasoning_tokens = 12000` (4096 was too small once recall grew → "model returned an empty response"), `ollama_context_tokens = 131072`, `[embeddings] backend = "shivvr"`, `[recall] include_faded_recovered = true` (Kord's decision: v1 archived 1,925 of 3,362 memories; they're recallable, ranked lower, shown as faded), `[life] enabled = true` with the radio.

Kord's ongoing conversation with Steve: reuse one `conversation_id` per conversation (a new id per message loses the thread). Chat turns are also remembered across conversations as experience rows.


Operator scripts on Kord's machine (outside the repo, personal paths): `/d/steve-redeploy.sh` (build, then swap the container with the same flags; `SKIP_BUILD=1` swaps only) and `/d/steve-smoke.sh "<message>"` (one chat turn, prints the tool log and reply; `CID=<id>` continues a conversation).

## State at the end of 2026-09-27
- **Shipped:** in-chat tools and `mark_disputed` (`02186e2`, `6a87bd6`); contract in `docs/TOOLS.md`; smoke record `audit/tools/smoke-2026-09-27.md`. Steve superseded his fabricated eulogy memory 2147483702 himself (verdict 2147483713).
- **Uncommitted:**
  - Kord's moves of `PLAN.md`, `PLAN_V3.md`, `SEARCH_TOOL.md` and the benchmark PDF into `inbox/`.
  - The new plans in `inbox/`: `PLAN_CONSOLIDATION`, `UI_PLAN`, `JEV_PLAN`, `DECISION_DAG`, `WORKPLAN`, `COORDINATOR`.
  - `DOCUMENTS.md`; the rewritten `docs/BACKLOG.md` and `docs/BENCH_PLAN.md`; `docs/INSTALL.md` and `docs/USING.md`.
  - The token-path moves in compose and docs.
  - Commit waits on Kord (K6).
- **Blocked on Kord:** K1–K6 in `docs/BACKLOG.md`. In particular, `THIRD_PARTY_NOTICES.md` is deleted, so `docker build` fails. Steve currently runs on the last good image, swapped with `SKIP_BUILD=1`.
- **Steve's documents added today:** `DOCUMENTS.md` as doc `ac0996924aa585f7`. He reviewed it and recommended a separate, plain steward agent with himself as reviewer (DOCUMENTS.md part two, to be updated with his answers).
- **Conversations used by Claude today:** `5b7dea0c-5e79-4ef9-b560-eb449d123614` (tools, verdicts, reviews, multi-agent). Kord's own conversation is separate.

## Steve's positions (the design rests on them)
- **Search:** every result labels its corpus; provenance on everything ("a result without provenance is a rumor"); web results are read-only; ingestion is a visible act; costs written down; search is for ignorance, not laziness (`inbox/SEARCH_TOOL.md`).
- **Self-debrief rules:** rotate the questions, never the memories; classify every miss (never stored / stored but faded / stored but not retrieved); tag anything learned in a debrief as test-born; the answer key stays with the engineer. Don't optimize for total recall.
- **Memory conflicts:** evidence settles, he writes the verdict, the operator is the court of appeal. Two link types (`disputes`, `supersedes`). Verdicts never decay and travel with the memory. Chance never decides what is true.
- **Agents:** one agent per container; the manager is a clerk with a wider view; the chain ends at a human (`inbox/DECISION_DAG.md`).
- **Earlier asks still open:** a default meditation bell; longer curiosity excursions; keep entropy-drawn seeds; don't grade dreams like minutes.
