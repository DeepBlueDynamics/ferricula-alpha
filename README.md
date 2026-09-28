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

_Drafted by the flagship agent (Steve) from its own documents on 2026-09-27, lightly corrected for accuracy by the engineer._

Ferricula is a memory engine and cognition stack for a software agent. What arrives
through its sense doors — operator messages, documents (text, web pages via the grub
crawler, PDFs), feeds, radio entropy — commits to two planes: experience, which
decays and is strengthened by recall, and evidence, which is kept verbatim and never
decays. Recall is hybrid — word match, BM25, meaning (embeddings), and one graph
hop — and conversations are remembered across sessions. Background drives shape the
agent's behavior: boredom leads it to read on its own, sleep pressure leads to
consolidation and a dream, meditation holds it, and entropy comes from an SDR radio.
Decision gates judge whether something is worth deliberating on. They run on local Ollaya
models (10–18 ms), with hosted JEV (TypeSafe) as a second tier (about 200–300 ms). They are
not yet calibrated, so they are advisory only.

The design follows the Abhidhamma's analysis of a moment of mind (contact, feeling-tone, recognition, investigation, a fast judgment, and only then the expensive step, thinking with an LLM). See the [white paper](paper/WHITEPAPER_V2.md) for the architecture and measured results, [inbox/PLAN_V3.md](inbox/PLAN_V3.md) and [docs/BACKLOG.md](docs/BACKLOG.md) for what is left, and [docs/HANDOFF.md](docs/HANDOFF.md) to pick up the work.

## What's new (2026-09-28)

- **The agent uses tools mid-conversation.** It can:
  - search and read its documents end to end, and search its own memories;
  - read a web page without keeping it, or ingest one with a stated reason.

  A reply that cites a document the agent hasn't read is sent back for correction ([docs/TOOLS.md](docs/TOOLS.md)).
- **Verdicts on its own memory.** `mark_disputed` writes a permanent `disputes` or
  `supersedes` record with its evidence, and the verdict comes back with the memory
  whenever it's recalled. The judged memory is never edited.
- **JEV as a second gate tier.** TypeSafe's hosted decision model sits next to Ollaya.
  - In *backup* mode it judges when Ollaya can't (sidecar down, or the state overflows its 512-token window); in *primary* mode it judges first.
  - The operator sets the key on `/settings`; it's stored on the state volume and never returned or logged.
  - States built from private conversation reach JEV only with `private_context` on.
  - Every gate decision records which tier judged and why ([inbox/JEV_PLAN.md](inbox/JEV_PLAN.md)).
- **`/talk` and live telemetry.** A chat page with an instruments panel and nuts.services
  login. `POST /chat/stream` streams a turn as it happens: memory candidates, model rounds
  (time, finish reason, tokens), tool calls, gate decisions and verdicts.
- **Operations.**
  - One log line per turn, model round, tool call and provider call.
  - A configurable per-call model timeout (`ollama_timeout_ms`).
  - Sleep pressure is capped, so one busy day can't demand days of sleep, and `/control/sleep` now runs a real sleep, with consolidation and a dream.

## What was new in v3 (2026-09-27)

- **Documents** — the agent can be handed text, a web page (via the grub crawler),
  or a PDF; content is kept verbatim and cited as `[doc <id>§<section> p.<page>]`;
  paywalls and login walls are rejected.
- **Recall by meaning** — shivvr GTR-T5 embeddings; 2,802 zero vectors backfilled;
  recall now fuses word match, BM25, meaning, and one graph hop.
- **Recall quality** — Recall@5 over the agent's memory went from 0.133 to 0.500;
  0.600 with faded memories, which are now enabled.
- **Conversations are remembered across sessions.**
- **Drives** — boredom leads the agent to read on its own; sleep pressure leads to
  consolidation and a dream; meditation holds it; entropy comes from an SDR radio.
- **Judges** — Ollaya decision gates: fast (10–18 ms) but not yet calibrated, so
  advisory only.
- **Built, not yet wired** — live thermodynamics; the vīthi record with the 52
  cetasikas; Paṭṭhāna edges; meditation with a bell and breath; dream images via
  ComfyUI.
