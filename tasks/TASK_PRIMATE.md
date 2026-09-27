# Task: Planned Primate 👾 — Vendor-in ferricula → ferricula-core + ferricula-cognition (Phase A)

You own **`crates/ferricula-core/` and `crates/ferricula-cognition/` only**. The workspace root belongs to Conscious Perch 🧆 (already scaffolded); `crates/ferricula-search` and `crates/ferricula-semantic` belong to Severe Booby 🥐. Do not touch their files. You may overwrite the placeholder `Cargo.toml`/`src/lib.rs` inside YOUR two crates.

## Step 0 — read first
- `/workspace/DeepBlueDynamics/ferricula_v2/PLAN.md` — especially §2 (source→destination map), §3 (data model), **§5a (internal Abhidharma agent — this replaces the Wisdom Kings)**, §7 Phase A, decisions #1 and #9.
- `/workspace/DeepBlueDynamics/ferricula_v2/scan/ferricula.md`.

## Hard rules
- Write ONLY inside `/workspace/DeepBlueDynamics/ferricula_v2/crates/ferricula-core/` and `.../ferricula-cognition/`.
- The source project `/workspace/DeepBlueDynamics/ferricula/` is **READ-ONLY** — it is your reference oracle; never edit it. Same for every other project in the workspace.
- Phase A is a **mechanical vendor-in**: copy module code as close to byte-identical as possible; only fix `use` paths, module wiring, and crate boundaries. Do NOT refactor, rename functions, or "improve" logic. Exception: the Wisdom-King removals below.
- Do not run cargo outside `ferricula_v2/`.

## Vendor-in map (from `/workspace/DeepBlueDynamics/ferricula/src/`)
**→ `crates/ferricula-core/src/`**: `engine.rs`, `memory.rs`, `persist.rs`, `model.rs`, `sparse.rs`, `graph.rs`, `skg.rs`, `prime_tree.rs`, `transform.rs`. Build a `lib.rs` that wires these modules and re-exports what ferricula's `lib.rs` re-exported for them. Bring the needed deps into the crate's `Cargo.toml` (from ferricula's Cargo.toml: `anyhow`, `serde`, `serde_json`, `postcard`, `roaring`; add others only if the compiler demands them). Edition 2024.

**→ `crates/ferricula-cognition/src/`**: `dream.rs`, `casting.rs`, `pali.rs`, `identity.rs`, `clock.rs`, `planner.rs`. Depends on `ferricula-core` (path dep). Bring deps the compiler demands (likely `serde`, `serde_json`, `anyhow`, and whatever `planner.rs`/`clock.rs` use for HTTP). Edition 2024.

## Wisdom-King removal (the ONLY intentional behavior-adjacent change)
1. **Do NOT copy `archetypes.rs`.** It stays behind. Anything that imports it: remove the import and the call sites' dependency on it in the most minimal way that compiles (if `dream.rs` or others reference archetype behaviors, replace with direct calls / the deterministic default the archetype stub would have produced — the scan notes those behaviors were stubs returning hardcoded outputs anyway). Document every such touch in VENDOR_NOTES.md.
2. **Strip Wisdom-King naming from doc-comments** in copied code (e.g. `memory.rs` `ResonanceGate` docs name Acala/Yamāntaka/etc.). Keep the enum variants and ALL behavior identical — gates stay `Fidelity`, `Lifecycle`, `Temporal`, `AgentCapacity`, `Craft` (rename `Craft` → `Load` ONLY if it does not ripple beyond a couple of files; otherwise leave it and note it). Rewrite the doc-comments in functional/Abhidharma terms.
3. **Create `crates/ferricula-cognition/src/abhidharma.rs`** per PLAN.md §5a — new code, keep it small:
   - `pub struct AbhidharmaContext` — fields: operation stage (enum `CittaVithi { Avajjana, Javana, Tadalambana }`), the memory's vedanā/emotion (reuse core's `Emotion`), `fidelity: f32`, gate readout (which gates passed/failed), and a free-form `question: String`.
   - `pub enum Decision { Approve, Reject, Defer }` (plus an optional reason `String`).
   - `pub trait InternalAgent { fn advise(&self, ctx: &AbhidharmaContext) -> Decision; }`
   - `pub struct NoAgent;` — implements `InternalAgent`, returns deterministic defaults (`Approve` for merges above fidelity gate, `Defer` otherwise — document the defaults).
   - `pub struct LlmAgent { /* wraps planner.rs's Anthropic client */ }` — implement as a compiling stub with a `TODO` that calls into `planner.rs`'s client plumbing; wiring it live is a later phase.
   - Do NOT wire `abhidharma.rs` into `dream.rs` call sites yet — that is Phase D. It just has to exist, compile, and have unit tests for `NoAgent` defaults.

## Tests
Ferricula's in-file `#[cfg(test)]` tests come along with the copied modules. They must pass: `cargo test -p ferricula-core -p ferricula-cognition`. If a test depended on `archetypes.rs`, port it to the functional equivalent or drop it with a note in VENDOR_NOTES.md.

## Deliverables
- The two crates, compiling and passing tests.
- `crates/ferricula-core/VENDOR_NOTES.md` and `crates/ferricula-cognition/VENDOR_NOTES.md` — every deviation from byte-identical copy, listed.

## Report
Print exactly `PRIMATE DONE` plus: files created, test results, and the full deviation list.
