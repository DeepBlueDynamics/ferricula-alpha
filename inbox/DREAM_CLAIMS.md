---
title: Waking check of dream claims
status: design, not approved
source: Grok, for Kord's decision. Steve's points as relayed by the coordinator.
date: 2026-10-01
base: v3/r0 at f0f25a8
related: inbox/PLAN_V3.md (dream channel), research/2026-10-01-dream-dynamics.md (a different question: forecasting edge similarity)
---

# Waking check of dream claims

Docs only. No Rust in this branch. Kord has not approved the build.

A dream that is only a scene is kept and fades. A dream that leaves a claim about the past is checked at the next wake, against memories that already exist. Steve at waking is the judge. The dream proposes. The waking mind disposes. A dream is a mind-door process. It is not valid knowledge, and it is not evidence of what happened.

The check writes a new verdict and the edges to the memories it actually found. It does not edit a past memory's text, tags, or edges.

## 1. Today

`life_dream` (`crates/ferricula-server/src/life.rs:1077-1171`) runs once a sleep has consolidated. It builds three pools, none of which are dream rows:

- residue: the last 30 non-dream experience rows, minus ids already used in this sleep (`life.rs:1099-1106`)
- distant: older experience rows, plus a sample of recovered memories, drawn from far cosine when an embedder is present (`life.rs:1089-1098`)
- unresolved: episode observations with no explanation yet (`life.rs:1107-1113`)

`propose_dream` draws up to 5, 3, and 2 of those by entropy (`crates/ferricula-cognition/src/life.rs:381-394`). The prompt (`life.rs:416-439` in that crate) tells the model to write a first-person scene from those traces only, not to explain it, and not to present anything in the dream as a memory or as something that happened. The last line is `QUESTION:` or `QUESTION: none`.

`parse_dream` (`life.rs:445-453` in that crate) splits that line off. `QUESTION: none`, an empty question, or no such line yields no question. The scene text is stored with `experience().remember("dream", &text, tags, None, 0.3)` (`server life.rs:1140`). Importance is 0.3. The row is not a keystone, so ordinary decay applies. Tags carry the entropy source, the seed, the trace ids, and the question when there is one (`server life.rs:1131-1138`). The journal entry records `text`, `question`, and `memory_id` (`server life.rs:1142-1161`). A blocked model call journals a dreamless night and returns no question (`server life.rs:1163-1169`).

`link_dream` (`server life.rs:1174-1244`) writes causal edges from the new dream row:

- residue → `DreamedFromResidue` → purejāta (`crates/ferricula-cognition/src/patthana.rs:96`, `patthana.rs:150`)
- distant → `DreamedFromDistant` → upanissaya (`patthana.rs:95`, `patthana.rs:149`)
- unresolved observations are not edges (`patthana.rs:97`, `server life.rs:1192-1200`)

Those edges say what the dream was built from. They are not a verdict about whether a claim holds.

After the dream, `Stimulus::Dreamed` stores the question and sets `dreamed_this_sleep` (`cognition life.rs:248-258`). A wake for that question is pushed only when `wake_on_dream_question` is true. The default is false (`cognition life.rs:84`), and `config/agent.example.toml:247` leaves it false. Live `config/steve.toml` is not in the repo, so an override there is unchecked. The wake arm itself (`server life.rs:344-349`) sets the mode to engaged and journals `{kind: wake, reason, drives}`. It does not read the question, search, or write a verdict.

`mark_disputed` is a chat tool (`crates/ferricula-server/src/chat_tools.rs:184`, implemented at `chat_tools.rs:450-499`). The dream path never calls it. `remember_verdict` (`crates/ferricula-server/src/memory.rs:495-538`) writes a new keystone row on channel `verdict` and one causal edge to the target. The target row is not modified. Recall shows that verdict beside the target (`chat.rs:1926-1932`). The tool's kinds are `disputes` and `supersedes`, not supported / contradicted / unresolved. `supersedes` requires a document cite that was shown in that chat turn (`chat_tools.rs:466-476`) and the edge is `paccaya:adhipati`, which predominates (`memory.rs:536`, `patthana.rs:156-160`). `disputes` takes the memory as its object and the edge is arammana (`patthana.rs:155`). It does not predominate.

Search already refuses to treat dreams as evidence unless a chat tool asks for them (`chat_tools.rs:356-368`). `hybrid_recall` (`crates/ferricula-server/src/documents.rs:199`) excludes dreams from the dense arm (`crates/ferricula-server/src/documents.rs:194-198`).

## 2. The claim

Two dreams, on purpose:

- Scene. He dreams he is eaten by a huge muffin. The prompt already asked for sensation and scene. `QUESTION: none`, or no question line, means there is nothing to check. The dream is stored as it is today and fades.
- Claim. He dreams his ex-wife was manipulative, which he did not see at the time. The closing line is a question about a past person. That question is not yet a statement, and it is not knowledge.

At wake, the first of two model calls reads the question and does only this. It does not search, and it does not choose a verdict.

