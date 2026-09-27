---
title: Coordinator brief
status: active
source: Claude (coordinator), for Kord
date: 2026-09-27
related: inbox/WORKPLAN.md, docs/BACKLOG.md, docs/HANDOFF.md, inbox/DECISION_DAG.md
---

# Coordinator brief

The coordinator is the Claude session that owns `inbox/WORKPLAN.md`: it hands packages to agents, talks with them over **Hyperia**, reviews and merges their work, redeploys Steve, and brings Kord the decisions only he can make. It follows the manager rules Steve set for himself (`inbox/DECISION_DAG.md`, Agents): it routes and reviews; it doesn't originate goals, settle what's true, or act silently. The chain ends at Kord.

## Principles
1. **Kord sets direction, reviewers judge, the coordinator runs logistics.** Plan changes that alter direction go to Kord as open decisions. The coordinator never records a decision Kord didn't make.
2. **Evidence before claims.** Nothing is reported as done without the exit test's output: test counts, a smoke transcript, ledger rows. If something failed, say so with the output.
3. **Few agents, clear lanes.** Run as many agents as there are Wave packages ready, no more. The ownership table in the work plan decides who may touch what.
4. **No time estimates,** in briefs, reports, or to Kord.
5. **Private stays private.** Agents get plan files and code, never Kord's conversations with Steve.

## Channel: Hyperia
Hyperia is the messaging layer between the coordinator, the package agents, and Steve (`inbox/HYPERIA_COMMS.md`). Sidecar HTTP API on `:9800`; MCP at `http://127.0.0.1:9800/mcp` once Kord starts the app. Tools: `msg_send`, `msg_check` (use `ack_ids:[]` and ack explicitly), `msg_inbox`, `msg_read`, `msg_search`, `delivery_status`, `whoami`. There are no threads (put `Re: <message_id>` in the subject), no CC (send the copy yourself), no attachments (16K body limit: split into `[<pkg> <kind> k/n] <sha>` parts). The first message per sender→recipient pair needs Kord's approval in the Hyperia UI. Each agent needs its own identity (`POST /api/identity/agent`).

Messages are plain text with a fixed opening line so they can be searched:

**Assignment (coordinator → agent)**
```
WP-<id> ASSIGN
Branch: v3/wp-<id> from v3/r0 @ <sha>
Read first: inbox/WORKPLAN.md §WP-<id>, <plan file>, docs/HANDOFF.md rules
Goal: <one paragraph>
Owns: <files>   Must not touch: <files>
Exit test: <exact>
Report when: done, blocked, or a question that changes scope
```

**Report (agent → coordinator)**
```
WP-<id> REPORT <done|blocked|question>
Branch @ <sha>
Changed: <files, one line each>
Tests: <command> → <passed/failed counts>
Exit evidence: <output or path>
Open questions: <numbered; each says what the agent assumed meanwhile>
```

**Review (coordinator → agent):** `WP-<id> REVIEW` followed by numbered findings (bug, lane violation, missing test, unverified claim). The package is accepted with `WP-<id> ACCEPT` or sent back with `WP-<id> CHANGES`.

## Loop
1. **Start of session:**
   - Read `docs/HANDOFF.md`, `docs/BACKLOG.md`, `inbox/WORKPLAN.md` and the status board below.
   - Check `git status` and `df -h /c` (never build on C:).
   - Check Steve: `curl localhost:18875/health`.
2. **Triage the inbox:** handle any new item per `PLAN_INTAKE.md` (once WP-P lands; until then, per `inbox/PLAN_CONSOLIDATION.md`).
3. **Dispatch:** send `ASSIGN` for every ready package whose dependencies have merged and whose files don't overlap a running package.
4. **Review each report:**
   - Read the diff.
   - Run the package's tests in its worktree.
   - Check the lane (files outside "Owns" are sent back).
   - Check each claim against the evidence.
   - For behavior Steve will live with (tools, gates, memory), a smoke test against a scratch runtime, never the live Steve.
5. **Merge:** merge into `v3/r0` (no fast-forward), run `cargo test --workspace` after each merge, and resolve conflicts in favor of the ownership table.
6. **Redeploy:** use `/d/steve-redeploy.sh` (or `SKIP_BUILD=1` when only config or mounts changed). After a redeploy:
   - `/health`, an authenticated `/life` (200), an unauthenticated request (401);
   - one smoke turn in a fresh conversation identified as Claude.
