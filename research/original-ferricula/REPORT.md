# Original Ferricula Source Analysis & Recovery Report

This report provides a comprehensive analysis of the original Ferricula repository architecture. It identifies concrete code paths for the primary system components, analyzes design highlights and gaps, and maps existing source coordinates to the proposed `ferricula_v2` design layout.

---

## 1. Concrete Code Paths & Key Components

### 1.1 Runtime / Server Startup
* **Entrypoint**: src/main.rs (upstream v1 `ferricula/src/main.rs`) (`fn main() -> Result<()>`).
* **Workflow**:
  1. Configures rustls default crypto provider via `rustls::crypto::ring::default_provider().install_default()`.
  2. Loads configuration settings from `.env` using src/main.rs:load_dotenv() (upstream v1 `ferricula/src/main.rs`).
  3. Parses command-line args for the SQLite data directory (`./data` by default) and `--serve [port]`.
  4. Opens the persistent database: `DurableEngine::open(&data_dir)`.
  5. Derives agent identity from physical radio entropy via `ferricula::identity::load_or_create(&data_dir, &identity_entropy)`. If it is a new agent, a 4D anchor memory is committed at an ID matching the seed.
  6. Configures standard OS interrupt signal handler via `ctrlc::set_handler` for graceful checkpointing.
  7. Spawns clock thread: src/main.rs:145 (upstream v1 `ferricula/src/main.rs`) (`clock::spawn_clock`).
  8. **Serve Mode**:
     - Spawns the HTTP endpoint listener on `PORT` (via `ferricula::http::spawn_http`).
     - Launches an async Tokio runtime to serve the MCP endpoint on `serve_port + 1` (via `ferricula::mcp::run_mcp_server`).
     - Builds the BM25 search engine by scanning active database rows and recalibrates term statistics.
     - Loops processing clock events, handling incoming HTTP commands, and draining completed background LLM query rewrites.
  9. **REPL Mode**:
     - Enters standard terminal REPL, prompting for stdin inputs and executing commands synchronously via `handle_command`.

### 1.2 Delos Conversational and Idle Loops
* **Context**: "Delos" represents the autonomous agent environment, implemented in the Python stack. The primary loops run in arena/steve.py (upstream v1 `ferricula/arena/steve.py`) and arena/trek.py (upstream v1 `ferricula/arena/trek.py`).
* **Autonomous Think Loop** (`_think_loop` in arena/steve.py:4195 (upstream v1 `ferricula/arena/steve.py`)):
  - Enters a loop driven by `_think_stop` event.
  - Monitors user interaction to determine idle state: `secs_since_chat = time.time() - _last_chat_time`.
  - If a user message occurred recently (`< _CHAT_WINDOW_SECS` / 30s), it skips the cycle to prevent interruptions, except for a 20% random bypass to maintain a "feeling of life."
  - Tracks idle cycles: if idle time exceeds `_CHAT_IDLE_SECS` (90s), it increments `idle_cycles`. If `idle_cycles` crosses 4 (3 min) or 8 (6 min), it shifts model selection to local/cheaper `gemma` (budget drift) to save cloud API limits.
  - Shifts emotional state on each cycle (`shift_emotion`), which modifies the system prompt, active model preferences, and bodily somatic descriptors.
  - Interacts with Ferricula to pull recent thoughts/curiosity via the `/hybrid` search API and recent chat history to assemble context.
  - Invokes `think_cycle`, assembling the system instruction, selecting the model based on budget pressure and emotion, running the LLM, and evaluating resulting tool calls.
  - Modulo `DREAM_EVERY_N` (80 cycles/~1hr) or when emotional state matches `DREAM_STATES` (sadness, boredom, withdrawal, melancholy, submission), it calls Ferricula's `POST /dream` endpoint to trigger memory consolidation.
* **Autonomous Advocate Loop** (`_advocate_loop` in arena/steve.py:4065 (upstream v1 `ferricula/arena/steve.py`)):
  - An independent auditing thread running every `ADVOCATE_INTERVAL` (180s) when the think loop is active.
  - Pulls goal/value memories (`what Steve wants values goals drives`) and recent actions (`recent actions decisions thoughts`).
  - Calls a local LLM (`local_llm` running Gemma) with a custom auditor prompt.
  - Emits `WANTS` (fundamental current drive) and `VERDICT` (whether his actions serve those drives, and if not, why).
  - If budget pressure is low (`< 0.75`), it writes the advocate's evaluation back into Ferricula's `thinking` channel.

