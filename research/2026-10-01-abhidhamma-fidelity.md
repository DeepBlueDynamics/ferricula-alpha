# Abhidhamma fidelity: six loose mechanisms

**Date:** 2026-10-01
**Base:** `v3/l6-abhidhamma` at `647efed66d7c795411b973e7c9ef522d1ef2f0d5` (`origin/v3/r0`). Quotes are from this worktree.
**Scope:** A specification for engineers. No code in this change.
**Code read:** `crates/ferricula-cognition/src/{life.rs,patthana.rs,vithi.rs,bhavana.rs,gates.rs}` and `crates/ferricula-core/src/memory.rs`. `Valence` is the enum those files import (`crates/ferricula-cognition/src/sati.rs`).

Claim labels: **verified** means the named file says it. **Definition-match** means a note's own sentence covers the case, and the note never names the case. **UNSUPPORTED** means the notes do not support the claim. Nothing below is offered as canon beyond those notes.

## Invariants the changes have to respect

1. Never delete.
2. Fading changes rank, not content.
3. Gates stay advisory until calibrated (`Judged::calibrated_answer` is `None` unless `provenance.calibrated` and not `state_truncated`).

`bhavana.rs` already states the amended lifecycle: decay lowers ojā (fidelity) as retrieval priority; text stays; a `ClusterIndex` points at raw members; the cycle fails if the store or the graph shrinks. Addendum A §A.1.1: "Decay governs retrieval priority, not deletion. Text is removed only by deliberate upekkhā or nirodha." and "The forgetting philosophy is intact. Forgetting still serves release, consent and non-clinging. It no longer happens as a side effect of time passing."

That addendum sentence allows a deliberate text removal. Invariant 1 forbids deletion. The two disagree. This spec does not resolve them by adding a purge. Cognition may change rank and lifecycle state. It does not delete text, vectors, or tags. `apply_release` already only flips `LifecycleState` and sets `text_purge_pending`. That flag is not authorization for these six mechanisms to delete.

## 1. Decay

### (a) Code today

`MemoryRecord::decay_tick` (`memory.rs`): keystones and non-`Active` records return immediately. Otherwise `fidelity *= exp(-effective_alpha())`, then clamped to `[0, 1]`. `effective_alpha` is `decay_alpha / (1 + ln(1 + consolidation_depth))`. The raw row is not in this struct. The module comment: "The raw data (tags, vector) lives in Engine; this tracks the memory's lifecycle, fidelity, decay rate, and relational metadata."

`MemoryRecord::new` / `new_at` set `decay_alpha` to `ALPHA_DEFAULT` (0.01), `fidelity` to 1.0, `keystone` false. Bounds: `ALPHA_MIN` 0.001, `ALPHA_MAX` 0.02. `FIDELITY_GATE` is 0.75 (`above_gate`).

`bhavana_cycle` calls `decay_tick` only for an entropy-selected subset of active non-keystone ids (phase 1, "priority only"). Fidelity below the gate appends a `ReleaseProposal` of kind `Upekkha`. State is not changed there. `MemoryStore::remove` drops an envelope. In these six files the only call is the `store_basics` test. `bhavana_cycle` errors if `store.len()` changes.

`forgive` is Active → Forgiven. `archive` is Forgiven → Archived. Neither function touches text.

### (b) Notes

`02_ABHIDHARMA_THERMODYNAMIC_MEMORY.md` §1: "Anicca (Impermanence): Decay is the default state of all conditioned phenomena (saṅkhāra)." §3.1 writes `f(t+Δt) = f(t) · exp(-α_eff · Δt)`. §3.2 matches `effective_alpha`. §3.3 then puts hearing/seeing at α₀ 0.010 (seeing keystone by default) and thinking at 0.015, and §3.3's bands auto-move `f < 0.75` to Forgiven and `f < 0.25` to Archived, "Slated for permanent garbage collection or ghost-echo extraction."

`10_PATTHANA_24_CAUSAL_CONDITIONS.md` §3.3 maps vigata to "Cache eviction hooks, dead-man timers, and garbage collection cascades" when energy passes a cutoff. `08_CAUSAL_DYNAMICS_APOHA_AND_COGNITIVE_CONTROL.md` row 23: "Garbage collection and cache eviction; the active process of a memory fading or being purged."

