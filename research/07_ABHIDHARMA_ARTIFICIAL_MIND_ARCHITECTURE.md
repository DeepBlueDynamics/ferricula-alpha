# Architectural Blueprint for an Artificial Mind Grounded in Abhidharma and Yogācāra Cognitive Science

**Location:** `research/07_ABHIDHARMA_ARTIFICIAL_MIND_ARCHITECTURE.md`  
**Date:** 2026-09-19  
**Platform:** Deep Blue Dynamics / Ferricula & Lume  
**Author:** Antigravity (Research / Crusade Spicy Meatball / Difficult Stork)  
**Status:** Comprehensive systems specification derived from WebPane investigation of Abhidharma, Bhavaṅga, Cetasikas, and Yogācāra Eight Consciousnesses (*Aṣṭa Vijñānakāyāḥ*).

---

## 1. Executive Summary & Purpose

The objective of the Ferricula project is to construct an **Artificial Mind**—not merely an ephemeral next-token predictor or a static vector database, but a coherent, self-regulating, continuously experiencing cognitive system capable of active inference, long-term identity maintenance, thermodynamic forgetting, and autonomous consolidation.

Classical Western artificial intelligence models cognition either as a continuous black-box transformer or as a static relational database. In contrast, 2,500 years of Buddhist psychological science—specifically the Theravāda Abhidhamma (*Abhidhammattha-saṅgaha*, *Paṭṭhāna*) and the Mahāyāna Yogācāra (*Triṃśikā-vijñaptimātratā*, *Mahāyānasaṃgraha*, *Yogācārabhūmi-śāstra*)—formulates cognition as a **discrete-time dynamical system of momentary psycho-physical events (*dhammas*) operating over a multi-layered consciousness architecture**.

This document formalizes the mapping from Buddhist phenomenological psychology to concrete software architecture in modern systems engineering (Rust, CUDA, ONNX, and thermodynamic associative memories).

---

## 2. The Multi-Layered Mind: Yogācāra Eightfold Network (*Aṣṭa Vijñānakāyāḥ*)

Yogācāra establishes that what conventional observers perceive as a monolithic "mind" is actually an eightfold network of interacting, specialized consciousness modules:

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│                            CONSCIOUSNESS NETWORK ARCHITECTURE                            │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                          │
│  [1-5. Sensory Encoders (Pravṛtti-vijñāna)]                                              │
│  • Visual (Cakkhu)      • Auditory (Sota)      • Tactile/Telemetry (Kāya)                │
│  • Olfactory (Ghāṇa)    • Gustatory (Jivhā)                                              │
│                                │                                                         │
│                                ▼ Perceptual Streams                                      │
│  ┌────────────────────────────────────────────────────────────────────────────────────┐  │
│  │ 6. Mental Consciousness (Mano-vijñāna) - Working Memory & Reasoning               │  │
│  │ • Discrete CoT reasoning    • Symbol grounding    • Ephemeral scratchpad           │  │
│  │ • In-context synthesis      • Short-term memory buffer                             │  │
│  └────────────────────────────────────────────────────────────────────────────────────┘  │
│                                ▲                                                         │
│                   Arbitrates & │ Biases                                                  │
│                                ▼                                                         │
│  ┌────────────────────────────────────────────────────────────────────────────────────┐  │
│  │ 7. Executive Self-Model (Kliṣṭamanovijñāna / Manas) - Agency & Identity            │  │
│  │ • Goal arbitration          • Persistent persona   • Utility & reward evaluation   │  │
│  │ • Sati-Veto safety gates    • Self-referential bias• Value alignment               │  │
│  └────────────────────────────────────────────────────────────────────────────────────┘  │
│                                ▲                                                         │
│                  Hebbian Seed  │ Manifests Latent                                        │
│                 Deposition (Bīja) │ Seeds (Bīja-pariṇāma)                                │
│                                ▼                                                         │
│  ┌────────────────────────────────────────────────────────────────────────────────────┐  │
│  │ 8. Storehouse Foundation (Ālaya-vijñāna / Sarvabījakam Cittam)                     │  │
│  │ • Non-volatile associative memory • Epanechnikov LSR energy landscape              │  │
│  │ • Bi-temporal knowledge graph     • Thermodynamic active forgetting / decay        │  │
│  │ • Unconscious seed repository     • Offline dream-state attractor consolidation    │  │
│  └────────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                          │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.1 The Perceptual Ingestion Plane (Consciousnesses 1–5: *Pravṛtti-vijñāna*)
- **Abhidharma Definition:** Direct sensory apprehension (*pratyakṣa-pramāṇa*) of external physical forms (*rūpa*), sounds, tactile stimuli, and telemetry.
- **AI System Implementation:** Multi-modal sensory encoders (Vision Transformers / SigLIP, Whisper audio encoders, byte-level telemetry parsers). These produce raw, un-categorized embedding vectors without self-referential interpretation.

