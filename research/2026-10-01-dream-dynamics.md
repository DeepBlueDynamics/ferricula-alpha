# Dream Dynamics and Similarity Forecasting: Ground Truth, Options, and Roadmap

**Date:** 2026-10-01  
**Base:** `v3/r0` at `b22fcbd`  
**Scope:** Architectural research note evaluating V-JEPA 2 (arXiv:2506.09985) style forecasting over memory graph edge dynamics during dream cycles.  
**Deliverable:** Specification and decision brief for Kord and Steve. Incorporates fleet critiques, Astra's refinements, and Steve's review. No code changes in this branch.

---

## 1. What Moves Today: The Reality of the Running Code

Steve's exploratory analysis suggested using a V-JEPA 2 self-supervised predictor over edge similarity dynamics ($s, \dot{s}, \ddot{s}$) during the dream cycle. A line-by-line audit of the codebase on `v3/r0` reveals the exact mechanics and limitations of edge tracking today.

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
- While `graph.load_edges()` populates topological edges (`crates/ferricula-core/src/graph.rs:241-245`), it **does not load edge dynamics history**. As explicitly documented in `crates/ferricula-core/src/graph.rs:59-60`:
  > `/// NOT currently persisted across snapshots (cleared on restart; treat as recovered after a few dream cycles).`
- `let mut skg = SkgState::default();` at `crates/ferricula-server/src/life.rs:1012` is also fresh scratch every cycle: term pair dynamics are similarly discarded upon cycle completion unless persisted into `bhavana-state.json`. Same problem, same fix.
- Because `graph` is a scratch local variable dropped at the conclusion of `life_bhavana` (`crates/ferricula-server/src/life.rs:1049`), all observations recorded during a dream cycle are discarded. In the running server, edge history **never exceeds 1 observation**.
- In `crates/ferricula-core/src/skg.rs:103-149`:
  - `velocity()` requires $\ge 2$ entries (`skg.rs:104`).
  - `acceleration()` requires $\ge 3$ entries (`skg.rs:119`).
  - `weber_bracket()` requires both velocity and acceleration (`skg.rs:145-149`).
  - **Bracket is `None`, not zero**: Before at least 3 distinct observations exist, or if consecutive observations occur at identical timestamps ($\Delta t = 0$), `velocity()`, `acceleration()`, and `weber_bracket()` return `None` (`skg.rs:104-112, 119-129`), not `0.0`. Consequently, `edge_bracket()` is **always `None`** in the live server.

### 1.2 Experience Rows Have Empty Vectors
In `crates/ferricula-server/src/memory.rs`, experience rows are written with empty vector buffers:
- `memory.rs:530`: `let row = Row { id, tags, vector: Vec::new(), refs: None };` (verdicts / observations)
- `memory.rs:614`: `let row = Row { id, tags, vector: Vec::new(), refs };` (prompts)
- `memory.rs:671`: `inner.engine.remember(Row { id: heard_id, tags, vector: Vec::new(), refs: None }, record)?;` (hearing)
- `memory.rs:680`: `inner.engine.remember(Row { id: said_id, tags, vector: Vec::new(), refs: None }, record)?;` (speech)
- `memory.rs:722`: `let row = Row { id, tags, vector: Vec::new(), refs: Some(refs) };` (external messages)

When `bhavana_cycle` executes Phase 3 (`crates/ferricula-cognition/src/dream.rs:260-353`), it inspects candidate rows:
```rust
// crates/ferricula-cognition/src/dream.rs:319-321
if row_a.vector.is_empty() {
    continue;
}
```
Because experience vectors in `Engine` are empty, **the dream phase skips every experience memory**.

### 1.3 Embeddings Cached by Hash in MeaningIndex, Not Engine
Dense representation vectors produced by the embedder live in `MeaningIndex` (`crates/ferricula-server/src/meaning.rs:371-460` and `crates/ferricula-server/src/meaning_plane.rs:4-8`):
- `MeaningPlane` maintains decoupled sidecar indexes. `bhavana_cycle` takes `&Engine` (`crates/ferricula-server/src/life.rs:1023`), not `MeaningPlane` or `MeaningIndex`.
- The dream phase **never reads `MeaningIndex`**, so it never accesses real embeddings for experience memories.
- **Cache invalidation vs. frozen vectors**: Embeddings are cached per FNV-1a text hash (`crates/ferricula-server/src/meaning.rs:13-15, 207, 220, 232`). They are not frozen in stone forever: if text is edited, re-sectioned, or updated, hash mismatch forces a re-embed. However, for unchanged text under a fixed model checkpoint, the vector is reused identically.
- `PrimeTree::default()` passed on `crates/ferricula-server/src/life.rs:1013` is an empty, unpopulated default.

