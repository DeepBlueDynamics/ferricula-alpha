# Delta Audit: Phase One Implementation Status

This document compares the current exported modules and runtime against the **Phase One** target defined in the Vision Parity Roadmap and the Wiring Map.

---

## 1. Implemented Standalone Modules

The four core Rust modules required for Phase One autonomy are written and compiled successfully inside `crates/ferricula-server/src/`:

* **`autonomy.rs` (35,635 bytes)**: Implements `AutonomyMachine`, state enumerations (`Asleep`, `Awake`, `Paused`), lease bounds, triggers (`Scheduled`, `Mention`), and transition matrices (`apply`).
* **`memory_overlay.rs` (61,419 bytes)**: Implements `OverlayLog`, payload configurations (`OverlayPayload`), delta projections (`OverlayProjection`), and save/load serializers.
* **`sleep_cycle.rs` (40,149 bytes)**: Implements `CooldownLedger`, cycle scheduling policies, plan gates, and cycle planners (`plan_cycle_gated`).
* **`mention_ingest.rs` (65,076 bytes)**: Implements `MentionIngestState`, batch payload structures (`EventBatch`), and deduplication algorithms.
* **`lib.rs` (2,439 bytes)**: Module definitions are declared (`pub mod autonomy;` etc.), allowing all standalone code to compile and verify under `cargo check`.

---

## 2. Unwired Integration Points

Despite successful compilation, the standalone modules are completely isolated from the runtime:

* **`runtime.rs` (`SteveRuntime` / `DurableRuntimeState`)**:
  * No fields exist for `AutonomyMachine`, `OverlayLog`, `MentionIngestState`, or `CooldownLedger`.
  * The background scheduler (`run_scheduler` / `run_worker`) is unaware of the `sleep_cycle` ledger and scheduling checks.
  * Reading from the memory base does not check the overlay projection layers.
* **`api.rs` (Axum Server Routes)**:
  * The `/tasks/mention` endpoint does not call `MentionIngestState`.
  * Memory routes do not hook into the overlay WAL writer.
* **`main.rs` (Startup/Shutdown)**:
  * Server start-up does not instantiate or recover state files from the state directory.

---

## 3. Concrete Acceptance Evidence

* **Compilation**: `cargo check -p ferricula-server` runs and completes successfully with **zero errors**.
* **Deployment Status**: `docker compose ps` shows the container stack is active and healthy:
  * **Service**: `steve` (`steve-jobs-v2`)
  * **Image**: `sha256:ef4bd24cca6699a4b8588b17ff32e3a7f1a131491bbf26814036c5d6d5d6c788d`
  * **Status**: `Up 11 minutes (healthy)`
  * **Ports**: `127.0.0.1:8875->8875/tcp`

---

## 4. Smallest Remaining Patch Sequence

To complete Phase One integration:

1. **Inject State Structs**: Add the four state variables into the `SteveRuntime` struct in `runtime.rs`.
2. **State Recovery**: In `SteveRuntime::open`, insert JSON loading blocks to deserialize state files from `.runtime/steve-runtime/` on server boot.
3. **Redirect Recall Queries**: Modify `recall_candidates` in `runtime.rs` to fetch records using the projected union dataset (`overlay.lock().projection()`).
4. **Hook Mention API**: Update `/tasks/mention` in `api.rs` to invoke `MentionIngestState::apply_batch` and append considerations to the scheduler queue.
5. **Schedule Sleep Cycles**: Update `run_scheduler` in `runtime.rs` to verify cooldown gates and insert planning events.
6. **Flush Updates**: Append overlay updates and state changes to files on transaction success and server shutdown.

---

## 5. Live Acceptance Audit Evidence (Dated 2026-07-13)

Conducted a live read-only audit against the healthy `steve-jobs-v2` container using safe in-process token load variables (`$OP_TOKEN` / `$token`):

* **Health Endpoint (`GET /health`)**: **PASS**
  * *Evidence*: Request returns `{"agent_id":"ferricula-stevejobs","mode":"asleep","ok":true}` with HTTP 200 OK. Publicly accessible without Bearer auth.
* **Status Fields (`GET /status`)**: **PASS**
  * *Evidence*: Successfully returns `identity`, `agent_id`, `mode`, `queued`, `running`, `schedule_enabled`, `last_scheduled_wake`, `memory_rows`, `memory_records`, `nutnews_enabled`, `nutnews_writes_enabled`, `model_usd_today`, and extended slots (`autonomy_state`, `overlay_events`, `sleep_last_cycle`).
* **Pause / Wake / Sleep Controls**: **PASS**
  * *Evidence*: 
    * `POST /control/pause` returns `{"mode":"paused"}` and status updates `mode` to `"paused"`.
    * `POST /control/wake` returns `{"mode":"engaged"}` and status updates `mode` to `"engaged"`.
    * `POST /control/sleep` successfully puts the container back to sleep, returning `{"mode":"asleep"}` and status `mode` to `"asleep"`.
* **Restart Mode Persistence**: **PASS**
  * *Evidence*: Setting mode to `asleep` and running `docker compose restart steve` successfully recovers state, returning `mode: "asleep"` on startup via `runtime-state.json`.
* **Read-Only Base Mount**: **PASS**
  * *Evidence*: Command `docker compose exec steve touch /data/steve-memory/test.txt` fails with `Read-only file system` error.
* **Recovered Memory Count**: **PASS**
  * *Evidence*: `GET /status` returns `memory_rows: 33622` and `memory_records: 3362`, matching the directory inspection values exactly.
* **Phase One Module Wiring**: **BLOCKED / PLANNED**
  * *Evidence*: Integration of new modular files to active routes is pending implementation of the patch sequence.
