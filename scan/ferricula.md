# Ferricula Code Survey

## 1. Purpose
Ferricula is a stateful agent memory system designed to act as the "experiential memory" of an agent. It integrates thermodynamic decay (fidelity loss over time), cognitive memory gates, relational graphs, a concepts prime-number tree, and dream consolidation cycles. It allows agent memories to decay naturally, get promoted to keystones (resisting decay), fold from event sequences into consolidated concepts, and disappear or leave "ghost echoes" upon forgetting.

## 2. Crate & Module Layout
Ferricula is structured as a library + binary crate with the following modules:
*   lib.rs (upstream v1 `ferricula/src/lib.rs`): Library entry point exporting modules and core engine structures.
*   main.rs (upstream v1 `ferricula/src/main.rs`): Setup, REPL command handler, HTTP / MCP server routing, and main clock event loops.
*   memory.rs (upstream v1 `ferricula/src/memory.rs`): Implements `MemoryRecord`, `LifecycleState`, thermodynamic decay math, and `ResonanceGate` definitions.
*   engine.rs (upstream v1 `ferricula/src/engine.rs`): Low-level in-memory storage (`Engine`) featuring roaring bitmap indexing over tags (`tag_index`) and tag ranges.
*   persist.rs (upstream v1 `ferricula/src/persist.rs`): Provides `DurableEngine` and `Persistence` managing postcard snapshot serialization and Write-Ahead Log (WAL) appending.
*   graph.rs (upstream v1 `ferricula/src/graph.rs`): Relational memory graph (`MemoryGraph`) with semantic, causal, and structural edges.
*   skg.rs (upstream v1 `ferricula/src/skg.rs`): Semantic Knowledge Graph calculations (co-occurrence frequency, significance).
*   prime_tree.rs (upstream v1 `ferricula/src/prime_tree.rs`): Concept hierarchy tree using Gödel prime factorization for membership.
*   http.rs (upstream v1 `ferricula/src/http.rs`): Launches synchronous HTTP API using `tiny_http`.
*   mcp.rs (upstream v1 `ferricula/src/mcp.rs`): Implements axum HTTP transport for the Model Context Protocol (MCP) server.
*   planner.rs (upstream v1 `ferricula/src/planner.rs`): LLM query rewrite planner.
*   identity.rs (upstream v1 `ferricula/src/identity.rs`): Generates agent keys and identity anchors.
*   pali.rs (upstream v1 `ferricula/src/pali.rs`): Translational mapping for Abhidhamma Buddhist Pali terminologies.
*   archetypes.rs (upstream v1 `ferricula/src/archetypes.rs`): Maps Wisdom King archetypes to cognitive operations.
*   casting.rs (upstream v1 `ferricula/src/casting.rs`): Trigram emotion and I Ching oracle casting helpers.
*   clock.rs (upstream v1 `ferricula/src/clock.rs`): Spawnable telemetry/cadence ticking clock.
*   inversion.rs (upstream v1 `ferricula/src/inversion.rs`): Vector-inversion drift checker.
*   tokenizer.rs (upstream v1 `ferricula/src/tokenizer.rs`): Simple regex tokenizer.
*   transform.rs (upstream v1 `ferricula/src/transform.rs`): String sanitizers.
*   sparse.rs (upstream v1 `ferricula/src/sparse.rs`) & model.rs (upstream v1 `ferricula/src/model.rs`): Data model structures (`Row`, `DistanceMetric`, `MemoryRef`).

## 3. Key Data Structures & Serialization Formats
1.  **Durable Engine Serialization (`persist.rs`)**:
    *   **Snapshots**: Snapshots are saved to `snapshot_v4.bin` using the `postcard` binary format.
    *   **Snapshot Envelope**: Format version 5 uses a version envelope consisting of a magic prefix `FERR` (4 bytes) + format version `5` (1 byte) + reserved bytes (3 bytes) + postcard bytes. It falls back to legacy V4/V3/V2/V1 formats when reading.
    *   **Write-Ahead Log (`wal.log`)**: Appends length-prefixed (4-byte LE size) `postcard` bytes representing the `WalEntry` enum (Upsert, Delete, Remember, UpdateRecord, RemoveMemory, and Connect).
2.  **Memory Envelope (`Row` and `MemoryRecord`)**:
    *   `Row`: Houses raw ID (`u32`), tags (`BTreeMap<String, String>`), dense vector (`Vec<f32>`), and reference coordinates (`MemoryRef`).
    *   `MemoryRecord`: Tracks thermodynamics (`fidelity` [0..1], `decay_alpha` decay rate [0.001..0.02], lifecycle state, and importance).

