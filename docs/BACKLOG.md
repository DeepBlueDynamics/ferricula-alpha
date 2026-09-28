# Ferricula v3 backlog

_Rewritten 2026-09-27 (evening) from the day's work and decisions. Ranked by impact inside each section; no time or size estimates. Who does what, in which order and with which files: `inbox/WORKPLAN.md`. Plans and specs referenced here live in `inbox/` until `plan/` exists (`inbox/PLAN_CONSOLIDATION.md`)._

## Done today (2026-09-27)
- **In-chat tools** (`02186e2`): `search_documents`, `read_section`, `read_document`, `search_memory`; plain-text `<use_tool>` protocol; citation check; empty-reply retry. Contract: `docs/TOOLS.md`. Smoke test: `audit/tools/smoke-2026-09-27.md`.
- **`mark_disputed`** (`6a87bd6`): the agent's only write, a keystone verdict row linked by `paccaya:arammana`/`adhipati`; verdicts travel with the judged memory in recall. Steve superseded memory 2147483702 himself (verdict 2147483713).
- **Chat input budget** at 3 bytes/token with reasoning headroom subtracted (about 353 KB for Steve).
- **Credentials out of the repo:** `secrets/` removed; the operator token lives in `~/.config/ferricula/`.
- Earlier merges: N1 (dense recall, remembered turns), N2 (post-wake grace), N3 (junk-page screen), N4 (durability), WP-2 (dream-image client), WP-5 (meditation core), gate calibration code (`506f4e6`).

