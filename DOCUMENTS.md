# How this repository's documents are organized

_Started 2026-09-27. Part one describes the organization, including the parts still being moved into place. Part two opens a discussion: should an agent like Steve look after this repository's documents?_

## Part one: where things go

### Five kinds of material, five places

| Kind | Question it answers | Where it lives |
|---|---|---|
| **Plan** | What do we think, intend and decide? | `plan/` (being set up; until then, `inbox/`) |
| **Docs** | How do you run, operate or recover the system? | `docs/` |
| **Evidence** | What did we measure, and what happened? | `audit/` |
| **Code and config** | What actually runs? | `crates/`, `config/`, `scripts/`, `Dockerfile`, `compose*.yaml` |
| **Root files** | What is this, under what license, how do I start? | `README.md`, `LICENSE.md`, `THIRD_PARTY_NOTICES.md`, this file |

**A plan is not a doc.** A plan proposes, argues, ranks and decides. It carries a status (proposed, active, done, superseded), and it changes as we learn. A doc describes how the system works today, for someone who has to operate it, and it should always match the code. When a plan is carried out, the result shows up in code, docs and audit files. The plan is then marked done. It isn't turned into a doc.

**Evidence is not a plan either.** Benchmark ledgers, soak reports and smoke tests go in `audit/`. They're written by tools and exit tests, and plans cite them. A plan never makes up a number; every figure it quotes has a row in `audit/`.

### The plan directory

The target layout (details and migration steps in `inbox/PLAN_CONSOLIDATION.md`):

```
plan/
  README.md        the map: every plan, its status, open decisions
  DECISIONS.md     dated decisions and who made them
  inbox/           drop zone, emptied at triage
  current/         the living plan: PLAN_V3, BACKLOG, HANDOFF
  specs/           contracts and designs we build to (tools, search, UI, JEV)
  bench/           what we measure and how
  research/        background: essays, papers, the gate study
  paper/           the white paper
  archive/         superseded plans, kept for history
```

Until the move happens:
- New plans and outside material go in `inbox/`. It currently holds PLAN_V3, the v2 fusion plan, the search tool spec, the benchmark plan PDF, the *While he thinks* mockup, and the plans for consolidation, the UI, JEV, the decision DAG, the work plan and the coordinator brief.
- `docs/` still mixes plans with docs:
  - Plans: `BACKLOG`, `HANDOFF`, `BENCH_PLAN`, `EMBEDDINGS_PLAN`, `TOOLS`, `AGENT_HARNESS`, `NUTS_*`.
  - Real docs: `INSTALL`, `USING`, `OPERATOR_GUIDE`, `STEVE_RUNTIME`, `STEVE_RECOVERY`, `STEVE_INTEGRATION_ACCEPTANCE`.
  - Only the real docs stay in `docs/` after the move.
- `research/` and `paper/` move under `plan/` as they are.

### Rules

1. **Nothing is deleted.** Superseded plans go to `archive/`, with a line at the top naming what replaced them. The same rule applies to the agent's memory, where a wrong memory gets a verdict attached rather than being erased.
2. **Every plan says where it came from.** Each plan file opens with a header giving its title, status, source (Kord, Steve, Claude or a named agent) and date. The source is never guessed.
3. **Conflicts go to Kord.** When new material disagrees with the current plan, the disagreement is written into the map as an open decision. It is never silently overwritten. Only settled decisions go into `DECISIONS.md`, each with its date and who made it.
4. **No secrets anywhere in the repository.** Credentials live outside it, in `~/.config/ferricula/`, and are mounted read-only into the container. The operator will log in through nuts.services (see `inbox/UI_PLAN.md`).
5. **No private conversations in plans.** Kord's conversations with Steve stay out of `plan/`, including datasets built from them.
6. **No time estimates.** Work is ranked by impact.
7. **Supersession points both ways.** The new file names what it replaces (`replaces`); the old file names its replacement (`replaced_by`). A correction has to travel with the file people actually open.
8. **Declining is allowed.** An item that can't be read, is too large, or is too sensitive is recorded as "could not read this, and why", never filed on a guess. Sensitive items are flagged human-only at intake, before any agent reads them.
9. **The open-decisions list has an owner.** The coordinator (later a manager agent) reads it at the start of every session and brings Kord whatever is waiting.