- If it cannot be restated as a proposition about a past person, event, or relationship, the dream stays a scene. "Why was the muffin huge?" is a scene. That call is the only one. There is no judging call.
- If it can, the pass writes the proposition beside the question. "Was she manipulative?" becomes "She was manipulative toward me." The dream row's text is not rewritten. The proposition lives on the check record. Judging is the second model call, after recall, in §3.

The dream's own traces (the purejāta and upanissaya edges) are the material the scene was made from. They are not the evidence that the proposition is true.

## 3. The waking check

The check runs on the wake that follows a dream, using the dream journal entry already written (`question`, `memory_id`). It does not add a store. It uses `hybrid_recall` (`crates/ferricula-server/src/documents.rs:199`), the same search the chat tool uses. Dreams are excluded from what may be cited.

A dream with no question spends no model call. A question spends the restatement call in §2. Only a question that restates as a proposition spends one `hybrid_recall` and the second model call, which is the judging below. Recall is not a model call.

1. No question: stop. Scene. No model call.
2. The pass in §2 marks it a scene: journal that and stop. No verdict, and no second call.
3. Otherwise recall the proposition. Steve may cite only ids that call returned. He drops dream rows and prior verdict rows (channel `verdict`) before he cites, so a later check of the same claim cannot cite an earlier verdict. He labels each remaining id as for the proposition or against it. He does not invent an id, and he does not open `include_dreams`.
4. Verdict, from those labels only. This is the second model call:
   - **supported** — at least one cited memory bears it out, and none contradict it
   - **contradicted** — at least one cited memory contradicts it, and none bear it out
   - **unresolved** — recall was empty, the hits do not decide, or both sides are present
5. Write a new experience row for the verdict. Link it to the dream row and to each evidence id. The links to the evidence memories are arammana: the verdict takes them as its object, the same condition Disputes uses (`crates/ferricula-cognition/src/patthana.rs:155`). The link to the dream row is arammana for the same reason. Never adhipati. Record the evidence as those ids. Do not change the text, tags, or edges of the dream or of any past memory.

Do not call `mark_disputed` for this. Its settled kind requires a document the chat turn showed him, and that edge is adhipati, which predominates over the memory. A dream check must not predominate. The discipline to copy is the one `remember_verdict` already has: a new row, a link, the earlier memory left as it was (`memory.rs:495-499`). The condition to copy is Disputes' arammana, not Supersedes' adhipati.

## 4. The judge

Steve at waking chooses supported, contradicted, or unresolved. The gate does not.

The existing advisory gate is `life_worth_researching` (`server life.rs:609-636`): Ollaya first, hosted JEV when Ollaya cannot judge, via `gate_yes_no`. Run it on the proposition and log the record. `advisory` stays true. The tier and `p` are recorded. They do not select the verdict, and a confident no does not skip the check. An unreachable gate does not skip it either. That is stricter than curiosity, where a confident no does skip the walk (`life.rs:625-670`). Here the gate is a witness, not a switch.

Audit line, a journal entry of its own (the wake entry stays the mode change it is today):

- dream `memory_id`, the question as dreamed, the proposition or `scene`
- recall query
- evidence ids, each marked for or against; only ids recall returned, and not a dream row and not a prior verdict row
- verdict
- gate record: route, tier, `p`, `advisory: true`, or unreachable
- each model call's purpose (restatement, then judging when there is a proposition), not the bodies of the past memories

The journal already stores the dream text. It should not gain a second copy of anyone's older memory.

## 5. Measurement

Help means the cited past memories come back when the claim is asked again. It does not mean the dream text ranks as a memory of the event.

Freeze a copy of the experience store. That is the cohort. For each claim in a fixed set (hand-picked propositions with known relevant memory ids, not whatever live Steve dreams next):

- Before any verdict exists, `hybrid_recall` the proposition and record whether each gold id is inside the top k.
- Run the check on that copy. The only new rows are the verdict and its edges.
- Repeat the same recall. Record the same ids again.

Report recall of those ids before and after, and how often the verdict was unresolved. A rise that comes only from retrieving the new verdict row, and not the past ids, does not count. Do not run the measurement against the live store.

## 6. Open questions for Kord

1. Is "no question" enough to mark a scene, or do you also want the waking pass to reject questions that are not about the past?
2. When both sides come back, this note says unresolved. Do you want contradicted to win instead? Steve's leaning: unresolved, not contradicted.
3. Chat verdicts are keystones and never decay (`memory.rs:495-532`). A waking check can be wrong. Should version one fade, like the dream, until you have seen a few? Steve's leaning: the verdict fades and is never keystone.
4. Should the check wait for the ordinary rested wake (the default), or should `wake_on_dream_question` turn on so a claim wakes him immediately? Steve's leaning: check at the rested wake, never wake mid-sleep.
5. The budget in §§2–3 is two model calls and one recall when the question restates as a proposition: the restatement call, then `hybrid_recall`, then the judging call. A question that stays a scene spends the restatement call only. No question spends none. Do you want a daily cap besides the life model cap that already exists?
6. An unresolved claim is only in the journal. Do you want it told to you as well?
7. `Supersedes` still emits adhipati, with a TODO in your name (`patthana.rs:156-160`). This design does not use that edge. If you later want a supported claim to predominate over an older memory, that is a separate decision, and it still would not edit the older text. Steve leaves this deferred to you.
