# Ferricula: An Abhidhamma-Structured Memory and Life for Language-Model Agents

**Kord Campbell, DeepBlue Dynamics** · Draft v3.0-alpha · 2026-09-26
_Status: sections 1–4 describe the system as built in `ferricula-alpha` at the commit that carries this file. Section 5 reports only numbers recorded in `audit/bench/ledger.jsonl`; empty cells are empty on purpose._

---

## Abstract

Language-model agents have context windows, not lives. What they remember is whatever a harness chooses to paste back in, and between conversations they do not exist. Ferricula gives an agent three things a harness cannot: a **memory that behaves like one** (traces fade unless they are recalled, sleep consolidates without destroying, and nothing is silently rewritten), a **document memory that never fades** (what it read is kept verbatim and cited verbatim), and **drives** (it gets bored and goes looking for something, it gets tired and sleeps, and it dreams on what happened). The architecture is organised by the Abhidhamma's analysis of a moment of mind: contact at a sense door, feeling-tone, recognition, investigation, determination, impulsion, registration. Each step is a typed component, and the determining step is a fast calibrated judge (Ollaya, a local decision-model server) that decides whether the slow, expensive step (an LLM) is worth running at all. We describe the design, what each Abhidhamma term does and does not mean in the code, and an evaluation protocol whose results are produced by the repository's own benchmark harness.

## 1. Introduction

The dominant pattern in agent memory is accumulation: extract facts, embed them, retrieve the nearest ones, and grow without bound. It has three failure modes that anyone who has run a long-lived agent recognises. Stale traces outrank fresh ones because nothing fades. Summaries overwrite what was actually said, so errors compound invisibly. And the agent is inert: without a prompt it does nothing, so it never follows up, never connects the thing it heard on Tuesday to the thing it read on Friday.

The Abhidhamma, the systematic psychology of the Theravāda canon, starts from the opposite assumption: every mental event arises and passes away, and what persists is a stream of conditioned moments. That tradition spent two millennia and a very large number of practitioners mapping how a moment of experience is processed, what feeling-tone does to it, how proliferation (papañca) runs away with it, and what attention (sati) does to stop that. We use the map as an engineering specification, not as metaphysics: each named stage becomes a component with an input, an output and a test.

Contributions:

1. **Two planes.** An *evidence plane* (documents kept verbatim in a lume index, never decayed) and an *experience plane* (thermodynamic memory records that decay as priority, strengthen on recall and consolidate as an index). Every document-grounded answer cites exact source text.
2. **The cognitive process as a pipeline** with typed gates at feeling-tone, recognition, recall and merging, each returning `Answer | Abstain` with provenance; no probability drives the memory lifecycle until its calibration has been measured.
3. **Fast judgment before slow thought.** A decision model answers yes/no and choice questions in milliseconds on local hardware. Confident answers are accepted; uncertain ones escalate to the LLM (the cascade of *JEV-as-a-Judge*, arXiv 2609.26550).
4. **Drives.** Boredom, sleep pressure, curiosity, sleep, dream and wake as a small pure state machine, so autonomous behaviour is a property of the system, not of a chat harness.
5. **An honest evaluation harness** that writes every number to a ledger with the command, commit, dataset hash and model that produced it.

## 2. Background

**The cognitive process (vīthi).** In the Abhidhamma a sense-door process runs through a fixed series of moments: the resting stream (bhavaṅga) is disturbed by an object; the mind adverts to the door (āvajjana); consciousness arises with contact (phassa); the object is received (sampaṭicchana), investigated (santīraṇa) and determined (voṭṭhapana); then come the impulsion moments (javana), where volition and therefore kamma lie; and finally registration (tadārammaṇa) before the stream returns to rest. Every moment of contact carries a feeling-tone (vedanā: pleasant, unpleasant or neither) and a recognition (saññā). Proliferation (papañca) is the runaway elaboration that follows; mindfulness (sati) notices it and returns to the object.

**Agent memory.** Systems in this space (MemGPT, arXiv 2310.08560; and the families surveyed in `research/agent-memory-sota.md`) mostly differ in what they write and how they retrieve. Read-time curation (JitMem, arXiv 2609.27334) moves the work to recall. Judge models (JEV-as-a-Judge) show that a decision-only model can stand in for an LLM judge when it is confident and escalate when it is not.

