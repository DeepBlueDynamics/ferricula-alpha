# Memory systems and Hoover et al. 2506.10801

**Date:** 2026-10-01
**Author:** nemesis8/n8-calm-panda (Grok 4.7)
**Package:** L4 in `inbox/OVERNIGHT_2026-10-01.md`
**Scope:** Research writing only. No code, no builds.
**Hoover source text:** `research/2506.10801_extracted.md` (arXiv:2506.10801v2, 2 Feb 2026). Every claim in Part 1 cites a section heading of that extraction. Notes corrected, and not edited, are the copies on `origin/v3/r0`: `research/lsr-hoover-paper-analysis.md` and `research/denseam-lsr-physics.md`.
**Agent-memory sources:** the papers named below, read as arXiv HTML on 2026-10-01. A number without that reading is marked UNVERIFIED.

---

## Part 1. Log-sum-ReLU / Epanechnikov energy (arXiv:2506.10801)

Authors on page 1 of the extraction: Benjamin Hoover (IBM Research, Georgia Tech), Zhaoyang Shi (Harvard), Krishnakumar Balasubramanian (UC Davis), Dmitry Krotov (IBM Research), Parikshit Ram (IBM Research). Venue line: 39th Conference on Neural Information Processing Systems (NeurIPS 2025).

### 1.1 What the energy is

Section **1 Associative Memories and Energy Landscapes** writes a general DenseAM energy (eq. 1)

$$E_\beta(x;\Xi)=-Q\Big(\sum_{\mu=1}^{M} F(\beta S(g(x),\xi^\mu))\Big)$$

with $g$ a vector operation (examples given there: binarization, layer normalization), $S$ a similarity, $\beta>0$ an inverse temperature, $F$ a separation function, and $Q$ a monotonic scaling. Classical Hopfield is the special case of sign patterns, a dot-product similarity, a quadratic $F$, and a linear $Q$, with capacity $M^\star=O(d)$. Power separations $F(x)=x^p$ ($p>2$) give $M^\star=O(d^p)$. The log-sum-exp (LSE) choice $F(x)=\exp(x)$, $Q(x)=\log x$ is cited there as the route to exponential capacity.

Section **2 Kernel Density Estimation and the Choice of Kernels** treats $\exp[-E]$ as an unnormalized kernel density estimate. Among symmetric, positive, normalized kernels, the Epanechnikov kernel $K_{\mathrm{epan}}(x)=\max\{1-x^2,0\}=\mathrm{ReLU}(1-x^2)$ minimizes the leading MISE (their eq. 2). Efficiency of another kernel is $\mathrm{Eff}(K)=\sigma_K/\sigma_{K_{\mathrm{epan}}}$.

Section **3 A New Energy Function with Emergent Memory Capabilities** then sets the LSE energy to

$$E^{\mathrm{LSE}}_\beta(x;\Xi)=-\frac{1}{\beta}\log\sum_{\mu=1}^{M}\exp\Big(-\frac{\beta}{2}\|x-\xi^\mu\|^2\Big)$$

and the log-sum-ReLU (LSR) energy, also called Epanechnikov energy, to eq. (3)

$$E^{\mathrm{LSR}}_\beta(x;\Xi)=-\frac{1}{\beta}\log\Big(\epsilon+\sum_{\mu=1}^{M}\mathrm{ReLU}\big(1-\frac{\beta}{2}\|x-\xi^\mu\|^2\big)\Big).$$

The separation drawn in their Figure 2 is $F(\beta x)=\mathrm{ReLU}(1+\beta x)$ with similarity $S=-\tfrac{1}{2}\|x-x'\|^2$, which is the same cutoff as $K_{\mathrm{epan}}$ once the kernel argument is the scaled Euclidean distance. Support is the Euclidean ball $S_\mu=\{x:\|x-\xi^\mu\|\le\sqrt{2/\beta}\}$. For $\epsilon=0$, energy is infinite outside the union of those balls (zero density). The Gaussian/LSE kernel has no such cutoff: section 3 says every stored pattern still contributes.