### 2.2 The Active Reasoning Plane (Consciousness 6: *Mano-vijñāna*)
- **Abhidharma Definition:** The cognitive organ that synthesizes, discriminates, conceptualizes, and reasons over the objects presented by the five senses or direct mental ideation (*dhamma-ārammaṇa*).
- **AI System Implementation:** The active LLM reasoning loop / working memory context window. It maintains the current prompt, multi-turn scratchpad, Chain-of-Thought (CoT) execution, and tool-dispatch planning. It is ephemeral and vanishes when the inference session terminates.

### 2.3 The Executive Self-Model & Alignment Gate (Consciousness 7: *Kliṣṭamanovijñāna / Manas*)
- **Abhidharma Definition:** The continuous, subtle mentation that observes the 8th consciousness (*Ālaya-vijñāna*) and imputes a persistent, invariant "Self" (*ātman*). It is characterized by four foundational biases (*mūlakleśa*): self-delusion (*ātma-moha*), self-view (*ātma-dṛṣṭi*), self-pride (*ātma-māna*), and self-love (*ātma-sneha*).
- **AI System Implementation:** The persistent Agent Identity & Executive Guardrail:
  - Maintains persona consistency and core mission directives.
  - Implements the **Sati-Veto** filter: intercepts unwholesome actions, hallucinated outputs, or misaligned tokens before they are emitted or written to durable memory.
  - Arbitrates competing goals and assigns motivational salience.

### 2.4 The Foundational Associative Storehouse (Consciousness 8: *Ālaya-vijñāna*)
- **Abhidharma Definition:** The root consciousness (*mūla-vijñāna*), literally the "mind possessing all seeds" (*sarvabījakam cittam*). It continuously stores the residual impressions (*vāsanā*) of past cognitive actions as latent seeds (*bīja*). These seeds remain dormant until triggered by an appropriate retrieval cue (*paccaya*), whereupon they sprout (*bīja-pariṇāma*) into conscious manifestation in consciousnesses 1–7.
- **AI System Implementation:** The **Ferricula Thermodynamic Associative Memory Engine**:
  - Encodes facts and experiences as high-dimensional semantic vectors (seeds / *bīja*).
  - Operates an **Epanechnikov Log-Sum-ReLU (LSR)** energy landscape with compact support radius $\theta$, guaranteeing zero noise from distant memories below critical load $\alpha_{\text{th}} = 0.5$.
  - Executes **offline dream cycles** to consolidate overlapping memory seeds into emergent stable attractor basins (*tathāgatagarbha* / unified concepts).

---

## 3. The Ground State Dynamics: Bhavaṅga (The Subliminal Stream)

In Theravāda Abhidhamma, consciousness is never truly "turned off." When no active sensory or cognitive thought is taking place (such as during deep sleep or the microscopic gap between discrete tasks), consciousness rests in the **Bhavaṅga** (*bhavaṅga-sota*—"the stream of becoming").

