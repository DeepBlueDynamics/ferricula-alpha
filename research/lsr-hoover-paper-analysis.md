# Primary Source Analysis: Hoover et al. (arXiv:2506.10801v2) and Comparison with Ferricula

**Document:** `research/lsr-hoover-paper-analysis.md`  
**Primary Source:** *Dense Associative Memory with Epanechnikov Energy*, Benjamin Hoover, Zhaoyang Shi, Krishnakumar Balasubramanian, Dmitry Krotov, Parikshit Ram (IBM Research, Harvard, UC Davis; NeurIPS 2025; arXiv:2506.10801v2, rev. 2 Feb 2026).  
**Raw Text Extraction:** [`research/2506.10801_extracted.md`](2506.10801_extracted.md) (22 pages extracted verbatim from local `2506.10801.pdf` via `pypdf`).  
**Audience:** Appalling Goldfish (Coordinator & Architect) & Added Armadillo (Engine).

---

## 1. Primary Mathematical Formulations (Verbatim Paper Equations)

### 1.1 The General Density–Energy Correspondence
An energy function $E: \mathbb{R}^d \to \mathbb{R}$ induces an unnormalized probability density $p(x) \propto \exp[-E(x)]$. Conversely, $E(x) \propto -\log p(x)$ (negative log-likelihood).

Under general DenseAM parameterization (Paper Eq. 1):
$$E_\beta(x; \Xi) = -Q\left( \sum_{\mu=1}^M F(\beta S(g(x), \xi^\mu)) \right)$$
where $g$ is vector normalization, $S$ is similarity, $\beta$ is inverse temperature, $F$ is a separation function, and $Q$ is monotonic scaling.

### 1.2 Classical Log-Sum-Exp (LSE) Energy
$$E_\beta^{\text{LSE}}(x; \Xi) = -\frac{1}{\beta} \log \sum_{\mu=1}^M \exp\left( -\frac{\beta}{2} \|x - \xi^\mu\|^2 \right)$$
- Corresponds to Kernel Density Estimation (KDE) with a **Gaussian kernel**.
- Because the Gaussian has infinite support ($\exp(-u) > 0 \; \forall u$), every stored memory exerts non-zero pull at every point in $\mathbb{R}^d$.

### 1.3 Log-Sum-ReLU (LSR) / Epanechnikov Energy (Paper Eq. 3)
Substituting the optimal Epanechnikov kernel $K_{\text{epan}}(u) = \max(1 - u^2, 0) = \text{ReLU}(1 - u^2)$:

$$E_\beta^{\text{LSR}}(x; \Xi) = -\frac{1}{\beta} \log \left( \epsilon + \sum_{\mu=1}^M \text{ReLU}\left( 1 - \frac{\beta}{2} \|x - \xi^\mu\|^2 \right) \right)$$

**Parameters:**
- $x \in \mathcal{X} \subseteq \mathbb{R}^d$: State vector.
- $\Xi = \{\xi^1, \dots, \xi^M\} \subset \mathbb{R}^d$: $M$ stored memory patterns.
- $\beta > 0$: Inverse temperature (effective kernel bandwidth $h = \sqrt{2/\beta}$).
- Metric: Negative squared Euclidean distance $S(x, \xi^\mu) = -\frac{1}{2}\|x - \xi^\mu\|^2$.
- $\epsilon \ge 0$: Small regularizer. For $\epsilon = 0$, defining $S_\mu \triangleq \{x \in \mathcal{X} : \|x - \xi^\mu\| \le \sqrt{2/\beta}\}$, then $\forall x \notin \bigcup_{\mu=1}^M S_\mu$, $E_\beta^{\text{LSR}}(x) = +\infty$.

---

## 2. Gradient, Fixed Points, and Emergent Minima

### 2.1 Energy Gradient
Let $B(x) \triangleq \left\{ \mu \in [M] : \|x - \xi^\mu\| \le \sqrt{2/\beta} \right\}$ be the active index set at $x$. For $\epsilon \to 0$ and $x \in \bigcup S_\mu$:

$$\nabla_x E_\beta^{\text{LSR}}(x; \Xi) = \frac{\sum_{\mu \in B(x)} (x - \xi^\mu)}{\sum_{\mu \in B(x)} \left( 1 - \frac{\beta}{2} \|x - \xi^\mu\|^2 \right)}$$

### 2.2 Stationary Points / Local Minima (Paper Proposition 2)
Stationary points require $\nabla_x E_\beta^{\text{LSR}}(x; \Xi) = 0 \iff \sum_{\mu \in B(x)} (x - \xi^\mu) = 0$:

$$x^* = \frac{1}{|B(x^*)|} \sum_{\mu \in B(x^*)} \xi^\mu$$

- **Isolated Case ($|B(x^*)| = 1$):** $x^* = \xi^\mu$. The stationary point is **identically the stored memory pattern**.
- **Emergent Case ($|B(x^*)| > 1$):** $x^*$ is an **arithmetic centroid (mean)** of the active subset $B(x^*)$. With probability 1, $x^* \notin \Xi$.

### 2.3 Single-Step Exact Retrieval (Paper Theorem 1)
Let $r = \min_{\mu \neq \nu} \|\xi^\mu - \xi^\nu\|$ be the minimum pairwise distance between memories.  
Let $S_\mu(\Delta) = \{x : \|x - \xi^\mu\| \le \Delta\}$ be a basin around $\xi^\mu$ with radius $\Delta \in (0, r)$.  
Setting $\beta = \frac{2}{(r - \Delta)^2}$:
- For any $x \in S_\mu(\Delta)$, $|B(x)| = \{\mu\}$ (only memory $\mu$ is active).
- With learning rate $\eta = 1 - \frac{\beta}{2}\|x - \xi^\mu\|^2$, gradient descent converges in **exactly one step**:
  $$x - \eta \nabla E = x - \left( 1 - \frac{\beta}{2}\|x - \xi^\mu\|^2 \right) \frac{x - \xi^\mu}{1 - \frac{\beta}{2}\|x - \xi^\mu\|^2} = \xi^\mu$$