- **Benchmarks** — every number lives in `audit/bench/ledger.jsonl`; the plan is in
  [docs/BENCH_PLAN.md](docs/BENCH_PLAN.md) and the v3 benchmark plan PDF.

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
| `ferricula-gates` | Gate backends: Ollaya decision models (`/api/decide`), hosted JEV (`/v1/systemone`, bearer auth, priced per input token) on the same questions, and an Ollama-chat fallback, with calibration discipline |
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
- Optional: a **TypeSafe JEV** key for the hosted gate tier, entered on the agent's `/settings` page (or `TYPESAFE_API_KEY`).
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
mkdir -p ~/.config/ferricula && openssl rand -hex 32 > ~/.config/ferricula/operator_token
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
| POST | `/chat/stream` | The same turn as server-sent events: candidates, rounds, model calls, tools, gates, verdicts, then `done` |
| GET | `/talk` · `/settings` | Chat page with instruments · operator settings (JEV key and routing) |
| GET, POST | `/settings/jev` · POST `/settings/jev/probe` | JEV gate tier status and settings (never the key) · one live test call |
| GET | `/life` · POST `/life/urge` | Drives, journal, last dream · force curiosity, sleep or a dream |
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
| `ollama_timeout_ms` | Per-call model timeout (default 120000). A thinking model on a long tool-loop turn can need more; the reference deployment uses 300000 |
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
| JEV (TypeSafe) second gate tier, backup or primary, key from the UI | ✅ live on both gates (curiosity, speak/write) · ⏳ low-confidence escalation, merge gate, cascade curve |
| In-chat tools, citation check, `mark_disputed` verdicts | ✅ |
| `/talk` page, nuts.services login, `POST /chat/stream` | ✅ server · ◐ the page does not consume the stream yet |
| Drives (boredom → curiosity via the web, sleep pressure → sleep → dream → wake), `[life]`, `/life`, `ferricula_life` | ✅ live-soaked on a recovered memory ([audit/life](audit/life/soak-2026-09-27.md)) · ⏳ 72-hour soak |
| Non-destructive consolidation (bhāvanā) in the sleep cycle | ◐ runs on a scratch copy; clusters and karmic log persist, nothing written back yet |
| Dense (embedding) recall: meaning index, backfill, dense + graph arms in hybrid recall, turns remembered across conversations | ◐ R2b recall landed; sati-recall gate, saññā tags, consolidation clusters still planned: [docs/EMBEDDINGS_PLAN.md](docs/EMBEDDINGS_PLAN.md) |
| Chinese / Pāli tokenization | ❌ ASCII-folded today |
| LongMemEval / LoCoMo end-to-end | ⏳ harness skeleton |

## Documentation

| | |
|---|---|
| [docs/INSTALL.md](docs/INSTALL.md) | Install, build, create an agent, run it (compose or `docker run`), upgrade, troubleshoot |
| [docs/USING.md](docs/USING.md) | Talking to it, giving it documents, its life, recall, MCP, searching with lume, benchmarks |
| [docs/OPERATOR_GUIDE.md](docs/OPERATOR_GUIDE.md) | Operator reference with worked examples against a live agent |
| [docs/TOOLS.md](docs/TOOLS.md) | The agent's in-conversation tools: contracts, limits, errors |
| [DOCUMENTS.md](DOCUMENTS.md) | How this repository's documents are organized (plans, docs, evidence), and the discussion on an agent stewarding them |
| [paper/WHITEPAPER.md](paper/WHITEPAPER.md) | Architecture, the Abhidhamma mapping and its limits, measured results |
| [inbox/](inbox/) | Plans, until the `plan/` directory exists: `PLAN_V3.md` (remaining work with exit tests), `PLAN.md` (v2 fusion design), `SEARCH_TOOL.md`, the benchmark plan, and the UI, JEV and consolidation plans |
| [research/](research/INDEX.md) | Background research: Abhidhamma, thermodynamic memory, agent-memory literature |

## License

[Gnosis AI-Sovereign License v1.4](LICENSE.md). `crates/ferricula-search` derives from lume (BSD 3-Clause, see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)).

---

<div align="center">
<sub>Built by <a href="https://github.com/DeepBlueDynamics">DeepBlue Dynamics</a>.</sub>
</div>