Appendix **E.1 Proof of Theorem 1** gives the gradient (eqs. 7–8): numerator $\sum_{\mu\in B(x)}(x-\xi^\mu)$, denominator $\epsilon$ plus the sum of the active ReLU terms, with $B(x)=\{\mu:\|x-\xi^\mu\|^2\le 2/\beta\}$.

### 1.2 What Hoover et al. actually prove and measure

**Exact retrieval of stored patterns.** Section 3, Theorem 1: let $r$ be the minimum Euclidean distance between memories and let $S_\mu(\Delta)$ be the ball of radius $\Delta\in(0,r)$ about memory $\mu$. With $\beta=2/(r-\Delta)^2$, gradient descent on LSR from any point in that ball returns exactly $\xi^\mu$. If the learning rate is set appropriately, it returns in one step. Appendix E.1 states that rate: $\eta\leftarrow\epsilon+\mathrm{ReLU}(1-\beta/2\|x-\xi^\mu\|^2)$, and shows $B(x)=\{\mu\}$ inside the basin. Remark 1 in section 3: at a stored memory and finite appropriate $\beta$, the LSR gradient is exactly zero. The LSE gradient is only approximately zero for finite $\beta$, and exactly zero at a memory only for $\beta=\infty$.

**LSE cannot keep exact memories and novel minima together.** Section 3, Definition 2 ($\varepsilon$-global emergence): every original memory is a strict local minimum and the set of novel local minima is non-empty. Definition 1 requires a novel minimum to sit at least $\varepsilon$ away from every stored pattern. Three cases are written out: finite $\beta$ moves LSE minima off the stored patterns; $\beta=\infty$ makes the stored patterns exact and empties the novel set; moderate $\beta$ can form a novel minimum only by merging basins, which ceases to be minima for the memories that merged. Proposition 1: for patterns drawn i.i.d. from a density fully supported on $\mathcal{X}$, LSE does not satisfy $\varepsilon$-global emergence for any $\beta>0$. Definition 3 (locally emergent memory) is the weaker, per-minimum version; the same section says LSE fails that too, because the exponential has infinite support.

**Emergent LSR minima are centroids, and capacity is exponential.** Section 3, Proposition 2: a local minimum is the average of the memories in the active set $B(x)$. If $|B|=1$ that average is the stored pattern. If $|B|>1$ it is not equal to any stored pattern (probability 1). Theorem 2, still in section 3, under uniform samples on a finite volume $V$:

- (a) With probability at least $\delta\in(0,1)$, all $M=\Theta\big(\sqrt{1-\delta}\,\exp(\alpha d)\big)$ memories are retrievable in the sense of Theorem 1, for a positive $\alpha$, with minimum distance at least $(V_d/V)^{-1/d}e^{-2\alpha}$ and $\beta$ no larger than the Theorem 1 value for a basin radius inside that distance. $V_d$ is the volume of the unit ball in $\mathbb{R}^d$. Appendix **E.3 Proof of Theorem 2** sets the success probability $\delta=1-M^2 e^{-2\alpha d}$, which rearranges to that $M$. So $1-\delta$ is the failure budget, not the success probability.
- (b) Each novel minimum $x^*$ has a basin of radius $r^*>0$ given as eq. (4), and gradient descent inside that basin returns exactly $x^*$.
- The number of $\varepsilon$-globally emergent memories is, with probability at least $1-M^{-2}$, the big-O exponential in their display (5): $\exp\big[M(V_d/V)(2/\beta)^{d/2}\log\big((eV/V_d)(\beta/2)^{d/2}\big)\big]$. For fixed $\beta$ and $d$ the bound grows with $M$ while $1<|B(x)|\le M$.

Proposition 3, same section, on a regular grid of volume $V$: the number of emergent memories is $\Theta\big((M^{1/d}-\lambda^{1/d}+1)^d\big)$, where $\lambda=\Theta\big(M V^{-1}(8/\beta)^{d/2}\big)$ and $\beta$ is such that $1<\lambda\le M$. The same paragraph says the count depends on the geometry of the stored set, can grow much faster than linear in $M$, and is naively at most $2^M$.