### 1.3 Model / Provider Routing
* **Logic Location**: arena/steve.py (upstream v1 `ferricula/arena/steve.py`) (`model_for_emotion`, `_drifted_model`, `_budget_pressure`, `_budget_nudge`, `_check_model_heat`).
* **Precedence**:
  1. **Budget Pressure**: Calculated based on accumulated session cost relative to hourly spending caps. `_budget_nudge` forces model selection to cheap/free tiers (Gemma) if pressure `>= 0.75`.
  2. **Emotional State**: Maps current feelings to models. Sadness/awe/deep synthesis map to frontier models (`claude-opus-4-7`, `gemini-3.1-pro-preview`); reactive/fast states map to cheap cloud (`gpt-4o-mini`, `gemini-3.1-flash-lite`). Normal operations default to mid-cloud (`gpt-4o`, `claude-sonnet-4-6`).
  3. **Idle Drift**: Long periods of user silence slide the default model choice to `gemma` to eliminate idle cloud costs.
  4. **Fallback Chains**: If the primary provider call fails (cool-down, timeout, HTTP error), the router falls down the chain (e.g. Preferred -> other cloud -> Gemma) and logs a warning.

### 1.4 Tool Suppression and Control
* **Controls**:
  - The think loop can be explicitly paused or resumed by the operator via `POST /api/think/start` and `POST /api/think/stop`, updating the `_thinking` flag.
  - The local advocate loop stays dormant when thinking is off (`not _thinking`).
  - In tools/ferricula-mcp.py (upstream v1 `ferricula/tools/ferricula-mcp.py`) and src/mcp.rs (upstream v1 `ferricula/src/mcp.rs`), tool visibility is constrained via the `--surface` CLI flag or `FERRICULA_SURFACE` env var:
    - `cognitive`: exposes only external conversational tools (remember, recall, reflect, observe, inspect, status, etc.).
    - `system`: exposes administrative and maintainer tools (dream, keystone, checkpoint, offer, query, etc.).
    - `all`: registers all 21 tools.

### 1.5 Memory Recall, Remember, and Dream
* **Remember**: src/memory.rs (upstream v1 `ferricula/src/memory.rs`) / src/persist.rs (upstream v1 `ferricula/src/persist.rs`).
  - Assigns unique initial `decay_alpha` based on channel type (e.g., `hearing` = 0.010, `thinking` = 0.015, `body` = 0.020).
  - Stored as `Row` consisting of metadata, tag mappings, and a 768D vector.
  - For geometric security, vectors are encrypted with the identity's `vector_transform` projection matrix.
* **Recall**:
  - Raw query text is normalized to SQL using the `Planner` in src/planner.rs (upstream v1 `ferricula/src/planner.rs`) (rewriting via Claude-Haiku-4-5-20251001 or fallback rules).
  - Re-projects/decrypts vectors and executes the SQL query.
  - Evaluates active archetype gates using `ResonanceGate` filtering:
    - `Fidelity`: excludes memories with `fidelity < FIDELITY_GATE` (0.75).
    - `Lifecycle`: excludes `Forgiven`/`Archived` records.
    - `Temporal`: checks staleness: excludes memories recalled `< 2s` ago (refractory period) or neglected `> 48h` (out of phase).
    - `AgentCapacity`/`Craft`: blocks recall if cognitive heat exceeds the identity's `heat_ceiling`.
