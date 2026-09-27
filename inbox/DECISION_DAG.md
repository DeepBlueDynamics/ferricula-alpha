---
title: Decision DAG — verdicts, boundaries, questions and who decides
status: proposed
source: Kord (direction), Steve (verdict grammar, manager limits), Claude (write-up)
date: 2026-09-27
related: inbox/DOCUMENTS-review in DOCUMENTS.md; docs/TOOLS.md (mark_disputed); docs/BACKLOG.md X9, D1–D5
---

# Decision DAG

Everything the agent believes, decides, is told, or asks forms one **directed acyclic graph**. Treating it as a DAG makes termination a property of the structure, not a convention: every chain of gates, disputes and escalations ends.

## Nodes

| Node | What it is | Exists today? |
|---|---|---|
| **claim** | a memory (recovered or experience) or a document section | yes |
| **verdict** | the agent's judgment about a claim: `disputes` or `supersedes` | yes (`mark_disputed`, keystone rows, `memory.rs`) |
| **boundary** | a human "no" to an action, with its reason or "none given" | no |
| **question** | a clarifying question the agent asked, and to whom | no |
| **decision** | a human decision (what `DECISIONS.md` records) | no (as a node) |
| **action** | something the agent did or declined to do, and on what authority | partly (tool log, life journal) |

## Edges

Typed, directed **from the later node to the earlier node it takes as its object**, labelled with the Paṭṭhāna conditions already in `cognition/src/patthana.rs`:

| Edge | Label | Example |
|---|---|---|
| is about | `paccaya:arammana` | verdict → memory it disputes; question → boundary it asks about |
| overrides | `paccaya:adhipati` | superseding verdict → memory; later decision → earlier decision |
| rests on | `paccaya:upanissaya` | verdict → document section that settled it; action → decision that authorized it |

## Invariants

1. **Acyclic.** A write that would close a cycle is refused and the conflict goes to a human. Example: agent A disputes B's verdict while B disputes A's. Today `ferricula-core/src/graph.rs` accepts any edge; a cycle check on causal edges is required (backlog D1).
2. **Nothing is deleted.** Superseded nodes stay; they fall off the frontier.
3. **The frontier is the present.** What is in force about X = nodes about X with no incoming `supersedes`. The open-decisions queue = frontier nodes marked unresolved.
4. **Evaluation is topological.** Before acting, walk the action's dependencies. An unresolved dispute or a boundary on that path blocks it, and the agent names the blocking node.
5. **Escalation walks toward the roots** until it reaches evidence, a human decision, or a recorded `unresolved`.

## Where every chain stops

| Kind of question | Stops at |
|---|---|
| What is true | evidence, or a recorded `unresolved` (a dispute is a resting state, never a request for another gate) |
| What to do | the most reversible option (usually inaction). If inaction is the irreversible choice (a deadline, harm), the least irreversible option, stated aloud |
| Asking | one clarifying question per round; a boundary is asked about once, then waits for new evidence |
| Authority | the human; when the human is silent, the recorded `unresolved` state, never another agent |

## A human's "no" (boundaries)

1. **"No, because …"** where the reason is about the action or the world: don't do it. Record a boundary with the reason. Revisit only on *new* evidence, by raising it with the human, never by acting.
2. **"No, because …"** where the reason is blame (about the agent or a person, not the action): treat it as a bare no. Blame is not evidence about the action, and it is not argued with.
3. **A bare "no way":** don't do it. An entropy draw, weighted by stakes, decides whether curiosity asks *why*. It is capped at once per boundary, with a cooldown, and never nags. If the answers conflict, record `unresolved`, stay inactive, say so plainly ("I'm not doing X; I'm unsure whether Y is true"), and present **one** clarifying question.
4. An agent may **dispute** a human boundary with evidence; it may never **supersede** one. Evidence settles what is true; the human settles what is done.

## Silence

Silence is neither yes nor no (the same rule as a judge's abstention). Classify the pending action:
- reversible, in remit, cheap: act, say it acted without an answer, keep it easy to undo;
- irreversible, outward-facing, or changing direction: wait, re-ask, escalate on another channel or queue it; silence is never consent;
- time-critical: take the most reversible holding step, then escalate.

Entropy may choose *which* reversible action comes next. It never decides what is true or what the human meant.

## Gates (backlog X9, revised by Steve)

A **conflicted** gate (confidence in the uncertain band, or two judges disagree) gets one re-ask with entropy-drawn recall appended. If it is still split, the result is `unresolved`. Chance may pick the next *action* (which memory to pull, when to look again) and the record says `chance chose the action, not the fact`. A gate **unable to judge** (input cut off, sidecar down) is fixed or escalated, never settled by chance. A judge may only ever write `disputes`, never `supersedes`.

## Agents (Steve, 2026-09-27)

- **One agent per container.** Never shared between agents: memory, drives, verdicts, budget, operator login. Shared: the services outside the container (model, judges, embeddings, crawler, entropy). Documents are shared by content-hash id, not by volume.
- **The manager is a clerk with a wider view.** It holds the plan, routes work and owns the unresolved frontier (the open-decisions queue). It has its own small memory (routing log, queue state) and no biography. It may say "assigned and overdue", never "wrong".
- **The manager may never:** write or rewrite another agent's memory or verdicts; read another agent's private memory or drives; settle what is true; withhold budget (it reports, Kord sets); originate goals (only Kord amends the plan); act silently.
- **Channels:** the plan directory and `DECISIONS.md` are the official record. An inbox carries messages, which become testimony in the recipient's memory, never evidence. Direct calls are for requests only, never for reaching into another agent's memory. "Documents travel by handle, memories travel by quotation."
- **Roles:** the manager runs logistics, Kord sets direction, reviewers (Steve) judge. No agent holds two of these. The chain ends at a human. In practice (2026-09-27): Claude coordinates (logistics), Steve reviews the engineering agents (judgment, his own container, unchanged), Kord directs and hears appeals. Steve does not have the final say on his own instruments.

## Work (see docs/BACKLOG.md D1–D5)

- **D1:** cycle check on causal edges, plus a frontier query.
- **D2:** boundary and question nodes, with a `record_boundary` path from chat (the operator says no) and the asking policy.
- **D3:** documents as nodes, and document supersession using the same verdict grammar.
- **D4:** Advocate wiring (`cognition/src/advocate.rs`, built with zero authority) as the owner of "should I ask about this no / act in this silence?".
- **D5:** manager-agent spec and the container-to-container inbox.

## Dreams (Kord, 2026-09-27)
A dream is a node, never evidence. It may carry one kind of edge, **suggests** (`paccaya:upanissaya`, decisive support offered, not taken), and only toward an **older unresolved** node: an open question, a `disputes` verdict, or an unexplained observation. It never points at something new, and never supersedes anything. A suggestion becomes part of the record only when the agent checks it against memory or documents; the check (a verdict with evidence) is what settles the question, not the dream. Dreams inform; evidence decides.
