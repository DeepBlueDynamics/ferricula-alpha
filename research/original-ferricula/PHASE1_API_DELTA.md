# API Spec: Phase One Integration

This document defines the smallest API changes required in [api.rs](file:///workspace/ferricula_v2/crates/ferricula-server/src/api.rs) to expose the state and configuration of the newly integrated Phase One autonomy modules.

---

## 1. Automatic Status Visibility

After injecting the new modules into `SteveRuntime`, the existing `GET /status` handler automatically exposes updated runtime fields:

* **`mode`**: Now dynamically mapped from `AutonomyMachine::state` (`ActivityMode::Paused`, `ActivityMode::Asleep`, `ActivityMode::Engaged`, etc.).
* **`queued`**: Dynamically returns the size of the runtime's pending task queue (`tasks.len()`).
* **`running`**: Returns the count of tasks actively locked by the background worker thread.
* **`last_scheduled_wake`**: Automatically updated on `WakeTrigger::Scheduled` triggers.

---

## 2. New Operator-Authenticated Endpoints

The following minimal endpoints must be patched into `api.rs`. They all require `Authorization: Bearer <OP_TOKEN>` headers.

### A. Autonomy Snapshot (`GET /control/autonomy`)
* **Mode**: Read-only
* **Response Shape**:
  ```json
  {
    "state": "asleep",
    "lease": null,
    "idle_secs": 180,
    "budget_pressure": 0.0,
    "policy": {
      "wakes_per_day": 4,
      "max_steps_per_lease": 10
    }
  }
  ```
* **Acceptance Command**:
  ```bash
  curl -H "Authorization: Bearer <OP_TOKEN>" http://127.0.0.1:8875/control/autonomy
  ```

### B. Pending Considerations (`GET /tasks/considerations`)
* **Mode**: Read-only
* **Response Shape**: Array of pending signals from `MentionIngestState`:
  ```json
  [
    {
      "dedupe_id": "nutnews:101",
      "sender": "operator",
      "text": "Steve, tell us about the NeXT Cube",
      "received_at": 1783961565
    }
  ]
  ```
* **Acceptance Command**:
  ```bash
  curl -H "Authorization: Bearer <OP_TOKEN>" http://127.0.0.1:8875/tasks/considerations
  ```

### C. Sleep Cycle Proposals (`GET /control/schedule/plan`)
* **Mode**: Read-only
* **Response Shape**: Evaluated plan using `plan_cycle_gated()`:
  ```json
  {
    "phase": "LightSleep",
    "proposal": "DeepSleep",
    "ready_for_planning": true,
    "cooldowns": {
      "ReadFeed": 1783965000,
      "AdvisoryReview": 1783968000
    }
  }
  ```
* **Acceptance Command**:
  ```bash
  curl -H "Authorization: Bearer <OP_TOKEN>" http://127.0.0.1:8875/control/schedule/plan
  ```

### D. Memory Overlay Status (`GET /memory/overlay`)
* **Mode**: Read-only
* **Response Shape**: Summary projection statistics and pending approvals:
  ```json
  {
    "total_events": 14,
    "pending_approvals": [
      {
        "event_id": "evt_rec_09",
        "decision": "AwaitingApproval",
        "payload": {
          "Ingest": {
            "text": "Steve visited Xerox PARC",
            "importance": 0.9
          }
        }
      }
    ]
  }
  ```
* **Acceptance Command**:
  ```bash
  curl -H "Authorization: Bearer <OP_TOKEN>" http://127.0.0.1:8875/memory/overlay
  ```

### E. Overlay Event Approval (`POST /memory/overlay/approve/{event_id}`)
* **Mode**: Mutating
* **Request Shape**: Empty body or:
  ```json
  {
    "decision": "Approve"
  }
  ```
* **Response Shape**:
  ```json
  {
    "event_id": "evt_rec_09",
    "status": "Approved"
  }
  ```
* **Acceptance Command**:
  ```bash
  curl -X POST -H "Authorization: Bearer <OP_TOKEN>" -H "Content-Type: application/json" -d '{"decision":"Approve"}' http://127.0.0.1:8875/memory/overlay/approve/evt_rec_09
  ```

### F. Advocate Status & History (`GET /control/advocate`)
* **Mode**: Read-only
* **Response Shape**: Returns advocate health and recent wants/verdict records:
  ```json
  {
    "last_run_at": 1783961565,
    "history": [
      {
        "ts": 1783961565,
        "wants": "Build beautiful products.",
        "verdict": "Alignment holds. Mentions are deliberated against values."
      }
    ]
  }
  ```
* **Acceptance Command**:
  ```bash
  curl -H "Authorization: Bearer <OP_TOKEN>" http://127.0.0.1:8875/control/advocate
  ```

---

## 3. Security Boundaries

* **No Compelled Execution**: API routes only read planning state or append considerations to the queue. No endpoint allows a caller to bypass the `AutonomyMachine` and directly compel Steve to run a tool or post a public message.
* **No Memory Leaks**: General memory content search remains restricted to the authenticated `/memory/recall` endpoint. The overlay status API only exposes metadata summaries and event envelopes to prevent private memory leaks.