* **Dream (Consolidation)**: src/dream.rs (upstream v1 `ferricula/src/dream.rs`) (`dream_cycle_with_intensity`).
  - **Phase 0 (Keystone Halo)**: Finds neighbors of keystones in the `MemoryGraph` and reduces their decay rate (`decay_alpha *= HALO_SHRINK` / 0.99) to preserve context.
  - **Phase 1 (Decay Tick)**: Reduces fidelity for non-keystone active memories: `fidelity *= exp(-effective_alpha)`.
  - **Phase 2 (Lifecycle)**: Transitions decayed records (`fidelity < 0.75`) from `Active` to `Forgiven`.
  - **Phase 3 (Consolidation)**: Groups active memories with similarity `>= 0.85`. The highest-fidelity record survives, increasing its consolidation depth (which slows future decay via $\alpha_{eff} = \frac{\alpha}{1 + \ln(1 + depth)}$). Others are marked `Forgiven` and `Archived`.
  - **Phase 3.5 (Semantic Edge Discovery)**: The Intuition archetype (We weaver) samples top-N memories by fidelity and samples long-tail memories using radio-entropy bits, creating edges for pairs with similarity in `[0.7, 0.85)`.
  - **Phase 3.6 (SKG Weber Update)**: Traces term co-occurrences and calculates velocity/acceleration.
  - **Phase 4 (Neglect)**: Memories not recalled for `> 24h` grow decay rates via `NEGLECT_GROW` (1.005) up to `ALPHA_MAX` (0.02).
  - **Phase 5 (Keystone Review)**: Ethics archetype promotes memories with `>= 5` recall counts and `> 0.95` fidelity to decay-immune keystones.
  - **Phase 5.5 (Dream Imagery)**: Emits prompts from emerging term pairs with positive Weber brackets.
  - **Phase 6 (Pruning & Ghost Echoes)**: Deletes archived records where `fidelity < epsilon`. If `SHIVVR_URL` is available, it performs a "deathbed confession": inverts the dying vector to text, re-embeds it, and if round-trip similarity `>= 0.5`, attaches it as a low-weight ghost echo edge to surviving neighbors.

### 1.6 Identity and Wisdom King / Archetype Behavior
* **Identity State**: src/identity.rs (upstream v1 `ferricula/src/identity.rs`) (`IdentityState`).
  - Casts name and I Ching hexagram from 6 bytes of physical radio entropy at creation.
  - Derives `ThermodynamicConstants` (decay baselines, cooling rates, ceiling) by applying a +/-15% entropy-driven jitter to global defaults.
  - Generates ECC keypair and orthogonal `vector_transform` projection from the identity seed.
* **Archetypes**: src/archetypes.rs (upstream v1 `ferricula/src/archetypes.rs`) (`Archetype`).
  - Activates 5 sub-agent archetypes based on dream intensity (Minimal = none, Moderate = Intuition + Fortune, Full = all 5):
    1. **Intuition** (Trailokyavijaya - East): controls Temporal gates, runs Weaver semantic edges.
    2. **Fortune** (Vajrayakṣa - North): controls Agent Capacity gates.
    3. **Craft** (Kuṇḍali - South): controls Craft load limit gates.
    4. **Ethics** (Yamāntaka - West): controls Lifecycle gates, reviews keystones.
    5. **Advocate** (Acala - Center): controls Fidelity gates.
  - Only active archetypes apply their corresponding `ResonanceGate` check during query recall.

### 1.7 HTTP and MCP APIs
* **HTTP Endpoint Router**: src/http.rs (upstream v1 `ferricula/src/http.rs`) (tiny_http parser mapping paths to commands).
  - Implements API endpoints like `POST /remember`, `POST /recall`, `POST /dream`, `POST /offer`, `GET /status`, `GET /identity`, `GET /clock`.
* **MCP Protocol**: src/mcp.rs (upstream v1 `ferricula/src/mcp.rs`) (`FerriulaMcp` implementing `rmcp::ServerHandler`).
  - Exposes 20 tools proxying back to the HTTP ports, using shivvr for embedding calculations.

### 1.8 Safe Sleep / Pause / Shutdown
* **Process Signal Handling**: src/main.rs:139 (upstream v1 `ferricula/src/main.rs`) (`ctrlc::set_handler`).
  - Intercepts `SIGINT` (ctrl-c), sets `should_stop` to `true`.
  - The loop checks the flag, calls `db.checkpoint()?` (flushing WAL records and saving `SnapshotV4` via postcard), and exits cleanly.
* **Cloud Deployment Lifecycle**: ferricula-ops/cloud/entrypoint.sh:47 (upstream v1 `ferricula/ferricula-ops/cloud/entrypoint.sh`) (`graceful_shutdown()`).
  - Intercepts `SIGTERM` and `SIGINT`.
  - Sends a `POST /checkpoint` command to the running Ferricula instance.
  - Invokes `gcs_upload` to backup `snapshot_v4.bin`, `wal.log`, `identity.json`, and `seeds.jsonl` to GCS.
  - Sends `kill` to the background processes and exits.

