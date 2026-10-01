# Dream Dynamics and Similarity Forecasting: Ground Truth, Options, and Roadmap

**Date:** 2026-10-01  
**Base:** `v3/r0` at `b22fcbd`  
**Scope:** Architectural research note evaluating V-JEPA 2 (arXiv:2506.09985) style forecasting over memory graph edge dynamics during dream cycles.  
**Deliverable:** Specification and decision brief for Kord and Steve. No code changes in this branch.

---

## 1. What Moves Today: The Reality of the Running Code

Steve's exploratory analysis suggested using a V-JEPA 2 self-supervised predictor over edge similarity dynamics ($s, \dot{s}, \ddot{s}$) during the dream cycle. A line-by-line audit of the codebase on `v3/r0` reveals that in the current implementation, **nothing moves**, and in fact **no edge dynamics survive across cycles**.

### 1.1 Fresh Scratch Allocation Every Cycle
In `crates/ferricula-server/src/life.rs:1001-1013`, the periodic dream handler `life_bhavana` builds fresh, unpersisted scratch instances on every single cycle:
```rust
// crates/ferricula-server/src/life.rs:1001-1013
let rows = self.experience().rows();
let mut engine = Engine::new();
let mut store = MemoryStore::new();
for (row, record) in &rows {
    if engine.upsert(row.clone()).is_ok() {
        store.insert(record.clone());
    }
}
let mut graph = MemoryGraph::new();
graph.load_edges(self.experience().edges());
let mut skg = SkgState::default();
let prime_tree = PrimeTree::default();
```
- `MemoryGraph::new()` (`crates/ferricula-core/src/graph.rs:64-67`) instantiates an empty graph with `edge_dynamics: HashMap::new()`.
- While `graph.load_edges()` populates topological edges (`crates/ferricula-core/src/graph.rs:73-94`), it **does not load edge dynamics history**. As explicitly documented in `crates/ferricula-core/src/graph.rs:59-60`:
  > `/// NOT currently persisted across snapshots (cleared on restart; treat as recovered after a few dream cycles).`
- Because `graph` is a scratch local variable dropped at the conclusion of `life_bhavana` (`crates/ferricula-server/src/life.rs:1049`), all observations recorded during a dream cycle are discarded. Edge history **never exceeds 1 observation**.
- In `crates/ferricula-core/src/skg.rs:103-149`, velocity $\dot{s}$ requires $\ge 2$ entries (`skg.rs:104`), acceleration $\ddot{s}$ requires $\ge 3$ entries (`skg.rs:119`), and the Weber bracket $B = \dot{s}^2 + s\ddot{s}$ requires $\ge 3$ entries (`skg.rs:145-149`). Consequently, `edge_bracket()` is **always `None`** in the live server.

### 1.2 Experience Rows Have Empty Vectors
In `crates/ferricula-server/src/memory.rs`, experience rows are written with empty vector buffers:
- `memory.rs:530`: `let row = Row { id, tags, vector: Vec::new(), refs: None };` (verdicts / observations)
- `memory.rs:614`: `let row = Row { id, tags, vector: Vec::new(), refs };` (prompts)
- `memory.rs:671`: `inner.engine.remember(Row { id: heard_id, tags, vector: Vec::new(), refs: None }, record)?;` (hearing)
- `memory.rs:680`: `inner.engine.remember(Row { id: said_id, tags, vector: Vec::new(), refs: None }, record)?;` (speech)
- `memory.rs:722`: `let row = Row { id, tags, vector: Vec::new(), refs: Some(refs) };` (external messages)

When `bhavana_cycle` executes Phase 3 (`crates/ferricula-cognition/src/dream.rs:260-353`), it inspects candidates:
```rust
// crates/ferricula-cognition/src/dream.rs:319-321
if row_a.vector.is_empty() {
    continue;
}
```
Because experience vectors are empty, **the dream phase skips every experience memory**. 