Addendum A §A.1.1 amends that: ojā decay is "Retrieval priority only. Low ojā ranks a memory lower; the text stays until a deliberate release." Upekkhā stays "Text → ∅, nimitta kept" and "is a deliberate act, not entropy." `PAPER_DRAFT.md`: "decay is retrieval priority; text leaves only by upekkhā or nirodha." `episode-causal-execution.md`: "ojā / jarā / vigata change retrieval priority. They do not delete text."

The channel α₀ table and the automatic Forgiven/Archived bands are in note 02. They are not in the addendum. The vigata-as-deletion sentences conflict with the addendum.

### (c) Smallest change

Keep `decay_tick` as a rank update on `fidelity` for active non-keystones. One call is one tick (`Δt = 1` in note 02's equation). Do not move `LifecycleState` from a fidelity number. Do not call `MemoryStore::remove` from decay. Do not implement note 02's `f < 0.25` garbage collection or note 10's vigata eviction. A below-gate row stays a `ReleaseProposal` until a deliberate `ReleaseDecision`.

`MemoryRecord::new` does not take a channel, so the hearing/seeing/thinking α₀ table is not applied. Leave that table alone in this change. Wiring it is a separate write-path decision and is not required to make fading rank-only.

### (d) Invariants

No conflict if the change is the one in (c). `decay_tick` already changes `fidelity` and not the Engine row. Auto-archive or eviction-on-fade would delete or would treat fading as content loss. Those conflict with invariants 1 and 2. Decay does not read a gate.

## 2. Recall strengthening

### (a) Code today

`MemoryRecord::on_recall`: `recall_count += 1`, `last_recalled = now`, `decay_alpha = max(ALPHA_MIN, decay_alpha * 0.95)`. It does not add to `fidelity`.

`on_neglect`: `decay_alpha = min(ALPHA_MAX, decay_alpha * 1.005)`. `bhavana_cycle` phase 4 calls it for active rows whose `staleness()` exceeds `neglect_seconds` (default 86400). `on_halo_touch` multiplies by 0.99 for active non-keystone neighbors of a keystone, and does not count a recall.

`LinkEvent::RecalledTogetherAgain.condition()` is `Paccaya::Asevana`. `vithi.rs` records a single `Stage::Impulsion`. It does not run seven impulsion moments and it does not call `on_recall`.

### (b) Notes

`10_PATTHANA` §3.2: "In the Abhidhamma's 17-moment citta-vīthi, the seven javana (impulsion) moments condition each other via āsevana: each successive repetition becomes faster, stronger, and more proficient." The next sentences are the note's own AI step: Hebbian `ΔW_ij ∝ Frequency(Co-activation)` and `α_decay ← α · γ`.

`citta-vithi.md` places javana at moments 9–15, one row: "The active transformation. Weight updates, spreading activation, associative logic." `02` §3.4 also writes recall as `α ← max(α_min, α × 0.95)` and `f ← min(1, f + 0.10)`, and neglect as `α ← min(α_max, α × 1.005)`. `08` row 12 calls āsevana "Hebbian synaptic plasticity" and "repeated traversal of the same graph paths."

The seven-javana sentence is intra-process. The α shrink and the `+0.10` fidelity bump are the notes' engineering lines. No note states that those two are the same event.

### (c) Smallest change

Keep `on_recall`'s α shrink (rank only) for a row the runtime actually surfaced. Keep `RecalledTogetherAgain → Asevana` for a pair recalled together again. Do not add the `+0.10` fidelity write. That number is note 02's engineering line, and restoring rank is not the seven-javana sentence.

Do not invent seven `Impulsion` stages inside one `Vithi`. Record seven javana moments only when the runtime has seven measured repetitions. Until then, intra-process āsevana is **UNSUPPORTED** as an implementation. Cross-episode repetition is the edge plus the α shrink, which is what note 10's AI sentence and note 02's 0.95 factor describe.

### (d) Invariants

No conflict. α and a repetition edge are rank and relation, not content. Strengthening must not be driven by an uncalibrated sati probability. A surfaced row is the runtime's fact that it was recalled. `SatiRecallGate` staying advisory is unchanged: its `relevance` must not be the thing that calls `on_recall`.

## 3. Vedanā tagging at write time

### (a) Code today

`MemoryRecord.emotion` is `Option<Emotion> { primary: String, secondary: Option<String> }`. `new` and `new_at` set `emotion: None`. No function in the six files assigns `emotion`.

`VedanaGate::judge(&self, agent, text) -> Judged<VedanaVerdict>` (`gates.rs`). `VedanaVerdict` is `{ valence: Valence, intensity: f32 /* 0..4 */, p: f32 }`. `Valence` is `Sukha | Dukkha | Neutral`. `NoModelGate` as `VedanaGate` always returns `Abstain(NoModel)` with `calibrated: false`. Nothing in `gates.rs` writes a record.

`vithi::signals::vedana(intensity)` maps `intensity / 4` onto a `Cetasika::Vedana` factor on the process record. The module comment says a factor is "a label attached to a measured signal" and "not a claim that the factor is experienced." `VithiBuilder::begin` does not attach vedanā.

`life::Trace` carries `valence: Valence` and `intensity` (comment: 0..4) into dreams. `propose_dream` sorts `today` by `intensity` and keeps five. It does not judge text.

### (b) Notes

`02` §1: "Vedanā (Feeling-Tone): The affective valence (pleasant, unpleasant, neutral) that acts as an exponential multiplier on consolidation." No base and no equation for that multiplier.

`citta-vithi.md` §4: "The Abhidhamma indicates that vedanā (feeling-tone: pleasant, unpleasant, neutral) acts as a multiplier for consolidation during the Javana phase. High-valence states (strong emotional or goal-relevant arousal) should \"hardcode\" memories, altering their `decay_alpha` or `importance` weights before the Tadālambana registration." No coefficient.

`03_ABHIDHARMA_TECHNICAL_PAPER.md`: "Affective scalar: Pleasant (+1), Unpleasant (-1), Neutral (0)." The same row's engineering cell adds "Valence/Arousal," which the +1/0/−1 cell does not define.

`12_SENSE_DOORS_AFFECT_AND_SITUATED_AWARENESS.md`: "The system computes an initial valence score ν ∈ [-1.0, 1.0] and arousal intensity α_arousal ∈ [0.0, 1.0]."

`2026-09-26-gates.md`: the vedanā verdict is `VedanaVerdict { valence, intensity ∈ 0..4, p }`; empty text "is not an abstention" because "the vedanā prompt directs mass to `adukkhamasukha`"; measured ECE is above the 0.05 lifecycle bar and "all 8 runs `lifecycle_authorized = false`."

Three tones (pleasant / unpleasant / neutral, or +1 / −1 / 0) are what `02`, `citta-vithi.md`, and `03` agree on. Intensity 0..4, arousal in [0, 1], valence in [−1, 1], and an exponential multiplier are not that definition. The multiplier's formula is **UNSUPPORTED**. Which of `decay_alpha` or `importance` moves, and by how much, is **UNSUPPORTED**.

### (c) Smallest change

At insert, before `tadālambana` / `Stage::Registration` commits the row:

1. Call `VedanaGate::judge(agent, text)` on the text being stored.
2. Read the tone only from `Judged::calibrated_answer()`. `None` (abstain, uncalibrated, truncated, or invalid) leaves `emotion` as `None`. Do not treat empty text as neutral unless that call returns a calibrated `Neutral`. The prompt rule in the gates note is not a stored tone.
3. Write `Emotion.primary` from `Valence`: `Sukha` pleasant, `Dukkha` unpleasant, `Neutral` neutral. Leave `secondary` as `None`. The three-tone notes do not define a second feeling.
4. Do not write `intensity` onto the record. Do not change `decay_alpha`, `importance`, `fidelity`, or the text. `vithi::signals::vedana` may still label the process. That label is not the stored tone.

`NoModelGate` therefore stores no tone, which is the current `emotion: None` behavior, reached on purpose.

### (d) Invariants

No conflict. A string on `Emotion` is metadata. Text, vector, and tags stay. Rank stays. An uncalibrated verdict does not move the lifecycle. Applying the unsupported exponential, or moving α from `intensity` while `calibrated_answer()` is `None`, would break invariant 3. Do not do that in this change.

## 4. Paṭṭhāna links between a new row and its causes

### (a) Code today

`patthana.rs`: `Paccaya` has all 24. `Paccaya::gloss` is an engineering string in the code, not a note. `LinkEvent::condition`:

| Event | Code comment | Condition |
|---|---|---|
| `CuriosityFromThread` | curiosity about a conversation thread | `Hetu` |
| `ReadFrom` | a document section and the memory of reading it | `Arammana` |
| `ReflectedOn` | a reflection written about something read | `Nissaya` |
| `RecalledTogether` | recalled together for one question | `Atthi` |
| `RecalledTogetherAgain` | recalled together again | `Asevana` |
| `DreamedFrom` | a dream built from these traces | `Upanissaya` |
| `NextTurn` | the next turn of a conversation | `Anantara` |
| `SimilarityHypothesis` | cosine-near, never co-recalled | `Sampayutta` |
| `ConsolidatedInto` | cluster member and its index entry | `Annamanna` |
| `AfterRelease` | written after another was deliberately released | `Vigata` |
| `Disputes` | verdict takes a memory as object, no winner | `Arammana` |
| `Supersedes` | verdict replaces a memory's claim and predominates | `Adhipati` |

`is_hypothesis` is true only for `SimilarityHypothesis`. The module comment: similarity-only edges stay hypotheses until a gate or the operator confirms them. No function in the other five files calls `LinkEvent::condition`.

Note 10 §4 prints a 13-variant sample enum. The code enum is the 24-row table, not that sample. Do not delete the extra variants to match the sample.

### (b) Notes

Note 10 §2 and note 08 §2.1 define each condition. The sentences used for the four cases in §7 are quoted there. Neither note names "a reflection caused by a reading," "a verdict superseding a memory," "a reply caused by an email," or "a dream drawing on traces."

Note 08 §2.2 says the 24 reduce to ārammaṇa, upanissaya, kamma, and atthi, and it does not list which of the other twenty fold into which root. `episode-causal-execution.md` records that gap and refuses to invent the fold. This spec also does not.

`episode-causal-execution.md`: "Support, Disconfirm, and Supersede also require an already-recorded link in {vipaka, kamma, purejata, pacchajata, upanissaya} between that observation and the hypothesis or its target."

### (c) Smallest change

When a new row is written, add an edge only for a cause whose note sentence covers that cause. Keep `SimilarityHypothesis` as a hypothesis. A gate may confirm it only through `calibrated_answer()`. Operator confirmation stays operator confirmation.

Change the four event mappings as in §7. Leave the other eight `LinkEvent` arms as they are in this change. `ReadFrom → Arammana` matches note 10's object condition for "what was read," which is a different event from a later reflection. `RecalledTogether → Atthi` matches note 10's "presence." `RecalledTogetherAgain → Asevana` is §2 above.

`AfterRelease → Vigata` uses vigata for "a new row written after a deliberate release," which is cessation-as-trigger. Note 10's vigata sentence is eviction that deletes. Do not implement that deletion. The edge may name the succession. It must not drop the released row's text.

### (d) Invariants

No conflict if edges are added and text is unchanged. Confirming a similarity edge from an uncalibrated gate would break invariant 3. A supersession that rewrites the old row's text would break invariants 1 and 2. §7 forbids that rewrite.

## 5. Sleep and consolidation

### (a) Code today

`life.rs` module comment: bhavaṅga is the awake resting phase (boredom rises). Sleep pressure is a separate number. `step`: crossing `sleep_threshold` pushes `Urge::Sleep` and `Urge::Consolidate` and sets `Phase::Asleep`. `Stimulus::Consolidated` during sleep pushes `Urge::Dream` when `dream_on_sleep` is set (default true) and this sleep has not dreamed. `propose_dream` sorts today's traces by `intensity`, keeps 5 (`residue`), draws 3 from `older` (`distant`) and 2 from `unresolved` by a splitmix stream. `DreamProposal::render_prompt` says the dream is built only from those traces, is not a report, and "is never treated as evidence." `parse_dream` splits off a `QUESTION:` line. `step` does not write memories.

`bhavana_cycle`: halo shrink; entropy-gated `decay_tick`; `ReleaseProposal`s without state changes; clusters become `ClusterIndex` values whose `centroid` is `weighted_centroid` of member vectors weighted by fidelity. The centroid is stored on the index. Member text, state, and vector are not written. Newly joined members get `consolidation_depth += 1`. Missing member–member edges are added as `EdgeKind::Semantic` with label `co-member:<id>`. Uncalibrated, abstaining, contradictory, low-`same_truth`, anchor, or missing-text clusters go to review via `judge_cluster` and do not deepen. Phase 4 `on_neglect`. The function errors if the store shrinks or the graph loses nodes or edges. `forgiven`, `archived`, and `pruned` on the report stay 0.

`apply_release` is the only path here that changes `LifecycleState` (`forgive` / `archive`), and only for an approved `ReleaseDecision`. It sets `text_purge_pending` and does not purge.

`LinkEvent::ConsolidatedInto → Annamanna` is the member-to-index condition in `patthana.rs`. `bhavana_cycle` itself links members to each other, not through `LinkEvent`.

### (b) Notes

`citta-vithi.md`: bhavaṅga is moments 1–3 (quiet context, vibration, arrest). Tadālambana is moments 16–17, a two-stage commit of the node and then the edges. The note does not describe sleep. `02` §4 describes a dream pipeline whose phase 3 replaces a cluster with one centroid, re-points edges, and archives ancestors, and whose §4.3 drops the dense vector below fidelity 0.10 after a ghost echo. Addendum A §A.1.1: "Centroid becomes a cluster node pointing at raw members. The members' α decreases (becoming structure), but their text is kept." `INDEX.md` puts "nightly calibration inside bhāvanā" in the plan row. That is the plan's name for the nightly cycle. It is not `citta-vithi.md`'s definition of bhavaṅga.

A.5.3: a recall write-back is an overlay `kind = reconstruction`; "Raw akkhara is never modified by recall."

### (c) Smallest change

Keep `bhavana_cycle` as an index: centroid on the cluster node, members kept, depth increased only for members that newly join, no prune. Do not implement note 02 §4.2–4.3 (replace members, archive ancestors, drop vectors).

Keep `Phase::Resting` as the bhavaṅga the `life.rs` comment already describes. Do not rename `Phase::Asleep` to bhavaṅga. `citta-vithi.md` does not call the maintenance cycle sleep. The dream row is a new row on the dream channel. It does not modify the traces it drew on.

`propose_dream` ranks residue by `intensity`. After §3, intensity on a trace is vedanā only when that trace's tone came from `calibrated_answer()`. Residue for untagged traces is unsorted by intensity (keep them in the order the caller passed, still truncated to 5). Distant and unresolved draws stay entropy draws.

### (d) Invariants

The index path does not conflict. Note 02's centroid replacement and vector drop would delete content. They conflict with invariants 1 and 2. `judge_cluster` already sends uncalibrated merge verdicts to review and does not deepen those members. Keep that. `text_purge_pending` on a deliberate release is the unresolved clash between invariant 1 and addendum upekkhā/nirodha. This change does not start performing that purge. Dream ranking must ignore intensity that did not come from a calibrated vedanā answer (invariant 3).

## 6. The gates

### (a) Code today

`gates.rs` module comment: decision is not generation. Implementations in this file: `NoModelGate` (always `Abstain(NoModel)`, `calibrated: false`) and `StaticMergeGate` ("NOT a judgment"; `calibrated` is a fixture flag). Traits: `VedanaGate::judge(agent, text)`, `SatiRecallGate::judge(agent, query, memory)` → `{ answers_query, relevance 0..3 }`, `MergeGate::judge(agent, members)` → `{ same_truth, contradiction }`, `TaskSucceededGate::judge(agent, transcript, evidence)` → `{ p }`. There is no saññā trait in this file.

`Judged::calibrated_answer` returns `None` unless the provenance is calibrated and the state is not truncated. `bhavana::judge_cluster` uses that. Uncalibrated means review, with the reason "merge gate not calibrated: no lifecycle change before measured calibration."

`vithi::Stage::Investigating` is a record ("known or new? feeling-tone, recognition"). It does not apply a cosine cutoff. `bhavana` clusters at cosine 0.85 (`consolidation_threshold`). Note 02's 0.92 short-circuit is not in these files.

### (b) Notes

`citta-vithi.md` §2: santīraṇa is where "a quality gate should sit. If an incoming nimitta is too similar to an existing memory (via cosine similarity), the vīthi should short-circuit here to prevent duplicate entropy." `02` moment 8 names the cutoff: similarity `> 0.92` short-circuits.

`2026-09-26-gates.md` lists the five gates, says the saññā trait is absent, and says lifecycle wiring waits on held-out ECE `< 0.05`. Empty text is not an abstention in the vedanā prompt. Abstention is not a negative answer.

Addendum A.0 item 5: "New gate: task_succeeded, with a strict judging rule." The addendum does not give it a Pāli condition. A Pāli name for `task_succeeded` is **UNSUPPORTED**. A.5.3's sati monitor measures retrieval skew and returns to the object. It is not the `SatiRecallVerdict.relevance` score in 0..3. That 0..3 range is the gates note's contract. Equating the two is **UNSUPPORTED**.

`PAPER_DRAFT.md` §3.5: "no lifecycle change before measured calibration."

### (c) Smallest change

Leave every gate advisory. Do not let `task_succeeded`, `relevance`, `intensity`, or an uncalibrated `p` call `forgive`, `archive`, `on_recall`, `decay_tick`, or `MemoryStore::remove`.

If a santīraṇa duplicate check is added later, the new process closes as `Close::DeterminedOnly` (no registration of the duplicate). The existing row stays, text included. The 0.92 figure may be the threshold only as note 02's engineering number, recorded as such. It is not a doctrine quote. Do not add it in the vedanā or paṭṭhāna work.

Saññā stays without a trait until that trait exists. Do not store a saññā class inside `Emotion`.

### (d) Invariants

No conflict while `calibrated_answer()` remains the only lifecycle reader and this change adds no reader. A short-circuit that deleted the older duplicate would break invariant 1. Closing the new process without registering it does not.

## 7. Four conditions for the next build

These are the mappings for "a new row and its causes." Code glosses are not evidence. Where the notes never name the situation, the condition is a definition-match or **UNSUPPORTED**, as marked.

### 7.1 A reflection caused by a reading — purejāta

**Code today:** `ReflectedOn → Nissaya`.

**Nissaya is UNSUPPORTED for this phrase.** Note 10: "Dependence Condition (Foundational Substrate / Infrastructure)." Note 08: "the underlying memory arena, base model weights, or hardware buffer that supports active computation." A reading is not that substrate.

**Condition to record: `Purejata`.** Note 10 §3.1: "Purejāta represents prior physical matter that arose earlier and persists long enough to become the object of a subsequent mind-moment." Note 08: "Phenomena that exist prior to an event and provide the conditioning context (past tokens conditioning present tokens)" with `t_cause < t_effect`. A reading that is already there, and that the reflection takes as its object, is that sentence. This is a definition-match. The notes do not use the words "reflection" or "reading" in that section.

Ārammaṇa is the object condition (note 10: "Object Condition (Sensory / Attentional Target)"; note 08: what the step is directed towards). Purejāta's sentence already says the prior matter becomes the object. One edge, `purejāta`, from the reading to the new reflection. Do not also require an ārammaṇa edge for the same cause. `ReadFrom → Arammana` stays the edge for the reading's own row, which is a different event.

### 7.2 A verdict superseding a memory — UNSUPPORTED

**Code today:** `Supersedes → Adhipati`. The code comment says the verdict "replaces a memory's claim" and "predominates over the memory wherever the two meet."

**Adhipati is UNSUPPORTED for this phrase.** Note 10: "Predominance Condition (Top-Down Executive Goal Override)." Note 08: "Executive goal priority, high-level task constraint, or dominating motivational drive (e.g. system instructions, user intent) that overrules local activations." A verdict is not that goal.

**No note names a condition for the supersession itself.** The operational rule that is written down, in `episode-causal-execution.md`: a Supersede is allowed only when a link in `{vipāka, kamma, purejāta, pacchājāta, upanissaya}` is already recorded between the evidence and the memory. That set is a precondition. It is not the label of a new supersession edge. Do not pick one of the five and call it the supersession.

Nearest sentence, not an assignment: note 08 pacchājāta is "reward signals that arise after an action but condition its future survival." It does not mention a verdict or a replaced claim. Pacchājāta stays **UNSUPPORTED** as the name of this event.

**Spec:** Do not emit `Adhipati` for `Supersedes`. Emit no new condition. The new verdict row may outrank the old row at retrieval only when one of those five links already exists. Outranking changes rank. The old row's text stays. Addendum A.5.3: "Raw akkhara is never modified by recall." `central-demo-falsifiable-tests.md`: a scoped observation "is never superseded, marked false, or edited." `Disputes → Arammana` may stay for "this verdict takes that memory as its object and names no winner," which matches the object condition. It is not a win.

### 7.3 A reply caused by an email — anantara

**Code today:** `NextTurn → Anantara`. The comment is "the next turn of a conversation." Email is not named.

**Condition to record: `Anantara`.** Note 10 §3.1: anantara and samanantara are the model of "autoregressive token generation and Markovian dialogue turns." Note 08 row 4: "State `S_t` ceases completely to make room for State `S_{t+1}` without intermediary gaps."

An email-specific condition is **UNSUPPORTED**. The word "email" in `research/` is vedanā dataset text, not a paccaya. A reply that immediately follows an email uses the same `anantara` edge as a reply that immediately follows any other message. If the email is also what the reply is about, that object is the existing object condition only when a separate object edge is already how `ReadFrom` works; do not invent an email variant. Samanantara (uninterrupted sequence, note 08 row 5) is a second definition of sequence and is not the row note 10 ties to dialogue turns. Use anantara.

### 7.4 A dream drawing on traces — upanissaya for distant traces only

**Code today:** every `DreamedFrom` is `Upanissaya`.

**Distant traces: `Upanissaya`.** Note 10 §3.2: upanissaya "operates across vast temporal and spatial distances. A deep past trauma or powerful insight can decisively condition a present thought without any intermediate physical contact." Note 08: "Heavy long-term memory influence, pervasive priors, or entrenched habituated patterns." `propose_dream`'s `distant` pool is the older set drawn by entropy. That pool is the definition-match. The notes do not use the word "dream" in those sentences. Calling the dream a bhavaṅga process is **UNSUPPORTED** (`citta-vithi.md` bhavaṅga is the idle stream).

**Today's residue: `Purejata`, not upanissaya.** Residue is today's traces, ordered by intensity. Note 10's upanissaya is across vast distance. Today's traces are prior context, which is purejāta's sentence in §7.1. Split `DreamedFrom` by pool.

**Unresolved observations: UNSUPPORTED.** No row in note 10 or note 08 says an unexplained observation is a named condition of a later dream. Draw them as material, and add no paccaya edge until a note assigns one.

The dream row does not modify the traces (invariant 1, and A.5.3).

## What the next two builds implement

1. **Vedanā at write:** §3. Calibrated three-tone on `Emotion.primary`, or `None`. No intensity, no α change, no text change.
2. **Links on a new row:** §7. `ReflectedOn` becomes `Purejata`. `Supersedes` emits no adhipati edge and does not rewrite the old row. `NextTurn` stays `Anantara`, including a reply to an email. `DreamedFrom` is `Upanissaya` for `distant`, `Purejata` for `residue`, and no condition for `unresolved`.

Decay, recall strengthening, sleep/consolidation, and the gates need no behavior change for faithfulness beyond the refusals in §1, §2, §5, and §6: do not delete on fade, do not add `+0.10` fidelity, do not replace cluster members with a centroid, do not let an uncalibrated gate drive the lifecycle.