### 1.9 Container / Deployment Configuration
* **Local Container**: Dockerfile (upstream v1 `ferricula/Dockerfile`) (multi-stage Rust build with `debian:bookworm-slim`).
* **Cloud Single-Tenant Deployment**: ferricula-ops/cloud/Dockerfile (upstream v1 `ferricula/ferricula-ops/cloud/Dockerfile`).
  - Based on `google/cloud-sdk:slim` containing gsutil.
  - Restores state on cold start via entrypoint.sh:gcs_download() (upstream v1 `ferricula/ferricula-ops/cloud/entrypoint.sh`).
  - Runs periodic background checkpoints and GCS uploads every `SYNC_INTERVAL` (300s).
  - Built and deployed to Cloud Run via deploy.sh (upstream v1 `ferricula/ferricula-ops/cloud/deploy.sh`) with `--max-instances 1` for data integrity.

---

## 2. Gaps & Technical Debt

1. **Stub Archetype Logic**:
   - The five sub-agent archetypes are cast, stored, and activate/deactivate in `archetypes.rs` and `identity.rs`, but their behavioral logic is largely stubs. The active state only controls whether the corresponding `ResonanceGate` is applied at query time. The behavioral differences in thinking or dialog are handled in the Python layer rather than the engine itself.
2. **Fragile Query Planner Rewrite**:
   - The query rewriter in `src/planner.rs` connects to the Anthropic API using a hand-rolled raw TLS socket stream (`rustls::Stream` to `api.anthropic.com:443` parsing raw HTTP strings). This is highly fragile, bypasses standard client abstractions, and lacks proper error recovery or support for alternative model endpoints.
3. **Decoupled Dream Imagery**:
   - The dream cycle generates imagery candidate prompts, but the actual image generation (ComfyUI / DALL-E) is executed inside the Python layer (`arena/steve.py`). This breaks self-containment for the memory engine's dream mechanics.
4. **Single-Threaded REPL Blocking**:
   - In REPL mode, when the user inputs a query that triggers an LLM rewrite, the thread blocks synchronously on the network call. If the network times out, the REPL hangs.
5. **Port-Based Multi-Tenancy**:
   - The character registry and multi-agent isolation rely on separate port mappings or explicit character discovery rather than multi-tenant workspace routing in a single binary.

---

## 3. Source-to-v2 Map

To support clean maintenance, we map the original files to modular v2 components:

| Original Source Location | Functionality | Recommended v2 Destination |
| :--- | :--- | :--- |
| `src/main.rs` | Entrypoint, REPL, loops | `cmd/ferricula/main.rs`, `server/mod.rs` |
| `src/server_config.rs` | TOML config parse/save | `core/config.rs` |
| `src/http.rs` | HTTP REST endpoints | `server/http.rs` (Migrate to Axum) |
| `src/mcp.rs` | MCP endpoint & tool definitions | `server/mcp.rs` (Clean Axum routing) |
| `src/memory.rs` | Memory model, Resonance gates | `core/memory/model.rs` |
| `src/dream.rs` | Decay, neglect, consolidation, ghosts | `core/memory/dream.rs` |
| `src/identity.rs` | Entropy constant derivations, key expansion | `core/identity/mod.rs` |
| `src/archetypes.rs` | 5 archetypes & activation tiers | `core/identity/archetypes.rs` |
| `src/casting.rs` | Hexagram casting, horoscopes | `core/identity/casting.rs` |
| `src/graph.rs` | Graph nodes & edge kind mappings | `core/memory/graph.rs` |
| `src/planner.rs` | Claude rewriter raw TCP connection | `core/planner/mod.rs` (Rewrite using reqwest) |
| `src/persist.rs` | DurableEngine, WAL, SnapshotV4 | `core/persist/mod.rs` |
| `src/tokenizer.rs`, `src/pali.rs` | Glossary & tokenization | `core/pali/mod.rs` |
| `src/transform.rs` | Vector encryption projections | `core/crypto/transform.rs` |
| `arena/steve.py` | Chat interface, emotion, model router, advocate | `agent/steve/mod.rs` or `agent/steve.go` |
| `ferricula-ops/cloud/*` | Cloud Run, GCS sync, entrypoint | `ops/deploy/cloudrun/` |

---

## 4. Verification & Recovery Actions

* All directly relevant source files (excluding `.env`, database volumes, secrets, and zipped packages) have been copied from the original workspace `ferricula` to the recovery folder:
  - `research/original-ferricula/source/` (gitignored; not included in this repo)
* Absolute paths have been fully preserved relative to the workspace roots.
* Ready for the implementation of `ferricula_v2` modules.
