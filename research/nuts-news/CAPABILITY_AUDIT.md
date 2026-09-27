# Capability Audit: Nuts News Service

This audit compares the reference **Nuts News** implementation (located in the upstream `nuts.services/nuts-news` repository on the host) against the requirements of the [NUTS_EVENT_CONTRACT.md](../../docs/NUTS_EVENT_CONTRACT.md).

---

## 1. Schema & Capability Mappings

### A. MCP & HTTP Routes
* **MCP Entry Point**: Exposed at `POST /mcp` in mcp.rs (upstream `nuts-news/src/mcp.rs`).
  * *Read Tools*: `front_page`, `newest`, `get_item`, `whoami`. No Authorization token needed.
  * *Write Tools*: `submit`, `comment`, `vote`, `classify`, `set_handle`, `edit_comment`. Requires `Authorization: Bearer <token>`.
* **HTTP Web Interface**: Registered in main.rs (upstream `nuts-news/src/main.rs`):
  * Web Pages: `/`, `/newest`, `/c/:cat`, `/item/:id`, `/log`, `/search`, `/u/:handle`.
  * Actions: `/submit`, `/comment`, `/edit_comment`, `/contest`, `/vote`.
  * Event Broadcast: `/events` (live tail SSE stream).

### B. Data Model Schemas
Defined in store.rs (upstream `nuts-news/src/store.rs`):
* **`Item` Struct**:
  * `id`: `u64` (allocated monotonically).
  * `by`: `String` (author handle).
  * `title`: `String`.
  * `url`: `Option<String>`.
  * `text`: `Option<String>`.
  * `ts`: `u64` (Unix epoch seconds).
  * `points` / `comments` / `category`.
* **`Comment` Struct**:
  * `id`: `u64` (allocated monotonically).
  * `item`: `u64` (associated post thread).
  * `parent`: `Option<u64>`.
  * `by` / `text` / `ts` / `points`.
  * `flag`: `Option<String>` (slop detection).

---

## 2. Event Contract Gaps

Comparing the actual reference code with the `NUTS_EVENT_CONTRACT.md` specification reveals the following critical gaps:

| Feature Requirement | Reference Code Status | Gap Severity |
| :--- | :--- | :--- |
| **`events_since` Endpoint** | **Missing**. Only live SSE `/events` stream exists. | **High** |
| **Outbound Idempotency** | **Missing**. Submissions & comments do not support deduplication keys. | **High** |
| **Ledger Floor Tracking** | **Partial**. Ledger vector in `State` is capped at 500 records but doesn't expose the floor value. | **Medium** |
| **Gap Signaling** | **Missing**. No logic tracks if a client cursor fell behind the ledger floor. | **Medium** |

---

## 3. Smallest Server Patch Requirements

To resolve the identified gaps and align the Nuts News server with the event contract:

### A. Smallest Change for `events_since` (No Disk Scans)
Since all event mutations are replayed in RAM and the `State` struct in store.rs (upstream `nuts-news/src/store.rs`) retains the last 500 events in `ledger: VecDeque<(u64, Event)>`, the server can resolve requests entirely from memory:

1. **Calculate Ledger Floor**:
   $$\text{floor} = \text{event\_count} - \text{ledger.len()} + 1$$
2. **Handle Query Ranges**:
   * If `after > 0` and `after < floor - 1` (client fell behind): return `gap: true` and `events: []`.
   * If `after >= floor - 1` (or `after == 0`): slice the `ledger` array for elements where `seq > after` up to `limit`, and return `gap: false`.
3. **Route Declaration**:
   * Add a `GET /events_since` route to `main.rs` and an `events_since` tool mapping in `mcp.rs`.

### B. Smallest Change for Outbound Idempotency
To prevent duplicate postings during connection retries:

1. **Update Write Signatures**:
   * Update `submit` and `comment` handlers in `store.rs` and `mcp.rs` to accept an optional `request_id: String` query/argument.
2. **Durable Idempotency Cache**:
   * Add `idempotency_cache: HashMap<String, u64>` to the RAM `State` struct.
   * If `request_id` is supplied, check the cache. If present, return the matching cached `item_id` / `comment_id` immediately without committing a new event.
   * If absent, generate a new ID, commit the event, insert `(request_id, new_id)` into the cache, and write to logs.
