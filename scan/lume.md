# Lume Code Survey

## 1. Purpose
Lume is a high-performance, FST-backed tagger and BM25 hybrid search engine. It acts as the "new cortex" or document memory sidecar in the unified agent memory architecture. It indexes, searches, and summarizes raw text, code repositories, and PDFs, blending lexical BM25 retrieval, spell checking, dense semantic embeddings, and Semantic Knowledge Graph (SKG) entity-graph co-occurrence boosts.

## 2. Crate & Module Layout
Lume is configured as a single binary crate with a rich library module design:
*   [lib.rs](file:///workspace/DeepBlueDynamics/lume/src/lib.rs): Core library module. Houses `Tagger`, `Entry`, `Token`, and tokenizer/ASCII folding helpers.
*   [main.rs](file:///workspace/DeepBlueDynamics/lume/src/main.rs): Command Line Interface (CLI) entrypoint. Implements command parsing and dispatch.
*   [bm25.rs](file:///workspace/DeepBlueDynamics/lume/src/bm25.rs): Field-aware BM25 engine (`Bm25Index`), `Section` parsing, and parameters (`Bm25Params`).
*   [spelling.rs](file:///workspace/DeepBlueDynamics/lume/src/spelling.rs): Trigram spell checker (`SpellIndex`) using roaring bitmaps.
*   [semantic_mesh.rs](file:///workspace/DeepBlueDynamics/lume/src/semantic_mesh.rs): Implements `EntityGraph` (nodes and similarity/relatedness edges) and a trigram `MarkovChain` generator.
*   [hybrid.rs](file:///workspace/DeepBlueDynamics/lume/src/hybrid.rs): Handles dense embedding requests, semantic session caching, and blending lexical + semantic + SKG scores.
*   [agent.rs](file:///workspace/DeepBlueDynamics/lume/src/agent.rs): Implements the autonomous Q&A agent loop and the MCP server.
*   [graph_search.rs](file:///workspace/DeepBlueDynamics/lume/src/graph_search.rs): Walks the entity co-occurrence graph to compute search-time query boosts and recall expansions.
*   [fast_retrieval.rs](file:///workspace/DeepBlueDynamics/lume/src/fast_retrieval.rs): Custom zero-dependency roaring bitmap implementation (`MiniRoaring`) and Gödel-style signature filters (`PrimeFilter`).
*   [regex.rs](file:///workspace/DeepBlueDynamics/lume/src/regex.rs): NFA pattern compiler for tagger dictionary regex entries.
*   [inversion.rs](file:///workspace/DeepBlueDynamics/lume/src/inversion.rs): Vector-to-text inversion helpers.
*   [stream.rs](file:///workspace/DeepBlueDynamics/lume/src/stream.rs): Real-time streaming match candidates.
*   [answer.rs](file:///workspace/DeepBlueDynamics/lume/src/answer.rs): Structured synthesis answers.
*   [crawl.rs](file:///workspace/DeepBlueDynamics/lume/src/crawl.rs): Web page crawler.

## 3. Key Data Structures & Serialization Formats
All metadata and indexing files are stored under the designated database directory (e.g., `.lume-index/`) as pretty-printed JSON files using `serde_json`:
1.  **Index State (`state.json`)**: Contains `IndexState` struct tracking configured directories, database paths, flags for semantic vectors/Ollama extraction, tag dictionary path, and `cached_files` mapping file paths to their modification times and sections.
2.  **BM25 Index (`bm25.json`)**: Persists `Bm25Index`. Stores all parsed `sections` (`Vec<Section>`), document counts, average lengths, field-specific frequencies, posting lists (`HashMap<Vec<u8>, MiniRoaring>`), and entity-specific posting lists. Custom serializers handle non-UTF-8 keys in binary vectors.
3.  **Spell Check Index (`spelling.json`)**: Persists `SpellIndex` consisting of unique words, vocabulary sets, and `trigram_postings` (`HashMap<String, MiniRoaring>`).
4.  **Entity Graph (`entity_graph.json`)**: Persists `EntityGraph` with `EntityNode` elements (representing concepts/entities) and `EntityEdge` elements tracking similarity (Jaccard) and relatedness (statistical z-score squashed with `tanh`).

## 4. Public API Surface
*   **Library API**: Exposure of tagger, indexers, search blend, and spelling correctors.
*   **Command-Line Interface (CLI)**:
    *   `lume index [-s] [-o] <dir>`: Initial or incremental index pass.
    *   `lume search <query>`: Runs lexical, semantic, or hybrid search.
    *   `lume generate`: Runs trigram Markov text generator with FST/GTR steering.
    *   `lume summarize`: Graph-guided document summarization.
    *   `lume agent <question>`: Stateful autonomous Q&A agent loop.
    *   `lume serve`: Launches HTTP MCP service.
    *   `lume crawl`: Recursively downloads web pages.
    *   `lume eval`: Performance evaluation suite.
*   **HTTP / MCP Endpoint**: Listen on HTTP (default port `5863`) using standard JSON-RPC 2.0 protocol (endpoints `/message`, `/mcp`, `/sse`). Exposes capabilities as tools:
    *   `lume_index(dir, db, semantic, ollama_entities, ollama_model, ollama_url, force, tag_dict)`
    *   `lume_search(query, db, spell_check, limit, alpha, graph)`
    *   `lume_generate(seed_word, db, limit, steer, attempts, threshold)`
    *   `lume_not_found(reason, attempted_queries)`

## 5. Chunking, Tokenization, Embedding, & Memory Logic
*   **Tokenization**:
    *   Strips hyphens and folds Latin characters to lowercase ASCII.
    *   Tokenizes on non-alphanumeric boundaries.
    *   FST dictionary matching walks byte-by-byte, inserting separator byte `0x1E` between tokens.
*   **Chunking**:
    *   Parses Markdown sections at `#` headers.
    *   Splits document bodies by `\n\n` (paragraphs) up to `25,000` characters (bytes).
    *   If a single paragraph exceeds the maximum limit, it splits it line-by-line (`\n`).
*   **Embedding & Semantic Retrieval**:
    *   Proxies semantic vector generation to an external Shivvr instance (`POST /temp/<session_id>/ingest`).
    *   Queries `GET /temp/<session_id>/search?q=<query>&n=60` to get cosine similarity hits.
    *   Tracks staleness through section content hashes (FNV-1a 64-bit on filename, title, and body) mapped to the remote chunk `source` field.
*   **Memory Blending**:
    *   Combines lexical BM25, dense semantic, and SKG graph scores.
    *   Supports two blend modes in `blend_hybrid_scores`:
        1.  *Multiplicative* (default): `BM25 * (1 + alpha * semantic + beta * SKG)`. Strong lexical results remain leaders.
        2.  *Normalized* (via `LUME_BLEND_NORM` environment variable): `(BM25 / BM25_max) + alpha * semantic + beta * SKG`. Allows semantic matches to overtake lexical ones.

## 6. Notable Cargo.toml Dependencies
*   `tantivy-fst = "0.5"`: Zero-dependency finite state transducers.
*   `ureq = { version = "2.9", features = ["json"] }`: Minimal synchronous HTTP client.
*   `serde = { version = "1.0", features = ["derive"] }` & `serde_json = "1.0"`: Serialization.

## 7. What is Finished vs. Half-Done vs. Broken
*   **Finished**:
    *   Core FST tagging and Latin folding.
    *   Field-aware BM25 (Classic, Plus, L variants) retrieval and spelling trigram indexes.
    *   Hybrid blending and query-expansion walk.
    *   HTTP MCP server, SSE, and autonomous agent loops.
*   **Half-Done**:
    *   On-the-fly fine-tuning of open embedding models (listed on the roadmap; currently static).
    *   Late-interaction / multi-vector (ColBERT / ColPali) visual document indexing (listed on the roadmap).
    *   Query vector inversion (`LUME_QUERY_INVERSION`): Functional debug feature that reconstructs query strings from embeddings, but slow.
*   **Broken / Limitations**:
    *   PDF parsing relies on an external Python script wrapper (`lib/lume_extractor.py`) which requires a python3 environment with `pypdf` installed.