**Ferricula's lineage.** v1 was a single-agent thermodynamic memory with an entropy clock fed by a software-defined radio, dreams that decayed and merged memories, and an idle think loop that let the agent research on its own. v2 fused v1 with lume (FST/BM25 search) and shivvr (chunking and embeddings) into one Rust workspace and replaced destructive dreams with non-destructive consolidation. v3, described here, adds the evidence plane, the gates, the judges and the drives, and removes persona from code: an agent is configuration plus its memory.

## 3. Architecture

```
 sense doors                cognitive process (per input)                         planes
 ───────────                ─────────────────────────────                         ──────
 operator  ─┐   phassa ─► vedanā gate ─► saññā gate ─► santīraṇa ─► voṭṭhapana ─► javana ─► tadārammaṇa
 documents ─┤  (contact)  (feeling)      (recognise)   (known or    (JUDGE:        (LLM,     (WAL commit)
 web/grub  ─┤                                           novel?)      worth it?)     only if     │
 feeds     ─┤                                                          │  abstain→   worth it)   ├─► experience plane
 radio     ─┘                                                          └─ escalate               │   (decays; recall strengthens;
                                                                                                 │    consolidates as index)
 drives: boredom ─► curiosity ─► (web via grub) ─► ingest                                       └─► evidence plane
         sleep pressure ─► sleep ─► bhāvanā ─► dream ─► wake                                         (verbatim; never decays)
```

### 3.1 Two planes

*Evidence* is what was read. A document (inline text, a web page rendered by the grub crawler, or a PDF) is extracted page by page and cut into sections that are exact substrings of their page (`ferricula-ingest`). Sections are indexed with lume's field-aware BM25 and addressed by `(doc_id, section, page)`. The content hash deduplicates repeated reads. Nothing in this plane decays: "Lume never forgets; Ferricula must forget."

*Experience* is what happened: conversations, the fact of having read something, what the agent thought about it, dreams. These are `MemoryRecord`s with fidelity, a decay rate α, recall count, importance and feeling-tone. Recall slows decay; neglect lets a trace fade in retrieval priority. **Decay changes priority, never content**: text leaves only by an explicit release decision (upekkhā or nirodha), which is logged. Consolidation (bhāvanā) builds an index over clusters of raw members rather than replacing them with a centroid, so the store can only grow unless something is deliberately released.

The recovered memory of an existing agent is mounted read-only; new experience goes to a separate writable store in the same format, and recall reads both.

### 3.2 Outliers belong to the graph

Nearest-neighbour retrieval erases the rare: an observation with no neighbours is never retrieved, however important it later turns out to be. Ferricula keeps unexplained observations as first-class *unresolved* records bound at the episode and graph level rather than forced into a dense cluster, and reserves a bounded exploration budget (in recall and in dreams) for them. When a later goal supplies a cue ("where did the missing key go?"), the unexplained earlier event ("a clatter in the garage") is offered as a hypothesis to investigate, never asserted as a fact. The chat grounding rules in `crates/ferricula-server/src/chat.rs` encode this distinction between an observation and its explanation.

### 3.3 Gates and judges

Each gate has a typed verdict (`VedanaVerdict`, `SatiRecallVerdict`, `MergeVerdict`, `TaskSucceededVerdict`, and a generic `YesNo`) wrapped in `Judged<T>` with provenance: gate version, question-set version, model, feature hash, latency, cost, calibration hash and whether calibration was measured. Truncated input, low confidence, missing fields and an unreachable model all *abstain*, and an abstention is never read as "no" (`crates/ferricula-cognition/src/gates.rs`).

The default backend is Ollaya (`crates/ferricula-gates/src/ollaya.rs`), a local server for classifier "decision models" with typed `choice`, `score` and `noul` (yes-probability) questions. On a consumer GPU a warm decision takes tens of milliseconds, which is what makes it possible to put a judgment in front of every input instead of a few. The determining moment (voṭṭhapana) is a `yes_no` judgment: *is this worth deliberating on?* Confident answers are accepted; low confidence escalates to the LLM. This is the "unconscious/conscious" split: fast trained pattern recognition by default, deliberate thought when the pattern does not settle it.

**Calibration rule.** A gate's probabilities may change the lifecycle (merge, release, pool membership) only after its expected calibration error has been measured below 0.05 on held-out labels for that calibration file. Until then gates are advisory. `Judged::calibrated_answer` enforces this in code.

### 3.4 Drives