Section **5 Discussion** adds that compact-support kernels other than Epanechnikov also produce emergent memories and even manifolds (their Figure 6 and section C). Epanechnikov is the MISE-optimal kernel in section 2; it is not claimed to be the only kernel with emergence.

**What the experiments measure.** Section **4.1 Quantifying the scaling of emergent memories**: $M$ patterns drawn uniformly in the $d$-dimensional unit hypercube; every subset centroid is tested for finite energy at $\epsilon=0$ and a small gradient. At critical $\beta$ they report orders of magnitude more emergent memories than stored patterns (Figure 3, left, log-scaled axes), including regimes where a majority ($>60\%$) of stored patterns are still recoverable and about 20 percent of the hypercube is still supported. Low $\beta$ can show local emergence (Definition 3) without global emergence (Definition 2).

Section **4.2 Generative quality of emergent memories**: a mixture of $k=10$ Gaussians in $d=8$, means uniform on the unit hypercube, $\sigma=0.1$; $M$ samples from that mixture are the stored patterns; $N=500$ queries start on a thin support boundary; error bars are the standard error over 5 seeds. LSR log-likelihood under the true density is comparable to, and occasionally slightly higher than, LSE, with more unique samples, while stored patterns stay recoverable. Section **5 Discussion** says the high-likelihood LSE samples are homogeneous: those 500 queries converge to the same $\sim 10$ memories. Section **6 Conclusion** repeats comparable log-likelihood and an order of magnitude more unique memories.

Section **4.3 Emergent memories in latent space**: 24 MNIST images in a 10-dimensional VAE, and 40 TinyImageNet images in a 256-dimensional pretrained VAE. Emergent memories are centroids of small subsets; decoded, they look like plausible generations. At the $\beta$ they chose, TinyImageNet LSR is globally emergent (every stored pattern recoverable) and the MNIST run is not. At that same $\beta$, LSR produces an order of magnitude more total memories than LSE, and LSE can retrieve at most $M$ memories. Section **D.6 Additional experiment: Scaling number of stored patterns**: all 60,000 MNIST training images, a $\beta$ at which about 50 percent of stored patterns remain retrievable; Figure 10 uses $\beta=0.11$, and the seed image's basin meets about 7.3k other stored patterns.

### 1.3 Corrections

Quoted lines are from the `origin/v3/r0` copies. Petrova (arXiv:2604.07401) and Rooke (arXiv:2601.01253) claims in `denseam-lsr-physics.md` were not re-read against those papers. They are UNVERIFIED here, except where the sentence also attributes a result to Hoover.

**`research/lsr-hoover-paper-analysis.md`**

1. Wrong line (§1.1): "where $g$ is vector normalization, $S$ is similarity, $\beta$ is inverse temperature, $F$ is a separation function, and $Q$ is monotonic scaling."
   Section **1 Associative Memories and Energy Landscapes** says $g:\mathbb{R}^d\to\mathbb{R}^d$ is a vector operation, with binarization and layer normalization as examples, not normalization as the definition.

2. Wrong line (§2.3): "For any $x \in S_\mu(\Delta)$, $|B(x)| = \{\mu\}$ (only memory $\mu$ is active)."
   Appendix **E.1 Proof of Theorem 1**: "For any $x\in S_\mu(\Delta)$, $B(x)=\{\mu\}$." The set equality is on $B(x)$. The cardinality is 1.

3. Wrong line (§2.3): "With learning rate $\eta = 1 - \frac{\beta}{2}\|x - \xi^\mu\|^2$, gradient descent converges in exactly one step."
   Appendix **E.1 Proof of Theorem 1** sets $\eta\leftarrow\epsilon+\mathrm{ReLU}\big(1-\beta/2\|x-\xi^\mu\|^2\big)$. The quoted rate equals that only for $\epsilon=0$ and a point inside the support, where the ReLU argument is positive.

