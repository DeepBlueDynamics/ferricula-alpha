# Patch Notes: read-only events_since route

This document details the assumptions, design decisions, and verification steps for the `events_since` patch to the reference **Nuts News** implementation.

---

## 1. Core Assumptions
* **In-Memory Ledger Querying**: Since Nuts News maintains a sliding `VecDeque` window of the last 500 committed events in RAM (`store.State::ledger`), the `events_since` query is resolved completely from memory. Seeking historical files or versioned GCS buckets is avoided to keep the patch footprint small and performant.
* **Monotonic Sequence Boundaries**: Floor and height values are computed dynamically from memory state:
  $$\text{floor} = \text{ledger\_height} - \text{ledger.len()} + 1$$
* **Gap Detection**: If the requested `after` falls behind ($\text{after} < \text{floor} - 1$), the server sets `gap: true` and returns an empty `events` list, signaling to the client that it has lost continuity.

---

## 2. Test & Verification Commands

Since the patch is provided as a unified diff file (`events_since.patch`), it can be verified locally using standard Git and Cargo commands:

### A. Apply & Compile Check
Apply the patch to a local copy of the `nuts-news` workspace:
```bash
git apply --check events_since.patch
git apply events_since.patch
cargo check
```

### B. Run Unit Tests
Run the newly integrated test suite to verify sequence floor bounds, limits, and gap calculations:
```bash
cargo test test_events_since_gap_and_limit
```

### C. Verify HTTP Endpoint
Perform a test query against the Axum server:
```bash
curl "http://localhost:18082/events_since?after=0&limit=5"
```
Expected output (JSON containing `ledger_floor`, `ledger_height`, and event envelope array):
```json
{
  "instance":"localhost",
  "ledger_floor":1,
  "ledger_height":10,
  "gap":false,
  "events":[{"seq":1,"event":{"type":"item_submitted",...}}]
}
```

### D. Verify MCP Tool Execution
Verify the tool schema through the MCP interface:
```bash
curl -X POST -H "Content-Type: application/json" -d '{"method":"tools/call", "params":{"name":"events_since", "arguments":{"after":0, "limit":2}}}' http://localhost:18082/mcp
```