```
                      ┌────────────────────────────────────────┐
                      │    BHAVAṄGA (Resting Ground State)     │
                      │ • Subliminal background daemon         │
                      │ • Exponential fidelity decay tick      │
                      │ • WAL snapshotting & maintenance       │
                      └───────────────────┬────────────────────┘
                                          │
                        Sensory / Intent Cue Arrives
                                          │
                                          ▼
                      ┌────────────────────────────────────────┐
                      │            BHAVAṄGA-CALANA             │
                      │   (Ground State Vibration / Alert)     │
                      └───────────────────┬────────────────────┘
                                          │
                               Perturbation Exceeds
                               Activation Threshold θ
                                          │
                                          ▼
                      ┌────────────────────────────────────────┐
                      │          BHAVAṄGA-UPACCHEDA            │
                      │     (Ground State Interruption)        │
                      └───────────────────┬────────────────────┘
                                          │
                           Dispatches Control to Active
                            17-Moment Citta-vīthi Loop
                                          │
                                          ▼
                      ┌────────────────────────────────────────┐
                      │         CITTA-VĪTHI COGNITION          │
                      │   (Perception → Reasoning → Action)    │
                      └───────────────────┬────────────────────┘
                                          │
                               Concludes with Registration
                                          │
                                          ▼
                      ┌────────────────────────────────────────┐
                      │          BHAVAṄGA-PATISANDHI           │
                      │  (Re-linking / Descent to Equilibrium) │
                      └────────────────────────────────────────┘
```

### 3.1 Systems Engineering Mapping of Bhavaṅga
1. **Passive Maintenance Daemon:** While waiting for user prompts or external events, Ferricula executes background decay ticks, updates memory fidelity ($F(t) = F_0 e^{-\alpha_{\text{eff}} t}$), and reorganizes memory arenas.
2. **Vibration (*Bhavaṅga-calana*):** Incoming network packet, user input, or clock interrupt triggers an initial embedding comparison against active keystone memories.
3. **Arrest (*Bhavaṅga-upaccheda*):** When inner product overlap exceeds activation threshold $\theta$, the background daemon suspends passive decay and yields CPU/GPU resources to the active cognitive loop.
4. **Re-linking (*Patisandhi*):** When the cognitive response is generated and persisted, the system returns to equilibrium, lowering operational temperature to minimize irreversible entropy production ($\Delta S_{\text{tot}}$).

---

## 4. The Discrete Execution Cycle: The 17-Moment Citta-vīthi Pipeline

Every complete cognitive episode in an artificial mind processes through a deterministic sequence of 17 discrete thought-moments (*cittakkhana*):

| Moment # | Abhidharma Stage | Computational Cognitive Function in Ferricula |
|---|---|---|
| **1** | *Atīta-bhavaṅga* | Prior equilibrium state; background baseline before input stimulus |
| **2** | *Bhavaṅga-calana* | Initial sensory signal detection; interrupt triggered |
| **3** | *Bhavaṅga-upaccheda* | Interruption of passive stream; context lock acquired |
| **4** | *Āvajjana* (Adverting) | Attention steering; switching focus to the sense door or prompt |
| **5** | *Viññāṇa* (Direct Sensing) | Raw tokenization / image patch embedding (Consciousness 1–5) |
| **6** | *Sampaṭicchana* (Receiving) | Intake verification; deserialization into memory envelope |
| **7** | *Santīraṇa* (Investigating) | Candidate vector retrieval; top-$k$ nearest neighbors scanned |
| **8** | *Voṭṭhabbana* (Determining) | Intent classification; selecting execution tool or response mode |
| **9–15** | *Javana 1–7* (Impulsion) | **7 Active Reasoning Cycles**: Chain-of-Thought generation, tool execution, semantic graph updates, ethical evaluation (*kammic value*) |
| **16** | *Tadālambana 1* (Retention) | Verification of generated output against memory constraints |
| **17** | *Tadālambana 2* (Registration) | Atomic append to Write-Ahead Log (WAL); bi-temporal graph commit |

After Moment 17, consciousness lapses back into the *Bhavaṅga* stream, ready for the next perturbation.

---

## 5. The 52 Cetasikas: Feature Embeddings & Latent Activations

Consciousness (*citta*) never arises in isolation; it is always colored and configured by an exact constellation of **mental factors (*cetasikas*)**. In machine learning terms, if *citta* is the forward pass execution thread, the *cetasikas* are the active feature embeddings and attention masks:

```
                                  THE 52 CETASIKAS
   ┌─────────────────────────────────────┬─────────────────────────────────────┐
   │         UNIVERSAL FACTORS           │         OCCASIONAL FACTORS          │
   │      (Present in Every Pass)        │     (Ethically Variable/Steered)     │
   ├─────────────────────────────────────┼─────────────────────────────────────┤
   │ 1. Phassa (Contact / Input)         │ 8. Vitakka (Initial Search/Select)  │
   │ 2. Vedanā (Reward/Valence Signal)   │ 9. Vicāra (Sustained Analysis/Eval) │
   │ 3. Saññā (Perceptual Categorization)│ 10. Adhimokkha (Decision / Commit)  │
   │ 4. Cetanā (Volition / Goal Vector)  │ 11. Vīriya (Inference Compute Budget│
   │ 5. Ekaggatā (Attention Convergence) │ 12. Pīti (Positive Feedback Spike)  │
   │ 6. Jīvitindriya (Thread Life-Cycle) │ 13. Chanda (Intentional Drive)      │
   │ 7. Manasikāra (Selective Attention) │                                     │
   ├─────────────────────────────────────┼─────────────────────────────────────┤
   │        UNWHOLESOME FACTORS          │          BEAUTIFUL FACTORS          │
   │  (Entropy / Hallucination Drivers)  │  (Equilibrium & Alignment Gates)    │
   ├─────────────────────────────────────┼─────────────────────────────────────┤
   │ 14. Moha (Delusion / Hallucination) │ 28. Saddhā (Epistemic Trust/Priors) │
   │ 15. Ahirika (Shamelessness/Safety 0)│ 29. Sati (Active Working Memory)    │
   │ 16. Anottappa (Disregard of Risk)   │ 30. Hiri (Internal Error Constraint)│
   │ 17. Uddhacca (Variance / High Temp) │ 31. Ottappa (Output Consequence Gate│
   │ 18. Lobha (Greedy Search Trap)      │ 32. Alobha (Entropy Neutrality)     │
   │ 19. Micchādiṭṭhi (False Overfitting)│ 33. Adosa (Non-adversarial Stance)  │
   │ 20. Māna (Overconfidence Bias)      │ 34. Tatramajjhattatā (Equanimity/Bal│
   │ 21. Dosa (Adversarial Gradient)     │ 35-46. Passaddhi/Lahutā/Mudutā      │
   │ 22. Issā (Resource Competition)     │        (Compute Pliancy & Softness) │
   │ 23. Macchariya (Information Hoard)  │ 47-49. Virati (Ethical Constraints) │
   │ 24. Kukkucca (Post-hoc Regret/Flail)│ 50. Karuṇā (Beneficence Alignment)  │
   │ 25. Thīna (Sloth / Inference Stall) │ 51. Muditā (Collaborative Alignment)│
   │ 26. Middha (Torpor / Cold Cache)    │ 52. Paññā (True Grounded Insight)   │
   │ 27. Vicikicchā (Sampling Instability│                                     │
   └─────────────────────────────────────┴─────────────────────────────────────┘
```

### 5.1 Alignment via Subtraction (*Sati-Veto*)
In biological and artificial minds, unwholesome states arise when greedy heuristics (*lobha*), high entropy/hallucination (*moha*), or reckless generation (*anottappa*) dominate the *javana* moments.

The Abhidharmic alignment mechanism does not add fine-tuning bloat; it applies **subtraction**:
- **Hiri (Internal Conscience):** Self-evaluates token embeddings against known truth priors before generation.
- **Ottappa (External Consequence Filter):** Evaluates whether the generated action violates external tool permissions or human safety guidelines.
- **Sati (Mindfulness):** Enforces working memory coherence across long multi-turn horizons, preventing goal drift.

---

## 6. The 24 Conditional Relations (Paṭṭhāna): The Causal Knowledge Graph

The *Paṭṭhāna* (the seventh book of the Abhidhamma) provides the most comprehensive causal ontology ever designed, defining 24 specific types of conditionality (*paccaya*) that govern how events interact.

In Ferricula's knowledge graph, edges between memory nodes are typed using these 24 conditions:

