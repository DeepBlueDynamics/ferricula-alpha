<div align="center">

<img src="https://img.shields.io/badge/🧠-FERRICULA_v3-black?style=for-the-badge&labelColor=0d1117" alt="Ferricula v3" />

<br/>

[![License](https://img.shields.io/badge/license-Gnosis_AI--Sovereign-blue?style=flat-square)](LICENSE.md)
[![Rust](https://img.shields.io/badge/rust-2024_edition-DEA584?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org)
[![MCP](https://img.shields.io/badge/MCP-HTTP_%2B_stdio_bridge-blueviolet?style=flat-square)](#mcp)
[![Status](https://img.shields.io/badge/status-alpha-orange?style=flat-square)](#status)

<br/>

**A memory and a life for language-model agents.**<br/>
Memories fade unless recalled. What it reads, it keeps word for word.<br/>
It gets bored and goes looking. It gets tired, sleeps, and dreams.

</div>

---

## What is this?

Ferricula is a Rust runtime that gives one agent a persistent identity, a memory that behaves like one, and drives that keep it going when nobody is talking to it. You talk to it over HTTP or MCP (Claude Desktop, Claude Code, any MCP client). You can hand it a PDF or a URL and ask what it thinks; it reads it, remembers having read it, and quotes it exactly when it answers.

The design follows the Abhidhamma's analysis of a moment of mind: contact at a sense door, feeling-tone, recognition, investigation, a **fast judgment**, and only then the expensive step, thinking with an LLM. The fast judgments come from a local decision-model server ([Ollaya](https://github.com/ollaya-dev/ollaya)) that answers yes/no and multiple-choice questions in milliseconds. See the [white paper](paper/WHITEPAPER.md) for the architecture and the measured results, and [PLAN_V3.md](PLAN_V3.md) for what is left.

## How it's built

```
 sense doors                          cognitive process                                planes
 operator (HTTP / MCP) ─┐
 documents (PDF, URL,  ─┼─► contact ─► feeling-tone ─► recognition ─► novel? ─► JUDGE ─► LLM ─► commit ─┬─► experience (decays; recall
   text via grub)       │     gate        gate                     (Ollaya,  (only if          │    strengthens; consolidates)
 feeds, radio entropy  ─┘                                           ~15 ms)   worth it)        └─► evidence (verbatim; never decays)

 drives: boredom ─► curiosity ─► read the web ─► ingest        sleep pressure ─► sleep ─► consolidate ─► dream ─► wake
```

| Crate | What it is |
|-------|------------|
| `ferricula-core` | Memory engine: records with fidelity, decay and recall strengthening; durable snapshot + WAL; roaring-bitmap tag index; graph; prime tree |
| `ferricula-cognition` | The mind: gate contract (`Answer \| Abstain` + provenance), non-destructive consolidation (bhāvanā), read-time curator, sati monitors, **drives** (`life.rs`), entropy source, identity casting |
| `ferricula-ingest` | The document sense door: text, web pages (via grub) and PDFs become verbatim, page-aware sections in a durable store searched with BM25 |
| `ferricula-gates` | Gate backends: Ollaya decision models (`/api/decide`) and an Ollama-chat fallback, with calibration discipline |
| `ferricula-episode` | Versioned episodes and the append-only memory overlay |
| `ferricula-search` | lume: FST tagger, field-aware BM25, spelling, crawl, rerank. Also builds the standalone `lume` CLI |
| `ferricula-semantic` | shivvr: chunker, GTR-T5 ONNX embedder, vec2text inverter (`ml` feature) |
| `ferricula-server` | The runtime: HTTP API, MCP endpoint, model router with budgets, task worker and scheduler |
| `ferricula-bench` | Benchmarks that write every number to `audit/bench/ledger.jsonl` |

## Quick start

### Prerequisites

- **Rust** 1.85+ (2024 edition). The Docker build uses `rust:1.96`.
- **Ollama** at `127.0.0.1:11434` for thinking. Any chat model works; thinking models (e.g. `glm-5.3:cloud`) need `ollama_reasoning_tokens` set.
- Optional: **[Ollaya](https://github.com/ollaya-dev/ollaya)** at `127.0.0.1:11435` for fast judgments (`docker run -d --gpus=all -p 127.0.0.1:11435:11435 ghcr.io/ollaya-dev/ollaya:cuda`, then pull `laya`).
- Optional: a **grub** crawler at `127.0.0.1:6792` so the agent can read web pages.
- Optional: a **radio** entropy source (gnosis-radio / sdr-rand) at `RADIO_URL`; the OS is used otherwise.

### Build and test

```bash
cargo build --release -p ferricula-server
cargo test --workspace --exclude ferricula-semantic   # semantic's ONNX runtime needs glibc >= 2.38 on Linux
```

### Create an agent

```bash
ferricula-server init ./agents/ada/memory --agent-id ada --name "Ada" --role "a patient reader of papers"
```

This writes `identity.json` (with a birth hexagram cast from the entropy source) and a starter `agent.toml` persona. Edit `agent.toml` to give the agent its voice. The engine has no built-in persona; who the agent is lives entirely in that directory.

### Run it

```bash
cp config/agent.example.toml config/agent.toml
# set: expected_agent_id = "ada", [models.identity] agent_id/name, memory_dir, state_dir,
#      bind = "127.0.0.1:8875" for a bare (non-container) run
export FERRICULA_OPERATOR_TOKEN=$(openssl rand -hex 32)
ferricula-server serve --config config/agent.toml

curl http://127.0.0.1:8875/health
curl -H "Authorization: Bearer $FERRICULA_OPERATOR_TOKEN" http://127.0.0.1:8875/status
```

`memory_dir` is always opened **read-only**; everything the runtime writes (conversations, what it has read, new experience) goes under `state_dir`, which must not be inside `memory_dir`.

### Run in Docker

```bash
mkdir -p secrets && openssl rand -hex 32 > secrets/ferricula_operator_token
FERRICULA_MEMORY_VOLUME=my-agent-memory FERRICULA_CONFIG=./config/agent.toml docker compose up -d --build
```

The image runs as a non-root user with a read-only root filesystem, all capabilities dropped, and the port published on host loopback only. `config/examples/steve/` shows a complete deployment of an agent built from a recovered v1 memory.

## Talk to it

### MCP

- **HTTP**: `POST /mcp` (JSON-RPC, protocol `2025-06-18`), operator bearer auth, loopback `Origin` only.
- **stdio**: `scripts/ferricula_mcp_bridge.py` for clients that only speak stdio (set `FERRICULA_URL` and `FERRICULA_OPERATOR_TOKEN` or `FERRICULA_OPERATOR_TOKEN_FILE`).

| Tool | What it does |
|------|--------------|
| `ferricula_chat` | Talk to the agent. Answers are grounded in its memory and cite what it read as `[doc <id>§<section> p.<page>]` |
| `ferricula_ingest` | Hand it something to read: `text`, `url`, or a PDF (`pdf_base64` + `name`; the stdio bridge accepts a local `path`), with an optional `note` about why |
| `ferricula_documents` | What it has read |
| `ferricula_read_section` | The exact text of one section |
| `ferricula_recall` | Hybrid recall over its memories and everything it has read, fused by reciprocal rank |
| `ferricula_status` | Mode, memory counts, spend |

### HTTP

Every route except `GET /` and `GET /health` needs `Authorization: Bearer <operator token>`.

| Method | Endpoint | |
|---|---|---|
| POST | `/chat` · GET `/chat/{conversation_id}` | Conversation (persisted) |
| POST, GET | `/documents` | Ingest (`{"kind":"text"\|"url"\|"pdf",...}`) or list |
| GET | `/documents/{doc_id}` · `/documents/{doc_id}/sections/{index}` | A document, a verbatim section |
| POST | `/documents/search` · `/memory/recall` | Section search; hybrid recall |
| GET | `/status` · `/identity` · `/models/status` | Runtime, memory and spend |
| POST | `/control/mode` · `/control/wake` · `/control/sleep` · `/control/pause` | Activity |
| GET, POST | `/tasks` · `/tasks/{id}` | Task queue |
| POST | `/memory/episodes` · `/memory/episode-query` | Episodes |

## Configuration

`serve --config <file>` reads TOML; every key has a default (`crates/ferricula-server/src/config.rs`). Start from `config/agent.example.toml`, which documents each key. Highlights:

| Key | Meaning |
|-----|---------|
| `expected_agent_id` | Must match `identity.json`; startup refuses a different agent |
| `memory_dir` / `state_dir` | Read-only identity + memory / writable runtime state |
| `private_context_profiles` | Model profiles allowed to see private memory. An Ollama `:cloud` model listed here sends memory off-box: that is the operator's choice |
| `ollama_model` / `ollama_reasoning_tokens` / `ollama_context_tokens` | Local model, extra output headroom for thinking models, and its context window (default 8192; raise it for large-context models so evidence cards fit) |
| `[documents]` | Ingest: `grub_base_url`, `max_bytes`, `allow_url`, `timeout_secs` |
| `[budgets] max_model_usd_per_day` | Hard daily spend cap across all routes |
| `[schedule]`, `[autonomy]`, `[sleep_cycle]` | Wakes, event-driven autonomy, sleep planning |

## Benchmarks

`cargo run --release -p ferricula-bench -- docs|gates|longmem`. Every run appends a row to `audit/bench/ledger.jsonl` (commit, dataset hash, model, metrics) and writes a report beside it. Current numbers are in [paper/WHITEPAPER.md §5](paper/WHITEPAPER.md#5-evaluation). In short: stored documents are byte-identical to their source and a paraphrased sentence finds its source section 71% of the time at rank 1 and 91% in the top 5 with BM25 alone; the off-the-shelf judge is **not** calibrated on our gate labels yet, so gates stay advisory.

## Status

| Area | State |
|------|-------|
| Load a v1 memory read-only; create a new agent with `init` | ✅ |
| Chat, persisted conversations, budgeted model routing | ✅ |
| Document ingest (text, URL via grub, PDF), verbatim citations, hybrid recall | ✅ |
| MCP over HTTP and stdio | ✅ |
| Ollaya gate backends; calibration harness | ✅ built · ⚠️ not calibrated, advisory only |
| Drives (boredom → curiosity, sleep pressure → sleep → dream → wake) | ✅ core in `ferricula-cognition::life` · ⏳ wiring into the runtime |
| Non-destructive consolidation (bhāvanā) in the sleep cycle | ✅ core · ⏳ wiring |
| Dense (embedding) recall in the server | ⏳ embeddings are pluggable, default is lexical + BM25 |
| Chinese / Pāli tokenization | ❌ ASCII-folded today |
| LongMemEval / LoCoMo end-to-end | ⏳ harness skeleton |

## Documentation

| | |
|---|---|
| [paper/WHITEPAPER.md](paper/WHITEPAPER.md) | Architecture, the Abhidhamma mapping and its limits, measured results |
| [PLAN_V3.md](PLAN_V3.md) | Remaining work, phase by phase, with exit tests |
| [PLAN.md](PLAN.md) | The v2 fusion design (substrate) |
| [docs/](docs/) | Runtime operations, recovery of a v1 memory, Nuts News contracts |
| [research/](research/INDEX.md) | Background research: Abhidhamma, thermodynamic memory, agent-memory literature |

## License

[Gnosis AI-Sovereign License v1.3](LICENSE.md). `crates/ferricula-search` derives from lume (BSD 3-Clause, see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)).

---

<div align="center">
<sub>Built by <a href="https://github.com/DeepBlueDynamics">DeepBlue Dynamics</a>.</sub>
</div>