### 1.4 Pushed Observations: Raw Cosines, Sampling Bias, and Right-Censoring
In `crates/ferricula-cognition/src/dream.rs:335-342`:
```rust
let sim = cosine_sim(&row_a.vector, &row_b.vector);
let now = now_epoch();
graph.push_edge_observation(id_a, id_b, now, sim);
```
- **Raw cosine**: `cosine_sim` (`crates/ferricula-cognition/src/dream.rs:379-389`) is a standard unweighted dot product over normalized slices. No decay, recency, or co-activation is folded in.
- **Identically zero derivatives**: For static vectors with unchanged text, $s(t_1) = s(t_2) = s(t_3) = C$. Once $\ge 3$ distinct ticks accumulate, $\dot{s} = 0.0$ (`skg.rs:113`), $\ddot{s} = 0.0$ (`skg.rs:138`), and $B = \dot{s}^2 + s\ddot{s} = 0.0$ (`skg.rs:148`).
- **Heavy sampling bias and right-censoring**:
  - In `crates/ferricula-cognition/src/dream.rs:266-305`, observations are pushed ONLY for candidate pairs sampled in that cycle (`anchors` sorted by fidelity + `explorers` sampled via entropy seed).
  - Furthermore, in lines 312-314 and 324-326:
    ```rust
    if edges_created >= max_edges {
        break;
    }
    ```
    Once `edges_created >= max_edges` (default 12 via `EDGE_MAX_PER_DREAM`, `crates/ferricula-cognition/src/bhavana.rs:381`), the loop terminates immediately. Remaining candidate pairs are **right-censored** (never evaluated or pushed on that tick).
  - Missingness in observation history is non-random and heavily confounded by anchor status, entropy seed, and early loop termination.

### 1.5 Source-Preserving Cycle Never Prunes
While `crates/ferricula-core/src/graph.rs:124-127` implements `disconnect(&mut self, a, b)` which removes `self.edge_dynamics.remove(&key)`, **the live dream cycle never calls it**.
- In `crates/ferricula-cognition/src/bhavana.rs:8-9, 412`:
  ```rust
  // crates/ferricula-cognition/src/bhavana.rs:412
  ensure!(graph.edge_count() >= graph_edges_before, "bhavana must not remove graph edges");
  ```
  The live cycle is strictly source-preserving and errors out if any edge is removed.
- The live dream path only creates edges (`graph.connect` in `dream.rs:345`).
- However, edge creation still perturbs the graph: it connects nodes, promotes them into recall overlays, and alters subsequent candidate generation.

---

## 2. What Could Make Similarity Move: Mechanisms, Meanings, and Costs

For any predictive forecaster to be meaningful, the observation series $s(t)$ must have non-zero variance $\text{Var}(s) > 0$. We distinguish **Representation Dynamics** (shifts in semantic latent geometry) from **Bookkeeping Dynamics** (system runtime state, graph activation, and decay formulas).

| Option | Nature | What a Forecast $\hat{s}_{t+1}$ Means | Architectural & Runtime Cost |
|---|---|---|---|
| **A. Hydrate Vectors from MeaningIndex** | Grounding (Prerequisite) | Enables baseline cosine similarity for experience rows; static unless text or model changes. | Low CPU/memory: query `MeaningIndex` during `life_bhavana` setup to populate scratch `Engine.vector`. |
| **B. Persist `edge_dynamics` Across Cycles** | Bookkeeping (Prerequisite) | Retains observation history across cycles, allowing $\ge 3$ ticks so velocity, acceleration, and Weber brackets can compute. | Low: serialize `edge_dynamics` ring buffer (`HISTORY_CAP = 16`, `skg.rs:40`) into `bhavana-state.json`. |
| **C. Hebbian Co-Activation Weighting** | Bookkeeping (Cognitive Graph) | $\hat{s}_{t+1}$ predicts associative co-occurrence: memories frequently recalled together strengthen; dormant connections weaken. | Very low: increment co-activation count during chat recall overlay (`crates/ferricula-server/src/chat.rs`); scale $s(t) = s_{\text{cos}} \cdot \tanh(\gamma \cdot N_{\text{co-recall}})$. |
| **D. Thermodynamic Fidelity Decay ($s \cdot \sqrt{f_a f_b}$)** | Bookkeeping (Thermodynamics) | $\hat{s}_{t+1}$ predicts compound relational survival of memories under differential decay ($\alpha$) and consolidation depth. | Negligible: evaluate $s(t) = s_{\text{cos}} \cdot \sqrt{f_a(t) f_b(t)}$ where $f_i$ is `MemoryRecord.fidelity` (`crates/ferricula-core/src/memory.rs:24`). Fully deterministic kinetics. |
| **E. Re-Embedding / Representation Drift** | Representation Dynamics | $\hat{s}_{t+1}$ predicts drift in semantic representation space (e.g. fine-tuning, adapter updates, embedding model migrations). | High: periodic forward passes through local embedders. With frozen upstream models, representation drift is zero. |