### 1.3 Embeddings Live in MeaningIndex, Not Engine
Dense representation vectors produced by the embedder live exclusively in `MeaningIndex` (`crates/ferricula-server/src/meaning.rs:371-460` and `crates/ferricula-server/src/meaning_plane.rs:4-8`):
- `MeaningPlane` maintains decoupled sidecar indexes.
- `bhavana_cycle` takes `&Engine` (`crates/ferricula-server/src/life.rs:1023`), not `MeaningPlane` or `MeaningIndex`.
- The dream phase **never reads `MeaningIndex`**, so it never has access to the real embeddings of the agent's experiences.
- Furthermore, `PrimeTree::default()` passed on `crates/ferricula-server/src/life.rs:1013` is an empty, unpopulated default.

### 1.4 Pushed Observations Are Raw, Frozen Cosines
Even for recovered memories that do have populated vectors in `Engine`:
- In `crates/ferricula-cognition/src/dream.rs:335-342`:
  ```rust
  let sim = cosine_sim(&row_a.vector, &row_b.vector);
  let now = now_epoch();
  graph.push_edge_observation(id_a, id_b, now, sim);
  ```
  `cosine_sim` (`crates/ferricula-cognition/src/dream.rs:379-389`) is a standard unweighted dot product over normalized float slices.
- **Nothing is folded in**: no decay, no recency weighting, and no Hebbian co-activation counts.
- **Vectors are frozen**: In `crates/ferricula-core/src/engine.rs:22-31`, `MemoryEngine::upsert` stores static `Row.vector` buffers. In `crates/ferricula-server/src/meaning.rs:440-446`, embedded vectors are stored in immutable `Arc<Vec<f32>>` hashed to text content. They are never mutated in-place or dynamically drifted.
- **Identically zero derivatives**: For any existing pair $(a, b)$, $s(t_1) = s(t_2) = s(t_3) = C$.
  $$\dot{s} = \frac{s_2 - s_1}{\Delta t} = 0.0 \quad (\text{skg.rs:113})$$
  $$\ddot{s} = \frac{\dot{s}_{21} - \dot{s}_{10}}{\Delta t_{\text{avg}}} = 0.0 \quad (\text{skg.rs:138})$$
  $$B = \dot{s}^2 + s \ddot{s} = 0.0 \quad (\text{skg.rs:148})$$
  Even if history persisted across cycles, the Weber bracket would be identically $0.0$.

### 1.5 Edge Pruning Wipes Historical Dynamics
When the dream phase prunes or disconnects an edge (`crates/ferricula-core/src/graph.rs:124-127`):
```rust
// crates/ferricula-core/src/graph.rs:124-127
pub fn disconnect(&mut self, a: u32, b: u32) {
    let key = canonical_key(a, b);
    self.edge_dynamics.remove(&key);
    ...
```
Any dynamics accumulated for the edge are unceremoniously dropped from memory.

---

## 2. What Could Make Similarity Move: Mechanisms, Meanings, and Costs

For any predictive forecaster (whether Taylor, ARIMA, or a learned V-JEPA head) to be meaningful, the scalar observation series $s(t)$ must actually exhibit non-zero variance $\text{Var}(s) > 0$. There are two distinct ontological classes of movement: **Representation Dynamics** (changes in semantic feature geometry) and **Bookkeeping Dynamics** (system runtime state, usage, and graph weighting).

| Option | Nature | What a Forecast $\hat{s}_{t+1}$ Means | Architectural & Runtime Cost |
|---|---|---|---|
| **A. Hydrate Vectors from MeaningIndex** | Grounding (Prerequisite) | Enables baseline cosine similarity for experience rows; similarity remains static unless representations move. | Low CPU/memory: query `MeaningIndex::get_vector` during `life_bhavana` setup to populate scratch `Engine.vector`. |
| **B. Persist `edge_dynamics` Across Cycles** | Bookkeeping (Prerequisite) | Retains observation history across cycles, allowing $\ge 3$ ticks so velocity, acceleration, and Weber brackets can compute. | Low: serialize `edge_dynamics` into `bhavana-state.json` (`crates/ferricula-server/src/life.rs:1014-1027`). Ring buffer of 16 pairs of `(u64, f32)` per tracked edge. |
| **C. Hebbian Co-Activation Weighting** | Bookkeeping (Cognitive Graph) | $\hat{s}_{t+1}$ predicts associative co-occurrence: memories frequently recalled together strengthen; dormant connections weaken. | Very low: increment co-activation count during chat recall overlay (`crates/ferricula-server/src/chat.rs`); scale $s(t) = s_{\text{cos}} \cdot \tanh(\gamma \cdot N_{\text{co-recall}})$. |
| **D. Thermodynamic Fidelity Decay ($s \cdot \sqrt{f_a f_b}$)** | Bookkeeping (Thermodynamics) | $\hat{s}_{t+1}$ predicts the compound relational survival of two memories under differential decay ($\alpha$) and consolidation. | Negligible: evaluate $s(t) = s_{\text{cos}} \cdot \sqrt{f_a(t) f_b(t)}$ where $f_i$ is `MemoryRecord.fidelity` (`crates/ferricula-core/src/memory.rs:24`). Fully deterministic decay kinetics. |
| **E. Re-Embedding / Representation Drift** | Representation Dynamics | $\hat{s}_{t+1}$ predicts drift in semantic representation space (e.g., fine-tuning, adapter updates, embedding model migrations). | High: requires periodic forward passes through local embedders. In a frozen-weights deployment, representation drift is zero. |