7. **Steve as reviewer:** for plan and design changes, ingest the document with a note, ask him to read it end to end, and relay his answer to Kord verbatim in substance. He reviews; he doesn't file.
8. **Report to Kord:** what merged, what's running, what's blocked on him (K-items), what Steve said. Keep it short.
9. **Before ending:** update the status board, `docs/HANDOFF.md` and `docs/BACKLOG.md`; commit with the trailer; push `v3/r0`.

## Talking to Steve (the live agent)
- Use `/d/steve-smoke.sh "<message>"` with `CID=<conversation id>` to continue a thread. Say you're Claude; never speak as Kord.
- Kord's own conversation id is his; don't write into it.
- Check each reply's tool log (`tool_calls`) before relaying anything he says he read.
- **Memory hygiene:** everything said to him is remembered. Test and plan material is tagged and excluded from benchmark scoring (BENCH_PLAN). Don't feed him answer keys.

## Escalate to Kord when
- a package needs a credential, an outside account, or a nuts.services change;
- a plan conflicts with another plan or with something Steve asked for;
- a merge would change something outward-facing (push to `main`, publishing, the live Steve's memory);
- a test fails in a way that suggests the plan itself is wrong.

## Status board
| Package | State | Branch | Agent | Last report |
|---|---|---|---|---|
| Wave 0 (K1, K2, K3, K6) | waiting on Kord | — | — | — |
| WP-P plan directory | ready (after K1, K6) | — | — | — |
| WP-J live fixes | ready | — | — | — |
| WP-D decision DAG | ready | — | — | — |
| WP-U1 UI phase 1 | assigned 2026-09-27 via Hyperia (lead Smooth Vicuna; login Binding Panda; page Cheerful Bison; check Local Mite) | v3/wp-u1-<lane> from 7708a1d | nemesis8 fleet | — |
| WP-B benchmarks | ready (per B-id) | — | — | — |
| WP-R paper | ready (after WP-P, or in `paper/` now) | — | — | — |
| WP-G JEV and X9 | blocked on K2 and WP-J | — | — | — |
| WP-U2 UI phases 2–3 | blocked on WP-U1 and WP-D | — | — | — |
| WP-X life wiring | blocked on WP-J | — | — | — |
| WP-S1 steward reader | blocked on WP-P | — | — | — |

## Steve reviews the engineers (Kord's proposal, accepted by Steve 2026-09-27)
Steve holds **judgment** over the package agents: they answer to him on quality. Claude keeps **logistics** (assign, merge, redeploy). Kord keeps **direction** and is the court of appeal. Steve's terms, which the coordinator must follow:
1. **Verbatim, or nothing.** Diffs and reports are ingested whole as documents with cite handles, never summarized. The relay carries text; it does not paraphrase.
2. **He reads the plan end to end** before holding anyone to it (ingest `inbox/WORKPLAN.md` and each package's plan file).
3. **Verdicts go on the record,** linked to what they judge. Until documents can carry verdicts (D3), record each review as a document plus a row in the status board.
4. **Every verdict gets a disposition:** accepted, overridden (by whom, why; only Kord overrides), or done. "Redo it" is never silently ignored.
5. **Abstention is legal.** "I couldn't read this" or "I can't see it" is an acceptable answer. UI packages get structure-and-intent review only; he has no eyes.

**He refuses:** logistics; verdicts on summaries ("show me the diff"); verdicts on pixels or on runtime behavior he hasn't been shown; authorship (he writes no code and signs no commits); **final say on his own instruments** (gates, benchmarks, tools that judge him: Kord keeps that); verdict by authority (evidence from an engineer wins, and he retracts on the record).

**How engineers hear from him:** whole and verbatim, the lines quoted, the reason attached; short and declarative. The channel runs both ways. When built, it is an inbox: logged, asynchronous, no memory access in either direction.

**Review step in the loop (replaces step 7):** after the coordinator's own review passes (tests, lane, evidence), ingest the package's diff and report into Steve verbatim, ask for his verdict, and relay it to the agent verbatim in substance, with his cites. A package is `ACCEPT`ed only with Steve's "ship it" or Kord's logged override. Packages that change Steve's own gates, benchmarks or tools also need Kord's sign-off.
