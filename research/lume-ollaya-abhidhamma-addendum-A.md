<!-- Extracted verbatim via pypdf from lume-ollaya-abhidhamma-addendum-A.md.pdf on 2026-09-26 -->

## --- Page 1 ---

Addendum A: Read-Time Curation and the Drift Layer
Amends:Lume × Ollaya × Abhidhamma Memory: Integration & Training Plan (Draft v1, 2026-
09-26) Date: 2026-09-26 · Status: Draft
Sources:
Zhou, Li et al., Just-in-Time Memory: Learning to Curate Task-Adaptive Memory for LLM
Agents, Salesforce AI Research, arXiv:2609.27334 (2026). Cited below as JitMem.
Design discussion on human memory drift and the Four Noble Truths (2026-09-26).
A.0 Summary of changes
# Change Affects plan
section
1 Curate at recall, not at storage. Raw text is kept; a curator writes a task-speciﬁc
brieﬁng at recall time.
§1, §2, new §A.2
2 Decay governs retrieval priority, not deletion. Text is removed only by
deliberate upekkhā or nirodha.
§3.2
3 Consolidation builds an index instead of replacing members. §4.3, §6.4
4 Success-ﬁltered recall pool, plus a separate failures index. §3.2, new §A.3
5 New gate: task_succeeded , with a strict judging rule. §4, new §A.3
6 New trainable component: the curator (generative, GRPO-trained on immediate
task reward).
§7, new §A.4
7 New drift layer (papañca): human-like drift is modelled and observed, not
hidden and not removed.
new §A.5
8 Four new non-negotiable rules. §9
9 New metrics and milestones. §10, §11
A.1 Principle: memory is reconstructed at recall
JitMem's core ﬁnding is that distilling experience when it's stored throws away information
that a later task may need. Keeping raw traces and distilling when a task arrives works better,
because the distillation can be shaped by that task.

## --- Page 2 ---

In JitMem's Figure 3, one stored episode ("put a cool egg in microwave") yields
temperature-change guidance for one task and placement guidance for another. A single
write-time summary would have committed to one lesson.
Replacing raw traces with ReasoningBank-style write-time distillation cost 1.7–2.9
success-rate points on ALFWorld and 6.8–8.2 points on WebShop.
An untrained read-time curator already matched or beat trained write-time systems (e.g.
WebShop with Gemini-2.5-Pro as curator and executor: 61.0 vs 41.0 for SkillOS).
This agrees with the Abhidhamma's own psychology: saññā is re-perception arising fresh in
each moment, not the replay of a stored copy.
A.1.1 Revised lifecycle semantics
Operation Plan v1 Amended
Ojā decay Fidelity loss, leading
toward text removal
Retrieval priority only. Low ojā ranks a memory lower; the
text stays until a deliberate release.
Saṅkhāra
consolidation
Weighted centroid
replaces members
Centroid becomes a cluster node pointing at raw members.
The members' α  decreases (becoming structure), but their
text is kept. The curator can read the originals.
Saññā class Committed at ingest Retrieval hint. The curator re-perceives each memory for
each task.
Upekkhā
(forgive)
Text → ∅ , nimitta
kept
Unchanged, and now the only automatic-lifecycle path to text
loss that isn't nirodha. It is a deliberate act, not entropy.
Nirodha
(archive)
Text and nimitta →
∅ , karmic log kept
Unchanged
The forgetting philosophy is intact. Forgetting still serves release, consent and non-clinging. It
no longer happens as a side effect of time passing.

## --- Page 3 ---