### Diagnostic Distinction: Representation vs. Bookkeeping
- **Representation dynamics** reflects how concepts shift relative to one another in the underlying latent space. Because Ferricula uses fixed upstream embedding checkpoints (`space = "bge-large-en-v1.5"` or Ollama embeddings), representation space is static.
- **Bookkeeping dynamics** reflects the agent's lived trajectory: which memories are reinforced by operator conversations, which are neglected, and how clusters consolidate.
- **Crucial insight**: A predictor trained on (C) or (D) does **not** learn semantic latent physics (like V-JEPA does on video patches); it learns the mathematical consequences of Ferricula's own scoring and decay formulas.

---

## 3. Smallest Honest v1: Roadmap and Concrete Files

Following the coordinator's critique, we must avoid premature modeling over static or trivial inputs. The v1 roadmap consists of four strict gates, mapped directly to concrete repository files:

```
[Phase A: Dynamic Signal] ───► [Phase B: Passive Logging] ───► [Phase C: Intervention Log] ───► [Phase D: Gated Actuation]
(MeaningIndex + Decay/Hebbian)  (Persistence & Taylor baselines)  (karmic.jsonl EdgeIntervention)   (Only if learned beats Taylor)
```

### 3.1 Phase A: Confirm and Ground Dynamic Observations
1. **Hydrate vectors in `life.rs`**: In `crates/ferricula-server/src/life.rs:1004-1009`, look up vectors in `self.meaning.index` so `engine` receives non-empty vectors for experience rows.
2. **Persist edge history**: In `crates/ferricula-server/src/life.rs:1015-1027`, include `edge_dynamics` in `BhavanaState` (`life/bhavana-state.json`) so histories accumulate across dream cycles up to `HISTORY_CAP = 16` (`crates/ferricula-core/src/skg.rs:40`).
3. **Define a dynamic observation**: In `crates/ferricula-cognition/src/dream.rs:335-342`, make `sim` dynamic by folding in fidelity or co-activation:
   $$s_{\text{observed}}(t) = s_{\text{cosine}}(a, b) \times \sqrt{f_a(t) \cdot f_b(t)}$$
   This guarantees that as memories age or consolidate according to Abhidhamma rules (`crates/ferricula-cognition/src/bhavana.rs`), edge similarity exhibits genuine movement.

### 3.2 Phase B: Passive Forecast Logging Against Baselines
Before introducing any learned neural head, instrument `bhavana_cycle` to emit passive forecast evaluations comparing against two closed-form baselines:
1. **Persistence Baseline**:
   $$\hat{s}_{t+1}^{\text{persist}} = s_t$$
2. **Taylor Baseline (Analytical extrapolation from Weber history)**:
   Using velocity $\dot{s}$ (`crates/ferricula-core/src/skg.rs:103-114`) and acceleration $\ddot{s}$ (`crates/ferricula-core/src/skg.rs:118-139`):
   $$\hat{s}_{t+1}^{\text{taylor}} = s_t + \dot{s} \Delta t + \frac{1}{2} \ddot{s} (\Delta t)^2$$
   *(clamped to $[0, 1]$)*.