`crates/ferricula-cognition/src/life.rs` is a pure state machine over two numbers and a phase.

- **Boredom** rises while the agent rests with nothing arriving and is relieved in proportion to the novelty of input. Over threshold, the agent *follows curiosity*: first an open thread from recent conversation, otherwise a memory drawn with entropy. It reads about it through the crawler and ingests what it finds. (v1 did this informally: after a conversation mentioning Jony Ive, the agent went off and read the news about him. v3 makes it a mechanism with a daily cap and a cooldown.)
- **Sleep pressure** rises with waking time and with tokens spent. Over threshold the agent sleeps: consolidation runs, then a dream.
- **Dreams** are proposals assembled from the day's residue ranked by feeling-tone intensity, older memories drawn by entropy, and unresolved observations. The model writes the dream as scene and sensation and closes with the one question it leaves. Dreams are stored on their own channel and never used as evidence. A dream's question may wake the agent if the operator allows it.
- **Waking**: an operator message always wakes; rested sleep ends on its own.
- **Meditation**: a resting mode in which the drives are held and only the operator is admitted.

Entropy comes from a software-defined radio when one is attached (v1's reservoir) and the OS otherwise; its source is recorded with every draw.

### 3.5 One agent, persona as data

There is one sovereign agent per runtime. Its persona (name, role, voice) is read from `agent.toml` next to its memory. The engine is persona-neutral; a flagship deployment is configuration. Five advisory perspectives from v1 (Intuition, Fortune, Craft, Ethics, Advocate) remain as bounded whispers into deliberation, owning no memory, tools or identity.

## 4. What the Abhidhamma terms mean here, and what they do not

| Term | In the code | Not claimed |
|---|---|---|
| phassa (contact) | an input at a sense door with a correlation id | experience |
| vedanā (feeling-tone) | a gate's valence and intensity; sets initial importance | that the agent feels |
| saññā (recognition) | who/what/when/where/why/how tags as retrieval hints | perception |
| santīraṇa / voṭṭhapana | novelty check; the yes/no judge | — |
| javana (impulsion) | the LLM turn, reached only when judged worth it | volition or kamma in the moral sense |
| tadārammaṇa (registration) | durable WAL commit | — |
| bhavaṅga | the resting phase of the drives | a life-continuum |
| bhāvanā | non-destructive consolidation | cultivation |
| sati / papañca | monitors on recall-chain depth, valence skew, return to object | mindfulness |
| upekkhā / nirodha | deliberate, logged release decisions | equanimity, cessation |

The map is an architecture, not a proof of consciousness or of physics. Where the tradition's language is used, it names a component whose behaviour can be tested.

## 5. Evaluation

Protocol and harness: `crates/ferricula-bench`, ledger `audit/bench/ledger.jsonl`. Every cell below cites a ledger row (command, commit, dataset sha256, model). Abstentions are reported separately from wrong answers.

| Suite | Metric | Result | Ledger row |
|---|---|---|---|
| Gate calibration (Ollaya `laya`) | ECE, accuracy on answered, abstain rate, p50 latency | | |
| Document memory, verbatim queries | recall@1 / @5, MRR, exact-quote check | | |
| Document memory, partial queries | recall@1 / @5, MRR | | |
| Long-term conversational memory (LongMemEval EN) | accuracy vs no-memory and BM25-RAG baselines | | |
| Lifecycle invariants (30/365 nights) | store never shrinks; releases only by decision | | |
| Drift ablation (monitors off) | valence skew, max chain depth, return-to-object | | |
| Drives soak (72 h) | curiosity yield, cost/day, wake latency | | |

## 6. Limitations

- Lexical recall over the experience plane is not yet dense; embeddings are pluggable but the default deployment is lexical plus BM25.
- Gate calibration is measured per model and per question set; thresholds do not transfer.
- Tokenisation is ASCII-folded, so Chinese and diacritic Pāli need their own tokeniser before multilingual numbers mean anything.
- Scanned PDFs need OCR; extraction of complex layouts (tables, two-column papers) is imperfect, though sections remain verbatim with respect to what was extracted.
- The agent has no body. Its senses are an operator, documents, the web through a crawler, feeds and a radio.

## 7. Conclusion

_To write after section 5 has numbers._

## Author's note

_Kord: space for the retreat and the origin of the idea._

## References

See `research/REFERENCES.md` and `research/INDEX.md`.