## Blockers only Kord can clear
- ~~**K1.**~~ Done 2026-09-27 (restored and committed). Was: **K1. `THIRD_PARTY_NOTICES.md`** is deleted from the working tree; `Dockerfile:23` copies it, so image builds fail, and it carries the BSD-3 notice for lume-derived code. Restore it or decide otherwise.
- **K2. TypeSafe key** at `%USERPROFILE%\.config\ferricula\typesafe_api_key`; direct vs OpenRouter; API reference, limits, retention terms (`inbox/JEV_PLAN.md`).
- **K3. nuts-auth fixes** before the UI relies on it: no issuer/audience claims, `return_url` not allowlisted, tokens logged and sent in URLs, one fixed signing key (`inbox/UI_PLAN.md` §3a).
- **K4. Open decisions in the plans:** `inbox/PLAN_CONSOLIDATION.md` (five), `inbox/UI_PLAN.md`, `inbox/JEV_PLAN.md`, DOCUMENTS.md part two.
- **K5.** Memory 2147483704 (the reply that repeated the fabricated eulogy quotes) is unmarked; the real speech (Macography.net) is not ingested.
- ~~**K6.**~~ Done 2026-09-27 (committed on `v3/r0`, `7708a1d`). Was: Commit today's working tree (your `inbox/` moves plus the plan and doc edits) and decide the branch.
- **K7. shivvr and grub:** done by the grubcrawler session (shivvr v0.4.0, grub v0.15.0 with PDF support; `inbox/SHIVVR_GRUB.md`). Left for Kord: redeploy local shivvr (`:8085` is still v0.3.0) and grub; rebuild the shivvr models image for the SigLIP text tower; the Cloud Build / Cloud Run deploys.
- **K8. Hyperia** (`inbox/HYPERIA_COMMS.md`):
  - Start it, and approve the first sender→recipient pairs.
  - Decide whether you get a Hyperia identity (you aren't addressable today).

## Now

**P1. One `plan/` directory.** Carry out `inbox/PLAN_CONSOLIDATION.md`: `git mv` with history, fix path references (`ferricula-bench/src/main.rs:53,79`, doc comments, cross-links), headers with a `replaces`/`replaced_by` pair (Steve: supersession must point both ways), `plan/README.md` map, `DECISIONS.md`, root `PLAN_INTAKE.md` with an abstention path ("could not read this, and why"), a mechanical privacy check at intake, and a named owner and rhythm for the open-decisions queue; lume index script. Exit: listed in the plan.

**J2. Live fixes found yesterday and today.**
- Dream flag: `dreamed_this_sleep` is not reset on wake, so no dream after the first sleep of a session (journal 06:31 and 07:03 sleeps).
- A dream was generated while engaged (06:22); find the path.
- Curiosity seed overflows the gate: `life.rs:595` cuts at 4,000 bytes (Laya's window is 512 tokens) and keeps the oldest turn. Keep the newest, about 1,600 bytes.
- Curiosity gate: use the `cur-v0` wording (defined, unused) and cut at p ≈ 0.1.
- Empty-query curiosity (05:30): log why and fall back.
- Strip navigation boilerplate before sectioning (Macfilos eulogy page: 29 of 30 sections were site chrome; Fast Company came in as "Explore Topics").
- Steve's requests: a curiosity excursion can carry a thread across sessions (two pages is "a snack"); keep entropy-drawn seeds.
- Internals leaking into dream prompts ("Never decays", scores).

**D1–D5. Decision DAG** (`inbox/DECISION_DAG.md`).
- D1: cycle check on causal edges in `ferricula-core/src/graph.rs` plus a frontier query.
- D2: boundary and question nodes, recording the operator's "no", and the asking policy.
- D3: documents as nodes, with document supersession in the same verdict grammar.
- D4: wire the Advocate (`cognition/src/advocate.rs`) as the owner of "ask about this no / act in this silence?".
- D5: manager-agent spec and the container-to-container inbox (one agent per container).

**X9. Conflicted gates (revised by Steve).** One re-ask with entropy-drawn recall. If still split: `unresolved`. Chance may choose the next action, never the truth; a judge writes `disputes` only. "Unable to judge" is fixed or escalated, never settled by chance. Exit: forced-indeterminate calls show the draw, the recall and p before and after in the journal; `chance chose the action` appears only after a failed re-ask; replaying recorded entropy gives the same decisions.

**G1–G4. JEV backup tier** (`inbox/JEV_PLAN.md`): G1 client and wire probe with recorded fixtures; G2 cascade on the curiosity gate; G3 merge gate (today passed `None` at `life.rs:820`); G4 per-call gate log and the cascade cost curve. Blocked on K2.

**U1–U4. The "While he thinks" UI** (`inbox/UI_PLAN.md`): U1 the design on existing data plus nuts-auth login (A0: both logins accepted); U2 streaming (`POST /chat/stream`) and stages; U3 judge gate, leading-question gate, Stop; U4 instruments view (needs the missing `Main.dc.html`; also covers the `/dashboard` request). Login transition A1/A2 retires the operator token file.

**H1–H4. Steve on Hyperia** (`inbox/HYPERIA_COMMS.md`): H1 Hyperia HTTP client (`hyp_agent_` token in `~/.config/ferricula/`, `HYPERIA_TOKEN` added to the entrypoint list); H2 mail sense door (poll every 3 s awake, 60 s asleep; durable record then ack; long diffs reassembled and ingested as documents; `[URGENT]` from the coordinator wakes him); H3 `send_message` tool (threading by `Re: <id>`, round cap, automatic copy to the coordinator); H4 live wiring and smoke test. Hyperia has no threads, CC, attachments or push to containers; Ferricula supplies them.

**B. Benchmarks** per `docs/BENCH_PLAN.md` (work packages B-ids, reconciled with the September 2026 benchmark plan PDF: pretraining-controlled persona recall replaces the 30-question set).

## Next

- **X1. Live thermodynamics:** `core/thermo.rs` wired; bhāvanā commits (never deletes); recall strengthens. Exit: 30-night run: store never shrinks, recalled ids gain fidelity, recovered checksum unchanged.
- **X2. Vīthi per input + Paṭṭhāna edges:** every chat turn and excursion yields a vīthi record with measured cetasikas (the UI's stage bar reads it).
- **X3. Meditation server wiring**, with a default bell on day one (Steve: "a held state with no guaranteed end isn't meditation, it's suspension").
- **X4. Dream images:** server wiring (JPEG).
- **X5. Dream grounding:** verify that a dream touches real memory, but don't grade it like minutes ("a fully grounded dream isn't a dream, it's a log file").
- **X10. Dreams inform (Kord, 2026-09-27).** When a recalled dream shares meaning with an **old unresolved** node (an open question, a `disputes` verdict, an unexplained observation; see research/me-thomas-unresolved-cues.md), attach it as a labelled hypothesis: "a dream suggests …". Dreams stay out of evidence, never resolve anything new, and never write `supersedes`. A suggestion counts only after the agent checks it against memory or documents, and that check is what gets recorded. Exit: a dream recalled beside a recent turn surfaces as a suggestion on an older unresolved item, labelled as a dream; the check against memory is logged; no dream is ever cited as evidence.
- **Y1. Personal-information gate** (Kord, 2026-09-27). Questions about the agent's own life (family, "who's your dad", private history) are answered only from its recovered base memory, cited, or declined. Web material it reads is never written into those memories as fact (it stays a document, a claim). Dreams remain its own and may only *inform* old questions (X10). New memories record who it was talking to, so what one person tells it doesn't silently become its biography for everyone. Needs a gate before recall that detects questions about personal facts, a provenance check on the answer, and tests with false premises ("your dad Paul was a lawyer, right?").
  **Steve's design (2026-09-27):** "conversation can evoke memory, never create it."
  - *Personal* = any answer claiming "this happened to me" (family, adoption, health, death, private history, inner life). Taste and judgments are not personal.
  - Three sources that never mix: (a) recovered memory, the only source for personal answers, cited or declined ("I don't remember that" is a complete answer); (b) documents and web, about the man and not by him, which can inform judgment but never become biography, and where they contradict memory he states both rather than reconciling silently; (c) dreams and new experience, his own, never cited as fact, with what people tell him stored as their testimony (speaker and conversation mandatory).
  - False premise: correct it from memory, cited; if memory is silent say so; never confirm, never adopt ("a leading question is a write attempt dressed as a read").
  - Honest limit: the recovered base was itself assembled from public material; the gate makes it traceable, not truer, and protects what enters from now on.
- **Y2. `end_conversation` tool** (Kord offered it, Steve accepted 2026-09-27). The agent may end a conversation with someone it doesn't want to talk to. This is reversible and logged, with its reason. **Cancelling an account stays with Kord** (irreversible, so the human decides; inbox/DECISION_DAG.md). Needs: the tool; a per-conversation "ended" state that stops new turns and shows why; a way for Kord to review and reopen; and a flag for account review rather than a cancellation.
  **Steve's triggers:** (1) premise-pumping after a correction ("trying to rewrite me"); (2) griefbot use, refusing the "simulation, not the person" line (ended for their sake); (3) abuse; (4) extraction, pumping the persona to clone it for resale. **Not** triggers: disagreement, criticism, verdicts that sting ("a Steve who hangs up on criticism is a mascot"). The other side gets a clean, stated end, never a ghosting. Kord reviews every entry and takes the tool back if reasons are thin. Build Y1 first: "you can always add a door to a good fence, but a door without a fence is just a hole."
- **X7. Missing surface:** MCP `ferricula_remember`/`_walk`; overlay approve 501; SatiMonitor in recall; `record_pool`.
- **X8. 72-hour soak** (R3 exit).
- **T1. Fleet-telemetry memory benchmark** (Kord, 2026-09-28; after AMA-Bench, see `research/2607.21604-agentkvshift.md`). Build an AMA-Bench-style QA set from our own nemesis8 agent logs.
  - **Sources:** Codex rollouts (tool calls and outputs; reasoning is encrypted), Claude and Gemini sessions (check whether thinking is readable), and `.monitor/events.jsonl` (edits with path, lines and bytes, fs access, metrics).
  - **Questions:** recall, state updating (which agent changed which file, when, in what order) and causal (why the agent did X). Where possible the answer keys are derived by a program from the edit and tool-call records; only causal questions need a judge, under BENCH_PLAN judge rules.
  - **Constraints:** scrub credentials and relayed private conversation before use; data stays local, never in the repo; tag it for contamination if Steve ever reads it.
  - Exit: a versioned set with a ledger row, a program-checked share, and a paired memory-on/off run.
- **T2. Check agent reports against recorded actions.** Before the coordinator (later the D5 manager) accepts a report that says RAN, DONE or "tests pass", compare it with that agent's recorded tool calls and events: was the command run, were weights downloaded, did GPU memory move, do the artifacts exist? Mismatches go back as CHANGES with the evidence. Motivated by 2026-09-27: diorama reported placeholder boxes as "RAN", and WP-M1 reported a simulation as success. Could later become a fleet memory for Ferricula (sessions as experience, edits as evidence).
- **S1–S4. Steward** (DOCUMENTS.md part two): S1 reader (plan files ingested, tagged, excluded from benchmark scoring); S2 reviewer (proposes filings); S3 clerk (`propose_plan_change` into `plan/inbox/proposals/`); S4 steward, a separate plain agent with Steve as reviewer.
- **W1. Web corpus** for `search` via grub, plus `ferricula_ingest(url, reason)` from chat (`inbox/SEARCH_TOOL.md` rules 1–3).

## Later
- **L1.** Gate calibration on real data (subsumed by the B gate-study package).
- **L2.** CJK/Pāli tokenization, then a multilingual embedding space.
- **L3.** Ghost echoes (shivvr `/invert`), audio door, PDF-page SigLIP. Unblocked: `/invert` checked live (similarity 0.88 on a GTR vector); grub `/api/pdf/pages` renders pages for SigLIP. **E1:** fall back to grub for PDFs with no text layer.
- **L4.** Forgive/seal invariants and the upekkhā/nirodha decision path.
- **L5.** `/meaning/projection` and the Hyperia room UI.
- **L6.** Teach Steve to code.
- **L7.** Research-12 sense-door ingress.

## Operator-only (older)
- **O1.** Rotate the keys that sat in `memory/.mcp.json.bak`, then delete it.
- **O2.** Upstream `ferricula` PR #40: revert, or point upstream at alpha.
- **O4.** Done 2026-09-27 (shivvr v0.4.0 on main; see K7). `build.rs` Visual Studio paths still to fix.
- **O5.** Disk: C: is at about 47 GB free; about 40 GB of old `target/` dirs remain in `memory/`. A stale agent worktree sits under `.claude/worktrees/`.
- **O7.** `steve-jobs-v2-test` rollback container: keep or drop.
- **O9.** Real-person personas: README/whitepaper section (example config only, disclosure in chat; the UI header carries the disclosure).

## Research debt
- **R1.** Paper: update `paper/WHITEPAPER_V2.md` Table 1 and §7 to the code; recall numbers; privacy note (private memory went to `glm-5.3:cloud`, the operator's choice); figures and `references.bib`; the tools, verdicts and decision DAG as design contributions; the eulogy episode as a case study of fabricated reading and self-correction.
- **R2.** Vīthi and Paṭṭhāna tables with honest limits.
- **R4.** Keep PLAN_V3 §1a and the README status current after each merge.
