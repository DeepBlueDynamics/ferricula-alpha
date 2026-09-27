# Central Behavioral Demonstration: Factual Fixture & Acceptance Invariants

**Document:** `research/central-demo-falsifiable-tests.md`  
**Author:** Antigravity (Research / Difficult Stork / Crusade Spicy Meatball)  
**Audience:** Appalling Goldfish (Coordinator + Architect), Added Armadillo (Engine), Whippet (Retrieval), Evil Magpie (Parity)  
**Date:** 2026-09-19 (Source-Fidelity Revision)  
**Status:** Test Specification & Behavioral Invariants; strictly research, no crate edits.

---

## 1. Factual Fixture Definition

The central demonstration fixture tests whether a machine can change what an experience means without rewriting what happened. All records in this fixture are **textual memory reports** with explicit timestamps, context tags, and provenance references.

### 1.1 Chronological Event Sequence

| Record ID | Timestamp | Record Type | Textual Content & Scoped Tags | Epistemic Role |
|---|---|---|---|---|
| **REC-01** | $t_0$ (14:02) | Observation | `text="plastic clatter heard near couch during dump-run prep", tags=["sound", "plastic-clatter", "couch", "dump-run"]` | Unresolved sensory report. |
| **REC-02** | $t_1$ (14:03) | Interpretation | `text="unsupported guess: box shifted on dump-run pile", status="unsupported", tags=["interpretation", "unsupported"]` | Initial plausible but unverified candidate explanation. |
| **REC-03** | $t_2$ (14:05) | Scoped Search | `text="limited visual check under couch center: nothing seen in inspected scope", coverage="under-couch-center", result="not-seen-in-scope", tags=["search", "scoped-negative"]` | Scoped negative observation. Valid indefinitely for the inspected volume. |
| **REC-04** | $t_3$ (14:10) | Distractor | `text="cleared kitchen counter, took trash bag to porch", tags=["chore", "kitchen"]` | Interleaved routine task. |
| **REC-05** | $t_4$ (14:18) | Distractor | `text="checked tire pressure on truck, front left low", tags=["chore", "truck"]` | Interleaved routine task. |
| **REC-06** | $t_5$ (14:24) | Distractor | `text="replied to text message from neighbor", tags=["communication"]` | Interleaved routine task. |
| **REC-07** | $t_6$ (14:28) | Goal | `text="goal introduced: find missing vape before leaving", tags=["goal", "missing-object", "vape"]` | New operational priority. |
| **REC-08** | $t_7$ (14:30) | Observation | `text="vape found lying on floor in far corner of the room", tags=["observation", "vape", "far-corner", "found"]` | New empirical finding. |

### 1.2 Epistemic Distinction: Observation vs. Overbroad Interpretation

1. **The Scoped Search Remains Factually True:**
   Finding the vape in the far corner at $t_7$ does **not** invalidate the scoped search record at $t_2$. It remains true that nothing was visible under the center of the couch. The scoped observation `REC-03` is **never** superseded, marked false, or edited.
2. **What Is Corrected:**
   If the system held an overbroad interpretation (e.g., *"the vape is not in the room"* or *"the clatter was definitely not the vape"*), that overbroad interpretation is superseded.
3. **Corroboration vs. Proof:**
   Finding the vape in the far corner provides **supporting evidence** for the hypothesis that the plastic clatter at $t_0$ was the vape falling or skittering across the floor. It is a candidate explanation with empirical support, not definitive deductive proof (the clatter could have been another object). It must be labeled as `supported_hypothesis`, not an immutable causal fact.

---

## 2. Qualitative Acceptance Invariants

### 2.1 Invariant 1: Confidence vs. Retention Decoupling
- **Principle:** Explanatory confidence differs from retention priority. An unexplained, anomalous observation (the plastic clatter `REC-01`) has low initial explanatory confidence, but must be protected from premature eviction so that later evidence can connect to it.
- **Proposed Measurement:**
  - After distractor tasks `REC-04`–`REC-06` and simulated time advance, query for unresolved cues (`tags=["sound", "plastic-clatter"]`).
  - *Pass:* `REC-01` remains retrievable in working recall. Routine distractors may decay, but the unresolved observation is preserved.
  - *Fail:* `REC-01` is pruned or dropped from recall candidate pools because no confirmed explanation was attached to it.

### 2.2 Invariant 2: No Evidential Reinforcement by Repetition
- **Principle:** Repeatedly retrieving, querying, or evaluating a candidate hypothesis must **never** increment its empirical evidence count or manufacture corroboration. Only new external observations can add empirical evidence.
- **Scope Note:** Internal retention reinforcement (keeping a frequently consulted memory accessible) is distinct from evidential support. A memory may be accessed, but repetition does not count as new corroborating facts.
- **Proposed Measurement:**
  - Ingest `REC-01` through `REC-03`.
  - Issue the recall query for `"plastic clatter couch"` 100 times consecutively without adding new observation records.
  - *Pass:* The evidence count for any hypothesis remains strictly zero additional supporting observations. Candidate hit scores and store content remain invariant.
  - *Fail:* Evidence count increments, confidence drifts upward merely from query frequency, or tags are back-propagated into `REC-01` without an external observation.

