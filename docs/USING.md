# Using Ferricula

Day-to-day use of a running agent. Installation is in [INSTALL.md](INSTALL.md). The detailed operator reference (examples against the live Steve) is [OPERATOR_GUIDE.md](OPERATOR_GUIDE.md).

Every route except `GET /` and `GET /health` needs `Authorization: Bearer <operator token>` (see INSTALL §4). Login is planned to move to nuts.services accounts (`inbox/UI_PLAN.md`); today it's the bearer token. Examples assume:

```bash
URL=http://127.0.0.1:8875
TOKEN="$(cat ~/.config/ferricula/operator_token)"
AUTH="Authorization: Bearer $TOKEN"
```

## Talking to it

- **Browser:** open `/`, paste the token, and chat.
- **HTTP:** `POST /chat` with `request_id` (new UUID each message), `conversation_id`, `message` (at most 8,192 bytes) and `reported_origin` (`human`, `agent`, `scheduler`, `tool` or `unknown`). **Reuse the same `conversation_id` for a whole conversation**: a new id per message loses the thread. `GET /chat/{conversation_id}` returns the stored turns.
- One turn runs at a time; a second concurrent request gets "conversation busy".
- A turn records what the model saw: memory candidates, document evidence cards, and `tool_calls`. Every turn is also remembered as experience, so later conversations can recall it.

## Giving it things to read

`POST /documents` with one of:

```json
{ "kind": "text", "title": "...", "text": "...", "note": "why I'm giving you this" }
{ "kind": "url",  "url": "https://...", "note": "..." }
{ "kind": "pdf",  "pdf_base64": "...", "name": "paper.pdf", "note": "..." }
```

- Documents are kept verbatim, split into sections, and cited as `[doc <doc_id>§<section> p.<page>]`.
- A `doc_id` is a content hash, so the same text always gets the same id and an edited text is a new document.
- Paywalls, login walls and near-empty pages are rejected.
- `GET /documents` lists what it has read. `GET /documents/{doc_id}` and `GET /documents/{doc_id}/sections/{index}` return exact text. `POST /documents/search {"query", "k", "doc_id"?}` is BM25 search.

## What it can do inside a conversation

In each message the agent can make up to 4 tool calls before answering:
- `search_documents`, `read_section` and `read_document` look things up in its documents.
- `search_memory` looks things up in its memories.
- `mark_disputed` is its one write: a permanent verdict that one of its memories is disputed, or superseded by evidence it was shown. The old memory is never changed; the verdict is shown with it from then on.

A reply that cites a document it wasn't shown this turn is sent back once for correction. The full contracts, limits and errors are in [TOOLS.md](TOOLS.md).

## Recall

`POST /memory/recall {"query": "...", "limit": N}` runs hybrid recall over the recovered memory, the experience store and document sections: word match, BM25, meaning (with shivvr) and one graph hop, fused by reciprocal rank. Faded memories are included when `[recall] include_faded_recovered = true`.

## Its life

With `[life] enabled = true` the agent has drives:
- **Boredom** rises when nothing happens; at the threshold it follows its curiosity: it searches with grub, reads a few pages and writes a reflection.
- **Sleep pressure** rises with waking time and work; at the threshold it sleeps, consolidates and dreams.
- An operator message always wakes it.

| Route | What it does |
|---|---|
| `GET /life?journal=N` | Phase (`resting`, `engaged`, `asleep`, `meditating`), boredom, sleep pressure, and the last N journal entries (excursions, sleeps, consolidations, dreams) |
| `POST /life/urge {"urge": "follow_curiosity" \| "sleep" \| "dream"}` | Force one now, within budgets |
| `POST /life/meditate` · `POST /life/end-meditation` | Hold the drives; ring the bell. There's no automatic bell yet. |
| `POST /control/wake` · `/control/sleep` · `/control/mode` · `/control/pause` | Activity control |

Budgets are hard limits: `[budgets] max_model_usd_per_day` caps all spending, and `[life] max_model_calls_per_day` caps autonomous calls. Curiosity also has a daily cap and a cooldown.

## Status

`GET /status` (mode, memory counts, spend), `/identity`, `/models/status`, `/meaning` (meaning index), `POST /embeddings/probe`.

## MCP

`POST /mcp` (JSON-RPC, protocol `2025-06-18`, bearer auth, loopback `Origin` only), or `scripts/ferricula_mcp_bridge.py` for stdio-only clients. Tools:
- `ferricula_chat`
- `ferricula_ingest`
- `ferricula_documents`
- `ferricula_read_section`
- `ferricula_recall`
- `ferricula_status`
- `ferricula_life`

`steve_chat`, `steve_recall` and `steve_status` remain as older aliases.

## Searching this repository's documents

The `lume` CLI (from `crates/ferricula-search`) builds a BM25 index of a directory's `.md`, `.pdf`, `.txt`, `.html` and code files. It honours a `.lumeignore`.

```bash
cargo run --release -p ferricula-search --bin lume -- index <dir> --db <dir>/.lume-index    # -s adds the shivvr semantic layer
cargo run --release -p ferricula-search --bin lume -- index update <dir> --db <dir>/.lume-index
cargo run --release -p ferricula-search --bin lume -- search "your query" --db <dir>/.lume-index
```

A single index over the planning documents (`plan/.lume-index`) is planned in `inbox/PLAN_CONSOLIDATION.md`. The exact argument order for `index update` wasn't checked against a live run.

## Benchmarks

```bash
cargo run --release -p ferricula-bench -- docs|gates|recall|longmem [--limit N] [--seed S] [--out audit/bench]
```

| Suite | Measures | Key options |
|---|---|---|
| `docs` | Finding the source section of verbatim, partial and paraphrased sentences (BM25, dense, hybrid) | `--corpus research`; env `BENCH_SHIVVR_URL` (`none` skips dense) |
| `gates` | Judge accuracy and calibration (ECE) on labeled sets | `--datasets research/gates/datasets --url http://127.0.0.1:11435 --model laya` |
| `recall` | Recall@k. With no flags: the synthetic set (`research/bench/synthetic/`, invented memories, built into a temp store). For a real agent: `--memory <copy of its memory dir> --queries research/bench/private/<set>.json` (the private folder is gitignored; a real person's memories never go in the repo) | `[--memory <dir> --queries <file>]` |
| `longmem` | LongMemEval (skeleton) | `--dataset …` or env `LONGMEMEVAL_PATH` |

Every run appends a row to `audit/bench/ledger.jsonl` (commit, dataset hash, model, metrics) and writes a report beside it. What will count as credible is set out in the benchmark plan (`inbox/`).

## Where state lives

Under `state_dir` (the runtime volume):

| Path | Contents |
|---|---|
| `operator-conversations.json` | Every chat turn |
| `documents/` | The verbatim document store |
| `experience/` | New memories: conversations, readings, reflections, dreams, verdicts |
| `life/` | `drives.json`, `counters.json`, `journal.jsonl`, `bhavana-state.json`, `karmic.jsonl` |
| Meaning index | Embeddings sidecar (exact file names not listed here) |

`memory_dir` (the recovered base) is never written.