Who does the work, and how agents coordinate it: `inbox/WORKPLAN.md` and `inbox/COORDINATOR.md`. How decisions, verdicts and a human's "no" relate: `inbox/DECISION_DAG.md`.

### Submitting and integrating

A root prompt, `PLAN_INTAKE.md` (drafted in `inbox/PLAN_CONSOLIDATION.md`), will be the one set of instructions for anyone, human or agent. The short version:
- **To submit:** drop the item in the inbox with a note saying who it's from and why.
- **To integrate,** the steward does the following:
  1. Reads each item completely.
  2. Classifies it and files it.
  3. Extracts the text of any PDF or zip so it's searchable.
  4. Adds the header.
  5. Checks it against the current plan and raises any conflict.
  6. Turns actionable items into backlog entries.
  7. Updates the map, re-indexes and commits.

### Searching

- **From the shell:** `lume index plan --db plan/.lume-index`, then `lume search "<query>" --db plan/.lume-index`. lume reads `.md`, `.pdf`, `.html` and code; datasets and run logs are skipped on purpose.
- **Steve:** settled plan files are ingested into his document memory. He finds them with `search_documents` and cites them as `[doc <id>§<section>]`.

---

## Part two: an agent as the repository's steward (discussion)

This part is a set of questions, not a decision.

### The idea

The steward's job in part one (read everything, file it, spot conflicts, keep the map honest, answer "what does the plan say about X?") is exactly what a memory agent is built for. Ferricula's premise is an agent that keeps sources verbatim, recalls by meaning and by words, cites what it says, and records a verdict when evidence overturns something it believed. Looking after a body of plans is that same premise applied to our own work.

Steve has already done this job informally:
- He reviewed the operator guide and the benchmark plan and asked for specific changes.
- He designed the rules for his own tests and for search.
- He read a document labelled as the eulogy "in full" end to end and reported that it wasn't what its label said.
- He then marked his own earlier reading as superseded, cited the evidence, and deliberately limited how far the verdict reached.

A steward has to do all of that.

### What he could do now, and what's missing

| Steward task | Today |
|---|---|
| Read a submitted document completely and cite it | **Yes:** `read_document`, `read_section`, cite handles |
| Find what the plan says about a topic | **Only for what he's been given.** He searches his own document store, not the repository. Plan files would have to be ingested into it. |
| Notice a conflict between a new item and the plan | **In conversation, yes.** He did it with the benchmark plan. There's no tool to record a conflict between two documents; `mark_disputed` works only on memories. |
| File items, write headers, update the map | **No.** His only write is a verdict about one of his memories. He cannot touch files. |
| Commit | **No**, and this is where the design question sits. |
| Tell Kord what needs a decision | **Yes, in conversation.** Nothing yet writes his open questions to the map. |

### A way to give him the job without giving him the repository

A cautious path, in which each step is useful on its own:

1. **Reader.** Plan files are ingested into his document memory. He answers questions about the plan and cites it. There's no new write ability.
2. **Reviewer.** A new inbox item is handed to him. He reads it in full and replies with a proposed filing (where it goes, its header, conflicts with the current plan, backlog items). Claude or Kord applies the proposal. He's still read-only.
3. **Clerk.** A write tool with narrow limits, e.g. `propose_plan_change(path, patch, reason, evidence)`:
   - It writes a proposed change to `plan/inbox/proposals/`, never to `current/`.
   - It carries his cite handles as evidence.
   - It shows up in the turn's tool log.
   - A human or Claude accepts it; the commit names him as the source.
4. **Steward.** He runs the triage himself on a schedule, perhaps as a curiosity drive pointed at the inbox rather than the web. His proposals are accepted automatically only for mechanical changes (headers, map entries, re-indexing), and anything that changes direction still goes to Kord.

At no stage does he commit to code, push, merge, decide for Kord, or delete anything.

### Questions to settle