4. Wrong line (§3.2): "All $M = \Theta\big(\sqrt{1-\delta}\,\exp(\alpha d)\big)$ stored memories are retrievable with high probability ($1-\delta$)."
   Theorem 2 in section **3 A New Energy Function with Emergent Memory Capabilities** says the probability is at least $\delta$. Appendix **E.3 Proof of Theorem 2** defines $\delta=1-M^2 e^{-2\alpha d}$. The factor $1-\delta$ is the failure budget inside the capacity formula, not the success probability.

5. Incomplete line (§3.2): "Under a regular grid (Proposition 3), the count scales as $\Theta\big((M^{1/d}-\lambda^{1/d}+1)^d\big)$."
   Proposition 3 in that same section also sets $\lambda=\Theta\big(M V^{-1}(8/\beta)^{d/2}\big)$ for $\beta$ such that $1<\lambda\le M$. The quoted sentence never defines $\lambda$.

The LSE formula, LSR eq. (3), the $\epsilon\to 0$ gradient, the centroid stationary point, and the exponential count bound (5) in that note match section 3 and appendix E.1. Those lines are not corrected.

**`research/denseam-lsr-physics.md`**

1. Wrong cell (table, Hoover row): "proves exponential capacity and $\Theta(M^{1/d})$ emergent minima."
   Exponential capacity is Theorem 2(a) in section 3. The emergent count is not $\Theta(M^{1/d})$. Theorem 2's display (5) is the exponential in $M(V_d/V)(2/\beta)^{d/2}\log(\cdots)$. Proposition 3 is $\Theta\big((M^{1/d}-\lambda^{1/d}+1)^d\big)$ with $\lambda$ as above. If $\lambda\ll M$, $(M^{1/d})^d=M$, which is a count of order $M$, not of order $M^{1/d}$.

2. Wrong lines (§3.2), attributed to Hoover et al. and Petrova et al. together:
   "$f_{\mathrm{LSR}}(z)=\big[\max(0, z-\theta)\big]^p=\big[\mathrm{ReLU}(z-\theta)\big]^p$" with "$\theta\in(0,1)$" and "$p\ge 2$", and
   "$E_{\mathrm{LSR}}(\mathbf{x})=-\frac{1}{p}\sum_{\mu=1}^{M}\big[\mathrm{ReLU}(\xi^\mu\cdot x-\theta)\big]^p+\frac{1}{2}\|x\|^2$."
   Section **3** eq. (3) is $-\frac{1}{\beta}\log\big(\epsilon+\sum\mathrm{ReLU}(1-\beta/2\|x-\xi^\mu\|^2)\big)$. No power $p$, no dot-product cutoff $\theta$, no $\tfrac{1}{2}\|x\|^2$ term. Whether Petrova uses the power-ReLU form is UNVERIFIED against arXiv:2604.07401; it is not Hoover's LSR.

3. Wrong line (§3.3 item 4): "Hoover et al. prove that between stored memory clusters, the finite overlap of support spheres creates $\Theta(M^{1/d})$ stable emergent local minima."
   Proposition 2 in section 3: a novel minimum is the arithmetic mean of the active set $B(x)$ when $|B(x)|>1$. The count is Theorem 2 display (5) and Proposition 3, not $\Theta(M^{1/d})$.

4. Wrong cell (§5, Energy Kernel row): "Replace cosine merge threshold with Epanechnikov LSR kernel ($p=2, \theta \approx 0.70$)."
   Section 3: the cutoff is the Euclidean ball of radius $\sqrt{2/\beta}$. The extraction states neither $p=2$ nor a cosine threshold $0.70$.

---

## Part 2. Current agent-memory systems

For each system: what it stores, how it forgets or decays, how it resolves a conflict, and numbers only from the primary paper. INDEX.md also lists Memora, STALE, and StateAuditor; those three were read. EnvProbe (arXiv:2606.31422), the embodied just-in-time safety memory (arXiv:2607.16247), and the INDEX row titled "Local arXiv Preprint on Neural Memory Architectures" (arXiv:2605.13438) were not fetched. Any mechanism for those three is UNVERIFIED.

