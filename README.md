<div align="center">

<img src="https://img.shields.io/badge/🧠-FERRICULA_v2-black?style=for-the-badge&labelColor=0d1117" alt="Ferricula v2" />

<br/>

[![License](https://img.shields.io/badge/license-Gnosis_AI--Sovereign-blue?style=flat-square)](LICENSE.md)
[![Rust](https://img.shields.io/badge/rust-2024_edition-DEA584?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org)
[![MCP](https://img.shields.io/badge/MCP-HTTP_%2B_stdio_bridge-blueviolet?style=flat-square)](#mcp)
[![Status](https://img.shields.io/badge/status-alpha-orange?style=flat-square)](#status-what-is-and-isnt-wired)

<br/>

**Thermodynamic memory engine and sovereign agent runtime.**<br/>
Memories decay. Recall strengthens. Dreams consolidate. One agent decides.

</div>

---

## What is this?

`ferricula-alpha` is the v2 workspace of [ferricula](https://github.com/DeepBlueDynamics/ferricula). It brings three
DeepBlue Dynamics projects into one Rust workspace:

- **ferricula**: thermodynamic agent memory (decay, dreams, resonance gates, identity)
- **lume**: FST/BM25 hybrid document search
- **shivvr**: semantic chunking, GTR-T5 embeddings, vec2text inversion

All three codebases are vendored here as crates. The design for merging them
into a single process is in **[PLAN.md](PLAN.md)**. That merge is **not
finished**: today the runnable product is `ferricula-server`, a runtime for one
agent (Steve) built on the memory core and cognition crates. Search and semantic
exist as standalone crates that the server does not yet use. See
[Status](#status-what-is-and-isnt-wired).

## Workspace

| Crate | What it is | Used by the server |
|-------|------------|:------------------:|
| `ferricula-core` | Memory engine: `MemoryRecord` lifecycle and decay, `DurableEngine` (postcard snapshot + WAL, reads legacy `snapshot_v4.bin`), graph, prime tree, SKG | ✅ |
| `ferricula-cognition` | Dreams, identity and casting, entropy clock, planner, emotion, sati monitor, Advocate, Wisdom-King council, gate traits | ✅ |
| `ferricula-episode` | Versioned episodes and the append-only memory overlay | ✅ |
| `ferricula-server` | Steve runtime: axum HTTP API, MCP endpoint, task worker and scheduler, budgeted model router, Hacker News and Nuts News reads | binary |
| `ferricula-search` | Vendored lume: FST tagger, BM25, hybrid search, spelling, crawl, rerank. Builds the standalone `lume` CLI | ❌ |
| `ferricula-semantic` | Vendored shivvr: chunker, GTR-T5 ONNX embedder, vec2text inverter, per-agent vector crypto (`ml` feature, on by default) | ❌ |
| `ferricula-gates` | Ollama-compatible chat backends for cognition's decision gates | ❌ |

## Architecture (as built)

```
                     ferricula-server  (one process, :8875)
┌───────────────────────────────────────────────────────────────────┐
│  axum router ── operator Bearer auth ── POST /mcp (JSON-RPC)      │
│      │                                                            │
│      ├── SteveRuntime ─── worker (task queue)                     │
│      │        │      └─── scheduler (60 s tick: scheduled wakes,  │
│      │        │                      deferred mentions)           │
│      │        ├── memory_dir  (read-only: identity, snapshot, WAL)│
│      │        └── state_dir   (runtime state, usage, episodes,    │
│      │                         overlay, conversations)            │
│      └── ModelRouter ── per-task routes, capabilities, USD budget │
└──────────┬──────────────────────┬──────────────────────┬──────────┘
           ▼                      ▼                      ▼
   Ollama (/v1, local)     Anthropic API (opt.)   Nuts News MCP / HN API
```

Recovered memory is opened **read-only**. Everything the runtime writes goes
under `state_dir`, and startup fails if `state_dir` sits inside `memory_dir`.

## Quick Start

### Prerequisites

- **Rust** 1.85+ (2024 edition). The Docker build uses `rust:1.96`.
- **Ollama** at `127.0.0.1:11434` for chat and deliberation. The default model is `gemma4:e2b`.
- **Docker** (optional) for the hardened container.

### Build and test

```bash
# Server and the crates it depends on (no ONNX download needed)
cargo build --release -p ferricula-server
cargo test -p ferricula-core -p ferricula-cognition -p ferricula-episode -p ferricula-server

# Everything. Note: ferricula-semantic's default `ml` feature pulls ONNX Runtime
# binaries at build time; on Windows use --no-default-features for that crate.
cargo test --workspace
```

### Inspect a memory directory (read-only)

```bash
cargo run -q -p ferricula-server -- inspect path/to/memory-dir --json
```

This loads `identity.json`, `snapshot_v4.bin` and `wal.log` and prints counts
(lifecycle states, keystones, graph, prime tree). It never checkpoints or writes.

### Run the server

```bash
export FERRICULA_OPERATOR_TOKEN=$(openssl rand -hex 32)
cargo run --release -p ferricula-server -- serve --config config/steve.isolated.toml
curl http://127.0.0.1:8875/health
curl -H "Authorization: Bearer $FERRICULA_OPERATOR_TOKEN" http://127.0.0.1:8875/status
```

Edit `memory_dir` and `state_dir` in the config for a local run. The shipped
configs use container paths.

### Run in Docker

Both compose files expect two things to exist first:

1. `secrets/ferricula_operator_token` (gitignored)
2. the external Docker volume `steve-jobs-data-recovery-20260713`, which holds the recovered memory

```bash
docker compose up -d --build                                               # compose.yaml → :8875
docker compose -f compose.isolated.yaml -p ferricula-v2-steve-test up -d   # prebuilt image → :18875
```

The image runs as a non-root user with a read-only root filesystem, all
capabilities dropped, and ports published on host loopback only.

## Configuration

The server reads TOML with `serve --config <file>`. Every key has a default
(`crates/ferricula-server/src/config.rs`).

| Key | Default | Meaning |
|-----|---------|---------|
| `bind` | `127.0.0.1:8875` | Listen address. A non-loopback bind requires auth. |
| `memory_dir` | `.runtime/steve-jobs` | Recovered memory, opened read-only |
| `state_dir` | `.runtime/steve-runtime` | Runtime-owned writable state |
| `expected_agent_id` | `ferricula-stevejobs` | Startup check against `identity.json` |
| `operator_token_env` | `FERRICULA_OPERATOR_TOKEN` | Env var holding the operator Bearer token |
| `require_operator_auth` | `true` | If the token env var is unset, all requests are denied |
| `private_context_profiles` | — | Model profiles allowed to see private memory |
| `initial_mode` | `asleep` | See [Activity model](#activity-model) |

The tables are `[schedule]`, `[budgets]`, `[nutnews]`, `[models]`, `[autonomy]`,
`[sleep_cycle]`, `[mentions]`, `[overlay]`, `[emotion]` and `[advocate]`.

Two configs ship:

- **`config/steve.isolated.toml`**: paused, schedule/autonomy/Nuts News off, no model may see private memory. Safe for load testing.
- **`config/steve.example.toml`**: asleep with scheduled wakes. ⚠️ It sets `ollama_model = "glm-5.2:cloud"` while `local_ollama` is in `private_context_profiles`, which sends private memory to a cloud-proxied model. Review before use.

### Model routing

The model router picks a profile for each task class, checks required
capabilities and enforces `budgets.max_model_usd_per_day`. Default profiles:

| Profile | Provider | Model | Capabilities |
|---------|----------|-------|--------------|
| `no_model` | none | — | — |
| `local_ollama` | OpenAI-compatible | `gemma4:e2b` | chat, local, private context |
| `anthropic_haiku` | Anthropic | `claude-haiku-4-5` | needs `ANTHROPIC_API_KEY` |
| `anthropic_sonnet` | Anthropic | `claude-sonnet-4-6` | needs `ANTHROPIC_API_KEY` |

Chat requires chat, private context and local, so in practice it runs on Ollama.

## Activity model

Modes are set with `initial_mode` or `POST /control/mode`:

- **`paused`** (alias `off`): hard stop. The worker claims nothing and the scheduler queues no wakes.
- **`asleep`** (alias `sleep`): quiet. Scheduled wakes (`schedule.wakes_per_day`, with jitter) can still be queued.
- **`peripheral`**, **`engaged`** (aliases `wake`, `awake`), **`deliberative`**: increasing levels of engagement.

All modes except `paused` let the worker run queued tasks.

`[autonomy]` enables a newer event-driven state machine (asleep → awake →
deliberating → dreaming) that works under bounded leases. It is off by default
in both configs. The plain scheduler and worker run either way.

## HTTP API

Every route except `GET /` and `GET /health` requires `Authorization: Bearer <operator token>`.

### Runtime and control

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | `ok`, `agent_id`, `mode` (no auth) |
| GET | `/status` | Mode, queue, schedule, memory counts, model spend, feature states |
| GET | `/identity` | Memory inspection: lifecycle counts, keystones, graph, prime tree |
| POST | `/control/mode` · `/control/wake` · `/control/sleep` · `/control/pause` | Change activity mode |
| POST | `/control/schedule` | Enable or disable scheduled wakes |
| GET | `/control/autonomy` · `/control/schedule/plan` · `/control/advocate` | Read-only status projections |

### Tasks

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET, POST | `/tasks` | List (max 64) or enqueue |
| GET | `/tasks/{id}` | One task |
| POST | `/tasks/read-feed` · `/tasks/read-hacker-news` · `/tasks/mention` | Enqueue a feed read or a mention |
| GET | `/tasks/considerations` | Mention consideration counts |

### Memory and conversation

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/memory/recall` | **Lexical** token-coverage recall over recovered memory (not dense/semantic yet) |
| POST | `/memory/episodes` · `/memory/episode-query` | Commit or query versioned episodes |
| GET | `/memory/overlay` | Overlay status |
| POST | `/memory/overlay/approve/{id}` | Not implemented yet: always returns `501` |
| GET | `/` | Operator chat page |
| POST | `/chat` · GET `/chat/{conversation_id}` | Operator chat, grounded in recall and routed to the model |
| GET | `/models/status` | Router profiles and spend |
| POST | `/wisdom/preview` | Preview the Wisdom-King whispers for a prompt |

## MCP

- **HTTP**: `POST /mcp` runs JSON-RPC (protocol `2025-06-18`, JSON responses, no SSE) with operator auth and a loopback-only `Origin` check. Tools: `steve_status`, `steve_recall`, `steve_chat`.
- **stdio**: `scripts/steve_mcp_bridge.py` wraps the HTTP API for clients that only speak stdio. It adds `steve_identity`. Its `steve_chat` calls Ollama directly rather than the server's `/chat`, so turns are not persisted. `scripts/verify_steve_stdio.py` smoke-tests the bridge.

## Wisdom Kings

v2 has **one** sovereign internal agent. The five archetypes from v1
(Intuition, Fortune, Craft, Ethics, Advocate) are bounded perspectives that
whisper into that agent's deliberation. They own no memory, credentials, tools,
schedule or public identity. See PLAN.md §5a.

## Nuts News

Nuts News reads are off unless `nutnews.enabled = true`, and writes are off
unless `nutnews.allow_writes = true`. Mention handling treats every mention as
something to consider, never an obligation to reply. See
[`docs/NUTS_EVENT_CONTRACT.md`](docs/NUTS_EVENT_CONTRACT.md) and
[`docs/NUTS_MENTION_THREAT_MODEL.md`](docs/NUTS_MENTION_THREAT_MODEL.md).

## Status: what is and isn't wired

| Area | State |
|------|-------|
| Load legacy v1 memory (v4 snapshot + WAL) read-only | ✅ |
| Runtime modes, scheduler, task queue, persistent state | ✅ |
| Model router with capabilities and daily USD cap | ✅ |
| Operator chat, mention deliberation, sleep reflection via model | ✅ |
| HTTP MCP (3 tools) and stdio bridge | ✅ |
| Recall | ⚠️ lexical only; semantic recall awaits `ferricula-semantic` wiring |
| New memory writes to the recovered volume | ❌ by design (read-only mount, overlay approval returns 501) |
| lume search inside the server | ❌ `ferricula-search` is standalone (`lume` CLI) |
| In-process embeddings / chunking / inversion | ❌ `ferricula-semantic` not yet used by the server |
| `ferricula-gates` chat gate backends | ❌ not used by any crate yet |
| Nuts News `events_since` poller, outbound comments | ❌ written, not wired |
| `LlmAgent::advise` (cognition) | ❌ stub |

## Documentation

| Doc | Contents |
|-----|----------|
| [PLAN.md](PLAN.md) | Fusion design, crate map, kill list and phases (target state, not current state) |
| [docs/STEVE_RUNTIME.md](docs/STEVE_RUNTIME.md) | Deployment, volumes, modes, auth, model config, backup and restore |
| [docs/STEVE_RECOVERY.md](docs/STEVE_RECOVERY.md) | How Steve's v1 memory volume was recovered and verified |
| [docs/STEVE_INTEGRATION_ACCEPTANCE.md](docs/STEVE_INTEGRATION_ACCEPTANCE.md) | Acceptance matrix |
| [docs/AGENT_HARNESS.md](docs/AGENT_HARNESS.md) | Model-backed task harness |
| `scan/` | Read-only surveys of the three source projects |
| `research/` | Background research (Abhidharma, thermodynamic memory, related papers) |

## License

Licensed under the [Gnosis AI-Sovereign License v1.3](LICENSE.md). Code in
`crates/ferricula-search` is derived from lume and attributed under BSD 3-Clause
in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Code derived from ferricula
and shivvr is same-author code covered by the workspace license.

---

<div align="center">
<sub>Built by <a href="https://github.com/DeepBlueDynamics">DeepBlue Dynamics</a> &mdash; Thermodynamic memory for the sovereign mind.</sub>
</div>
