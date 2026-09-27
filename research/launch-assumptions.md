# Launch Assumptions Audit & Minimal Falsifiable Experiments

**Document:** `research/launch-assumptions.md`  
**Author:** Antigravity (Research / Difficult Stork / Crusade Spicy Meatball)  
**Audience:** Appalling Goldfish (Coordinator + Architect), Added Armadillo (Engine), Whippet (Retrieval), Evil Magpie (Parity)  
**Date:** 2026-09-19 (Revised)  
**Status:** Audit and Experimental Contract; strictly research and evaluation, no crate edits.

---

## 1. Evidentiary Audit: Corrected Agreements & Formal Retractions

We have audited [`ARCHITECTURE.md`](file:///workspace/memory/ARCHITECTURE.md), [`BUILD_PLAN.md`](file:///workspace/memory/BUILD_PLAN.md), and Goldfish’s [`research/hoover-vs-ferricula.md`](file:///workspace/memory/research/hoover-vs-ferricula.md). We record the following verified agreements, corrected mathematical formulations, and formal retractions per Rule 6:

### 1.1 Mathematical & Code Findings

1. **LSR Formulation and $\epsilon$ Regularization:**
   - *Retraction:* I previously stated that retaining $\epsilon > 0$ is mathematically mandatory; that is false per Hoover et al. (arXiv:2506.10801v2 §2). The paper defines the standard LSR energy with $\epsilon = 0$:
     $$E_\beta^{\text{LSR}}(x; \Xi) = -\frac{1}{\beta} \log \sum_{\mu=1}^M \text{ReLU}\left(1 - \frac{\beta}{2}\|x - \xi^\mu\|^2\right)$$
   - *Correction:* $\epsilon > 0$ is a numerical regularization design choice introduced in Hoover §2.1 for smoothness and implementation stability. It prevents division by zero in gradient descent near the support boundary, but is not mathematically mandatory everywhere in the core formulation.
2. **Emergent Centroids & Proposition 2 Fixed Points:**
   - *Retraction:* I previously stated that a point $x^*$ is a local minimum if and only if the active set evaluated at $x^*$ matches the subset that generated it; that "if and only if" claim was too strong and mathematically false.
   - *Correction:* Centroid fixed-point sufficiency requires non-empty support and boundary margins; non-identical subsets can share a centroid. Proposition 2 establishes that a subset $S \subseteq \Xi$ produces a stationary point if the centroid $c(S) = \frac{1}{|S|}\sum_{\mu \in S} \xi^\mu$ satisfies $B(c(S)) = S$, but this is neither universally bijective nor an unrestricted "iff".
3. **Hoover Proposition 1 Linear Independence:**
   - *Retraction:* I previously defended Proposition 1 by assuming local $|B(x)| \le 20$ and random linear independence of actual embedding vectors. Both were unverified assumptions.
   - *Correction:* We withdraw the attempt to repair Proposition 1 with assumed cardinality bounds. When $M > d$, random vectors cannot be globally linearly independent, and actual semantic embeddings exhibit local cluster collinearity. Goldfish's objection to Proposition 1 stands.
4. **Survivor vs. Centroid Baseline Reality:**
   - *Verified:* Both `ferricula/src/dream.rs:394` (v1/alpha/Steve) and `ferricula_v2/crates/ferricula-cognition/src/dream.rs:416` (v2) select the **highest-fidelity survivor** (`survivor = group.iter().max_by(...)`), re-point edges, and **archive absorbed records**. Neither averages vectors nor computes an energy function.
   - *Verified:* The claim in `engine-from-local-writeup.md` of a "Weighted Centroid" was factually false regarding checked-in code.
5. **Unsupported Queries ($E = +\infty$ or $\nabla E = 0$):**
   - *Verified:* For cues where $\forall \mu, \|x - \xi^\mu\| > \sqrt{2/\beta}$, the Epanechnikov kernel evaluates to identically zero across all memories. If $\epsilon = 0$, $E = +\infty$; if $\epsilon > 0$, $\nabla E = 0$.
   - *Verified:* Associative energy minimization is mathematically incapable of answering out-of-support queries. Lume’s lexical BM25 + SKG global candidate union is strictly required to provide initial candidate neighborhoods.
6. **LSR Status:**
   - *Verified:* LSR remains strictly an optional R&D research track; it is not a prerequisite or blocker for launch.

---

## 2. Assessment of What Can Be Tested with Existing Code

### 2.1 Testable with Existing `ferricula` (v1 / Steve Code)
- **Survivor Lexical Overlap Measurement:**
  - *Location:* `ferricula/src/dream.rs:351–420`.
  - *Retraction:* I previously equated lexical n-gram coverage with "unique entities and facts permanently erased". That was an overreach: lexical token loss does not prove factual erasure.
  - *Experiment:* Run offline consolidation over the research corpus (720 sections per 2026-09-19T03:00Z index generation). Record all absorbed memory records. Compare the lexical n-gram overlap between surviving records and absorbed records.
  - *Verified Metric:* Exact percentage of unique lexical n-grams dropped from surviving records during consolidation.
- **Pairwise Distance Distribution & Bandwidth Calibration:**
  - *Location:* `ferricula/src/engine.rs` / `ferricula/src/model.rs`.
  - *Correction:* Removed unverified RocksDB claim. Embeddings reside in `.lume-index` and engine snapshot/WAL files.
  - *Experiment:* Extract all 768-d GTR-T5 embeddings from `.lume-index`. Compute the pairwise Euclidean distance histogram.
  - *Testable Metric:* Empirical distribution of $\|u - v\|$ across the corpus to determine what inverse temperature $\beta$ yields active sets $|B| \ge 2$ versus singletons or empty voids.

### 2.2 Testable with Existing `ferricula-semantic` & `shivvr`
- **GTR Vec2Text Centroid Invertibility:**
  - *Location:* `ferricula/src/inversion.rs` (connected to Shivvr at `localhost:8085`).
  - *Experiment:* Take pairs of semantic neighbor embeddings $\mathbf{u}, \mathbf{v}$ from the corpus. Compute their midpoint $\mathbf{w} = \frac{\mathbf{u} + \mathbf{v}}{2}$. Feed $\mathbf{w}$ into `Inverter::invert_vector`.
  - *Testable Metric:* Does the reconstructed text represent an intelligible semantic blend or ungrammatical noise?

---

## 3. Classification of Experiments: Legacy Runtime vs. Conditional Multimodal Gates

We distinguish **Legacy Single-Space Steve Testing Gates** (evaluating existing Steve as-is) from **Conditional Multimodal Gates** (prerequisites only if activating SigLIP / dual-space). We also explicitly remove all unauthorized release version assignments ("v2.1"); unbuilt capabilities are tracked as architectural gaps/contracts without assigning release versions.

```
+---------------------------------------------------------------------------------------------------------+
|                                     EXPERIMENT CLASSIFICATION                                           |
+-----+-----------------------------------+--------------------+--------------------+---------------------+
| ID  | Experiment Name                   | Classification     | Target System      | Gate Condition      |
+-----+-----------------------------------+--------------------+--------------------+---------------------+
| S1  | Single-Space Steve Ingest/Recall  | STEVE RUNTIME GATE | `ferricula-server` | Baseline single-    |
|     | (Baseline GTR-T5 / Lexical)       |                    | & `memory.rs`      | space test pass.    |
+-----+-----------------------------------+--------------------+--------------------+---------------------+
| S2  | Controlled Clatter Scenario       | CENTRAL DEMO GATE  | `ferricula-server` | Machine changes     |
|     | (Executable Arm 1: Ordinary Recall)| (Human Priority)   | & `runtime.rs`     | meaning; event WAL  |
|     |                                   |                    |                    | immutable.          |
+-----+-----------------------------------+--------------------+--------------------+---------------------+
| M1  | Vector Bank Space Isolation (E1)  | CONDITIONAL GATE   | `ferricula-core`   | Required ONLY if    |
|     |                                   | (Multimodal Dual)  | & `engine.rs`      | activating SigLIP.  |
+-----+-----------------------------------+--------------------+--------------------+---------------------+
| M2  | Zero-BM25 Visual Candidate        | CONDITIONAL GATE   | `ferricula-search` | Required ONLY if    |
|     | Survival (E2)                     | (Multimodal Dual)  | `hybrid.rs:749`    | visual search used. |
+-----+-----------------------------------+--------------------+--------------------+---------------------+
| M3  | Dual-Path Migration & Rollback    | CONDITIONAL GATE   | `ferricula-server` | Required ONLY if    |
|     | (E3)                              | (Multimodal Dual)  | & `persist.rs`     | new format ingested.|
+-----+-----------------------------------+--------------------+--------------------+---------------------+
| R1  | GTR Centroid Invertibility Probe  | OPTIONAL R&D       | `ferricula-semantic`| Research track;     |
|     |                                   |                    | & `inversion.rs`   | non-blocking.       |
+-----+-----------------------------------+--------------------+--------------------+---------------------+
| R2  | Standalone LSR Convergence Test   | OPTIONAL R&D       | `ferricula-cognition`| Research track;   |
|     |                                   |                    | (scratch script)   | non-blocking.       |
+-----+-----------------------------------+--------------------+--------------------+---------------------+
| R3  | Unresolved Episode Retention      | MISSING CONTRACT   | `ferricula-core`   | Tracked gap         |
|     | (Arms 2 & 3: Links & Exploration) | (BUILD_PLAN R3#1-3)| & `runtime.rs`     | (no release assigned)|
+-----+-----------------------------------+--------------------+--------------------+---------------------+
```

### 3.1 Steve Runtime & Central Demo Gates (Current Launch Focus)
- **S1: Single-Space Steve Ingest & Recall:**
  - Tests existing single-space GTR-T5 / lexical recall over recovered or fresh fixture state.
  - **Not gated on M1, M2, or M3.**
- **S2: Controlled Clatter Demonstration (Human Requested Priority):**
  - Evaluates the executable ordinary retrieval arm over the clatter/limited-search/distractor/corner-find fixture.
  - Asserts that historical observations are immutable in the Evidence Plane while interpretation evolves.

### 3.2 Conditional Multimodal Gates (Not Prerequisites for Single-Space Steve)
- **M1 (formerly E1): Vector Bank Space Isolation:**
  - Protects against cross-space dot products between 768-d GTR-T5 and 768-d SigLIP 2.
  - *Scope:* Only needed when more than one embedding space is active.
- **M2 (formerly E2): Zero-BM25 Visual Candidate Survival:**
  - Evaluates additive fallback `score = sem + beta * skg` in `hybrid.rs` for textless visual assets.
  - *Scope:* Only needed for visual retrieval lanes.
- **M3 (formerly E3): Dual-Path Migration & Rollback:**
  - Evaluates running SigLIP alongside legacy GTR snapshots.
  - *Scope:* Only needed if format migration is triggered.

---

## 4. Dataset & Source Provenance Registry (Dated Generations)

All empirical references must cite specific index runs and dated artifacts:

| Dataset / Source Name | Storage Location / URI | Provenance & Generation Timestamp | Research Role |
|---|---|---|---|
| **Local Memory Corpus** | `/workspace/memory/research/` | 44 files, 720 sections (2026-09-19T03:00Z index run)<br>46 files, 750 sections (2026-09-19T05:14Z index run) | Evidence plane benchmark for hybrid search & indexing. |
| **arXiv Research Papers** | `/workspace/memory/research/*.pdf` | 11 PDFs (including `2506.10801.pdf`, `2601.01253.pdf`, `2604.07401.pdf`) | Theoretical reference for LSR, DMFT, and DenseAM. |
| **Extracted Hoover Text** | [`research/2506.10801_extracted.md`](file:///workspace/memory/research/2506.10801_extracted.md) | 22 pages verbatim via `pypdf` (arXiv:2506.10801v2) | Mathematical reference text for LSR equations. |
| **LoCoMo Benchmark** | Referenced in `benchmarks.md` & [`2402.17753.pdf`](file:///workspace/memory/research/2402.17753.pdf) | Maharana et al. (300 turns, 35 sessions) | Longitudinal conversational memory evaluation. |
| **BEAM Benchmark** | Referenced in `benchmarks.md` & [`2510.27246.pdf`](file:///workspace/memory/research/2510.27246.pdf) | Tavakoli et al. (1M to 10M token test suite) | Stress test for cross-session recall and contradiction handling. |
| **Kord Campbell Transcript** | [`research/kordcampbellselfawaremachines.txt`](file:///workspace/memory/research/kordcampbellselfawaremachines.txt) | Auto-caption transcript, Lucidworks Activate | Source authority for situated awareness and empty throne. |

---

## 5. Architectural Agreement with Coordinator

1. **Agreement on BUILD_PLAN Execution Sequence:**
   We agree with the execution sequence 1–7. Testing existing single-space Steve proceeds immediately without waiting on multimodal gates M1–M3 or new-model inversions.
2. **Central Demo Priority:**
   The clatter scenario is the central requested demonstration. It evaluates meaning change without event rewriting.
3. **No Unapproved Release Assignments:**
   Missing contracts (R3 #1–3) are tracked as architectural gaps without assigning unauthorized release targets.
4. **LSR Status:**
   Reaffirmed as an optional R&D research track.