LoCoMo is described differently by papers that all cite Maharana et al. Mem0 §3.1: 10 conversations, about 600 dialogues and 26,000 tokens each, about 200 questions each; adversarial questions dropped. A-MEM §4.1: average 9K tokens, up to 35 sessions, 7,512 question-answer pairs, and it keeps an adversarial category. Those two descriptions are not reconciled here.

### Mem0 (Chhikara et al., arXiv:2504.19413)

**Stores.** Section 2.1: an LLM extracts salient candidate facts from the newest message pair plus a stored conversation summary and the last $m$ messages ($m=10$ in their run). A vector database holds the memories. The graph variant Mem0$^g$ (section 2.2) stores entities and relation triplets in Neo4j.

**Forgets.** Base Mem0 has an explicit DELETE operation (section 2.1). There is no time-decay schedule in that section. Mem0$^g$ does not physically remove a contradicted relationship; it marks the relationship invalid (section 2.2).

**Conflicts.** For each candidate fact, the top $s$ similar memories ($s=10$) go to an LLM tool call that chooses ADD, UPDATE, DELETE, or NOOP (section 2.1). DELETE is "removal of memories contradicted by new information." UPDATE augments an existing memory with complementary information. Mem0$^g$ runs an LLM conflict check and, on a contradiction, marks the old relationship obsolete rather than deleting it, "to enable temporal reasoning."

**Numbers** (LOCOMO, GPT-4o-mini for extraction). Abstract: 26% relative LLM-as-a-Judge gain over OpenAI; graph memory about 2% higher overall than base Mem0; 91% lower p95 latency than full context and more than 90% token cost saved. Table 2: Mem0 overall J $66.88\pm 0.15\%$, 1,764 memory tokens, total p95 latency 1.440 s; Mem0$^g$ overall J $68.44\pm 0.17\%$, 3,616 tokens, total p95 2.590 s. Section 4.3: full context still leads at about 73% J with total p95 about 17.117 s; Mem0's 1.44 s is described there as a 92% reduction and Mem0$^g$'s 2.6 s as an 85% reduction. Table 1 prints per-category F1, BLEU-1, and J, but the HTML column headers did not survive extraction, so those twelve cells are not assigned to categories here.

### Zep / Graphiti (Rasmussen et al., arXiv:2501.13956v1)

**Stores.** Graphiti builds a knowledge graph from conversational episodes and structured business data. Edges carry a bi-temporal pair: $t_{\mathrm{valid}}$ and $t_{\mathrm{invalid}}$ for when the fact held in the world, and $t'_{\mathrm{created}}$ and $t'_{\mathrm{expired}}$ for when the system recorded or invalidated it (§2.2.3).

**Forgets.** Not by deletion. A contradicted edge is invalidated by setting $t_{\mathrm{invalid}}$. The paper calls the update non-lossy: current state and history both remain (§2.2.3 and the introduction).

**Conflicts.** An LLM compares a new edge with semantically related edges between the same entity pair. Temporally overlapping contradictions are invalidated. "Graphiti consistently prioritizes new information when determining edge invalidation" (§2.2.3).

**Numbers.** DMR, §4.2: Zep 94.8% with gpt-4-turbo versus the paper's stated MemGPT figure of 93.4% on the same benchmark; Zep 98.2% with gpt-4o-mini. The 93.4% is Zep's report of MemGPT, not a figure copied from the MemGPT paper below. LongMemEval$_s$ (§4.3, conversations about 115,000 tokens): gpt-4o with Zep 71.2% accuracy, search 0.684 s, total 2.58 s, about 1.6k context tokens (Table 2); accuracy gain 18.5% over that paper's baseline for gpt-4o and 15.2% for gpt-4o-mini; response time reduced by about 90% (§4.3.2). The absolute baseline accuracy was not in the rows captured here.

### MemGPT / Letta (Packer et al., arXiv:2310.08560)

