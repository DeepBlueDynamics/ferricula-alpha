# Idempotency Notes: write idempotency patch

This document details the assumptions, design decisions, and verification steps for the `write_idempotency` patch to the reference **Nuts News** implementation.

---

## 1. Core Assumptions
* **In-Memory Idempotency Cache**: Since all state is replayed and stored in RAM (`store.State`), the idempotency mappings are persisted in-memory under `State::idempotency` mapping `request_id: String` to `IdempotentRecord` (which holds `payload_hash` and `response_id`).
* **Non-Blocking Locks**: Checking the idempotency cache is performed under a read lock (`state.read()`) prior to executing the database commit. Once the database commit succeeds, a brief write lock (`state.write()`) records the new transaction ID, preventing deadlocks or blocking during GCS/file disk I/O.
* **Eviction Bounds**: To prevent memory leaks, the cache size is strictly bounded to the last 1000 keys using a FIFO queue (`VecDeque`). Evicted keys lose idempotency guarantees, resulting in new items being created on re-submission (safe default for older retries).
* **Payload Verification**: Replays must supply the exact same payload. Different payloads with the same `request_id` result in an `Idempotency conflict` error, preventing hijacking or collisions.

---

## 2. Test & Verification Commands

Since the patch is provided as a unified diff file (`write_idempotency.patch`), it can be checked and verified using the commands below:

### A. Apply & Compile Check
Apply the patch to a local copy of the `nuts-news` workspace:
```bash
git apply --check write_idempotency.patch
git apply write_idempotency.patch
cargo check
```

### B. Run Unit Tests
Run the integrated idempotency test suite (verifying replay recovery, conflicts, and bounds eviction):
```bash
cargo test test_write_idempotency
```

### C. Verify MCP Write Idempotency
Call the MCP tool with a custom `request_id` key:
```bash
curl -X POST -H "Content-Type: application/json" -d '{"method":"tools/call", "params":{"name":"submit", "arguments":{"title":"Idempotent Post", "request_id":"req_unique_999"}}}' http://localhost:18082/mcp
```
Subsequent requests with the same payload return the cached item ID. Requests with modified payloads return:
```json
{
  "isError": true,
  "content": [{"type": "text", "text": "Idempotency conflict: request_id reused with different payload"}]
}
```