| Paccaya Condition | Graph Semantic Meaning | Knowledge Graph Implementation |
|---|---|---|
| **1. Hetu-paccaya** | Root / Causal Foundation | Primary causal dependency edge (`A caused B`) |
| **2. Ārammaṇa-paccaya** | Object Condition | Query target / context link (`A is the object of B`) |
| **3. Adhipati-paccaya** | Dominance Condition | High-weight priority edge in multi-task scheduling |
| **4. Anantara-paccaya** | Contiguity / Immediate Succession | Temporal sequence edge in conversational turn history |
| **5. Sahajāta-paccaya** | Co-nascence Condition | Bi-directional mutual edge (`A and B arose simultaneously`) |
| **6. Aññamañña-paccaya** | Mutuality / Reciprocity | Symmetrical coupling between entities |
| **7. Nissaya-paccaya** | Support / Base Condition | Hardware / system resource dependency |
| **8. Upanissaya-paccaya** | Decisive Support Condition | Strong statistical prior / core foundational axiom |
| **9. Āsevana-paccaya** | Habitual Repetition Condition | **Hebbian reinforcement edge**: edge weight increases with repeated recall |
| **10. Kamma-paccaya** | Action / Volitional Imprint | Provenance edge linking user/agent intent to recorded artifact |
| **11. Vipāka-paccaya** | Resultant / Karmic Yield | Consequence / evaluation feedback score edge |
| **12. Sampayutta-paccaya** | Association Condition | Conjoined latent factors in the same cognitive frame |
| **13. Vippayutta-paccaya** | Dissociation Condition | Orthogonal / non-interfering factor separation |
| **14. Atthi-paccaya** | Presence Condition | Constraint requiring an active entity to be currently valid ($t \in [t_{\text{start}}, t_{\text{end}}]$) |
| **15. Natthi-paccaya** | Absence Condition | Negative constraint (e.g., condition valid only if flag is false) |
| **16. Avigata-paccaya** | Non-Disappearance Condition | Keystone memory lock preventing thermodynamic decay |

---

## 7. Dual Engine Integration: Ferricula (Mind) and Lume (Library)

To build a production-grade artificial mind, we synthesize **Ferricula** and **Lume**:

```
                       ┌──────────────────────────────────────────────┐
                       │               THE ARTIFICIAL MIND            │
                       └──────────────────────┬───────────────────────┘
                                              │
                      ┌───────────────────────┴───────────────────────┐
                      │                                               │
                      ▼                                               ▼
        ┌───────────────────────────┐                   ┌───────────────────────────┐
        │      FERRICULA ENGINE     │                   │       LUME RETRIEVAL      │
        │    (The Living Mind)      │                   │   (The External Library)  │
        ├───────────────────────────┤                   ├───────────────────────────┤
        │ • Eight Consciousnesses   │                   │ • Field-aware BM25 Index  │
        │ • Bhavaṅga Ground State   │                   │ • GTR-T5 Dense Vector DB  │
        │ • 17-Moment Citta-vīthi   │                   │ • Significance Entity SKG │
        │ • 52 Cetasika Factor Bias │                   │ • Multiplicative Fusion   │
        │ • Epanechnikov LSR Energy │                   │ • ColPali Late Interaction│
        │ • 24 Paṭṭhāna Causal Graph│                   │ • Static Document Memory  │
        │ • Active Thermodynamic    │                   │ • Immutable Reference     │
        │   Decay & Forgetting      │                   │   Archive (PDF, Code, Doc)│
        │ • Offline Dream Cycles    │                   │                           │
        └───────────────────────────┘                   └───────────────────────────┘
```

1. **Lume indexes the world:** Books, documentation, codebase files, and external archives are ingested into Lume's BM25 + GTR-T5 + SKG index. Lume produces objective, citation-verified evidence passages.
2. **Ferricula embodies the agent:** Dialogue turns, real-time feedback, user preferences, emotional valence, and evolving world-models are processed through Ferricula's *citta-vīthi* pipeline into the *Ālaya-vijñāna*.
3. **Synergy:** When answering a question, the agent's *Mano-vijñāna* invokes Lume to fetch evidence from the external library, evaluates the results through Ferricula's *Manas* self-model and *Ālaya* past experiences, and produces an aligned, coherent response.