---

## 3. Core Theorems on Capacity and Emergence

### 3.1 Failure of LSE to Exhibit Global Emergence (Paper Proposition 1)
- In LSE, for finite $\beta$, local minima are close to but distinct from original memories (original patterns are never exact minima).
- For $\beta \to \infty$, original memories are exact minima, but the set of novel minima is empty.
- When LSE merges basins to create novel minima, the constituent original memories cease to be local minima.
- **Conclusion:** LSE cannot simultaneously achieve exact recall of all stored memories and retain novel emergent minima.

### 3.2 Simultaneous Exact Memorization and Emergence in LSR (Paper Theorem 2)
Under uniform sampling on volume $V$:
1. **Exponential Capacity:** All $M = \Theta\left(\sqrt{1-\delta} \exp(\alpha d)\right)$ stored memories are retrievable with high probability ($1-\delta$).
2. **Emergent Basin Existence:** Each emergent centroid $x^* = \frac{1}{|B(x^*)|} \sum_{\mu \in B(x^*)} \xi^\mu$ possesses its own distinct basin of attraction $S_{x^*}(r^*)$ of radius $r^* > 0$ (Eq. 4 in paper).
3. **Emergent Count Scaling:** The number of globally emergent memories scales as:
   $$\mathcal{O}\left( \exp\left[ M \frac{V_d}{V} \left(\frac{2}{\beta}\right)^{d/2} \log\left( \frac{eV}{V_d} \left(\frac{\beta}{2}\right)^{d/2} \right) \right] \right)$$
   Under a regular grid (Proposition 3), the count scales as $\Theta\left( (M^{1/d} - \lambda^{1/d} + 1)^d \right)$.

---

## 4. Rigorous Comparison with Ferricula Codebase (`ferricula_v2`)

| Dimension | Hoover et al. (arXiv:2506.10801v2) | `ferricula_v2` Current Implementation | Status & Gap |
|---|---|---|---|
| **Energy Function** | $E_\beta^{\text{LSR}} = -\frac{1}{\beta} \log\left(\epsilon + \sum \text{ReLU}\left(1 - \frac{\beta}{2}\|x - \xi\|^2\right)\right)$ | **Absent.** No energy function or gradient descent is defined in `ferricula-cognition` or `ferricula-core`. | **Architectural Gap:** `ferricula_v2` does not execute energy minimization. |
| **Similarity Metric** | Negative squared Euclidean distance: $-\frac{1}{2}\|x - \xi\|^2$. | Cosine similarity: $\frac{\mathbf{u} \cdot \mathbf{v}}{\|\mathbf{u}\|\|\mathbf{v}\|}$ (`ferricula-core/src/vector.rs:17`). | **Metric Mismatch:** LSR proofs require squared Euclidean metric for Epanechnikov kernel equivalence. |
| **Consolidation Algorithm** | Emergent local minima form at centroids $x^* = \frac{1}{\|B\|} \sum_{\mu \in B} \xi^\mu$ **without deleting** original patterns $\xi^\mu$. | `dream.rs:416` (`consolidate_group`): Selects single survivor with `max_by fidelity`, transfers edges, and **archives all other group members**. | **Operational Contradiction:** Code archives absorbed records; Hoover preserves originals and adds emergent minima. |
| **Consolidation Clustering** | Epanechnikov radius $\|x - \xi\| \le \sqrt{2/\beta}$. | Hardcoded cosine threshold: `cosine >= 0.85` (`dream.rs:328`). | **Threshold Mismatch:** Fixed cosine cut does not reflect temperature parameter $\beta$. |
| **Reconciliation of Prior Notes** | Eq. 3 requires $-\frac{1}{\beta} \log(\dots)$ with squared Euclidean argument. | Prior note `denseam-lsr-physics.md §3.2` wrote a dot-product power sum without $\log$. | **Corrected:** `denseam-lsr-physics.md` was mislabeled; Hoover Eq. 3 is now verified and documented. |

---

## 5. Architectural Recommendations for Engine Integration

1. **Keep `v2.0` Engine Intact (No Immediate Crate Edits):**
   - In accordance with PLAN §8.3 ("Formats frozen for v2.0") and current stability invariants, do not attempt to replace `consolidate_group` with an unverified energy solver in v2.0.
2. **Path for v2.1 Cognitive Energy Module:**
   - If LSR is to be implemented, create a standalone module `ferricula-cognition::energy::lsr`:
     - Implement exact Eq. 3 with normalized vectors (where $\|x - \xi\|^2 = 2 - 2 \langle x, \xi \rangle$, reconciling Euclidean distance with unit hyperspheres).
     - Under unit normalization:
       $$1 - \frac{\beta}{2}\|x - \xi^\mu\|^2 = 1 - \beta(1 - \langle x, \xi^\mu \rangle) = (1 - \beta) + \beta \langle x, \xi^\mu \rangle$$
       This allows Epanechnikov support to be computed via dot products with threshold $\theta = 1 - \frac{1}{\beta}$.
     - When $|B(x)| > 1$ during offline dream cycles, instead of archiving the group, create an explicit synthetic `Emergent` record whose vector is the normalized centroid $\frac{\sum \xi^\mu}{\|\sum \xi^\mu\|}$, leaving constituent evidence nodes intact with causal provenance edges.