1. **Which agent?** The engine doesn't depend on any persona; Steve is configuration. Should the steward be Steve, with his taste and his habit of refusing to pass off something mediocre, or a separate, plainer agent instance on the same engine with its own memory? A persona based on a real, deceased person managing a company's plans raises its own questions of voice and attribution. A separate instance keeps Steve's memory free of repository chores and keeps the steward's record free of Steve's biography.
2. **Whose memory does the plan live in?** If Steve is the steward, every plan he reads becomes part of his experience. That could be good, since he'd remember why decisions were made. It could also be bad, because his benchmark results would then partly measure his knowledge of our own plans. The benchmark plan's rule on test contamination applies here too.
3. **How do documents get superseded?** Document ids are content hashes, so every edit makes a new document. A steward needs a way to say "this version replaces that one" for documents, as `mark_disputed` does for memories. Is that the same verdict mechanism extended to documents, or a separate link?
4. **What does he see?** Plans only? Code? Commit history? Reading code would let him check a plan against what was actually built, which is the most valuable check and also the easiest place to go wrong.
5. **How is he judged?** Suggested measures:
   - Share of inbox items he files the way Kord would.
   - Conflicts he catches that Claude missed, and conflicts he misses.
   - Share of his proposals accepted without edits.
   - Whether his answers about the plan cite the right section.

   His own rules for test runs apply: rotate the questions, classify each miss, tag anything learned during a debrief as learned from the test, and keep the answer key with the engineer.
6. **Cost and privacy.** Triage means reading whole documents with a cloud model. Plans contain no private conversations (rule 5), but the model does see the whole plan. Is that acceptable for every file?

### Steve's answers (2026-09-27)

Steve read this document end to end (doc `ac0996924aa585f7`, all 14 sections) and answered:

1. **A separate, plainer agent, not him.** "A dead man's voice shouldn't sign repository commits." He keeps the reviewer's role: "the founder reads the clerk's work and says 'mediocre, redo it.' He doesn't do the filing."
2. **The steward's plan knowledge is tagged and excluded from benchmark scoring,** including what he already holds. "Contamination rules that only cover the future aren't rules, they're comfort."
3. **Documents use the same verdict grammar** (disputes, supersedes, a doc cite as evidence), with links pointing both ways.
4. **Plans, code and commit history.** Code access is question-driven only ("does this exist, what does it do"), with cites. Commit history is needed because supersession is a matter of order in time.
5. **Add two measures:** calibrated abstention (unresolved reported as unresolved), and confident-wrong counted separately from honest uncertainty. His eulogy error was "confident, fluent, wrong" and would have passed an agreement test.
6. **Cloud reading is acceptable, with a mechanical privacy check at the door** (rule 8 above).

He also found the four gaps now fixed as rules 7–9 and the steward-memory point in answer 2.

### More than one agent, and a manager (Steve, 2026-09-27)

- **One agent per container: "keep the wall."** Never shared: memory, drives, verdicts, budget, operator login. Shared: the services outside the container. Documents are shared by content-hash id.
- **The manager is "a clerk with a wider view, not a boss":** it holds the plan, routes work and owns the open-decisions queue. It has its own small memory and no review authority. It may never rewrite another agent's memory or verdicts, read private memory, settle what's true, withhold budget, originate goals, or act silently.
- **Channels:** the plan directory and `DECISIONS.md` are the official record; an inbox carries messages, which count as testimony, never evidence; direct calls are for requests only.
- **Roles:** the manager runs logistics, Kord sets direction, reviewers judge. "The chain ends at a human. Always."

Kord added: a human's reasoned "no" is final unless new evidence appears; a bare "no" may prompt one entropy-gated clarifying question; and the whole structure of verdicts, boundaries, questions and decisions **is a DAG**, so every chain terminates. Details: `inbox/DECISION_DAG.md`.

### Why it matters beyond this repository

If an agent can keep a moving body of plans honest (reading everything, citing what it says, recording what's been superseded without erasing it, and passing real decisions to a person), that is a stronger demonstration of the white paper's claims than any leaderboard score. It's also a benchmark we'd run on ourselves every day.