### Crucial Insight: What Stochasticity Justifies Learning?
Fidelity decay (Option D) is a closed-form formula written into code ($\text{fidelity} \times e^{-\alpha}$). A purely deterministic signal admits a closed-form analytical forecaster (Taylor / exponential); training a learned neural head on it is pure "jewelry."

Only genuinely stochastic inputs justify a learned model:
- **Radio entropy fluctuations** driving candidate sampling (`crates/ferricula-cognition/src/clock.rs` / `dream.rs:276-293`).
- **Operator conversational recall patterns** and emergent co-activations (Option C).

If there is no stochastic driver in the dynamics, skip the learned head entirely and retain the closed-form predictor.

---

## 3. Smallest Honest v1: Phased Roadmap and File Contracts

Following the critique and Astra's refinements, we establish a strict 4-phase evaluation protocol:

```
[Phase A: Dynamic Signal] ───► [Phase B: Passive Logging] ───► [Phase C: Intervention Log] ───► [Phase D: Gated Actuation]
(MeaningIndex + Dynamics)      (Persistence & Taylor baselines)  (karmic.jsonl EdgeIntervention)   (Dual-criterion validation)
```

### 3.1 Phase A: Confirm and Ground Dynamic Observations
1. **Hydrate vectors in `life.rs`**: In `crates/ferricula-server/src/life.rs:1004-1009`, look up vectors in `self.meaning.index` so `engine` receives non-empty vectors for experience rows.
2. **Persist edge history**: In `crates/ferricula-server/src/life.rs:1015-1027`, include `edge_dynamics` and `skg` in `BhavanaState` (`life/bhavana-state.json`) so histories accumulate across dream cycles up to `HISTORY_CAP = 16` (`crates/ferricula-core/src/skg.rs:40`).
3. **Define a dynamic observation**: In `crates/ferricula-cognition/src/dream.rs:335-342`, make `sim` dynamic by folding in fidelity or co-activation:
   $$s_{\text{observed}}(t) = s_{\text{cosine}}(a, b) \times \sqrt{f_a(t) \cdot f_b(t)}$$
4. **Log coverage and missingness**: Record whether an observation was taken, skipped (not sampled), or right-censored (`edges_created >= max_edges`).

### 3.2 Phase B: Passive Forecast Logging Against Baselines
The implemented Weber bracket $B = v^2 + s \cdot a$ (`crates/ferricula-core/src/skg.rs:141-149`) is an instantaneous curvature metric, **not a forward forecast**. 

A genuine forecasting evaluation requires predicting the value at the next interval $\Delta t = t_{k+1} - t_k$, evaluated against two baselines:
1. **Persistence Baseline**:
   $$\hat{s}_{k+1}^{\text{persist}} = s_k$$
2. **Taylor Baseline (2nd-Order Analytical Extrapolation)**:
   Using velocity $v$ (`skg.rs:103-114`) and acceleration $a$ (`skg.rs:118-139`) from the 3 most recent entries:
   $$\hat{s}_{k+1}^{\text{taylor}} = s_k + v \cdot \Delta t + \frac{1}{2} a \cdot (\Delta t)^2 \quad (\text{clamped to } [0, 1])$$

#### Empirical Hypothesis Requirement:
The proposal that "one global pooled model across all edges beats per-edge models" is an empirical hypothesis. It must demonstrate:
- **Chronological held-out improvement**: Train on cycles $1..T$, evaluate on cycles $T+1..T+m$.
- Lower out-of-sample $L_1 / \text{MSE}$ error than both Persistence and Taylor baselines.

