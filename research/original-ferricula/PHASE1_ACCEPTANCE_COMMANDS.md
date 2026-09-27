# Host Acceptance Test Suite: Phase One

This document defines the host terminal commands and expected results to verify the integration of Phase One autonomous core functions in Ferricula V2.

---

## 0. Safe Token Initialization

To prevent credentials from being recorded in terminal history, load the token in-process before executing test curls:

### For Linux/macOS (Bash/Zsh):
```bash
OP_TOKEN=$(cat secrets/ferricula_operator_token)
```

### For Windows (PowerShell):
```powershell
$token = Get-Content secrets/ferricula_operator_token
```

Use the corresponding variables (`$OP_TOKEN` or `$token`) in the headers of all curl requests below. Do not type or echo the raw token value.

---

## 1. Config & Docker Environment Checks (Executable Today)

### A. Configuration Parsing & Data Inspection
* **Command**:
  ```bash
  docker compose exec steve /usr/local/bin/ferricula-server inspect /data/steve-memory
  ```
* **Expected Output**:
  ```text
  Steve Jobs (ferricula-stevejobs)
  rows=33622 memories=3362
  active=3362 forgiven=0 archived=0 keystones=1778
  graph=3362 nodes/0 edges prime_tree=1762 terms/3362 nodes/3524 members
  ```

### B. Docker Compilation
* **Command**:
  ```bash
  docker compose build --no-cache steve
  ```
* **Expected Output**:
  ```text
  Successfully built image ... sha256:ef4bd24cca6699a4b8588b17ff32e3a7f1a1
  ```

---

## 2. Autonomy & State Persistence API (Executable Today)

### A. Get Engine Status
* **Command (Bash)**:
  ```bash
  curl -H "Authorization: Bearer $OP_TOKEN" http://127.0.0.1:8875/status
  ```
* **Command (PowerShell)**:
  ```powershell
  curl -H "Authorization: Bearer $token" http://127.0.0.1:8875/status
  ```
* **Expected Output**:
  ```json
  {"identity":"Steve Jobs","agent_id":"ferricula-stevejobs","mode":"asleep","queued":0,"running":0,"schedule_enabled":true,"last_scheduled_wake":1783961565,"memory_rows":33622,"memory_records":3362,"nutnews_enabled":true,"nutnews_writes_enabled":false,"model_usd_today":0.0}
  ```

### B. Pause Engine
* **Command (Bash)**:
  ```bash
  curl -X POST -H "Authorization: Bearer $OP_TOKEN" http://127.0.0.1:8875/control/pause
  ```
* **Command (PowerShell)**:
  ```powershell
  curl -X POST -H "Authorization: Bearer $token" http://127.0.0.1:8875/control/pause
  ```
* **Expected Output**:
  ```json
  {"mode":"paused"}
  ```

### C. Scheduled Wake / Resume Trigger
* **Command (Bash)**:
  ```bash
  curl -X POST -H "Authorization: Bearer $OP_TOKEN" http://127.0.0.1:8875/control/wake
  ```
* **Command (PowerShell)**:
  ```powershell
  curl -X POST -H "Authorization: Bearer $token" http://127.0.0.1:8875/control/wake
  ```
* **Expected Output**:
  ```json
  {"mode":"engaged"}
  ```

### D. Direct Mention Enqueueing
* **Command (Bash)**:
  ```bash
  curl -X POST -H "Authorization: Bearer $OP_TOKEN" -H "Content-Type: application/json" -d '{"item_id": 101, "comment_id": 202, "by": "operator", "text": "Steve, tell us about the NeXT Cube"}' http://127.0.0.1:8875/tasks/mention
  ```
* **Command (PowerShell)**:
  ```powershell
  curl -X POST -H "Authorization: Bearer $token" -H "Content-Type: application/json" -d '{"item_id": 101, "comment_id": 202, "by": "operator", "text": "Steve, tell us about the NeXT Cube"}' http://127.0.0.1:8875/tasks/mention
  ```
* **Expected Output**:
  ```json
  {"created_at":1783965077,"error":null,"finished_at":null,"id":"cb1d8182-775b-43fb-bee1-a6eef0a6be75","kind":"direct_mention","payload":{"by":"operator","comment_id":202,"item_id":101,"text":"Steve, tell us about the NeXT Cube"},"result":null,"source":"nutnews-mention","started_at":null,"status":"queued"}
  ```

### E. Memory Semantic Recall
* **Command (Bash)**:
  ```bash
  curl -X POST -H "Authorization: Bearer $OP_TOKEN" -H "Content-Type: application/json" -d '{"query":"Apple", "limit": 1}' http://127.0.0.1:8875/memory/recall
  ```
