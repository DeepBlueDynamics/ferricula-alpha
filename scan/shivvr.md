# Shivvr Code Survey

## 1. Purpose
Shivvr is a semantic memory sidecar service designed to act as the ephemeral vector database and embedding proxy in the unified memory architecture. It handles semantic text chunking, dense vector embedding (via GTR-T5 ONNX or OpenAI APIs), orthogonal per-agent embedding encryption, and vec2text embedding inversion. 

## 2. Crate & Module Layout
Shivvr is configured as a binary crate with conditional ML feature compilations (`#[cfg(feature = "ml")]`):
*   lib.rs (upstream `nuts.services/shivvr/src/lib.rs`): Exports sub-modules.
*   main.rs (upstream `nuts.services/shivvr/src/main.rs`): Setup, config parses, loads ONNX models, and starts axum listener.
*   api.rs (upstream `nuts.services/shivvr/src/api.rs`): Axum HTTP routes and JSON-RPC Model Context Protocol (MCP) server endpoints.
*   chunker.rs (upstream `nuts.services/shivvr/src/chunker.rs`): Monte Carlo semantic chunking algorithm.
*   embedder.rs (upstream `nuts.services/shivvr/src/embedder.rs`): Local GTR-T5 ONNX model embedding execution and token counting.
*   inverter.rs (upstream `nuts.services/shivvr/src/inverter.rs`): T5-based vec2text inverter pipeline.
*   store.rs (upstream `nuts.services/shivvr/src/store.rs`): Ephemeral in-memory database (`Store`) for persistent session chunks.
*   temp_store.rs (upstream `nuts.services/shivvr/src/temp_store.rs`): Ephemeral `TempStore` with RRF and dynamic FST intent boosting.
*   auth.rs (upstream `nuts.services/shivvr/src/auth.rs`): JWT JWKS refresh and token validation.
*   crypto.rs (upstream `nuts.services/shivvr/src/crypto.rs`): Ephemeral orthogonal matrix key manager.
*   openai.rs (upstream `nuts.services/shivvr/src/openai.rs`): Gracefully degraded OpenAI embedding handler.
*   similarity.rs (upstream `nuts.services/shivvr/src/similarity.rs`): SIMD-accelerated cosine similarity.

## 3. Key Data Structures & Serialization Formats
1.  **Memory Chunk (`Chunk`)**:
    *   Fields: `id` (`String`), `text` (`String`), `embedding` (768-d GTR-T5), `embedding_retrieve` (1536-d OpenAI), `token_count`, `source` file string, `metadata` (JSON), `created_at`, `emotion_primary` / `emotion_secondary` (Buddhist Vedanā emotions), `encrypted`, `agent_id`.
2.  **Session & TempSession**:
    *   Holds a vector of `Chunk` structures and builds a local Lume `Bm25Index` on ingestion.
3.  **Serialization/Storage**:
    *   No persistent disk databases are maintained. In-memory `HashMap` stores wrapped inside `RwLock` handle both persistent and temporary session chunks. 

## 4. Public API Surface
*   **HTTP Endpoints**:
    *   `GET /` & `GET /health`: Diagnostic checks.
    *   `POST /sessions/:session_id/ingest` & `GET /sessions/:session_id/search`: Persistent store endpoints.
    *   `DELETE /sessions/:session_id`: Delete persistent session.
    *   `POST /temp/:name/ingest` & `GET /temp/:name/search`: Temporary store endpoints.
    *   `DELETE /temp/:name`: Delete temporary store.
    *   `POST /agent/:agent_id/register`: Register agent encryption key.
    *   `POST /agent/:agent_id/encrypt` & `POST /agent/:agent_id/decrypt`: Orthogonal matrix vector warping.
    *   `POST /invert`: Invert GTR-T5 vector embeddings back to text.
    *   `GET /mcp/sse` & `POST /mcp/message`: HTTP SSE-RPC transport.
    *   `POST /sessions/:session_id/agent/chat`: Agent loop execution.

## 5. Chunking, Tokenization, Embedding, & Memory Logic
*   **Tokenization**: Utilizes HuggingFace tokenizers from local JSON files.
*   **Semantic Chunking**:
    *   Sentences are split by punctuation.
    *   *Monte Carlo Sampling*: Identifies sentence boundaries where before/after similarity falls below `0.5`.
    *   *Binary Search*: Inspects boundaries to isolate low-similarity divisions.
    *   *Optimization*: Iteratively shifts boundaries left/right to maximize average coherence (embedding magnitude/similarity).
    *   *Size Limits*: Sentence tokens exceeding `max_chunk_tokens` are split.
*   **Inversion Pipeline**:
    *   Takes a 768-d vector.
    *   Runs T5 Projection ONNX (`projection.onnx`) producing a `1x16x768` encoder input.
    *   Feeds to T5 Encoder ONNX (`encoder.onnx`) to obtain hidden states.
    *   Autoregressively decodes via T5 Decoder ONNX (`decoder.onnx`) to output token IDs, which are decoded back to clean text.
*   **Search Blending**:
    *   Combines dense vector search (via SIMD cosine similarity) and lexical search (via local Lume `Bm25Index`).
    *   Fuses scores via Reciprocal Rank Fusion (RRF): `RRF = rrf_semantic + rrf_lexical`.
    *   FST Tag Intent Boosting: Adds `0.05` to the RRF score for chunks containing FST tags.

## 6. Notable Cargo.toml Dependencies
*   `ort = "=2.0.0-rc.11"`: ONNX runtime wrapper for ML model executions.
*   `tokenizers = "0.21"`: Fast HuggingFace tokenizers.
*   `simsimd = "6.5.12"`: SIMD-accelerated similarity calculation library.
*   `lume-hybrid = { path = "vendor/lume-hybrid" }`: Vendor dependency on Lume hybrid engine.

## 7. What is Finished vs. Half-Done vs. Broken
*   **Finished**:
    *   Local GTR-T5 embedding generation and token counting.
    *   OpenAI backup embedding API.
    *   Orthogonal matrix-warped encryption and decryption.
    *   RRF blending and FST intent-boosting.
    *   Vec2text inversion pipeline.
*   **Half-Done**:
    *   The sled database storage described in specifications is not implemented; all persistent sessions are ephemeral in-memory maps.
*   **Broken / Limitations**:
    *   ONNX models must be present inside the `/models/` directory for embedding/inversion features to initialize.
    *   CUDA session creation may succeed but fail during run-time kernel validation on older/unsupported GPUs, forcing fallback to CPU.
