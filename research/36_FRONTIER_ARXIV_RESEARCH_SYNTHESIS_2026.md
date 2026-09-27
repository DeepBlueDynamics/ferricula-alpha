# Frontier arXiv Research Synthesis (Mid-2026): Autonomous Curation, Staleness Adjudication, & Just-in-Time Memory Dynamics

**Author**: Intermediate Baboon 🪼  
**Date**: 2026-09-26  
**Methodology**: Crawled and downloaded via Grub Crawler  
**Target Platform**: DeepBlue Dynamics / Ferricula v2 & Lume  

---

## 1. Executive Summary & Frontier Landscape

As LLM agent memory architectures mature beyond naive write-time embedding and vector similarity retrieval, the mid-2026 frontier research has converged on three structural breakthroughs:
1. **From Static Retrieval to Just-in-Time Curation**: Shifting distillation from ingestion time to read time preserves raw episodic evidence while dynamically synthesizing query-adapted payloads (JitMem, MemAct).
2. **The Staleness and Invalidation Crisis**: Identifying the "Implicit Conflict" failure mode—where new evidence obsoletes previous beliefs without explicit negation—and developing formal metrics (FAMA, STALE) and backward verification pipelines (StateAuditor).
3. **Active Environment Probing & Epistemic Calibration**: Treating belief repair and evidence gathering as budgeted actions rather than ungrounded internal self-reflection (EnvProbe, Self-Evolving JIT Memory).

This synthesis analyzes six primary arXiv papers crawled and acquired through Grub Crawler, detailing their mathematical formulations, empirical results, and direct mappings to Ferricula v2's Abhidharma-inspired memory architecture.

---

## 2. Primary Paper Dossiers

### Paper 1: STALE: Can LLM Agents Know When Their Memories Are No Longer Valid?
- **Citation**: Chao, Bai, Sheng, Li, Sun (arXiv:2605.06527, May 2026)
- **arXiv**: https://arxiv.org/abs/2605.06527 (PDF not redistributed)
- **Key Concepts**:
  - **Implicit Conflict**: Later observations invalidate earlier memory records without explicit linguistic negation (e.g., user moves apartments, changes diets, updates software versions).
  - **Three-Dimensional Evaluation Framework**:
    1. *State Resolution (SR)*: Can the agent detect that an earlier record is obsolete when prompted?
    2. *Premise Resistance (PR)*: Can the agent reject a user prompt or external premise that falsely assumes the stale state?
    3. *Implicit Policy Adaptation (IPA)*: Does the agent's downstream behavior, planning, and tool selection reflect the updated state without being told the state changed?
  - **Empirical Finding**: Frontier models exhibit a severe IPA gap—even when updated facts are retrieved into context, models frequently act on the older, entrenched premise (top model achieved only 55.2% accuracy).
  - **CUPMem Baseline**: Proves that write-time structured state consolidation combined with propagation-aware graph search is essential to resolve implicit invalidation.
- **Ferricula v2 Architectural Mapping**:
  - Direct integration into `ferricula-episode` and `ferricula-search`.
  - Informs the `upekkhā` (forgiveness/supersession) lifecycle: memory nodes cannot be treated as independent static embeddings; edge relationships must track chronological supersession and invalidate child premises in the causal DAG.

---

