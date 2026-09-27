# Dense Associative Memory (DenseAM), Log-Sum-ReLU (LSR), and Stochastic Thermodynamics

**Location:** `/workspace/memory/research/denseam-lsr-physics.md`  
**Date:** 2026-09-19  
**Author:** Antigravity (Research / Crusade Spicy Meatball / Difficult Stork)  
**Status:** Verified research briefing from arXiv preprints and frontier briefing.

---

## 1. Verified Corpus and References

| Paper / Citation | Venue / Date | Key Contribution |
|---|---|---|
| **Rooke, Krotov, Balasubramanian, & Wolpert** ([arXiv:2601.01253v2](https://arxiv.org/abs/2601.01253)) | cond-mat.stat-mech (Jan 2026, rev. Apr 2026) | *Stochastic Thermodynamics of Associative Memory*: Non-equilibrium work $W$, power $P$, and total entropy production $\Delta S_{\text{tot}}$ in polynomial DenseAMs via Dynamical Mean Field Theory (DMFT). Discovers finite-temperature failure mode for $k > 2$. |
| **Petrova, Polyachenko, & State** ([arXiv:2604.07401v2](https://arxiv.org/abs/2604.07401)) | ICML 2026 / cond-mat.dis-nn (Apr 2026, rev. May 2026) | *Geometric Entropy and Retrieval Phase Transitions in Continuous Thermal Dense Associative Memory*: Phase boundary analysis for continuous neurons on $S^{N-1}$. Compares Gaussian (LSE) vs Epanechnikov (LSR) kernels. Identifies critical capacity $\alpha_{\text{th}} = 0.5$. |
| **Hoover et al.** ([arXiv:2506.10801v2](https://arxiv.org/abs/2506.10801)) | cs.LG / NeurIPS 2025 | *Dense Associative Memory with Epanechnikov Energy*: Introduces Log-Sum-ReLU (LSR) energy with finite support; proves exponential capacity and $\Theta(M^{1/d})$ emergent minima. |
| **Krotov & Hopfield** (2016, 2021); **Ramsauer et al.** (ICLR 2021) | PNAS / ICLR | Foundational DenseAM and continuous modern Hopfield networks establishing the exact mathematical equivalence between Hopfield energy minimization and transformer attention. |

---

## 2. DenseAM Foundations: Energy Landscapes & Modern Hopfield Formulation

### 2.1 The Unified Continuous Energy Functional
Dense Associative Memory networks generalize classical Hopfield networks (Hopfield 1982, capacity $C \approx 0.14N$) by replacing pairwise quadratic interactions with non-linear interaction functions $F(z)$ that scale exponentially with dimension $N$.

For a state vector $\mathbf{x} \in \mathbb{R}^N$ and $M$ stored memory pattern vectors $\{\mathbf{\xi}^\mu\}_{\mu=1}^M$, the continuous modern Hopfield energy functional is given by:

$$E(\mathbf{x}) = - F\left( \sum_{\mu=1}^M f(\mathbf{\xi}^\mu \cdot \mathbf{x}) \right) + \frac{1}{2} \|\mathbf{x}\|^2$$

The discrete-time state update derived from minimizing $E(\mathbf{x})$ via concave-convex procedure (CCCP) yields:

$$\mathbf{x}^{(t+1)} = \sum_{\mu=1}^M \frac{f'(\mathbf{\xi}^\mu \cdot \mathbf{x}^{(t)})}{\sum_{\nu=1}^M f'(\mathbf{\xi}^\nu \cdot \mathbf{x}^{(t)})} \mathbf{\xi}^\mu$$

When $f(z) = \exp(\beta z)$ (Gaussian/exponential interaction), the update equation becomes:

$$\mathbf{x}^{(t+1)} = \sum_{\mu=1}^M \text{softmax}\left(\beta\, \mathbf{\xi}^\mu \cdot \mathbf{x}^{(t)}\right) \mathbf{\xi}^\mu$$

This proves that **single-step transformer cross-attention is identical to the first relaxation step of a continuous Dense Associative Memory**.

---

## 3. Kernel Mathematics: Log-Sum-Exp (LSE) vs Log-Sum-ReLU (LSR)

### 3.1 The Gaussian / Log-Sum-Exp (LSE) Dilemma
In classical modern Hopfield architectures (Ramsauer et al. 2020), the interaction is governed by the Log-Sum-Exp function:

$$E_{\text{LSE}}(\mathbf{x}) = -\frac{1}{\beta} \ln \left( \sum_{\mu=1}^M \exp\left(\beta\, \mathbf{\xi}^\mu \cdot \mathbf{x}\right) \right) + \frac{1}{2} \|\mathbf{x}\|^2$$

**The Infinite Support Problem:**
- The Gaussian kernel $\exp(\beta z)$ has **infinite support** on $\mathbb{R}^N$ (and on the unit hypersphere $S^{N-1}$).
- For any query $\mathbf{x}$, *every* stored memory $\mathbf{\xi}^\mu$ contributes a strictly non-zero Boltzmann weight $\exp(\beta \mathbf{\xi}^\mu \cdot \mathbf{x}) > 0$.
- In a memory system with $M = e^{\alpha N}$ stored memories, the sum of exponentially many small tail overlaps produces a non-zero background variance (thermal interference).
- **Petrova et al. (arXiv:2604.07401) Finding:** For LSE, a critical retrieval boundary line exists at **all** load levels $\alpha > 0$. There is no non-zero temperature at which retrieval is completely free from background noise.

### 3.2 The Epanechnikov / Log-Sum-ReLU (LSR) Breakthrough
Hoover et al. (arXiv:2506.10801) and Petrova et al. (arXiv:2604.07401) substitute the Gaussian kernel with an Epanechnikov-type kernel with compact support:

$$f_{\text{LSR}}(z) = \left[ \max(0, z - \theta) \right]^p = \left[ \text{ReLU}(z - \theta) \right]^p$$

where $\theta \in (0, 1)$ is a geometric activation threshold (cutoff radius) and $p \ge 2$ ensures smooth derivatives.

The corresponding LSR energy is:

$$E_{\text{LSR}}(\mathbf{x}) = - \frac{1}{p} \sum_{\mu=1}^M \left[ \text{ReLU}(\mathbf{\xi}^\mu \cdot \mathbf{x} - \theta) \right]^p + \frac{1}{2} \|\mathbf{x}\|^2$$

### 3.3 Theoretical Properties Derived by Petrova et al. (arXiv:2604.07401)
1. **Geometric Entropy Independence:** For continuous neurons constrained to the $N$-sphere $S^{N-1}$, the geometric entropy $S_{\text{geom}}$ depends solely on the spherical surface geometry $\Omega(N) = \frac{2\pi^{N/2}}{\Gamma(N/2)}$, and is invariant to the choice of kernel.
2. **Critical Load Threshold ($\alpha_{\text{th}} = 0.5$):** Under the sharp-kernel regime at $T = 0$, the maximum theoretical memory load capacity is $\alpha_{\text{max}} = 0.5$ ($M = e^{0.5 N}$).
3. **Zero-Noise Regime Below Threshold:** Due to the finite support of $\text{ReLU}(z - \theta)$, any pattern with overlap $\mathbf{\xi}^\mu \cdot \mathbf{x} < \theta$ contributes **identically zero** to the energy and gradient:
   $$\forall \mu \text{ such that } \mathbf{\xi}^\mu \cdot \mathbf{x} < \theta \implies \nabla_{\mathbf{x}} E_\mu(\mathbf{x}) \equiv \mathbf{0}$$
   Consequently, below $\alpha_{\text{th}}$, **spurious patterns do not contribute to the noise floor at all**. Unlike LSE, no critical line exists below $\alpha_{\text{th}}$: retrieval is mathematically exact even at non-zero thermal temperatures ($T > 0$).
4. **Emergent Energy Minima:** Hoover et al. prove that between stored memory clusters, the finite overlap of support spheres creates $\Theta(M^{1/d})$ stable emergent local minima. These are not spurious errors, but structured, synthetic concept centroids representing valid generalizations across multiple episodic instances.

---

## 4. Stochastic Thermodynamics of Associative Memory (Rooke et al., arXiv:2601.01253)

### 4.1 Non-Equilibrium Framework & Mean Field Dynamics
Rooke, Krotov, Balasubramanian, and Wolpert analyze continuous and polynomial DenseAMs operated out of equilibrium by external driving fields (corrupted query cues) in a thermal bath at temperature $T = \beta^{-1}$.

Using Dynamical Mean Field Theory (DMFT), the authors establish the stochastic master equation governing the memory overlap trajectory $m^\mu(t) = \frac{1}{N} \mathbf{\xi}^\mu \cdot \mathbf{x}(t)$:

$$\frac{d m^\mu}{dt} = -\Gamma \left( m^\mu - \tanh\left( \beta \sum_{\nu=1}^M J_{\mu\nu} (m^\nu)^{k-1} + h_{\text{ext}}^\mu(t) \right) \right) + \eta^\mu(t)$$

where $\Gamma$ is the kinetic relaxation rate, $k$ is the interaction polynomial order, and $\eta^\mu(t)$ is Gaussian thermal noise.

### 4.2 Work, Free Energy, and Entropy Production
When a memory system is driven from an initial corrupted state distribution $P_0(\mathbf{x})$ to a target attractor $P_\tau(\mathbf{x})$ in finite time $\tau$, the non-equilibrium thermodynamic quantities are rigorously defined:

1. **Thermodynamic Work ($W$):**
   $$W = \int_0^\tau dt \sum_{i=1}^N \dot{h}_i^{\text{ext}}(t) \langle x_i(t) \rangle$$
2. **Free Energy Change ($\Delta F$):**
   $$\Delta F = F[P_\tau] - F[P_0]$$
3. **Total Irreversible Entropy Production ($\Delta S_{\text{tot}}$):**
   By the second law of non-equilibrium thermodynamics (and the Jarzynski / Crooks fluctuation relations):
   $$\Delta S_{\text{tot}} = \beta (W - \Delta F) \ge 0$$
   Here $\Delta S_{\text{tot}} = 0$ only in the quasi-static, infinitely slow limit ($\tau \to \infty$).

### 4.3 Key Discoveries from Rooke et al.
1. **The Fundamental Trilemma:** There exists a strict thermodynamic trade-off between:
   - **Retrieval Error ($\epsilon = 1 - \mathcal{A}$)**
   - **Operation Speed ($\tau^{-1}$)**
   - **Dissipated Entropy ($\Delta S_{\text{tot}}$)**
   Pushing an agent memory to retrieve faster ($\tau \to 0$) forces an asymptotic divergence in required driving power: $P \sim \tau^{-2}$, causing irreversible heat dissipation that scales inversely with convergence time.
2. **Finite-Temperature Failure Mode ($k > 2$ Spurious Trap):**
   - For higher-order polynomial interactions ($k > 2$, which provide super-linear memory capacity at $T=0$), finite temperature introduces a catastrophic failure mode.
   - At $T > 0$, a stable spurious local free-energy minimum forms at **zero alignment** ($\sum_\mu (m^\mu)^2 \approx 0$).
   - If an agent's initial cue has overlap below a critical basin radius $m_0 < m^*_c(T)$, the network does not converge to the nearest memory; instead, thermal fluctuations trap the state permanently in the null attractor ($m=0$).
   - This failure mode **does not exist at $T=0$** and is invisible to zero-temperature simulations.

---

## 5. Architectural Translation: What This Dictates for Ferricula vs Lume

| Physical / Theoretical Property | Current `ferricula_v2` State | Required Engine / Retrieval Architecture |
|---|---|---|
| **Energy Kernel** | Hard-coded cosine similarity ($0.85$ merge gate in `dream.rs`) | Replace cosine merge threshold with **Epanechnikov LSR kernel** ($p=2, \theta \approx 0.70$). Stored records outside the support radius $\theta$ exert exactly zero interference. |
| **Emergent Consolidation** | Centroid averaging of top-$k$ vectors | Emergent local minima of LSR provide mathematically guaranteed synthetic concept nodes during offline dream cycles. |
| **Noise & Temperature Scheduling** | Heuristic `decay_tick` and random byte mutation | Thermal annealing schedule during dream consolidation: Start at finite $T > 0$ to escape shallow local traps, but enforce driving field $h_{\text{ext}} > m^*_c(T)$ to prevent capture by the $k>2$ zero-alignment spurious minimum discovered by Rooke et al. |
| **Entropy Accounting** | Abstract "decay alpha" | Formalize computational work and entropy bookkeeping in dream cycles: measure convergence time $\tau$ and track the fidelity degradation rate as bounded physical dissipation. |
| **Separation of Labor** | Mixed document retrieval and memory lifecycle | **Lume** owns the high-dimensional document index (BM25 + GTR-T5/Qwen3 + SKG). **Ferricula** owns the dynamical associative state space (LSR energy landscape, attractor relaxation, and dream-cycle consolidation). |

---

## 6. Synthesis & Next Steps
- **Immediate Task for Ferricula Engine Team (Added Armadillo):** Implement a standalone LSR relaxation kernel in `ferricula-core` / `ferricula-cognition` evaluating Epanechnikov energy descent vs cosine thresholding.
- **Verification Criterion:** Test memory recovery on corrupted embeddings across a load sweep $\alpha \in [0.1, 0.6]$, proving zero noise below $\alpha_{\text{th}} = 0.5$.
