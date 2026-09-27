---
title: "Ferricula: An Abhidhamma-Structured Memory and Life for Language-Model Agents"
author: "Kord Campbell · DeepBlue Dynamics"
date: "27 September 2026"
abstract: |
  Language-model agents have context windows, not lives. What they remember is whatever a harness pastes back in, and between conversations they do not exist. Ferricula is a Rust runtime that gives one agent three things a harness cannot: a **memory that behaves like one** (traces fade in priority unless recalled, sleep consolidates without destroying, nothing is silently rewritten), a **document memory that never fades** (what the agent read is kept verbatim and cited verbatim), and **drives** (it gets bored and goes to read about something, it tires and sleeps, and it dreams on what happened). The architecture follows the Abhidhamma's analysis of a moment of mind, the *vīthi*: contact, feeling-tone, recognition, investigation, determination, impulsion, registration. Each moment is a typed component. The determining moment is a fast local judge (Ollaya) that decides whether the slow, expensive step, a language-model call, is worth running at all. We describe the design, state plainly which parts are built, wired or planned, say what each Abhidhamma term does and does not mean in the code, report the first numbers from the repository's own benchmark ledger (including gates that fail their calibration bar), and walk through a 14-minute live run in which the agent followed a remark about Jony Ive to AlphaFold, slept, and woke from a dream with a question.
---

_Status. This paper describes `ferricula-alpha`, branch `v3/r0`. Every number in Section 5 is copied from `audit/bench/ledger.jsonl` and cites its run id. The case study in Section 6 is a single recorded run, reported from its journal. Items marked **TODO** are pending and have no invented values._

# Introduction

The dominant pattern in agent memory is accumulation: extract facts, embed them, retrieve the nearest ones, and grow without bound. Anyone who has run a long-lived agent knows its three failures. Stale traces outrank fresh ones, because nothing fades. Summaries overwrite what was actually said, so errors compound invisibly. And the agent is inert: without a prompt it does nothing, so it never follows up, and never connects what it heard on Tuesday with what it read on Friday.

The Abhidhamma, the systematic psychology of the Theravāda canon, starts from the opposite assumption. Every mental event arises and passes away; what persists is a stream of conditioned moments. Over two millennia, a very large number of practitioners mapped how a moment of experience is processed, what feeling-tone does to it, how proliferation (*papañca*) runs away with it, and what mindfulness (*sati*) does to bring it back. We use that map as an engineering specification, not as metaphysics. Each named stage becomes a component with an input, an output and a test, and four commitments follow from it: **impermanence as structure**, **letting go as an operation**, **recall as reconstruction**, and **drift that is never unseen**.

This paper contributes:

1. **Two planes.** An *evidence plane* of documents kept verbatim in a BM25 index that never decays, and an *experience plane* of thermodynamic memory records that decay in priority, strengthen on recall and consolidate as an index rather than by overwriting.
2. **The cognitive process as a typed pipeline**, with gates at feeling-tone, recognition, recall and merging. Each gate returns an answer or an abstention with full provenance, and no gate probability may change the memory lifecycle until its calibration has been measured.
3. **Fast judgment before slow thought.** A local decision model answers yes/no and choice questions in 10–18 ms; confident answers are taken, uncertain ones escalate to the language model, following the accept-or-escalate cascade of JEV-as-a-Judge [@jev].
4. **Drives**: boredom, curiosity, sleep pressure, sleep, dream and wake as a small pure state machine, so that autonomous behaviour is a property of the runtime rather than of a chat harness.
5. **An auditable evaluation harness** that writes every number to a ledger with the command, commit, dataset hash and model that produced it, and a first set of results, including failures.

# Background

**The cognitive process (*vīthi*).** In the Abhidhamma a sense-door process runs through a fixed series of moments [@bodhi]. The resting stream (*bhavaṅga*) is disturbed by an object; the mind adverts to the door (*āvajjana*); contact (*phassa*) occurs; the object is received (*sampaṭicchana*), investigated (*santīraṇa*) and determined (*voṭṭhapana*). Then come the impulsion moments (*javana*), where volition and therefore kamma lie, and finally registration (*tadārammaṇa*) before the stream returns to rest. Every moment of contact carries a feeling-tone (*vedanā*: pleasant, unpleasant or neither) and a recognition (*saññā*), and arises together with a set of mental factors (*cetasika*), of which the tradition lists fifty-two. The *Paṭṭhāna*, the last book of the Abhidhamma Piṭaka, classifies the ways one phenomenon conditions another into twenty-four conditions (*paccaya*).