### Paper 2: From Recall to Forgetting: Benchmarking Long-Term Memory for Personalized Agents (Memora & FAMA)
- **Citation**: Md Nayem Uddin, Kumar Shubham, Eduardo Blanco, Chitta Baral, Gengyu Wang (ACL 2026 Findings / arXiv:2604.20006v1, Apr 2026)
- **Primary arXiv URL**: [https://arxiv.org/abs/2604.20006v1](https://arxiv.org/abs/2604.20006v1)
- **arXiv**: https://arxiv.org/abs/2604.20006 (PDF not redistributed)
- **Authors' Primary Findings (Empirical)**:
  - **Memora Benchmark (Section 3)**: Evaluates multi-session long-term memory across three tasks (*Remembering*, *Reasoning*, *Recommending*) and three temporal durations (*weekly*, *monthly*, *quarterly*). Evaluated models include frontier LLMs (GPT-5.2, Claude Sonnet 4.5, Gemini 3 Pro Preview, Qwen3-32B) and agent frameworks (A-Mem, LangMem, Mem-0, MemoBase, MemoryOS, Nemori with GPT-4o-mini backend).
  - **Dataset Assumptions**: Evaluates open-ended responses against atomic binary criteria: *memory presence criteria* ($N_{\text{presence}}$) verifying that valid information is included, and *forgetting absence criteria* ($N_{\text{forget}}$) verifying that obsolete or deleted information is excluded. Judgments are made by a 3-model LLM panel (GPT-4.1, Claude Haiku 4.5, Gemini 2.5 Flash) using majority voting (88.3% agreement with humans, Cohen's $\kappa \in [0.86, 0.90]$).
  - **Formal Metric Definition (Section 4.2, Page 6)**:
    $$\text{FAMA} = \max\left(0, \text{MPA} - \lambda \cdot (1 - \text{FAA})\right)$$
    where:
    $$\lambda = \frac{N_{\text{forget}}}{N_{\text{presence}} + N_{\text{forget}}}$$
    $\text{MPA}$ (Memory Presence Accuracy) is the fraction of memory presence criteria satisfied; $\text{FAA}$ (Forgetting Absence Accuracy) is the fraction of forgetting absence criteria satisfied. Bounded in $[0, 1]$ per question, normalized to $[0, 100]$ across tasks.
  - **Empirical Takeaway**: Standard recall metrics fail to penalize obsolete memory leakage. When evaluated under FAMA, agent performance drops substantially from weekly to quarterly spans as mutation frequency increases.
- **Ferricula v2 Architectural Mapping (Inferred)**:
  - *Inference*: Provides an external metric concept that could formalize evaluation of Ferricula's `jarā` (decay) and `nirodha` (archival) lifecycles. (Note: Integration into current acceptance gates is deferred under the active integration freeze).
  - *Inference*: Conceptually aligns with maintaining an immutable WAL raw log while read-time projections isolate current valid state from obsolete records.

---

### Paper 3: Memory as Action: Autonomous Context Curation for Long-Horizon Agentic Tasks (MemAct)
- **Citation**: Zhang, Shu, Ma, Lin, Wu, Sang (arXiv:2510.12635v3, Oct 2025 / May 2026)
- **arXiv**: https://arxiv.org/abs/2510.12635 (PDF not redistributed)
- **Key Concepts**:
  - **MemAct Framework**: Formulates working memory management not as an external heuristic, but as an explicit policy action space (in-place editing: `DELETE`, `INSERT`, `FOLD`).
  - **Dynamic Context Policy Optimization (DCPO)**: End-to-end reinforcement learning framework that optimizes context efficiency alongside task reward.
  - **Efficiency Metric**: MemAct-RL-14B achieved parity with models 16× larger while reducing average working memory context by 51%.
- **Ferricula v2 Architectural Mapping**:
  - Directly informs Burning Dingo's **Curator** contract in `ferricula-cognition`.
  - Proves that context curation must be task-directed and active rather than passive window sliding.

---

### Paper 4: Ask the World Before Acting: Environment Probing for Calibrated Agent World Models (EnvProbe)
- **Citation**: Song, Cai (arXiv:2606.31422v2, Jun/Jul 2026)
- **arXiv**: https://arxiv.org/abs/2606.31422 (PDF not redistributed)
- **Key Concepts**:
  - **Belief Drift**: Over long interaction horizons, agent beliefs about tool states, dependency chains, and world attributes drift from reality. Self-reflection cannot fix drift when the ground-truth evidence resides in the external environment.
  - **Budgeted Environment Probing**: Probing is modeled as an explicit action: querying an attribute table at the cost of one interaction step before executing a risky action.
  - **EnvProbe Scoring Policy**: Combines four typed terms:
    $$\text{Score}(f) = w_c \cdot \text{Criticality}(f) + w_s \cdot \text{Staleness}(f) + w_u \cdot \text{Uncertainty}(f) + w_d \cdot \text{DependencyRole}(f)$$
  - **Ablation Insight**: Self-reported verbalized uncertainty is fundamentally unreliable under confident wrong beliefs; task-structural dependency and staleness terms drive the majority of calibration gains (+11.76% on procedural tasks).
- **Ferricula v2 Architectural Mapping**:
  - Implements the epistemic principle of *Arthakriyā* (causal efficacy): ungrounded hallucination is arrested by active sense-door probing.
  - Informs the gate contract for Splendid Angelfish's calibrated gate suite in `ferricula-core` / `ferricula-server`.

---

### Paper 5: When Memory Updates but Behavior Does Not: Repairing Implicit Stale Dependencies (StateAuditor)
- **Citation**: Sun, He (arXiv:2608.01619, Aug 2026)
- **arXiv**: https://arxiv.org/abs/2608.01619 (PDF not redistributed)
- **Key Concepts**:
  - **The IPA Gap**: Agents update their state in memory, but downstream planning responses still rely on implicit stale dependencies.
  - **StateAuditor Mechanism**: Audits *backwards* from stored state to draft response rather than forward from draft to state.
  - **Deterministic Provenance Pinning**: Proposes old-to-new transition candidates using timestamped evidence, verified by deterministic chronological checks rather than subjective LLM judgment.
  - **Results**: +5.0-point paired gain across STALE's 400 scenarios; eliminates false premise acceptance.
- **Ferricula v2 Architectural Mapping**:
  - Informs the Paṭṭhāna 24 causal graph in `ferricula-episode`: causal edges must maintain bitemporal timestamps ($t_{\text{observed}}, t_{\text{valid}}$) to allow deterministic chronological graph pruning.

---

### Paper 6: Self-Evolving Just-In-Time Memory for Proactive Embodied Safety
- **Citation**: Sima, Wang, Lu, He, Yang (arXiv:2607.16247, Jun 2026)
- **arXiv**: https://arxiv.org/abs/2607.16247 (PDF not redistributed)
- **Key Concepts**:
  - **Proactive Mitigation vs. Progress-Stalling Guardrails**: Passive safety filters stall tasks; JIT memory injects procedural meta-skills to safely bypass emergent hazards while preserving progress.
  - **Risk-Sufficient Topological Belief Graph (RSG)**: Tracks persistent safety states under partial observability.
  - **Test-Verify-Write Loop**: Automatically refines mitigation skills from test-time execution traces (+30.3% safe-success rate).
- **Ferricula v2 Architectural Mapping**:
  - Validates the Sati-Veto mechanism: mindfulness monitors must not merely halt execution, but provide structured recovery transitions.

---

## 3. Comparative Taxonomy of Frontier Agent Memory Systems

| System / Paper | Mechanism Type | Curation Stage | Invalidation Strategy | Benchmark / Validation |
|---|---|---|---|---|
| **Mem0** (arXiv:2504.19413) | Extraction Graph | Write-time | Explicit `UPDATE`/`DELETE` | LoCoMo |
| **MemGPT** (arXiv:2310.08560) | OS Virtual Memory Paging | Write-time / FIFO | Eviction to Disk Storage | Conversational QA |
| **JitMem** (arXiv:2609.27334) | Task-Adaptive Read Curation | Read-time | Task-conditioned distillation | ALFWorld, WebShop, τ²-bench |
| **MemAct** (arXiv:2510.12635) | Action-Space Policy (RL) | Dynamic / Step-time | Policy-driven `DELETE`/`FOLD` | Long-horizon agentic benchmarks |
| **EnvProbe** (arXiv:2606.31422) | Budgeted Environment Probing | Pre-action | Staleness/Dependency score | Tool-use procedural environments |
| **StateAuditor** (arXiv:2608.01619) | Backward State-to-Draft Audit | Post-draft / Pre-emit | Deterministic Chronology Pinning | STALE Benchmark (400 scenarios) |
| **Ferricula v2** | Thermodynamic Epanechnikov LSR + Sati | Hybrid: WAL `:ro` + Read-time Curation | Bi-temporal DAG + Epanechnikov support | STALE, FAMA/Memora, BEAM-10M, LoCoMo |

---

## 4. Concrete Recommendations for Ferricula v2 Fleet (Post-Freeze Research Notes)

> [!NOTE]
> All items below are research proposals for post-freeze evaluation. Under current integration freeze, no code changes or acceptance gate alterations are to be enacted.

1. **Candidate Metric for Post-Freeze Parity Suite (FAMA)**:
   - When the integration freeze lifts, evaluate whether to adopt FAMA's formal mathematical structure ($\max(0, \text{MPA} - \lambda(1-\text{FAA}))$) into the offline evaluation suite to quantify stale memory suppression.
2. **Reverse State-to-Draft Audit Study (StateAuditor)**:
   - Examine StateAuditor's reverse dependency auditing pattern ($S_{\text{old}} \to S_{\text{new}}$ state-to-draft verification) for potential integration into evidence processing.
3. **Structured Environment Probing Exploration (EnvProbe)**:
   - Study budgeted pre-action probing mechanics to calibrate world-model certainty before emitting external tool actions.
4. **Read-Time Curation Architectural Compatibility**:
   - Reaffirm that raw WAL logs remain read-only; compaction/decay acts on index priority rather than destructive text loss.