**Stores.** Section 2: main context is system instructions (read-only), a fixed working-context block of unstructured text (key facts and persona, writable only by function calls), and a FIFO queue of recent messages. External context is recall storage (the message database) and archival storage (arbitrary-length text objects, PostgreSQL plus pgvector in the document experiments).

**Forgets.** From the context window, not from the store. The queue manager warns at a token threshold (example: 70% of the window) and, at the flush threshold (example: 100%), evicts a fraction of messages (example: 50%), writes a new recursive summary of what was evicted, and leaves the evicted messages in recall storage indefinitely (§2.2). No time-decay of archival text is stated in §2.1–2.2.

**Conflicts.** No ADD/UPDATE/DELETE/invalidation operator is stated in §2.1–2.2. The agent rewrites working context itself through function calls (Figure 4). Whether two stored facts contradict is left to that rewrite. UNVERIFIED beyond those sections.

**Numbers.** The paper's own multi-session chat comparison is ROUGE-L recall and an LLM judge against a recursive-summary baseline (section 3.1, Table 2). The cell values were not in the lines captured here, so no MemGPT accuracy percentage is stated from this paper. The 93.4% DMR figure above is Zep's citation.

### A-MEM (Xu et al., arXiv:2502.12110v11)

**Stores.** Section 3.1, Zettelkasten-style atomic notes. Each note is $m_i=\{c_i,t_i,K_i,G_i,X_i,e_i,L_i\}$: original interaction content, timestamp, LLM keywords, LLM tags, an LLM contextual description, a dense embedding of the concatenated text fields, and a set of links.

**Forgets.** Sections 3.1–3.3 do not define a decay schedule or a delete operation. Memory evolution replaces a neighbor note in the set with an updated note (§3.3).

**Conflicts.** No contradiction operator is stated in §3.1–3.3. Link generation (§3.2) asks an LLM which of the nearest notes to connect. Evolution (§3.3) may rewrite a neighbor's context, keywords, and tags, and the new note replaces the old one in $\mathcal{M}$. That is an overwrite of those attributes, not an invalidation timestamp.