Record passive prediction errors on every tick:
$$L_1^{\text{persist}} = |s_{t+1} - \hat{s}_{t+1}^{\text{persist}}|, \quad L_1^{\text{taylor}} = |s_{t+1} - \hat{s}_{t+1}^{\text{taylor}}|$$
Any proposed learned forecaster must demonstrate a statistically significant reduction in $L_1$ error over the Taylor baseline across hundreds of dream cycles.

### 3.3 Phase C: Intervention Logging in `life/karmic.jsonl`
Acting on forecasts (pruning or reinforcing edges) perturbs the graph topology and invalidates observational scoring. An intervention log is mandatory.

In `crates/ferricula-cognition/src/karmic.rs:55-104`, extend `KarmicEvent` with an explicit edge intervention variant:
```rust
// Proposed extension to crates/ferricula-cognition/src/karmic.rs
pub enum KarmicEvent {
    ...
    EdgeIntervention {
        from: u32,
        to: u32,
        action: EdgeAction, // Connect, Disconnect, Reinforce, Prune
        reason: String,
        sim_observed: f32,
        forecast_persist: f32,
        forecast_taylor: f32,
        forecast_model: Option<f32>,
    },
}
```
In `crates/ferricula-server/src/life.rs:1028-1036`, these events flush directly into `self.life.dir.join("karmic.jsonl")`. This allows offline evaluation to separate **untouched edges** (valid for forecasting evaluation) from **intervened edges** (performative / self-fulfilling).

### 3.4 Phase D: Actuation Gated on Forecast Competence
The dream cycle may only consult forecasters for pruning (`disconnect`) or promotion (`connect` / `keystone`) after:
- The passive log shows the model beats the Taylor baseline on untouched edges ($p < 0.01$).
- Decisions are explicitly conditioned on logged interventions.

---

## 4. Open Decisions for Kord and Steve

These architectural decisions govern the scope and philosophy of dream dynamics; they are highlighted for decision, not decided here:

1. **Representation Drift vs. Cognitive Bookkeeping**:
   - *Question:* Should Ferricula's dream predictor aim to forecast shifts in semantic representation space (which requires periodically re-embedding text or training dynamic adapters), or should it explicitly forecast the dynamics of the agent's cognitive graph (Hebbian co-activation, retrieval priority, and thermodynamic decay)?
   - *Context:* If the latter, framing this as "V-JEPA 2" is an analogy to visual patch latents; in practice, it is an autoregressive forecaster over a scalar relational graph.

2. **Durable Home for Edge Dynamics**:
   - *Question:* Should `edge_dynamics` history be persisted inside `life/bhavana-state.json` (as part of the cognition/life runtime state), or should it be stored in `MemoryGraph` inside the core database / engine persistence (`crates/ferricula-core/src/persist.rs`)?
   - *Trade-off:* State JSON keeps it isolated to cognitive dreaming without modifying the core storage format; core persistence ensures edge history survives across all client types and CLI tools.

3. **Hydration Path for Dream Vectors**:
   - *Question:* How should `life_bhavana` obtain vectors for experience rows?
   - *Option A:* Populate `engine.vector` directly from `MeaningPlane::index` during the scratch setup in `life.rs:1001-1009`.
   - *Option B:* Refactor `bhavana_cycle` to take `&MeaningIndex` directly as an explicit sense door parameter rather than relying on `Engine`'s internal vector table.

4. **Time Metric for Edge Velocity ($\Delta t$)**:
   - *Question:* In `crates/ferricula-core/src/skg.rs:109,126`, $\Delta t$ is computed from `u64` tick timestamps (`dt = t2 - t1`). When dream cycles occur at irregular wall-clock intervals (or during catch-up wakeups), should $\Delta t$ be measured in wall-clock seconds (`now_epoch()`), or in integer dream cycle ordinal counts ($k, k+1, k+2$)?
   - *Trade-off:* Wall-clock seconds accurately reflect real-world decay intervals; discrete cycle counts prevent numerical instability when dream cycles run in rapid succession during testing or catch-up.

5. **Decision Threshold for Predictive Actuation**:
   - *Question:* What threshold of predictive error reduction over the Taylor baseline ($s + \dot{s}\Delta t + \frac{1}{2}\ddot{s}\Delta t^2$) is required before Steve allows the dream phase to autonomously prune edges based on forecasted decay?