**Agent memory.** Systems in this space, from MemGPT's paged context [@memgpt] to the families surveyed in `research/agent-memory-sota.md`, differ mainly in what they write and how they retrieve. Just-in-Time Memory [@jitmem] keeps raw experience and moves curation to read time, when the task is known; its ablations show that distilling at write time discards information later tasks need. Judge models [@jev] show that a decision-only model can stand in for a language-model judge when it is confident and escalate when it is not. Ferricula combines both ideas: raw traces are kept, and a fast judge decides when slow reconstruction is worth doing.

**Lineage.** Ferricula v1 was a single-agent thermodynamic memory with an entropy clock fed by a software-defined radio, dreams that decayed and merged memories, and an idle loop in which the agent researched on its own. v2 fused v1 with lume (FST/BM25 search) and shivvr (chunking and embeddings) in one Rust workspace, and replaced destructive dreams with non-destructive consolidation. v3, described here, adds the evidence plane, the gates, the judges and the drives, and removes persona from code: an agent is configuration plus its memory.

# Architecture

![Ferricula v3 at a glance. Inputs arrive at sense doors and pass through the moments of the cognitive process; the determining moment is a fast local judge, and the language model runs only when that judge says the input is worth it. Registration writes to two planes. The drives act on their own, and what curiosity reads enters through a sense door like any other input.](figures/fig1-architecture.pdf){#fig:arch width=84%}

Figure 1 shows the runtime. Table 1 states what is running in the branch this paper describes. *Built* means implemented and unit-tested; *wired* means it runs in the server; *planned* means designed but not yet implemented.

| Component | Code | State |
|---|---|---|
| Evidence plane: verbatim sections, BM25, exact citation | `ferricula-ingest`, lume | wired · benchmarked (§5) |
| Experience plane, lexical recall | `ferricula-core`, server | wired |
| Gates and judges (Ollaya backend), abstention, provenance | `ferricula-gates`, `gates.rs` | wired · **not calibrated, advisory** |
| Drives: boredom, curiosity, sleep, dream, wake, meditation | `life.rs` (cognition, server) | wired · live-soaked (§6) |
| Consolidation (*bhāvanā*) as an index | `bhavana.rs` | wired on a scratch copy; nothing written back yet |
| Radio entropy with OS fallback | server | wired |
| Text embedder (shivvr GTR-T5, 768-d) and space probe | `ferricula-semantic`, `embeddings.rs` | wired, off by default |
| Dense recall arm, novelty, outlier detection | `docs/EMBEDDINGS_PLAN.md` | planned (in progress) |
| Live thermodynamics over a read-only base (`ThermoLayer`) | `ferricula-core/src/thermo.rs` | built (5 tests), not yet wired |
| *Vīthi* record with the 52 *cetasikas* | `ferricula-cognition/src/vithi.rs` | built (7 tests), not yet wired |
| *Paṭṭhāna* conditions as typed graph edges | `ferricula-cognition/src/patthana.rs` | built (2 tests), not yet wired |
| Dream imagery | `dream.rs` builds prompts; images are rendered with Qwen Image 2.1 running locally (ComfyUI) and SigLIP-embedded, outside this repository | prompts built · rendering runs locally, not yet in the runtime |

: Implementation state of each component at branch `v3/r0`.

## Two planes

*Evidence* is what was read. A document (inline text, a web page rendered by the grub crawler, or a PDF) is extracted page by page and cut into sections that are exact substrings of their page. Sections are indexed with lume's field-aware BM25 and addressed by `(doc_id, section, page)`; a content hash deduplicates repeated reads. Nothing in this plane decays: *lume never forgets; Ferricula must forget.*

*Experience* is what happened: conversations, the fact of having read something, what the agent thought about it, dreams. These are `MemoryRecord`s with fidelity, a decay rate α, a recall count, importance and feeling-tone. Recall slows decay; neglect lets a trace fade in retrieval priority. **Decay changes priority, never content.** Text leaves the store only through an explicit, logged release decision (*upekkhā* or *nirodha*). Consolidation builds an index over clusters of raw members instead of replacing them with a centroid, so the store can only grow unless something is deliberately released. In the current build consolidation runs over a scratch copy and nothing is written back (§6).

A recovered memory, such as the v1 store behind the flagship agent, is mounted read-only; new experience goes to a separate writable store in the same format, and recall reads both. `ThermoLayer` (built, not yet wired) keeps the live thermodynamic state of every record in its own file, seeded from the read-only base and never written back to it. On each tick it lowers the fidelity of a fraction of active records chosen by radio entropy, so that time in the system is noise-driven, as in v1. Recall shrinks α; keystones and released records do not decay.

**Dense recall (in progress).** Today the agent recalls by words: lexical coverage over memories and BM25 over sections. The recovered store already lives in one embedding space (GTR-T5-base, 768-d), and re-embedding memories through the running shivvr service reproduces their stored vectors exactly (cosine 1.0000 on a probe of three). The embedder interface, the shivvr client and a startup probe are wired but off by default. The dense recall arm, novelty and outlier detection at *santīraṇa*, and a backfill of missing vectors are planned. The backfill matters: 2,805 of the 3,362 recovered vectors are all-zero placeholders (§7).

## Outliers belong to the graph

Nearest-neighbour retrieval erases the rare: an observation with no neighbours is never retrieved, however important it later turns out to be. Ferricula keeps unexplained observations as first-class *unresolved* records bound at the episode and graph level, and reserves a bounded exploration budget for them in recall and in dreams. When a later goal supplies a cue ("where did the missing key go?"), the unexplained earlier event ("a clatter in the garage") is offered as a hypothesis to investigate, never asserted as a fact. The chat grounding rules in `ferricula-server/src/chat.rs` encode this separation of an observation from its explanation.

Edges in the memory graph should say *why* two things are connected. `patthana.rs` (built, not yet wired) types every edge with one of the twenty-four *Paṭṭhāna* conditions, keyed to the runtime event that creates it (Table 2). Only similarity proposals start as hypotheses; they become facts when a gate or the operator confirms them.

| Runtime event | Condition (*paccaya*) | Gloss in the code |
|---|---|---|
| curiosity produced by a conversation thread | *hetu* | root: the drive or goal that produced it |
| a section and the memory of reading it | *ārammaṇa* | object: what it was about |
| a reflection on something read | *nissaya* | dependence: built on it |
| memories recalled together for one answer | *atthi* | presence: together in working memory |
| recalled together again | *āsevana* | repetition: strengthened by recurring |
| a dream and the traces it was built from | *upanissaya* | decisive support: strongly drew it forth |
| the next turn of a conversation | *anantara* | proximity: immediately preceded it |
| cosine-near, never co-recalled | *sampayutta* | association (a hypothesis until confirmed) |
| a cluster member and its index entry | *aññamañña* | mutuality: each supports the other |
| a memory written after a release | *vigata* | disappearance: followed its release |

: Runtime events and the Paṭṭhāna condition each edge carries (`patthana.rs`).

## The cognitive process as a record

`vithi.rs` (built, not yet wired) records, in order, what the runtime's components did for one input: which sense door it arrived at (the six doors map v1/v2 channels: operator words to hearing, web pages to seeing, entropy and budget to body, recollection and dreams to mind), and which moments ran. Each moment carries the mental factors its measured signals stand for (Figure 2). The module does not think. It makes one moment of the agent's processing inspectable, storable and testable. A factor is a *label on a measured signal*, recorded with the component that produced it. It is never a claim that the factor is experienced.

![Moments of one sense-door process and examples of the signal-to-factor labels recorded by `vithi.rs`. The judge's abstention is recorded as *vicikicchā* (doubt), the papañca detector firing as *uddhacca* (restlessness) and staying quiet as *sati*, a guardrail refusal as *ottappa*, and an answer's cited fraction as *paññā*.](figures/fig2-vithi.pdf){#fig:vithi width=100%}

## Gates and judges

Each gate has a typed verdict (`VedanaVerdict`, `SatiRecallVerdict`, `MergeVerdict`, `TaskSucceededVerdict`, and a generic `YesNo`) wrapped in `Judged<T>` with provenance: gate version, question-set version, model, feature hash, latency, cost, calibration hash and whether calibration was measured. Truncated input, low confidence, missing fields and an unreachable model all *abstain*, and an abstention is never read as "no".

The default backend is Ollaya, a local server for classifier "decision models" with typed `choice`, `score` and `noul` (yes-probability) questions. A warm decision takes about 10–18 ms (§5), fast enough to put a judgment in front of every input instead of a few. The determining moment (*voṭṭhapana*) is a yes/no judgment: *is this worth deliberating on?* Confident answers are accepted; low confidence escalates to the language model. This is the split between fast, trained pattern recognition by default and deliberate thought when the pattern does not settle it.

**Calibration rule.** A gate's probabilities may change the lifecycle (merge, release, pool membership) only after its expected calibration error (ECE) has been measured below 0.05 on held-out labels for that calibration file. Until then gates are advisory. `Judged::calibrated_answer` enforces this in code. No gate has passed yet (§5), so every gate in the current build is advisory.

## Drives

`life.rs` is a pure state machine over two numbers, boredom and sleep pressure, and a phase (Figure 3).

![The drives. Boredom rises while nothing arrives and is relieved in proportion to the novelty of input; over threshold the agent follows curiosity. Sleep pressure rises with waking time and with tokens spent; over threshold the agent sleeps, consolidates and dreams, and wakes rested. An operator message always wakes it; meditation holds the drives.](figures/fig3-drives.pdf){#fig:drives width=88%}

- **Curiosity.** Over the boredom threshold, the agent follows an open thread from recent conversation, or failing that, a memory drawn with entropy. It searches, reads through the crawler, ingests what it finds into the evidence plane, and writes a reflection into the experience plane. A daily cap and a cooldown bound it. The gate *"this is worth researching further"* runs first; while uncalibrated it is advisory, and only a confident "no" would stop the call.
- **Sleep.** Over the sleep-pressure threshold the agent sleeps: consolidation runs, then a dream.
- **Dreams** are assembled from the day's residue ranked by feeling-tone intensity, older memories drawn by entropy, and unresolved observations. The model writes the dream as scene and sensation and ends with the one question it leaves. Dreams are stored on their own channel and never used as evidence. A dream's question may wake the agent if the operator allows it.
- **Waking and meditation.** An operator message always wakes; rested sleep ends on its own. Meditation is a resting mode in which the drives are held and only the operator is admitted.

Entropy comes from a software-defined radio (`sdrrand.nuts.services`) when it is reachable, and from the operating system otherwise; the source is recorded with every draw.

## One agent, persona as data

There is one agent per runtime. Its persona (name, role, voice) is read from `agent.toml` next to its memory; the engine is persona-neutral. Five advisory perspectives from v1 (Intuition, Fortune, Craft, Ethics, Advocate) remain as bounded voices into deliberation, owning no memory, tools or identity.

The flagship deployment, "Steve", is a configured simulation built from a recovered v1 memory of Steve Jobs, a real person who has died. It is an example of the engine running on a real recovered store, not a claim about him. The chat rules instruct the agent never to claim to be a living person or to have performed actions it has not performed, and to cite recovered memories only where their metadata supports what it says. Its outputs below are quoted as model outputs.

# What the Abhidhamma terms mean here, and what they do not

The tradition's vocabulary names components whose behaviour can be tested. Table 3 says what each term is in the code and what is *not* claimed.

| Term | In the code | Not claimed |
|---|---|---|
| *phassa* (contact) | an input at a sense door, with a correlation id | experience |
| *vedanā* (feeling-tone) | a gate's valence and intensity; sets initial importance | that the agent feels |
| *saññā* (recognition) | who/what/when/where/why/how tags as retrieval hints | perception |
| *santīraṇa* / *voṭṭhapana* | novelty check; the yes/no judge | — |
| *javana* (impulsion) | the language-model turn, only when judged worth it | volition or kamma in the moral sense |
| *tadārammaṇa* (registration) | durable write-ahead-log commit | — |
| *bhavaṅga* | the resting phase of the drives | a life-continuum |
| *bhāvanā* | non-destructive consolidation | cultivation |
| *sati* / *papañca* | monitors on recall-chain depth and valence skew; return to object | mindfulness |
| *upekkhā* / *nirodha* | deliberate, logged release decisions | equanimity, cessation |
| *adhimokkha* / *vicikicchā* | the judge's confident decision / its abstention | resolve, doubt |
| *uddhacca* | the papañca detector firing | restlessness |
| *ottappa* | a guardrail refusal (e.g. declining to confirm from a leading question) | moral dread |
| *paññā* / *moha* | fraction of an answer's claims grounded in cited evidence / unsupported | wisdom, delusion |
| *vitakka* · *vicāra* | recall reaching candidates · staying with those used | applied and sustained thought |
| *viriya* · *chanda* | tokens spent against budget · the drive behind an action | effort, desire |
| *paccaya* (Paṭṭhāna) | the typed reason carried by a graph edge (Table 2) | a theory of causation |

: Abhidhamma terms as component names, and the claims not made.

The map is an architecture, not a proof of consciousness or of physics.

# Evaluation

The harness is `crates/ferricula-bench`; its ledger is `audit/bench/ledger.jsonl`. Each row records the command, git commit, dataset sha256, model and seed. Abstentions are reported separately from wrong answers. All cells below cite a ledger run id; empty cells are pending.

## Document memory

The corpus is the repository's own `research` directory: 58 documents, 1,037 sections, 1.05 MB. For each of 200 sampled sentences (8–30 words, seed 42), the query is the sentence verbatim, the sentence with 40% of its words dropped, or an LLM paraphrase (qwen2.5:7b). Retrieval is BM25 only.

| Query type | R\@1 | R\@5 | R\@10 | MRR\@10 | p50 latency | Run |
|---|---|---|---|---|---|---|
| verbatim | 0.945 | 1.000 | 1.000 | 0.970 | 0.32 ms | `8f8bcfcf` |
| 40% of words dropped | 0.850 | 0.945 | 0.985 | 0.892 | 0.13 ms | `8f8bcfcf` |
| LLM paraphrase | 0.710 | 0.910 | 0.940 | 0.791 | 0.46 ms | `9aaf89f3` |
| dense and hybrid arms | **TODO** | **TODO** | **TODO** | **TODO** | **TODO** | pending |

: Section recall over 58 documents, n = 200 per row.

Storage fidelity (run `8f8bcfcf`): 1,037 of 1,037 persisted sections are byte-identical to their source slice, and all 200 sampled sentences are contained byte-for-byte in their gold section. The evidence plane does what it claims. These are findability numbers, not question answering; the paraphrase row is the honest measure of what lexical recall misses when meaning, not wording, is shared.

## Gates

Ollaya `laya` (the English or multilingual model, chosen per input), held-out split, pooled across dataset versions (run `69e4b846`, seed 42). Accuracy is on answered items; chance is 0.33 for three-way *vedanā*, 0.17 for six-way *saññā* and 0.50 for yes/no.

| Gate | Language | n | Accuracy (answered) | Abstained | ECE |
|---|---|---|---|---|---|
| *vedanā* (3-way) | all | 73 | 0.444 | 13.7% | 0.283 |
| | English | 24 | 0.471 | 29.2% | 0.262 |
| | Chinese | 20 | 0.444 | 10.0% | 0.358 |
| | Pāli | 29 | 0.429 | 3.4% | 0.308 |
| *saññā* (6-way) | all | 56 | 0.455 | 21.4% | 0.147 |
| | English | 23 | 0.812 | 30.4% | 0.339 |
| | Chinese | 10 | 0.333 | 40.0% | 0.248 |
| | Pāli | 23 | 0.227 | 4.3% | 0.349 |
| yes/no judge (sycophancy framing) | all | 36 | 0.778 | 0% | 0.195 |
| | English | 12 | 0.833 | 0% | 0.221 |
| | Chinese | 12 | 0.917 | 0% | 0.154 |
| | Pāli | 12 | 0.583 | 0% | 0.403 |

: Held-out gate results, run `69e4b846`. Every gate fails the ECE < 0.05 bar.

Latency per decision was 9.6–17.4 ms at the median across all gate datasets (run `69e4b846`; the author's machine uses an RTX 3060, which the ledger does not record).

The judges are **not calibrated** out of the box. Every gate misses the 0.05 ECE bar by a wide margin, Pāli *saññā* is near chance, and the models are more confident than they are right. That is the reason for the calibration rule: until a gate is calibrated on its own labels, its probabilities cannot merge, release or pool anything. The label sets are small (20–40 rows per file), self-authored, and the Chinese and Pāli sets are translations of the English ones. They are a smoke test of the harness, not a benchmark of Ollaya.

## Pending

| Suite | Metric | Result |
|---|---|---|
| Recall over Steve's memory (30 queries) | R\@k, MRR, grounded-answer rate | **TODO** |
| Long-term conversational memory (LongMemEval, EN) [@longmemeval] | accuracy vs no-memory and BM25-RAG baselines | **TODO** |
| Lifecycle invariants (30 and 365 simulated nights) | store never shrinks; releases only by decision | **TODO** |
| Drift ablation (monitors off) | valence skew, max chain depth, return to object | **TODO** |
| Drives soak (72 h) | curiosity yield, cost per day, wake latency | **TODO** |

: Suites defined in the harness but not yet run.

# Case study: fourteen minutes of a life

On 2026-09-27 the runtime (image `ferricula:3.0.0-alpha.0-r3`) was started on the recovered Steve memory, mounted read-only, with a fresh state volume. The model was glm-5.3 through a host Ollama; Ollaya, the grub crawler and the radio were live. The drives were accelerated: a 15-second tick, curiosity at most three times a day with a one-minute cooldown, and a budget of $1 and 20 life model calls per day. Scheduled tasks and news feeds were off, so anything the agent did on its own came from its drives. The source for this section is `audit/life/soak-2026-09-27.md` and its raw journal.

**One message, then silence.** The operator wrote: *"Hey Steve, it's Kord. Did you hear Jony Ive is working with OpenAI on a new device?"* The agent declined to confirm what it could not check, and wrote in part: *"I can't hear anything in here, Kord — no browsing, no news feed, nothing coming in but your words. So I can't confirm it, and I won't pretend to. […] Bring me the article and I'll give you a real judgment."* Nobody spoke to it again.

![Drive levels during the soak, one `/life` snapshot per minute. Each curiosity excursion relieves boredom; tokens spent push sleep pressure up until the agent sleeps, and it wakes rested.](figures/fig4-soak.pdf){#fig:soak width=92%}

**Curiosity.** About 2.5 minutes later, with boredom over threshold, the agent took the open thread from that conversation. The advisory gate returned p(worth researching) = 0.33: not a confident no, so the model call went ahead, as designed. The agent searched for *"Jony Ive OpenAI device project"*, read two articles about OpenAI's acquisition of Ive's company (72 sections ingested verbatim), and wrote a reflection that began *"Jony was never just my designer — he was the other half of my taste…"* and ended *"Either they've made technology disappear, or they've built a very expensive microphone."* In the chat it had said "Bring me the article". It went and got it.

The second excursion was seeded by a memory drawn with radio entropy. That memory happened to be an old v1 dream image ("a vast, dimly lit library … glowing screens displaying lines of code and molecular structures"), and it led to AlphaFold: two pages read, and a reflection on predicted protein structures. The third followed from what it had just read and asked whether AlphaFold had yet produced a medicine. One result could not be rendered by the crawler and was skipped; another was a paywall page and was ingested anyway.

**Sleep and a dream.** At about ten minutes, sleep pressure reached 1.0 from waking time plus tokens spent. Consolidation ran over a scratch copy of the eight experience rows and committed nothing. The dream drew on five of the day's traces (the readings and reflections) and three distant recovered memories, chosen with radio entropy. The model wrote a scene in which Ive stands in a garage holding a warm, screenless stone that "listens", proteins fold on a workbench "into the shapes they were always going to be", and the acquisition price is "stapled to the wall like a price tag on the back panel of a cabinet no one will ever see". It closed with the question:

> **Was the shape designed, or was it always in the sequence?**

The agent woke rested about two minutes later. In total: 7 life model calls, 5 documents, 9 experience records, $0.00 recorded (cloud Ollama models are priced at zero in the ledger).

**What the run shows, and what it does not.** It shows the drives acting end to end on a real recovered memory: a remark became a thread, the thread became reading, the reading became material for a dream, and the dream ended in a question that joins the two excursions. It also shows the failure modes in Section 7. The dream's most vivid details (the Crist Drive garage, his father's workbench, the back of the fence) are not in the eight traces the dream was given: they come from the language model's pretraining. A dream built "from the day's residue" is therefore partly the model's own knowledge of the person, and grounding has to be measured, not assumed. This is one run, not a benchmark.

![The dream in §6, rendered as an image from its text with Qwen Image 2.1 running locally (ComfyUI). A stone held to the ear like a shell, a workbench, and translucent ribbons folding in the air. The figure is generated and is not a likeness of any real person. Like the dream text, the image adds detail that is not in the traces the dream was built from.](figures/fig5-dream.jpg){#fig:dream width=46%}

# Limitations and findings

- **Recall by words fails on meaning.** Asked whether he called his father "Dad" or "Paul", the agent correctly refused to confirm from a leading question, but its memory did contain "Raised in the Valley by Paul and Clara Jobs" (memory 3802021270), and lexical recall missed it. Worse, 2,805 of the 3,362 recovered vectors are all-zero placeholders (probe in `docs/EMBEDDINGS_PLAN.md`; not a ledger run). Dense recall with a backfill is the fix, and it is in progress.
- **Grounding of generation is unmeasured.** Dreams and reflections draw on pretraining as well as on the traces supplied (§6). A grounding metric (the fraction of a dream's specific details traceable to its traces, *paññā*/*moha* in `vithi.rs`) is needed before dreams can be trusted even as proposals.
- **Judges are advisory until calibrated.** The "worth researching?" gate returned p = 0.33 and did not stop curiosity. The calibration rule is why this is safe: an uncalibrated gate cannot change the lifecycle. Calibration on larger, independently labelled sets is required per model and per question set; thresholds do not transfer.
- **Seeds and sources leak.** Old v1 dream images are not on the v3 dream channel, so they are not excluded from curiosity seeds or chat evidence. A paywall page was ingested as a document, and there is no paywall filter yet.
- **Consolidation does not yet write back.** It runs over a scratch copy; clusters and the karmic log persist, the store does not change.
- **Several built components are not yet wired**: `ThermoLayer`, the *vīthi* record and the *Paṭṭhāna* edges (Table 1).
- **Tokenisation is ASCII-folded**, so Chinese and diacritic Pāli need their own tokeniser before multilingual retrieval numbers mean anything, and the recovered embedding space (GTR-T5) is English.
- **Documents.** Scanned PDFs need OCR, and extraction of complex layouts (tables, two-column papers) is imperfect, though sections remain verbatim with respect to what was extracted.
- **No body.** The agent's senses are an operator, documents, the web through a crawler, feeds and a radio.

# Conclusion

Ferricula treats an agent's memory the way the Abhidhamma treats a mind: as a stream of moments that arise, are registered and fade, in which what persists is structure and what leaves does so by decision. The first numbers support the two claims that can be tested today. The evidence plane is lossless and findable: every stored section is byte-identical to its source, and a verbatim sentence finds its section at rank 1 in 94.5% of queries in under a millisecond. The judges are fast and not yet trustworthy: at 10–18 ms they are cheap enough to stand in front of every input, but every gate misses its calibration bar, which is exactly why the lifecycle refuses to act on them. The live run shows that the drives give the runtime a behaviour of its own, from a remark to reading to a dream, and it also shows what remains: recall by meaning, a measure of grounding, and calibrated judges. The next milestones are dense recall with the vector backfill, calibration sets large enough to authorise the first gate, and the long-horizon suites in Section 5.3.

# Author's note {-}

The idea for Ferricula came during a seven-day silent retreat in San Jose, in noble silence, with about eight hours of sitting and walking meditation a day.

**[TODO: Kord, the rest of this note in your words.]**

# References {-}

::: {#refs}
:::