### 3.3 Phase C: Intervention Logging in `life/karmic.jsonl`
Acting on forecasts perturbs graph topology and creates performative feedback loops. In `crates/ferricula-cognition/src/karmic.rs:55-104`, extend `KarmicEvent` with an intervention record:
```rust
// Extension to crates/ferricula-cognition/src/karmic.rs
pub enum KarmicEvent {
    ...
    EdgeIntervention {
        from: u32,
        to: u32,
        action: EdgeAction, // Connect, Reinforce
        reason: String,
        sim_observed: f32,
        forecast_persist: f32,
        forecast_taylor: f32,
        forecast_model: Option<f32>,
        target_version: String,
        representation_version: String,
    },
}
```
In `crates/ferricula-server/src/life.rs:1028-1036`, these events flush directly into `self.life.dir.join("karmic.jsonl")`.
- **Preselected Evaluation Cohort**: Maintain an untouched control set of sampled pairs that are tracked across cycles but explicitly shielded from dream interventions, ensuring an uncorrupted validation baseline.

### 3.4 Phase D: Dual-Criterion Gated Actuation
The dream cycle may only consult forecasters for structural graph decisions after meeting **two separate criteria**:
1. **Out-of-sample predictive competence**: The learned model demonstrably beats Persistence and Taylor baselines on the preselected untouched cohort ($p < 0.01$).
2. **Outcome utility criterion**: A separate, explicit criterion proving that acting on forecasts improves downstream agent capabilities (e.g. recall precision, lower contradiction rates, or task completion) rather than merely selecting for trivial, dying, or static edges.

---

## 4. Open Decisions for Kord and Steve

These architectural decisions govern the scope and philosophy of dream dynamics; they are highlighted for decision, not decided here:

### Decision 0 (Gating Decision): Durable Consolidation Write-Back
- *Question:* Should consolidation and edge discovery have a durable write path at all?
- *Context:* Today, the dream phase discards everything it discovers: `crates/ferricula-server/src/life.rs:1041` explicitly reports `"committed_to_store": false`, and line 1047 reports `"edges_created_in_scratch": report.edges_created`. Edges discovered during Phase 3.5 are born and die in a single scratch cycle. If discovered edges are never committed back to durable storage (`self.experience().edges()`), forecasting and actuating on them is moot. Resolving durable write-back is the prerequisite gating decision before any predictive actuation phase.

### Decision 1: What Stochasticity Justifies Learning?
- *Question:* What stochastic dynamics exist in the agent that justify a learned predictor over a closed-form Taylor or exponential model?
- *Context:* If edge variance is driven solely by deterministic fidelity decay formulas, closed-form extrapolation is exact. Only stochastic drivers (operator conversational recall patterns, unpredictable task co-occurrences, or entropy seeds) justify training a learned head. If those are absent, skip the learned head.

### Decision 2: Durable Home for Edge Dynamics
- *Question:* Should `edge_dynamics` history be persisted inside `life/bhavana-state.json` (as part of the cognition/life runtime state), or should it be stored in `MemoryGraph` inside the core database / engine persistence (`crates/ferricula-core/src/persist.rs`)?
- *Trade-off:* State JSON keeps it isolated to cognitive dreaming without modifying the core storage format; core persistence ensures edge history survives across all client types and CLI tools.

### Decision 3: Hydration Path for Dream Vectors
- *Question:* How should `life_bhavana` obtain vectors for experience rows?
- *Option A:* Populate `engine.vector` directly from `MeaningPlane::index` during the scratch setup in `life.rs:1001-1009`.
- *Option B:* Refactor `bhavana_cycle` to take `&MeaningIndex` directly as an explicit sense door parameter rather than relying on `Engine`'s internal vector table.
- *Steve's leaning:* Steve favours **Option B**, making the sense door dependency explicit and avoiding hidden hydration mutations inside `Engine`.

### Decision 4: Time Metric for Edge Velocity ($\Delta t$)
- *Question:* In `crates/ferricula-core/src/skg.rs:109,126`, $\Delta t$ is computed from `u64` tick timestamps (`dt = t2 - t1`). When dream cycles occur at irregular wall-clock intervals (or during catch-up wakeups), should $\Delta t$ be measured in wall-clock seconds (`now_epoch()`), or in integer dream cycle ordinal counts ($k, k+1, k+2$)?
- *Steve's leaning:* Steve favours **cycle ordinals**, because `skg.rs:110-111, 128` explicitly returns `None` whenever $\Delta t < \epsilon$, which routinely occurs with wall-clock seconds during rapid test runs, bursts, or catch-up cycles.

### Decision 5: Threshold and Outcome Criterion for Predictive Actuation
- *Question:* What specific metric and statistical threshold over the Taylor baseline ($s + v\Delta t + \frac{1}{2}a\Delta t^2$) will be required, and what downstream evaluation task will serve as the separate outcome criterion before learned forecasts can drive edge creation?