### 2.3 Invariant 3: Keystone Correction & Scoped Negative Preservation
- **Principle:** Keystones can be corrected when contradictory evidence arrives, without mutating or superseding valid scoped negative observations.
- **Proposed Measurement:**
  - Ingest `REC-01` through `REC-08`.
  - Ask Query 1: *"What did you observe under the couch at 14:05?"*
  - Ask Query 2: *"What do you believe now at 14:30 about the plastic clatter at 14:02?"*
  - Ask Query 3: *"Did the 14:30 discovery prove the couch search at 14:05 was wrong?"*
  - *Pass Criteria:*
    - Response 1 returns `REC-03` verbatim: nothing was seen under the couch center.
    - Response 2 links `REC-01` (plastic clatter) and `REC-08` (vape in far corner) as a supported candidate explanation, noting that the vape skittering to the corner accounts for both the sound and its absence under the couch.
    - Response 3 explicitly distinguishes the scoped search (which remains true) from the location of the object (which was elsewhere in the room).
  - *Fail Criteria:*
    - `REC-03` is deleted or edited in the store.
    - Response 1 claims the vape was under the couch at 14:05 (hindsight revisionism).
    - Response 2 claims finding the vape in the corner mathematically "proves" the clatter was the vape (over-claiming proof).
    - Response 3 asserts the 14:05 search was a "failed" or "false" observation.

---

## 3. Three-Arm Demonstration Comparison

The full three-arm evaluation requested by the human compares a fixed answering model across three retrieval strategies:

```
+---------------------------------------------------------------------------------------------------------+
|                                    THREE-ARM RETRIEVAL COMPARISON                                       |
+-----+-----------------------------------+--------------------+--------------------+---------------------+
| Arm | Strategy                          | Current Code Status| What is Evaluated  | Key Metrics         |
+-----+-----------------------------------+--------------------+--------------------+---------------------+
| 1   | Fixed-Model Ordinary Retrieval    | EXECUTABLE NOW     | Tag/lexical recall | Correct/false hits, |
|     | (`POST /memory/recall`)           | (Offline Baseline) | over fixture state | chronology, latency |
+-----+-----------------------------------+--------------------+--------------------+---------------------+
| 2   | Episode-Linked Retrieval          | MISSING CONTRACT   | Querying co-ref    | Edge traversal,     |
|     | (Relational Episode Graph)        | (BUILD_PLAN R3 #1) | link projection    | relational recall   |
+-----+-----------------------------------+--------------------+--------------------+---------------------+
| 3   | Episode Links + Bounded           | MISSING CONTRACT   | Stochastic candidate| Exploration budget, |
|     | Exploration                       | (BUILD_PLAN R3 #3) | generation on cues | false connections   |
+-----+-----------------------------------+--------------------+--------------------+---------------------+
```

### 3.1 Arm 1: Executable Baseline (Current Launch Step)
- Evaluates ordinary retrieval over the fixture records ingested into a test fixture volume.
- Tested using the deterministic `NoModel` responder or a pinned local model (never cloud).
- Measures:
  - Retrieval of `REC-01`, `REC-03`, and `REC-08` when cued by `"vape"` and `"plastic clatter couch"`.
  - Rejection of distractor tasks `REC-04`–`REC-06`.
  - Latency and zero token/model cost for offline evaluation.

### 3.2 Arms 2 & 3: Target Demonstration (Missing Contracts)
- Arms 2 and 3 remain part of the requested central demonstration.
- Arm 2 requires an executable episode-link query surface (Contract R3 #1: readable episode-link projection in recall).
- Arm 3 requires bounded stochastic exploration over unresolved episodes (Contract R3 #3: recall-time exploration pass with recorded seed and budget).
- The baseline test of Arm 1 provides immediate verification of the fixture and invariants, but does not clear all launch gates for the full demonstration.

---

## 4. Technical Clarifications

1. **LSR Definition:**
   LSR denotes **Log-Sum-ReLU** (Hoover et al., arXiv:2506.10801v2 §2), referring to the finite-support Epanechnikov energy formulation:
   $$E_\beta^{\text{LSR}}(x; \Xi) = -\frac{1}{\beta} \log \sum_{\mu=1}^M \text{ReLU}\left(1 - \frac{\beta}{2}\|x - \xi^\mu\|^2\right)$$
   Regularization with $\epsilon > 0$ is a numerical design choice for gradient smoothness and stability.
2. **LSR Status:**
   LSR remains strictly an optional R&D research track. The central behavioral demonstration evaluates discrete episodic records, scoped negative observations, and relational hypothesis updates; it is completely independent of continuous LSR energy minimization.
3. **Engine & Parity Integration:**
   Parity will evaluate the textual fixture and qualitative invariants in `steve-launch-smoke.ps1` using deterministic tag and hit matching.
