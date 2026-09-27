# Runtime Wiring Map: V2 Integration

This document outlines the precise types, insertion points, and patch sequences to integrate the new standalone V2 modules ([autonomy.rs](../../crates/ferricula-server/src/autonomy.rs), [memory_overlay.rs](../../crates/ferricula-server/src/memory_overlay.rs), [sleep_cycle.rs](../../crates/ferricula-server/src/sleep_cycle.rs), and [mention_ingest.rs](../../crates/ferricula-server/src/mention_ingest.rs)) into the Axum server runtime.

---

## 1. Struct Insertion Points

Add the new state-management structs directly to `SteveRuntime` in [runtime.rs](../../crates/ferricula-server/src/runtime.rs#L120-L135):

```rust
pub struct SteveRuntime {
    pub config: RuntimeConfig,
    pub inspection: Inspection,
    state_path: PathBuf,
    usage_path: PathBuf,
    state: Mutex<DurableRuntimeState>,
    notify: Notify,
    nutnews: NutNewsClient,
    council: WisdomCouncil,
    memory: MemoryRuntime,
    router: ModelRouter,
    transport: HttpInferenceTransport,
    persona: String,
    
    // === NEW PHASE ONE AUTONOMY ARTIFACTS ===
    pub autonomy: Mutex<AutonomyMachine>,
    pub overlay: Mutex<OverlayLog>,
    pub ingest: Mutex<MentionIngestState>,
    pub cooldowns: Mutex<CooldownLedger>,
}
```

---

## 2. Startup Recovery Sequence

Implement recovery logic in `SteveRuntime::open` in [runtime.rs](../../crates/ferricula-server/src/runtime.rs#L140-L150):

```rust
// 1. Recover memory overlay log
let overlay_path = config.state_dir.join("memory-overlay.json");
let overlay = if overlay_path.exists() {
    OverlayLog::load(&overlay_path)?
} else {
    OverlayLog::new(OverlayConfig::default())?
};

// 2. Recover autonomy machine state
let autonomy_path = config.state_dir.join("autonomy-state.json");
let autonomy = if autonomy_path.exists() {
    let snapshot = serde_json::from_reader(File::open(&autonomy_path)?)?;
    AutonomyMachine::recover(snapshot, now_epoch())?
} else {
    AutonomyMachine::with_defaults()
};

// 3. Recover mention ingestion tracker
let ingest_path = config.state_dir.join("mention-ingest.json");
let ingest = if ingest_path.exists() {
    MentionIngestState::load(&ingest_path)?
} else {
    MentionIngestState::with_defaults()?
};

// 4. Recover cooldown ledgers
let cooldowns_path = config.state_dir.join("cooldowns.json");
let cooldowns = if cooldowns_path.exists() {
    serde_json::from_reader(File::open(&cooldowns_path)?)?
} else {
    CooldownLedger::default()
};
```

---

## 3. Integration Mappings

### A. Mention Ingestion & Consideration Queue
* **Endpoint**: `/tasks/mention` in [api.rs](../../crates/ferricula-server/src/api.rs).
* **Action**: Parse request body into `EventBatch`. Lock state: `runtime.ingest.lock()`. Invoke `ingest.apply_batch(batch, now_epoch())`.
* **Queueing**: For each surfaced `Consideration` where `may_enqueue_consideration()` is true and `response_required` is false, map to a `TaskRecord` with `TaskKind::DirectMention` and push to runtime task queue.

### B. Sleep / Dream Proposals
* **Cadence Integration**: In `run_scheduler` in [runtime.rs](../../crates/ferricula-server/src/runtime.rs#L295):
  * Read `runtime.cooldowns.lock()` and `runtime.config.schedule` (as `SleepCyclePolicy`).
  * Check if due via `sleep_cycle::cycle_is_due(...)`.
  * If due, compute the proposal plan using `sleep_cycle::plan_cycle_gated(...)`.
  * Enqueue `TaskKind::ReadFeed` or `TaskKind::AdvisoryReview` tasks depending on the computed `CyclePlan`.
  * Update `cooldowns.lock().note_run(...)` and serialize to `cooldowns.json`.

### C. Advisory Advocate Reviews
* **Scheduler Execution**: In `execute()` (**runtime.rs** line 351) under `TaskKind::AdvisoryReview`:
  * Read tension/activation state from `autonomy.lock()`.
  * Run hybrid search query on the union `overlay.lock().projection()` for stated values.
  * Formulate prompt and execute `complete_with_budget()` using `TaskClass::Deliberate`.
  * Append results as `OverlayPayload::Advocate(AdvocateVerdict)` to overlay log. Do not write to memory.

---

## 4. Operational Hazards

* **Lock Ordering (Deadlock Hazard)**:
  * **Rule**: Mutexes must always be locked in a top-down hierarchical order.
  * **Order**: `autonomy` $\rightarrow$ `cooldowns` $\rightarrow$ `ingest` $\rightarrow$ `overlay`. Never lock in reverse or hold locks during async `.await` model completions.
* **Crash-Recovery Consistency**:
  * Write-overlay updates (`overlay.save()`) must successfully flush to disk using atomic rename operations (`atomic_json_write`) *before* returning 200 OK to the client.
* **Privacy Safeguards**:
  * Cloud profiles (e.g. `anthropic_sonnet`) must be explicitly audited for `ModelCapability::PrivateContext` before Deliberate/Advisory payloads are sent.

---

## 5. Smallest-Safe Patch Sequence

### Patch 1: Library Declarations
Expose new files in `lib.rs`:
```rust
pub mod autonomy;
pub mod memory_overlay;
pub mod sleep_cycle;
pub mod mention_ingest;
```

### Patch 2: Runtime Injection
Add fields and startup recovery code to `SteveRuntime` and `SteveRuntime::open` in `runtime.rs`.

### Patch 3: Ingest Integration
Hook Axum API routes in `api.rs` `/tasks/mention` to call `MentionIngestState::apply_batch`.

### Patch 4: Scheduler Hook
Modify `run_scheduler` in `runtime.rs` to call `sleep_cycle::cycle_is_due` and map output to the task worker queue.

### Patch 5: Read-Union Redirection
Update `recall_candidates` in `runtime.rs` to query the projected union memory dataset `runtime.overlay.lock().projection()` instead of the raw legacy memory base directory.

---

## 6. Verification & Acceptance Checks

1. **Verify Cargo Compile**:
   * Command: `cargo check -p ferricula-server` (must report zero errors).
2. **Verify Memory Protection Invariant**:
   * Command: `touch .runtime/steve-jobs/write-test.txt` (must fail with read-only error).
3. **Verify Mention Ingestion Invariant**:
   * Command: `curl -H "Content-Type: application/json" -d @batch.json http://127.0.0.1:8875/tasks/mention` (must return 200 and enqueue `DirectMention`).
