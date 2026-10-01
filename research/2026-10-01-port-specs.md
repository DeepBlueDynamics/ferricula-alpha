# Port specs: goal utility, Bayesian BM25, geometric access

Research only. Nothing here is a port. Kord decides whether any of these three are built. Checked against `origin/v3/r0` at `cbc5ff77094925aa527e4f80f3afd9ea124e1598` (L5 inventory merged; the recall overlay is still Steve's `853d199`, "nudge, not dominance"). Quotes are from `research/legacy-src` on the main checkout (that tree is gitignored and is not part of this commit) and from the crates at that hash.

These are items 3, 7, and 1 of the inventory in `research/legacy-inventory.md`. The inventory marks each worth having. This note says each one waits.

## Recommendation

| # | Capability | Recommendation |
|---|---|---|
| 3 | Goal utility | **later** |
| 7 | Bayesian BM25 | **later** |
| 1 | Per-agent geometric access | **later** |

- Goal utility: **later**. The memex text is a spec, and that tree has no implementation. The live chat loop has no goal object. The spec weight `0.7 * gu + 0.3 * lr` would dominate reciprocal-rank fusion the way the first overlay defaults did. A port is a sidecar log and a fusion-scale nudge. It does not write a memory row.
- Bayesian BM25: **later**. The pure math in `ferricula-original/src/bb25.rs` is real. `score_to_probability` does not steer rank or the lifecycle until judgments have held-out ECE < 0.05. `log_odds_fusion` does not replace reciprocal-rank fusion.
- Geometric access: **later**. It does not matter for a single-agent local deployment today. The same key on both sides is a no-op for ranking. `crates/ferricula-semantic/src/crypto.rs` keeps keys in memory (lost on restart) and returns the input unchanged when the dimension does not match. Do not wire it now.

## Invariants a port keeps

These already hold for the merged overlay, and each capability below has to keep them.

- A memory row is never deleted. Fading, a low utility, or a low probability changes rank. The text stays.
- Stored text is never rewritten. Reconstruction and compression in the memex spec drop or invent content. They are out of scope here.
- The recovered store is read-only. New state lives under `state_dir/overlay/`, the same idea as `recall-stats.json`, or in a new experience vector written at ingest. Recovered vectors and recovered text stay as they are.
- Document sections are not memories. The overlay already skips them (`overlay_key` in `crates/ferricula-server/src/recall.rs` returns `None` for `CandidateKind::DocumentSection`). A utility or a probability does not pull a section into the memory lifecycle.

## Shared rank scale

`fuse_arms_all` (`crates/ferricula-server/src/recall.rs`) sums `weight / (RRF_K + rank)` with `RRF_K = 60.0`, then calls `apply_ranking`. A lone rank-1 hit scores `1/61`. Rank 1 and rank 10 differ by `1/61 - 1/70`.

`apply_ranking` then adds `s * ln(1 + recalls) - f * age_days / (age_days + h)` for memory candidates only, and re-sorts. Equal scores keep reciprocal-rank order. The installed defaults are in `crates/ferricula-server/src/recall_overlay.rs`:

```rust
pub const DEFAULT_S: f64 = 0.003;
pub const DEFAULT_F: f64 = 0.002;
pub const DEFAULT_H: f64 = 30.0;
```

The file comment records why: fused scores are about `1/61`, and `s = 0.15` made the overlay the dominant term. One citation (`s * ln 2`) moves a memory a few places. Displayed `effective_fidelity` uses fixed weights `0.15` and `0.5`, independent of the ranking weights. A port does not reuse those display weights as ranking weights.

`chat.rs` records a recall only for an id named `[memory N]` in the completed reply that this turn actually showed (or a duplicate of a shown row). Showing a candidate is not a recall. The overlay file is the only write (`record_cited_recalls`).

## 3. Goal utility

**later.**

### (a) What the legacy text actually does

`research/legacy-src/memex/SPEC.md` is a specification. There is no `compute_goal_utility` implementation in that tree. Design principle 6: "Global utility over local relevance. What helps achieve goals beats what matches queries. Track injection outcomes to learn what actually helps."

`recall` says layer 2 reranks by goal utility, similarity, and recency:

```python
def recall(
    query: str,
    agent_id: str,
    k: int = 10,
    time_range: tuple[int, int] | None = None,
    context_id: str | None = None,  # If None, uses agent's current context
    include_archived: bool = False
) -> list[Observation]:
```

The same docstring also warps the query embedding by a context transform before the search. That warp is geometric access (section 1 below), not utility. A goal-utility port does not add it.

The injection API is specified twice, and the two copies disagree on types. The first:

```python
def record_injection(
    observation_id: str,
    goal_id: str,
    agent_id: str,
    timestamp: int | None = None
) -> str:

def update_injection_outcomes(
    goal_id: str,
    outcome: str,  # succeeded | failed | abandoned
    timestamp: int | None = None
) -> int:

def get_injection_history(
    observation_id: str,
    agent_id: str
) -> list[Injection]:

def compute_goal_utility(
    observation_id: str,
    goal_id: str | None,  # None = general utility
    agent_id: str
) -> float:
```

The first `compute_goal_utility` returns a 0–1 score, "0.5 = no data, neutral prior." The second copy requires `goal_id: str` and returns `alpha / (alpha + beta)`:

```python
def compute_goal_utility(
    observation_id: str,
    goal_id: str,
    agent_id: str
) -> float:
    injections = get_injection_history(observation_id, agent_id)
    if not injections:
        return 0.5  # No history, neutral prior
    successes = sum(1 for inj in injections if inj.goal_outcome == "succeeded")
    failures = sum(1 for inj in injections if inj.goal_outcome == "failed")
    alpha = 1 + successes  # Prior: 1 success
    beta = 1 + failures    # Prior: 1 failure
    return alpha / (alpha + beta)
```

Abandoned is a stored outcome. The estimator does not count it. An observation whose injections are all abandoned returns `1/2`, the same as no history. The sketch also does not filter history by `goal_id`, even though the docstring says "for this type of goal." The count is observation-wide.

The rank formula in the same file:

```python
def rank_candidates(
    candidates: list[Observation],
    query: str,
    current_goal: Goal,
    agent_id: str
) -> list[tuple[Observation, float]]:
    # ...
        lr = compute_similarity(embed(query), obs.vec_large)
        gu = compute_goal_utility(obs.id, current_goal.id, agent_id)
        score = 0.7 * gu + 0.3 * lr
```

The comment says global utility dominates. Later the same file gives a second set of weights, `RERANK_WEIGHTS`, with `goal_utility` at `0.35`, `vec_large_similarity` `0.25`, `temporal_proximity` `0.20`, `exact_match` `0.10`, `trust` `0.10`. Neither formula is implemented.

The SQL shape is a table `injections` (`id`, `observation_id`, `goal_id`, `agent_id`, `timestamp`, nullable `goal_outcome` of `succeeded | failed | abandoned`). The learning-loop comment says the embedding model stays fixed and only the rerank learns.

Decay, compression, and reconstruction sit in the same spec. `compress_observation` drops `content` and `vec_large`. `reconstruct_range` writes inferred content at confidence `0.7`. Those change text. They are not part of this port.

### (b) Where it would live, and what would call it

A new module `crates/ferricula-server/src/goal_utility.rs`, beside `recall_overlay.rs`. Durable state is a sidecar under `state_dir/overlay/` (a sibling of `recall-stats.json`), not a column on a memory row and not the recovered store.

The write hook sits next to `record_cited_recalls` in `crates/ferricula-server/src/chat.rs`. It records the same event that function already records: a cited id that was shown. It records that event only when the turn carries an explicit goal id. The live chat path has no goal today (`chat.rs` does not mention a goal). `GoalReport` in `crates/ferricula-episode/src/model.rs` (`goal_id`, `created_at`, `target_entity`, `description`, `tags`) is the episode projection. The episode adapter rejects an empty or duplicate `goal_id`. That is a different subsystem. A port takes a goal id on the turn. It does not read episode reports and pretend they were this chat.

The outcome is an explicit `succeeded | failed | abandoned` for that goal id, matching `update_injection_outcomes`. `TaskSucceededGate::judge` (`crates/ferricula-cognition/src/gates.rs`) returns a `Judged<TaskSucceededVerdict>`. `Judged::answer` still returns the uncalibrated verdict. `Judged::calibrated_answer` returns `None` unless `provenance.calibrated` and the state is not truncated. A gate may supply the outcome only when `calibrated_answer()` is `Some`. Until then the outcome is the explicit label, or it is absent.

The rank hook is after `apply_ranking`, still inside the memory-candidate pass: add `g * (utility - 0.5)` with `g` on the order of `DEFAULT_S` (`0.003`). Utility `0.5` adds zero. Use the optional `goal_id` form so a citation with no active goal stays at the prior and adds nothing. Document sections are unchanged.

`0.7 * gu` on a fused score of about `1/61` is the hazard. A neutral prior is `0.5`, and `0.7 * 0.5 = 0.35`, about twenty-one times a lone rank-1 term (`0.35 * 61`). The alternate weight `0.35 * 0.5 = 0.175` is still about ten times that term. Five successes and no failures give `alpha = 6`, `beta = 1`, utility `6/7`. At `g = 0.003` the nudge is `0.003 * (6/7 - 0.5) = 0.003 * 5/14`, which is smaller than the gap between reciprocal-rank 1 and reciprocal-rank 10. As successes grow, utility approaches 1 and the nudge approaches `g/2 = 0.0015`, still under one citation (`0.003 * ln 2`). That is the scale that matches `853d199`.

### (c) Interaction with the overlay and the calibration rule

The overlay and the utility log count the same citation. They answer different questions: the overlay counts how often a memory was cited and how old it is; utility counts whether the goal of that citation later succeeded or failed. Both are rank terms after fusion. Neither writes a row.

Stacking them is safe only while each term stays on the `0.003` scale. A port that adds the spec's `0.7` weight on top of `apply_ranking` makes utility the ranking, and the overlay's nudge disappears under it. The tests in `recall.rs` that expect a memory score near `1/61 - fade` (`fuse_interleaves_sources_and_keeps_section_text`) would fail in spirit even if the numbers were updated: the fused order would no longer be a reciprocal-rank order.

The calibration rule is in `paper/WHITEPAPER_V2.md`: a gate's probabilities may change the lifecycle (merge, release, pool membership) only after held-out ECE for that calibration file is below 0.05. Until then gates are advisory. `Judged::calibrated_answer` is the code check. `Calibration.lifecycle_authorized` is that flag (`crates/ferricula-gates/src/lib.rs`). Run `69e4b846`: vedanā ECE 0.283, saññā 0.147, yes/no 0.195. No gate has passed. Goal utility is not itself a gate. It becomes a lifecycle input if a port treats `TaskSucceededGate::answer()` as a success label and then writes that into rank, merge, forgive, release, or pool membership. The allowed use before ECE < 0.05 is an explicit outcome, recorded in the sidecar, applied as the small nudge above.

### (d) Invariants

The sidecar is the only new write. `record_injection` does not update a memory. `update_injection_outcomes` does not update a memory. Utility is not stored on the row, and it is not the displayed fidelity (that stays the overlay's `0.15` / `0.5` formula). Abandoned does not move the prior. No history does not move the prior. Recovered text and recovered vectors are untouched. Nothing is deleted.

### (e) Tests that would prove it

- No injections returns `0.5`, and the rank delta is `0`.
- Injections that are only `abandoned` return `0.5`.
- Successes `s` and failures `f` return `(1 + s) / (2 + s + f)`.
- A citation writes a sidecar row only when the id was shown (including a duplicate of a shown row) and the turn has a goal id. Showing without `[memory N]` writes nothing. A cited id that was not shown writes nothing. A document cite writes nothing.
- The outcome update changes the sidecar and leaves every memory row's text and bytes alone.
- Five successes at `g = 0.003` move the candidate by less than `1/61 - 1/70`. A candidate with utility `1` still does not pass a memory whose reciprocal-rank lead is larger than `g/2`.
- `0.7 * gu + 0.3 * lr` is not the term added to the fused score.
- `calibrated_answer() == None` does not count as `succeeded` or `failed`.
- Recovered-store bytes are unchanged. Document-section order is unchanged apart from the memory re-sort.

### (f) Risks, and what it would break

The `0.7` weight, or the `0.35` weight used as a fraction of a unit-scale score, replaces reciprocal-rank order. That repeats the bug Steve fixed when `s` was `0.15`. Writing utility onto the memory row couples a learning signal to the recovered store, which this tree treats as read-only. A silent default goal, or a hook that treats an episode `GoalReport` as the goal of the current chat, trains the log on turns the episode system never saw. Using `answer()` instead of `calibrated_answer()` lets an uncalibrated judge (ECE 0.195 on the yes/no gate in run `69e4b846`) label memories as helpful. The overlay tests that pin scores to `1/61 - fade` encode the current contract; a dominant utility term breaks that contract even after the assertions are edited.

## 7. Bayesian BM25

**later.**

### (a) What the legacy code actually does

`research/legacy-src/ferricula-original/src/bb25.rs` is implemented. The module line: "Bayesian BM25 (BB25): calibrated probability from BM25 relevance scores."

```rust
pub struct Bb25Config {
    pub k1: f64,    // BM25 saturation (default 1.2)
    pub b: f64,     // length normalization (default 0.75)
    pub alpha: f64, // sigmoid steepness (default 1.0)
    pub beta: f64,  // sigmoid midpoint (default 0.0)
    pub base_rate: Option<f64>,
}

pub struct Bb25State {
    pub config: Bb25Config,
    pub n_updates: u64,
    pub grad_alpha_ema: f64,
    pub grad_beta_ema: f64,
    pub alpha_avg: f64, // Polyak average
    pub beta_avg: f64,
}
```

`CorpusStats::add_document`, `remove_document`, `avgdl`, and `idf` maintain corpus counts. `idf` is `ln((n - df + 0.5) / (df + 0.5) + 1)` and can be negative. The term scorer clamps it:

```rust
pub fn bm25_term_score(
    tf: f64, df: f64, n_docs: f64, doc_len: f64, avgdl: f64, cfg: &Bb25Config,
) -> f64
```

Zero `tf` returns `0`. Otherwise `idf.max(0)` times the usual `k1` saturation and `b` length norm. `avgdl` is floored at `1.0` inside the scorer. `CorpusStats::avgdl` returns `1.0` when `n_docs` is `0`.

The probability path:

```rust
pub fn likelihood(score: f64, alpha: f64, beta: f64) -> f64  // sigmoid(alpha * (score - beta))
pub fn tf_prior(tf: f64) -> f64                              // 0.2 + 0.7 * min(tf/10, 1)
pub fn norm_prior(doc_len_ratio: f64) -> f64                 // peaks at doc_len_ratio 0.5; about 0.3 to 0.9
pub fn composite_prior(tf: f64, doc_len_ratio: f64) -> f64   // clamp(0.7 * tf_prior + 0.3 * norm_prior, 0.1, 0.9)
pub fn posterior(lik: f64, prior: f64, base_rate: Option<f64>) -> f64
pub fn score_to_probability(score: f64, tf: f64, doc_len_ratio: f64, state: &Bb25State) -> f64
```

`posterior` is one Bayes update of likelihood with prior, then a second update when `base_rate` is `Some`. `clamp_prob` clamps to `[1e-10, 1 - 1e-10]`. `score_to_probability` is `posterior(likelihood(...), composite_prior(...), state.config.base_rate)`.

```rust
pub fn update(state: &mut Bb25State, score: f64, label: f64, lr: f64)
```

Online sigmoid SGD. Momentum `0.9`, bias correction, learning rate `lr / (1 + 0.01 * t)`, `alpha` clipped to at least `0.01`, Polyak averages `alpha_avg` and `beta_avg`. `label` is `1.0` or `0.0`. This is not a held-out ECE measurement.

```rust
pub fn auto_estimate(corpus: &CorpusStats) -> (f64, f64)
```

Returns `(1.0, 0.0)` when `n_docs < 2` or the sample is empty. Otherwise `beta` is the median of BM25 term scores for the top 50 document-frequency terms, and `alpha` is `clamp(1 / std, 0.1, 10)`. That is a corpus heuristic. It is not calibration, and it does not set `lifecycle_authorized`.

```rust
pub fn log_odds_fusion(p1: f64, p2: f64, weight: f64) -> f64
```

`sigmoid(weight * logit(p1) + (1 - weight) * logit(p2))`. The comment says `weight = 0.5` is equal and `weight > 0.5` favors `p1`.

`remove_document` exists so corpus counts can drop a document. A port does not call it to delete a memory. Corpus statistics over recovered text are derived. Dropping a row to refresh them violates the invariant.

### (b) Where it would live, and what would call it

The pure functions move to a new `crates/ferricula-search/src/bb25.rs`, next to `bm25.rs`. `Index::search` stays the classic ranker:

```rust
pub fn search(
    &self,
    query: &str,
    variant: SearchVariant,
    params: &Bm25Params,
    tagger: Option<&Tagger>,
) -> Vec<SearchHit>
```

Today the server builds `ArmList { arm: "bm25", weight: 1.0, ... }` in `lexical_lists` (`recall.rs`) from section rank, and `fuse_arms_all` uses that rank. The BM25 score itself is discarded once the list is ordered.

A port reports `score_to_probability` on the bm25 arm's candidate, as a field beside the section hit. The caller is the place that already builds that arm (`lexical_lists`, or the section search that feeds it). `fuse_arms_all` does not read the field. `update` is not called from a gate unless `calibrated_answer()` is `Some` and that gate's calibration file has `lifecycle_authorized` (held-out ECE < 0.05). `auto_estimate` may fill an initial `alpha` and `beta` for display. It is labeled a heuristic in the provenance of the number, the way `Calibration.sha256` labels a gate.

### (c) Interaction with the overlay and the calibration rule

The overlay runs after fusion, on memory candidates, and does not look at section BM25. A probability sitting on a section hit does not change `apply_ranking`. Replacing the arm's contribution `weight / (60 + rank)` with `log_odds_fusion` would. Two probabilities near `0.5` fuse near `0.5` (`logit(0.5) = 0`), and a probability near `1` saturates near `1`. That range is unit scale. Reciprocal-rank terms are about `0.016`. Mixing them, or substituting one for the other, undoes the scale Steve set.

The whitepaper's rule is the same one as in section 3. BB25's `update` learns from a relevance label. A label that comes from a gate is a lifecycle use of that gate's probability as soon as the learned `alpha` and `beta` change what merge, forgive, release, or pool membership does. Reporting the probability next to a rank is advisory, which is what every current gate is (`paper/WHITEPAPER_V2.md`, run `69e4b846`). Feeding it into those decisions waits for ECE < 0.05 on the judgments that trained it. `auto_estimate` does not satisfy that wait: it never sees a label.

`Judged::answer` remaining available is why the call has to be `calibrated_answer`. The uncalibrated path still returns a verdict.

### (d) Invariants

The probability is a number beside a rank. It does not delete a section, a memory, or a posting. It does not rewrite section text. `remove_document` is not how recovered text leaves the index. Fading stays the overlay's rank term. A low probability does not archive, forgive, or compress anything. The recovered store is not opened to store `Bb25State`.

### (e) Tests that would prove it

The legacy file already checks `sigmoid(0) = 0.5` and that `bm25_term_score` is `0` at `tf = 0` and positive otherwise. A port keeps those, and adds:

- `score_to_probability` stays inside `(0, 1)` for a finite score.
- `posterior` with `base_rate: None` is the single update. With `Some`, it is the second update.
- `auto_estimate` on `n_docs < 2` returns `(1.0, 0.0)`, and a comment or a type distinguishes that pair from a calibration file.
- `update` leaves `alpha >= 0.01`.
- `log_odds_fusion(0.5, 0.5, w) = 0.5` for a finite `w` in `(0, 1)`.
- Attaching a probability to a bm25 candidate does not change the order `fuse_arms_all` produces from the same ranks.
- `update` is not called when `calibrated_answer()` is `None`.
- No test fixture deletes a memory row in order to refresh `CorpusStats`.

### (f) Risks, and what it would break

`log_odds_fusion` as the hybrid ranker replaces reciprocal rank with a unit-scale number and makes the overlay's `0.003` term invisible. Calling `auto_estimate` a calibration would set a probability that looks measured and is only the median and spread of the corpus. Training `update` on `answer()` labels from run `69e4b846` (ECE 0.147 to 0.283) writes that error into `alpha` and `beta`. Using the probability to merge, forgive, release, or pool breaks the whitepaper rule directly. `CorpusStats::idf` without `.max(0.0)` and `bm25_term_score` with it will disagree if a port displays one and ranks with the other. `remove_document` used as a memory operation deletes a row the invariant forbids, and it would break the "never delete" tests the dream path already pins (forgiven, archived, and pruned stay at zero in the thirty-nights test, per the inventory).

## 1. Per-agent geometric access

**later.** It does not matter for a single-agent local deployment today.

### What it protects against

It protects a searcher who has the vectors and does not have the seed. The same orthogonal transform on the stored vector and the query preserves cosine. A different transform does not, so the neighbor the owner would find is noise for anyone else.

It does not hide the text stored beside the vectors. Alpha memories carry `tags.text`. A leaked store is still readable. The operator of one local agent holds that text and would hold the seed, so the trust boundary of today's deployment does not move. It would matter for a second agent, or for a copied ANN index that leaves without the seed.

`research/legacy-src/memex/geometric-trust/README.md` prints a sample as the output of `geometric_access.py`: true similarity `0.9578`, correct key `0.9578`, wrong key `0.0014`. This note did not re-run that script. The demo itself (`research/legacy-src/memex/geometric_access.py`, and the copy under `geometric-trust/`) defines one function, `cos_sim`, and otherwise draws a random orthogonal matrix with `scipy.stats.ortho_group` at dimension 128. Alpha's own test is weaker than that sample on purpose: `different_key_destroys_cosine` asserts an absolute cosine gap greater than `0.1`, not a value near zero.

### (a) What the legacy code actually does

The orthogonal key the spec describes is `research/legacy-src/myoo/mingwang/memex/keygen.py`. The module doc: same key preserves cosine, different key destroys it, `Q @ Q.T = I`. The seed is a hexagram cast truncated to 32 bits. This note does not record a seed value.

```python
def generate_key(seed: int, dim: int) -> np.ndarray:
    # QR of a seeded Gaussian, columns signed by the diagonal of R

def generate_keys_for_agent(seed: int, roles: Optional[list[str]] = None) -> dict[str, np.ndarray]:

def encrypt(embedding: np.ndarray, key: np.ndarray) -> np.ndarray:
    return (embedding.astype(np.float32) @ key).astype(np.float32)

def decrypt(encrypted: np.ndarray, key: np.ndarray) -> np.ndarray:
    return (encrypted.astype(np.float32) @ key.T).astype(np.float32)

def verify_orthogonality(key: np.ndarray, tol: float = 1e-5) -> bool:
```

`generate_key` builds `Q` by QR. Roles get one matrix each, at that role's dimension (the module doc names 384 for organize and 1536 for retrieve). Same seed at two dimensions is two matrices.

Alpha already has two implementations. Neither is called from `ferricula-server` (no `VectorTransform`, `CryptoManager`, or `transform` use under `crates/ferricula-server`).

`crates/ferricula-core/src/transform.rs` is a sparse stand-in, permutation plus sign flips, O(n):

```rust
impl VectorTransform {
    pub fn from_seed(seed: &[u8; 32], dim: usize) -> Result<Self>
    pub fn encrypt(&self, vec: &[f32]) -> Vec<f32>  // permute, then flip signs
    pub fn decrypt(&self, vec: &[f32]) -> Vec<f32>  // unflip, then unpermute
}
```

`orthogonal_from_seed` and `warp` are the dense-matrix leftovers; `warp` is `crates/ferricula-core`, not the context warp in the memex `recall` docstring. Tests at dimension 768: round trip, `same_key_preserves_cosine` within `1e-5`, `different_key_destroys_cosine` with gap `> 0.1`, and the permutation is a bijection. `encrypt` writes `dim.min(vec.len())` components and leaves the rest `0`. A short or long vector is silently truncated, not rejected.

`crates/ferricula-semantic/src/crypto.rs` is the dense matrix the Python key matches in shape:

```rust
impl AgentKeys {
    pub fn new(agent_id: &str, organize_data: &[f32], organize_dim: usize,
               retrieve_data: Option<&[f32]>, retrieve_dim: Option<usize>) -> Result<Self>
    pub fn encrypt(&self, embedding: &[f32], role: &str) -> Vec<f32>  // v @ Q
    pub fn decrypt(&self, embedding: &[f32], role: &str) -> Vec<f32>  // v @ Q^T
}
```

The comment on the fields says organize is 768×768 and retrieve is 1536×1536. When `embedding.len() != dim`, both `encrypt` and `decrypt` return `embedding.to_vec()`. A dimension mismatch fails open to plaintext. `CryptoManager` documents itself on the struct: "Keys are lost on restart." `register_keys` stores `AgentKeys` in a process-local map. `get_keys` returns `None` after a restart.

`crates/ferricula-cognition/src/identity.rs` builds an X25519 static secret and public key. That is a different primitive. It does not transform an embedding.

### (b) Where it would live, and what would call it

The transform to use is `ferricula_core::transform::VectorTransform::from_seed`, with the 32-byte seed persisted outside the vector file (server config, not inside the ANN). `CryptoManager` is the wrong store: the key disappears on restart, and a later process that fails open will search plaintext while believing the vectors are sealed.

Call sites, both with the same seed:

- Write path: `AgentRuntime::meaning_sync_writes` and `embed_text` / `embed_texts` in `crates/ferricula-server/src/meaning_plane.rs`. Those embed new experience rows and sections through `TextEmbedder::embed`. A port transforms the new experience vector after the embed, before it is stored.
- Read path: `AgentRuntime::dense_arm` in the same file. It embeds each query segment, then `index.search(v, cfg.dense_k, ...)`, and builds `ArmList { arm: "dense", weight: 1.0, ... }` plus the graph arm. The graph hop dots the query against `meaning_vector`. Those stored vectors have to be in the same space as the query they are dotted with.

Recovered vectors stay plaintext. The startup probe in `crates/ferricula-server/src/embeddings.rs` re-embeds recovered samples and requires cosine at least `PROBE_MIN_COSINE` (`0.999`) against the stored vector. Transforming recovered vectors fails that probe and rewrites a read-only store.

One index searched with one query vector cannot serve plaintext recovered rows and transformed experience rows. `dense_arm` would search the recovered partition with the plaintext query and the experience partition with the transformed query, then merge. Sections stay in the space they were embedded in. Mixing a transformed experience vector into the recovered partition breaks both the probe and the graph hop, which reads recovered vectors as plaintext.

### (c) Interaction with the overlay and the calibration rule

The overlay adjusts fused scores after both the lexical arms and the dense arm exist. It keys `m:<id>` and `x:<id>` and never reads the vector. Transforming a vector does not change citation counts, age, or fidelity. A bug that stores a transformed vector under a recovered id would change who the dense arm returns, and the overlay would then nudge those wrong neighbors. The recovered partition staying plaintext is what keeps today's dense order for recovered memories.

Geometric access is not a probability and is not a gate. The calibration rule still binds the temptation to treat "wrong key, low cosine" as a reason to merge, forgive, release, or drop a row. Low cosine here means the searcher lacks the seed. It is not a judgment about the memory. No ECE measurement makes that cosine into a lifecycle input. The whitepaper bar stays where it is: gates, and only gates, and only after ECE < 0.05.

Same key on the query and the stored vector preserves cosine, so for one agent the dense ranking of the transformed partition matches the plaintext ranking. That is a no-op. The cost (two partitions, a seed that must outlive the process, a refuse-on-mismatch rule `crypto.rs` does not have) buys nothing until a second searcher exists.

### (d) Invariants

Recovered vectors and recovered text are byte-identical after the port. New experience text is unchanged; only the stored vector of a new experience row may differ, and it differs by `VectorTransform` of a persisted seed. Nothing is deleted when a key is missing: the row is absent from that searcher's neighbor list, and the text is still in the row. Fading stays a rank term in `apply_ranking`. A dimension mismatch refuses the write and the query. It does not return plaintext and it does not zero-fill, which is what `AgentKeys::encrypt` and `VectorTransform::encrypt` do today.

### (e) Tests that would prove it

- The existing `encrypt_decrypt_roundtrip`, `same_key_preserves_cosine`, and `dimension_768` tests still pass.
- A dimension mismatch returns an error. The current `crypto.rs` behavior (return the input unchanged) fails this test on purpose.
- `CryptoManager` is not on the write path. Restarting the process and opening the seed from config still finds the experience row.
- After ingest of a new experience row, recovered vector bytes are unchanged, and the embeddings probe still passes at cosine `>= 0.999` on plaintext recovered samples.
- Searching the experience partition with the seed ranks the new row where the plaintext cosine would. Searching it without the seed does not return that row as the plaintext neighbor.
- Searching the recovered partition with a transformed query does not reorder recovered hits relative to the plaintext query. One local agent with one seed leaves recovered dense order unchanged.
- `tags.text` of a transformed experience row is still the original text.
- The graph hop does not dot a transformed query against a plaintext recovered vector.

### (f) Risks, and what it would break

Fail-open in `crypto.rs` looks like success and stores or searches plaintext. Keys lost on restart make every transformed vector unreadable, or, with fail-open, readable by accident. `VectorTransform::encrypt` dropping the tail of a longer vector silently writes a different space than the embedder produced, and the probe's `0.999` check would not cover experience if experience is excluded from it. Rewriting recovered vectors breaks `PROBE_MIN_COSINE` and the read-only recovered store. One mixed ANN with one query vector ranks either the recovered partition or the experience partition and scrambles the other; `dense_arm` and the graph hop both assume one space today (`meaning_plane.rs`). The permutation-and-sign transform is not the QR matrix in `keygen.py`. Cosine preservation is the property to test, not byte-identical `Q`. Treating X25519 in `identity.rs` as this key would seal nothing about embeddings. Wiring it for a single local agent changes the dense path, the probe story, and the on-disk vectors, and the ranking of that one agent's own memories stays the ranking they already have.

## What this note does not do

No code is ported. No sizes and no schedules are estimated. Decay, compression, reconstruction, offer/commons, LoCoMo, shelf, the audit bench, centroid merge, precise SQL query, and one-container-per-agent stay in the inventory. The three lines in the recommendation table are the decision this file prepares.