**Numbers.** LoCoMo, their Table, GPT-4o-mini, F1 / BLEU-1: A-MEM multi-hop 27.02 / 20.09, temporal 45.85 / 36.67, open-domain 12.14 / 12.00, single-hop 44.65 / 37.06, adversarial 50.03 / 49.47; average answer length 2,520 tokens. Same table, MemGPT on their run: multi-hop F1 26.65, temporal 25.52, single-hop 41.04, adversarial 43.29, length 16,977 tokens. GPT-4o A-MEM temporal F1 is 39.41 and single-hop F1 is 48.43 (below that table's MemGPT single-hop F1 of 60.16). They also report DialSim; those cells were not captured.

### LiCoMemory (Huang et al., arXiv:2511.01448v2)

**Stores.** CogniGraph, three layers (§3.1): a session summary plus keyword keys; an entity–relation layer that keeps identifiers rather than long descriptions; the original dialogue chunks. Cross-layer hyperlinks, timestamps, and session ids tie them together. The graph is an index onto the text, not a replacement of the text.

**Forgets.** Section 3 describes incremental update: refresh or create a session summary, extract triples, and on a duplicate triple attach another source hyperlink instead of adding a node. No time-decay and no edge-invalidation schedule appear in §3.1–3.2. Anything beyond those sections is UNVERIFIED.

**Conflicts.** The stated mechanism in the update section is type-aware and semantic duplicate detection, plus summary update. A contradiction policy (delete, invalidate, or keep both) is not stated there.

**Numbers.** Introduction: up to 23% accuracy over the second-best baseline on LoCoMo and LongMemEval, with lower input tokens and latency. The absolute accuracy cells for the LiCoMemory row were not in the HTML lines captured, so they are not repeated here.

### JitMem (Zhou, Li, Liu, Yavuz, Joty, arXiv:2609.27334v1)

**Stores.** A memory bank of raw trajectories: task text plus the full observation–action sequence. Nothing is summarized at write time (Method, Memory Bank). At deployment the bank starts empty and grows online.

**Forgets.** No decay is stated. The write policy appends a trajectory only when an executor-as-judge calls the task successful, so failures stay out of the bank by default. Section 4.2 is said to ablate storing every trajectory labeled success or failure; those ablation cells were not captured. The read-time payload is ephemeral context, not a stored replacement of the trace.

**Conflicts.** Not a fact-store. Two traces are not merged or invalidated. The curator writes a new task-conditioned payload from the retrieved raw traces; the same trace can yield different payloads for different tasks (Introduction).

**Numbers.** Abstract: absolute success-rate gains over the strongest baseline of 16.2 (ALFWorld), 16.3 (WebShop), and 3.9 ($\tau^2$-bench). Untrained JitMem with Gemini-2.5-Pro as curator and executor: WebShop success rate 61.0 versus 41.0 for SkillOS. Compact payloads cut input tokens by 50.3%–56.3% and executor steps by 28.4%–31.4% relative to write-time methods (Introduction). INDEX.md's "+1.7–8.2 points" wording was not the sentence in this abstract; it is not used as a JitMem result.

### MemAct (Zhang et al., arXiv:2510.12635v3)

**Stores.** Working memory is the agent context: a sequence of addressable interaction records $z_i=(a_i,o_i,\mathrm{id}_i)$ (§3.2). This is context curation, not a separate long-term fact store. The paper discusses external memory controllers as the alternative it does not take.

**Forgets.** By deletion. The Prune&Write action deletes records by id and inserts a new memory-action record in place (§3.1). There is no time-decay.

**Conflicts.** No user-fact invalidation operator is stated. The learned policy decides which context records to prune. A deleted span is physically removed and the trajectory is segmented (DCPO, §3.3) because a causal KV cache would otherwise keep the deleted tokens' influence.

**Numbers.** Abstract: MemAct-RL-14B matches models $16\times$ larger and reduces average context length by 51%. Section 4: multi-objective accuracy 59.1% versus Qwen3-235B at 53.1% and Tongyi-DeepResearch at 56.0%; average input context about 3,500 tokens per step; total tokens $8.2\times 10^4$ versus $16.7\times 10^4$ for Qwen3-235B (about 51% lower) and $19.3\times 10^4$ for Search-R1-14B. A-MEM as a fixed-rule baseline in that table uses $3.9\times 10^4$ tokens at 39.9% accuracy. Those A-MEM figures are MemAct's baseline, not the A-MEM paper's LoCoMo table.

### Others named in `research/INDEX.md`

**Memora and FAMA** (arXiv:2604.20006) is a benchmark and a metric, not a memory store. Memora spans weekly, monthly, and quarterly conversations and scores remembering, reasoning, and recommending. Table 1, average / maximum prior sessions to consolidate, then average / maximum mutations: weekly 5.3 / 26.0 and 2.7 / 11.0; monthly 17.3 / 99.0 and 8.8 / 43.0; quarterly 28.4 / 309.0 and 14.8 / 94.0. FAMA rewards use of still-valid memory and penalizes use of obsolete or deleted memory. It does not itself decay a store.

**STALE** (arXiv:2605.06527) is a benchmark of implicit memory conflict, not a store. Three probes: state resolution, premise resistance, implicit policy adaptation (IPA). Table 2: best overall accuracy in their evaluation is Gemini-3.1-pro at 55.2%. The same table's discussion: Qwen3.5-27B Type I state resolution 76.0% versus Type I IPA 39.0%; Gemini-3.1-pro state resolution falls from 92.0% (Type I) to 69.0% (Type II), and IPA from 71.0% to 55.0%. Most memory frameworks they plugged in fall below 10% overall.

**StateAuditor** (arXiv:2608.01619) is a repair pass over stale implicit dependencies. It audits backward from a draft response along a dependency graph and lets only verified transitions trigger repair. The paper says what is verified is provenance and chronology, not semantic supersession. On STALE's full protocol it reports a matched control at 0.692, which is +0.6 over the predecessor and not significant, and it attributes the STALE gain to the transition machinery. A separate draft-audit-repair pipeline raises current-preference accuracy (user-clustered $p<.01$); the paper says a matched control shows most of that external gain is the draft-side audit. It cites STALE's write-side prototype at 91% state resolution and 32% IPA; that pair is StateAuditor's citation, not a sentence re-read in the STALE HTML.

---

## Part 3. Abhidhamma fidelity

Judgments use only the project's research notes on `origin/v3/r0`. They do not add a reading of the Pāli. "Faithful" means the mechanism matches the doctrine as that note states it. "Loose" means the note's own definition and the mechanism share a name and part of the structure. "Wrong" means the mechanism contradicts the doctrine as the note states it.

| Mechanism | Term in the notes | Judgment | Why |
|---|---|---|---|
| Decay | *anicca*; *jarā* / *ojā*; text loss only by *upekkhā* or *nirodha* | Loose | `02_ABHIDHARMA_THERMODYNAMIC_MEMORY.md` §1 says decay is the default of conditioned things and retention takes work. Addendum A §A.1.1 then amends *ojā* decay to retrieval priority only, and says text is removed only by deliberate *upekkhā* or *nirodha*, not by time passing. Rank decay without text loss follows that amendment and does not match the un-amended "decay is the default state" sentence. `10_PATTHANA_24_CAUSAL_CONDITIONS.md` §3.3 still maps *vigata* to eviction past an energy cutoff, which contradicts §A.1.1. |
| Recall strengthening | *āsevana-paccaya* | Loose | `10_PATTHANA_24_CAUSAL_CONDITIONS.md` §3.2 defines *āsevana* as the seven *javana* moments inside one *citta-vīthi* conditioning each other. The same paragraph then treats frequent later recall as a lower decay rate. A cross-session bump of `recall_count` / fidelity (overnight L2) is the second sentence, not the intra-process definition. |
| Vedanā tagging | *vedanā* (pleasant, unpleasant, neutral) | Loose | `citta-vithi.md` §4 and `03_ABHIDHARMA_TECHNICAL_PAPER.md` give three tones, scored $+1/-1/0$. `2026-09-26-gates.md` §1 stores `valence` plus `intensity` in $0..4$. The three-way tone matches. Intensity, and the "exponential multiplier on consolidation" in `02` §1, are not part of the three-tone definition those notes give. |
| Paṭṭhāna links | the 24 *paccaya*; overnight L3 names *ārammaṇa*, *adhipati*, *upanissaya* | Loose | `10_PATTHANA_24_CAUSAL_CONDITIONS.md` §2 lists all 24, then §4's enum implements a subset, and §3 glosses *anantara* as token prediction, *upanissaya* as a DenseAM attractor, and *vigata* as garbage collection. Those are the note's analogies. Three link labels are not the 24 conditions. |
| Sleep and consolidation | *saṅkhāra* consolidation; *bhavaṅga*; *bhāvanā* | Loose | Addendum A §A.1.1 and `PAPER_DRAFT.md` §3.3 say consolidation adds a cluster index and keeps member text. That part matches the amended note. `citta-vithi.md` maps *bhavaṅga* to an idle context window and *tadālambana* to a two-stage write, not to a sleep rewrite. The INDEX plan row calls nightly calibration "inside *bhāvanā*". The notes never state a sleep-consolidation doctrine to map onto. |
| The gates | *vedanā*, *saññā*, *sati*; *santīraṇa* as the quality gate | Loose | `2026-09-26-gates.md` §1 defines five typed gates returning answer or abstain: *vedanā*, *saññā* (no trait yet), *saṅkhāra*-merge, *sati*-recall, `task_succeeded`. *Vedanā* and *sati* use terms the notes define. `citta-vithi.md` §2 puts a redundancy check at *santīraṇa*, which is one moment, not this gate set. `task_succeeded` has no Abhidhamma term in these notes. The same gates note says Ollaya is absent and measured chat latency is 1.4–3.3 s, against the plan row's sub-millisecond claim. |

No row is marked faithful. The closest piece is the three-way *vedanā* label inside a verdict that also carries an intensity the notes' three-tone definition does not have.