* **Command (PowerShell)**:
  ```powershell
  curl -X POST -H "Authorization: Bearer $token" -H "Content-Type: application/json" -d '{"query":"Apple", "limit": 1}' http://127.0.0.1:8875/memory/recall
  ```
* **Expected Output**:
  ```json
  {"hits":[{"fidelity":1.0,"id":1190,"importance":1.2,"keystone":true,"refs":null,"score":1.33,"state":"active","tags":{"channel":"hearing","text":"Woz leaves Apple. Steve takes over Mac project."}}],"query":"Apple"}
  ```

---

## 3. Read-Only Verification (Executable Today)

### A. Core Base Mutation Denial
* **Command**:
  ```bash
  docker compose exec steve touch /data/steve-memory/test.txt
  ```
* **Expected Output**:
  ```text
  touch: cannot touch '/data/steve-memory/test.txt': Read-only file system
  ```

---

## 4. Phase One Features [BLOCKED / PLANNED]

The following verification steps require code integration of the standalone modules and are currently blocked by missing API routes or unwired internal states:

### A. [BLOCKED/PLANNED] State Recovery Verification
* **Missing Wiring**: `SteveRuntime::open` does not load state files from disk yet.
* **Test Command**:
  ```bash
  docker compose restart steve && docker compose logs steve | grep -E "Recovered|Loaded"
  ```
* **Expected Logs after wiring**:
  ```text
  [INFO] Loaded overlay log from memory-overlay.json (12 events)
  [INFO] Recovered autonomy machine in state Asleep
  ```

### B. [BLOCKED/PLANNED] Deduplicated Mention Ingestion
* **Missing Wiring**: `/tasks/mention` enqueues directly without parsing through `MentionIngestState`.
* **Test Command**: Repeat the Direct Mention command twice with the same `item_id`.
* **Expected Result after wiring**: The second request returns a skip record or matches duplicate deduplication cursor limits rather than queuing a duplicate task.

### C. [BLOCKED/PLANNED] Writable Overlay Log Append
* **Missing Wiring**: Ingestion write endpoint (`/memory/remember`) is not mapped in `api.rs`.
* **Test Command (Bash)**:
  ```bash
  curl -X POST -H "Authorization: Bearer $OP_TOKEN" -H "Content-Type: application/json" -d '{"text":"Steve visited Xerox PARC", "importance": 0.9}' http://127.0.0.1:8875/memory/remember
  ```
* **Expected Result after wiring**: Append success code returned, and checking `/data/steve-runtime/memory-overlay.json` on the named volume reveals the Xerox event:
  ```bash
  docker compose exec steve cat /data/steve-runtime/memory-overlay.json
  ```

### D. [BLOCKED/PLANNED] Union Projection Search
* **Missing Wiring**: `/memory/recall` reads only from core database without checking the overlay log.
* **Test Command (Bash)**:
  ```bash
  curl -X POST -H "Authorization: Bearer $OP_TOKEN" -H "Content-Type: application/json" -d '{"query":"Xerox"}' http://127.0.0.1:8875/memory/recall
  ```
* **Expected Result after wiring**: Xerox memory returned from overlay log projection.

### E. [BLOCKED/PLANNED] Cooldown Sleep Proposal
* **Missing Wiring**: Schedule planning engine endpoint `/control/schedule/plan` is not mapped.
* **Test Command (Bash)**:
  ```bash
  curl -H "Authorization: Bearer $OP_TOKEN" http://127.0.0.1:8875/control/schedule/plan
  ```
* **Expected Result after wiring**: Returns proposal state:
  ```json
  {"phase": "LightSleep", "proposal": "DeepSleep", "ready_for_planning": true}
  ```

### F. [BLOCKED/PLANNED] Advocate Advisory Evaluation
* **Missing Wiring**: Manual run route `/tasks/advisory-run` is not mapped in `api.rs`.
* **Test Command (Bash)**:
  ```bash
  curl -X POST -H "Authorization: Bearer $OP_TOKEN" http://127.0.0.1:8875/tasks/advisory-run
  ```
* **Expected Result after wiring**: Returns execution status, and `docker compose exec steve cat /data/steve-runtime/advisory-history.json` contains the advocate wants/verdict.

---

## Blocked Verification Matrices

* **Blocked on Nuts Events**:
  * Feeds test commands (`/tasks/read-feed`) require mock local RSS XML pages to populate feed cards in memory without making outbound calls.
* **Blocked on Selected Model (Ollama/Claude)**:
  * Advocate evaluation checks and mention deliberations require either local `Ollama` running (`ollama serve`) or standard `ANTHROPIC_API_KEY` configurations to populate text completions.