## 4. Public API Surface
*   **CLI REPL mode**: Interactive console supporting commands: `remember`, `recall`, `dream`, `status`, `inspect`, `keystone`, `connect`, `disconnect`, `neighbors`, `terms`, `skg`, `upsert`, `delete`, `query`, `touch`, `clock`, `offer`, `checkpoint`.
*   **HTTP Server (port 8765)**: Synchronous endpoints `/remember`, `/recall`, `/dream`, `/status`, `/identity`, `/clock`, `/checkpoint`, `/terms`, `/query`, `/offer`, `/inspect/{id}`, `/keystone/{id}`, `/neighbors/{id}`, `/inversion/{id}`.
*   **MCP Server (default port 8766)**: Serves JSON-RPC at `/mcp` via Server-Sent Events (SSE). Exposes tools:
    *   `ferricula_remember(text, channel, importance, keystone)`
    *   `ferricula_recall(query)`
    *   `ferricula_reflect(thought, importance)`
    *   `ferricula_observe(path, summary)`
    *   `ferricula_inspect(id)`
    *   `ferricula_status` / `ferricula_identity` / `ferricula_health`
    *   `ferricula_dream` (triggers consolidation)
    *   `ferricula_keystone(id)`
    *   `ferricula_neighbors(id)`
    *   `ferricula_connect(a, b, label, kind)`
    *   `ferricula_disconnect(a, b)`
    *   `ferricula_clock` / `ferricula_checkpoint` / `ferricula_terms`
    *   `ferricula_query(sql)`
    *   `ferricula_inversion_check(id)`
    *   `ferricula_embody`
    *   `ferricula_offer_entropy(source)`

## 5. Chunking, Tokenization, Embedding, & Memory Logic
*   **Tokenization**: Uses a basic regex-based whitespace/punctuation boundary tokenizer in `tokenizer.rs`.
*   **Embeddings**: Bypasses local embedders; sends requests to Shivvr (`POST /temp/ferricula/ingest` or `/temp/ferricula/ingest`) to fetch vectors.
*   **Thermodynamic Decay**:
    *   Active records decay exponentially: `fidelity *= (-effective_alpha).exp()`.
    *   `effective_alpha = decay_alpha / (1 + ln(1 + consolidation_depth))`.
    *   Recall shrinks decay rate: `decay_alpha *= RECALL_SHRINK` (0.95), bounded at `ALPHA_MIN` (0.001). Neglect grows decay rate: `decay_alpha *= NEGLECT_GROW` (1.005), capped at `ALPHA_MAX` (0.02).
    *   Proximity to keystones shrinks decay rate: `decay_alpha *= HALO_SHRINK` (0.99).
*   **Resonance Gates**: Recalls pass through multiple filters (ResonanceGates): Acala (Fidelity gate), Yamāntaka (Lifecycle state gate), Trailokyavijaya (Temporal threshold), Vajrayakṣa (Agent heat capacity), and Kuṇḍali (Cognitive load ceiling).
*   **Dream Cycles**: Updates timestamps and ticks down decay. Records below `FIDELITY_GATE` (0.75) are set to `Forgiven` or `Archived` lifecycle states. Similar memories merge, increase consolidation depth, and update relationship edges.

## 6. Notable Cargo.toml Dependencies
*   `postcard = { version = "1.1", features = ["use-std"] }`: Postcard binary serialization.
*   `roaring = { version = "0.11", features = ["serde"] }`: Roaring bitmaps.
*   `rmcp`: Streamable HTTP server transport and MCP tool bindings.
*   `tiny_http = "0.12"`: Synchronous HTTP server.
*   `sqlparser = "0.59"`: SQL query syntax parser.
*   `ratatui = "0.26"` & `crossterm = "0.27"`: Terminal dashboard.

## 7. What is Finished vs. Half-Done vs. Broken
*   **Finished**:
    *   Low-level Engine, tag indexing, roaring bitmaps.
    *   Postcard snapshot serialization, WAL appending and restore logic.
    *   Resonance gates, thermodynamic ticking formulas.
    *   HTTP tiny_http server and MCP Server interface.
*   **Half-Done**:
    *   Anchor identity memory creation (silently bypasses dimension mismatches).
    *   SQL parsing: Custom executor is basic and handles only direct column equalities or simple ranges.
*   **Broken / Limitations**:
    *   The 5 Wisdom King archetypes loop is stubbed (`src/archetypes.rs` behaviors return hardcoded/mocked outputs).
    *   The I Ching casting oracle is partially stubbed (lacks full 64 trigram corpus and live integration with SDR-rand entropy stream).