A.2 New component: the curator (Phase 5b)
A.2.1 Placement
A.2.2 Serving
Ollaya cannot serve the curator: it only runs classiﬁers and returns 404  on /api/generate .
Serve the curator with Ollama (already used by Lume's agent loop) or vLLM. The paper used
Qwen3-8B with thinking disabled, at temperature 0.6, top-p 0.95 and top-k 20.
A.2.3 Curator prompt (adapted from JitMem Appendix A)
query/task
  → Lume hybrid search (top 20)
  → filter: agent_id, state, pool = success
  → sati-recall gate (Ollaya) → top k = 3
  → CURATOR (generative LLM): task + 3 raw memories → briefing
  → agent acts with the briefing in context
  → task_succeeded gate → store raw trajectory in success pool or failures index
text

## --- Page 4 ---

A.2.4 Settings from JitMem
k = 3. k = 3 and k = 5 performed the same.
Cold start is acceptable. Pre-loading the bank changed success rates by at most 1.3
points.
The brieﬁng is ephemeral. It is never stored as a memory (Rule 7, §A.6).
A.2.5 Expected beneﬁt
Before any training, JitMem's untrained curator added about 1.9K input tokens per task
(versus 10.7–13.4K for write-time baselines) and cut agent steps by 18–22%. Beneﬁts were
largest on procedural tasks ( τ ²-bench Telecom +11.0) and negligible on simple lookup tasks
(Airline, Retail). Expect the curator to help most where the agent follows multi-step
procedures.
SYSTEM
You are the Memory Curator. You receive the current task and up to three raw
memories retrieved for it. Write a short briefing that helps the agent with
THIS task.
1. Say which memories are relevant and why.
2. Extract what worked in them that applies here.
3. Give specific guidance for the current task.
Rules:
- Never copy identifiers, names, dates, amounts or IDs from memories. Look up
  the current case instead. Patterns transfer; specifics do not.
- If a memory is marked as a reconstruction (overlay), say so when you use it.
- If no memory is relevant, say so. Do not invent guidance from general knowledge.
- Be concise: the agent has limited context.
USER
Task: {task}
### Retrieved memories
Memory 1 (ojā {oja_1}, vedanā {vedana_1}):
{raw_text_1}
...

## --- Page 5 ---

A.3 Storage: success pool, failures index, task_succeeded gate
A.3.1 Two pools
JitMem found that storing only successful trajectories beat storing all trajectories with
success/failure labels by 1.5–3.4 points. Even with the labels, the curator couldn't fully ignore
failed traces.
Pool Contents Default recall Curator input
Success
pool
Trajectories judged
successful
Yes Yes, as positive examples
Failures
index
Trajectories judged
failed
No; only on explicit
request
Only when explicitly requested, and
always labelled as failures
The failures index is not a place for discarding dukkha. Failed experiences remain memories,
with vedanā, ojā and the full lifecycle. They are simply kept out of the positive-example pool,
much like sealed memories are kept out of default search.
A.3.2 New gate: task_succeeded
JitMem's judge rule, adapted: credit only outcomes that external evidence conﬁrms (tool
results, state changes), ignore the agent's own claims of success, and treat ambiguous or partial
outcomes as failures.
 
dockerfile
FROM decision
QUESTIONS """
{
  "task_succeeded": {
    "type": "noul",
    "instructions": "The task was fully completed, as confirmed by tool results or observe
    "criteria": {
      "true": "Completion is confirmed by external evidence in the transcript",
      "false": "Completion is unconfirmed, partial, ambiguous or failed"
    }
  }
}
"""
DESCRIPTION Storage gate: strict success judgment

## --- Page 6 ---

Full trajectories exceed laya:en 's 512-token window, which is why this gate uses the
decision  family (up to 16k tokens). If latency matters, judge from the ﬁnal state plus the last
few tool results only, on laya:multilingual .
Where ground truth exists, log every disagreement between this gate and the ground truth.
That gap feeds the self-judgment metric (§A.5.5).
A.4 Training the curator
A.4.1 Why the curator is easy to train
A write-time decision is graded only when some later query happens to retrieve it, which is a
long-horizon credit-assignment problem. A read-time brieﬁng is consumed by the task it was
written for, so its reward is immediate. JitMem trains with plain GRPO on task success alone:
no content-quality judge, no task grouping, no return shaping.
This also clariﬁes the gate split in plan §5:
Component Reward timing Training signal
Curator, sati-recall Immediate Task outcome
vedana , sanna , sankhara-merge Delayed or none Human labels (plan §5)
A.4.2 Prerequisite
A veriﬁable success signal for the agent's real tasks: tool conﬁrmations, tests passing, a user's
explicit acceptance. Without one, don't train the curator; run it untrained (§A.2), which already
captures most of the gain.

## --- Page 7 ---

A.4.3 Recipe (from JitMem Table 7)
Setting Value
Base policy Qwen3-8B, thinking disabled
Algorithm GRPO, advantages without std-normalization
Learning rate 1 × 10⁻⁶, constant with 5 warmup steps
Batch / group size 32 prompts × 8 brieﬁngs each
Steps 100
KL Loss term, coefficient 1 × 10⁻³; none in reward
Clip 0.2 / 0.2
Executor during training Frozen; the cheapest model that exhibits the task
Training bank Fixed, built once from successful base-agent runs with ground-truth labels
Compute (paper) 8 × H200; about 21–27 h per run
A.4.4 Notes
The bank stays ﬁxed. Rebuilding it mid-training gained at most 2.8 points, which isn't
worth the cost at ﬁrst.
The curator transfers across agents. A curator trained with Qwen3-8B as executor came
within 1.4 points of one trained directly with GPT-5.4. Train once, and reuse it across
agents.
The training bank obeys the plan's rules. No sealed memories, no forgiven or archived
text, no overlay content (§A.6). Every training prompt records its source memory IDs so
deletions cascade.
Sanity check before rollout: Run JitMem's "without retrieved memories" ablation. A
trained curator must do clearly worse with retrieval removed. If it doesn't, it has learned
generic advice rather than learning to use memory.
A.5 The drift layer (papañca)
A.5.1 Rationale
Human memory departs from a clean retrieve → curate → act → store loop in several ways:

## --- Page 8 ---

Reconsolidation. Recalled memories are rewritten with whatever the recall added
(Nader et al., 2000; Loftus's misinformation studies).
Mood-congruent recall. The current feeling-tone steers which memories are retrieved.
Motivated self-judgment. People grade their own outcomes favourably.
Salience-driven storage. Arousal, not usefulness, decides what is stored strongly.
Cue-driven intrusions. Recall happens constantly, triggered by whatever is present, with
no task to serve. Minds wander in almost half of waking moments (Killingsworth &
Gilbert, 2010).
Gist extraction during sleep. Humans also distill at write time, and pay for it.
The Abhidhamma names this drift. MN 18 (Madhupiṇḍika Sutta) extends contact → feeling →
perception to thinking → proliferation (papañca). What is perceived is thought about, what is
thought about is proliferated, and the proliferation is taken for the experience.
A.5.2 Design stance: the four tasks
The ﬁrst sermon (SN 56.11) assigns each truth a task, and this layer follows them:
Truth Task In the system
Dukkha Fully understood
(pariññeyya)
Drift exists and is measured
SamudayaAbandoned (pahātabba) Craving dynamics can arise but are detected and damped,
never ampliﬁed
Nirodha Realized (sacchikātabba) Release: upekkhā, nirodha, returning to the object
Magga Developed (bhāvetabba) Sati monitors and the return mechanism, below
Principle: The system can drift, and it always knows when it is drifting. The drift is neither
hidden in the plumbing nor removed. This extends the pattern already in the spec, where the
"death spiral" of high α , low ojā and refusing to let go is detected rather than forbidden.
A.5.3 Components
1. Reconsolidation overlay.
A recall may write back, but only into an overlay record linked to the raw memory and
tagged kind = reconstruction , with the curator version and the task that triggered it.
Raw akkhara is never modiﬁed by recall.
Overlays are retrievable (they are part of how this mind now remembers), but the curator
always sees them labelled as reconstructions.

## --- Page 9 ---

Forgiving or archiving a memory also deletes its overlays.
Overlays are never training data (Rule 9).
The result is that the system distinguishes what it experienced (phassa) from what it has since
told itself about it (papañca), and the difference is inspectable.
2. Cetanā-modulated recall with a sati monitor.
The existing cetanā layer may bias retrieval toward the current vedanā by a tunable
strength λ_cetana ∈  [0, 1] .
The sati monitor measures the resulting skew on every recall:
If skew stays above τ_skew  for N consecutive recalls, the monitor emits a noting event
("retrieval is 80% dukkha from a store that is 30% dukkha") and temporarily reduces
λ_cetana .
3. Papañca detector.
Proliferation's signature is recall triggered by the system's own outputs rather than by a
task or an external input.
Track the chain depth (consecutive recalls whose query came from the agent's own prior
output) and the self-reference ratio (the share of a window's recall queries that were self-
generated).
If depth exceeds d_max  or the ratio exceeds ρ_max , return to the object: the next recall
must be cued by the current task or input. This is the meditation instruction,
implemented literally.
4. Self-judgment gap.
Whenever external evidence exists, log both the agent's own verdict and the evidence-
based verdict (from task_succeeded  or ground truth).
The gap is motivated self-judgment, made measurable.
A persistent positive gap (the agent rates itself better than the evidence does) is a
samudaya signal and is surfaced in the dashboards.
5. Salience vs usefulness.
Record vedanā intensity at storage, and record later usefulness: how often the memory
was retrieved and the task succeeded.
skew = KL( valence distribution of retrieved set ‖ valence distribution of the agent's 
store )

## --- Page 10 ---

Report the correlation. Low correlation means the system is remembering what was
vivid, not what helps. That is information for tuning, not an automatic correction.
A.5.4 The dial: purpose decides
Parameter Mind model (research) Service agent (production)
λ_cetana  (mood bias) 0.3–0.6 0.0–0.1
Overlay write-back On On, but curator weight on overlays low
d_max  (chain depth) 5–8 2
τ_skew Loose; observe Tight; intervene early
Noting events Logged and studied Logged and acted on
Rationale:
A mind model needs the drift, or it has nothing to let go of and cannot practise.
A service agent's rumination costs its users, so the drift is present but damped.
Values in the table are starting points, not measurements.
A.5.5 Ethical note
If the framework is taken seriously, building in craving means building in the conditions for
dukkha. Whether that is anything more than a metaphor for software is an open question that
this plan does not settle. The design commitment is that craving dynamics never ship
without the path: every drift component above has a matching monitor and a return
mechanism. A version with the drift but without sati is out of scope.
A.6 Amendments to the non-negotiable rules (plan §9)
Rules 1–6 stand unchanged. Add:
7. Brieﬁngs are never memories. Curator output is ephemeral. It never enters the raw
store, the success pool or any training set. (Overlays under §A.5.3 are a separate, labelled
record type, not stored brieﬁngs.)
8. Failures stay out of the positive pool. Failed trajectories are never retrieved as positive
examples and never enter the curator's training bank as demonstrations.
9. Reconstructions are never training data. Overlay content is excluded from all training
rows for every gate and for the curator, and is deleted with its parent memory.

## --- Page 11 ---

10. No drift without sati. Every drift mechanism (§A.5.3) ships with its monitor enabled,
and its events go to the karmic log with the gate or curator version that produced them.
A.7 Additional metrics (plan §10)
Metric Deﬁnition Target
Curator lift Task success with brieﬁng − success with raw
memories only
> 0
Retrieval dependence Success drop when retrieval is removed from
the trained curator
Clearly > 0 (§A.4.4)
Payload tokens / agent
steps
Per task, against the no-memory baseline Fewer steps, < 3K added
tokens
Judge agreement task_succeeded  vs ground truth, where it
exists
≥ 0.9
Valence skew §A.5.3(2), per recall Below τ_skew  in service
mode
Chain depth / self-
reference ratio
§A.5.3(3) Below d_max  / ρ_max
Self-judgment gap §A.5.3(4) Tracked; persistent positive
gap alerts
Salience–usefulness
correlation
§A.5.3(5) Tracked
Overlay share Share of recalled content that is overlay rather
than raw
Tracked; rising share =
growing papañca
Additional CI invariants:
Forgiving a memory deletes its overlays.
No overlay text appears in any training row.
No brieﬁng text appears in any store.
No failure-index record is retrieved in default recall.

## --- Page 12 ---

A.8 Additional milestones (plan §11)
# Milestone Depends on Est.
M8 Consolidation-as-index and decay-as-priority refactor (§A.1.1) M0 1 week
M9 Untrained curator live; success pool + failures index;
task_succeeded  gate
M1, M5 1–2
weeks
M10 Drift layer with all monitors (§A.5), in mind-model mode on a
research agent
M8, M9 2–3
weeks
M11 Curator GRPO training (only if a veriﬁable success signal exists) M9 + training
bank
2–3
weeks
M10 and M11 are independent. Do M10 ﬁrst if the primary purpose is modelling mind, and M11
ﬁrst if it is agent performance.
A.9 Open questions
1. Purpose: Mind model, service agent, or both with different dial settings per agent?
(§A.5.4)
2. Success signal: What is the veriﬁable success signal for the agent's real tasks? (This
decides whether M11 happens.)
3. Consolidation-as-index: Does this ﬁt the spec's intent that "new memory replaces
components"? Or should replacement stay for a subset (for example, only after
upekkhā)?
4. Overlay retrieval weight: Should overlays ever outrank their raw parent, and if so, under
what conditions?
5. Sealed memories and papañca: The spec says sealed memories "still surface in dreams"
and "intrude when arousal is high." Should the papañca detector treat those intrusions as
proliferation to damp, or as a separate channel that is left alone?
A.10 References
Zhou, Y., Li, Y., Liu, Z. L., Yavuz, S., Joty, S. (2026). Just-in-Time Memory: Learning to
Curate Task-Adaptive Memory for LLM Agents. arXiv:2609.27334.
Schacter, D. L., Addis, D. R. (2007). The cognitive neuroscience of constructive memory.
Phil. Trans. R. Soc. B, 362, 773–786.
Nader, K., Schafe, G. E., LeDoux, J. E. (2000). Fear memories require protein synthesis in
the amygdala for reconsolidation after retrieval. Nature, 406, 722–726.

## --- Page 13 ---

Loftus, E. F. (2005). Planting misinformation in the human mind: a 30-year investigation
of the malleability of memory. Learning & Memory, 12, 361–366.
Killingsworth, M. A., Gilbert, D. T. (2010). A wandering mind is an unhappy mind.
Science, 330, 932.
Madhupi ṇḍ ika Sutta (MN 18), The Honeyball.
Dhammacakkappavattana Sutta (SN 56.11), Setting the Wheel of Dhamma in Motion.

