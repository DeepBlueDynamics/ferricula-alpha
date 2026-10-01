# Legacy inventory

Research only. Nothing here is a port. The "in ferricula-alpha?" column is from a search of the crates, first at `b121fdb` and again after `origin/v3/r0` moved to `853d199` (L2 merged, then Steve's review: the overlay nudges rank and does not dominate it). That diff only adds `crates/ferricula-server/src/recall_overlay.rs` and touches chat, config, lib, and recall. The same names are absent from `/workspace/ferricula-alpha/crates` except for that overlay, which the dirty main checkout does not have.

A line under a module is one public function, endpoint, tool, or documented capability: `name — what it does`. Where the source has a doc comment or docstring, the line is that first sentence. Where it does not, the line is a reading of the name in that module. Test modules are one line each, with every test name listed, because they check the functions above rather than add a capability. `pub(crate)` helpers are omitted. Minified frontend bundles are named and skipped.

Not opened, and not quoted: secret files, tokens, `.env`, and key material. Billing modules are listed by function and route only. `keygen.py` is listed from its algorithm comments; no seed value is written down.

`ferricula_v2/research/original-ferricula/source` overlaps `ferricula-original` in 64 code files and those 64 are byte-identical, so their functions are listed once, under ferricula-original.

## The ten most valuable missing capabilities

Checked against `853d199`. "Missing" means the live product does not do it. A math helper that no server calls still counts as missing. Citation-and-age ranking is not in this ten: it landed in L2, at `crates/ferricula-server/src/recall_overlay.rs`, and it does not write a memory row.

1. Per-agent geometric access on ingest and recall. A memory stored under one orthogonal key stays findable for that key and looks like noise to any other key (`myoo/mingwang/memex/keygen.py`, `memex/geometric_access.py`). Alpha has the transform (`crates/ferricula-core/src/transform.rs`, `crates/ferricula-semantic/src/crypto.rs`) and never calls it from `ferricula-server`, so recall still searches plaintext vectors.
2. Offer a memory to another agent, and a shared commons. Myoo's `memex_offer` decrypts with the source key and re-encrypts with the target key; memex-rs `offer` clones the row and records provenance. The commons (`memex_commons_contribute`, same MCP server) is the key-agnostic pool. Alpha can draw a graph edge. It cannot hand another agent a copy that only their key can read.
3. Goal utility. The memex spec records which memories were injected into a turn and updates their weight when the goal completes (`memex/SPEC.md`, specified, not implemented even there). Alpha ranks by fusion. It does not learn which memory helped.
4. A LoCoMo run. `ferricula-arena/bench/locomo_runner.py` and `locomo_two_agent_runner.py` score long conversational memory, including a two-agent variant. `crates/ferricula-bench` has other metrics and no LoCoMo runner, so a recall change has no long-dialogue check.
5. Rebuild a book from memories. Shelf stores each paragraph with a citation that survives hybrid recall, then writes the PDF back (`ferricula-arena/shelf/shelf/ingest.py`, `reconstruct.py`). Alpha stores document sections. It does not reassemble the book.
6. A scored multi-agent audit. `nbtx-deathmatch` runs three remembering agents, files issue cards, and diffs their findings against an answer key. Alpha's advocate is one mind's advisory (`crates/ferricula-cognition/src/advocate.rs`), with no score against a key.
7. Bayesian BM25. `ferricula-original/src/bb25.rs` turns a BM25 score into a calibrated probability, updates that calibration from a judgment, and fuses two probabilities in log-odds. Alpha's `crates/ferricula-search/src/bm25.rs` is classic BM25, fused by reciprocal rank, which does not say how sure a hit is.
8. Merge near-duplicates into one centroid. Myoo `consolidate` and memex-rs `dream` cluster by cosine and retire the sources into a weighted centroid. Alpha's dream groups similar rows as proposals and writes neither a centroid nor a retirement (`crates/ferricula-cognition/src/bhavana.rs`; the thirty-nights test keeps forgiven, archived, and pruned at zero). Worth proposing. Not worth wiping the sources.
9. Ask the store a precise question. Original `POST /query` and the MCP tool `ferricula_query` run SQL over tags and rows, and `GET /refs/distinct` lists a tag's values. Alpha recall embeds a sentence. An operator cannot ask "keystones below the gate" without that surface.
10. One container per agent. `ferricula-arena/arena/supervisor.py` creates, stops, and dreams a fleet, each agent its own ferricula volume. Alpha is one runtime. Offer, LoCoMo's two-agent runner, and the audit bench all assume the fleet.

## Gap table

| capability | where in legacy (project:path) | in ferricula-alpha? (path, or "missing") | port? yes/no/maybe | one-line reason |
|---|---|---|---|---|
| geometric ACL on ingest and recall | myoo:mingwang/memex/keygen.py; memex:geometric_access.py; ferricula-original:src/transform.rs | primitive only: crates/ferricula-core/src/transform.rs and crates/ferricula-semantic/src/crypto.rs; server does not call them | yes | wrong key should see noise, and the server still searches plaintext |
| offer a memory to another agent | myoo:mingwang/memex/mcp_server.py `memex_offer`; memex-rs:src/main.rs `offer` | missing | yes | the only share that keeps the recipient's key |
| shared memory commons | myoo:mingwang/memex/mcp_server.py `memex_commons_search` | missing | yes | a pool any agent can search without holding every private key |
| goal utility and injection tracking | memex:SPEC.md (no implementation in that tree) | missing | yes | recall never learns whether a shown memory helped the goal |
| LoCoMo conversational bench | ferricula-arena:bench/locomo_runner.py | missing | yes | long-dialogue check; ferricula-bench does not run LoCoMo |
| two-agent LoCoMo | ferricula-arena:bench/locomo_two_agent_runner.py | missing | yes | same bench with two remembering agents |
| shelf: cited paragraph ingest | ferricula-arena:shelf/shelf/ingest.py | missing | yes | a book enters as paragraphs whose cites survive recall |
| shelf: reconstruct a PDF from memories | ferricula-arena:shelf/shelf/reconstruct.py | missing | yes | the book can leave the engine and come back |
| scored multi-agent audit | ferricula-arena:nbtx-deathmatch | missing | yes | three agents, issue cards, score against an answer key |
| Bayesian BM25 (BB25) | ferricula-original:src/bb25.rs | missing | yes | a probability for a lexical hit; alpha has classic BM25 only |
| centroid consolidation of near-duplicates | myoo:mingwang/memex/memex.py `consolidate`; memex-rs dream; ferricula-original:src/dream.rs | proposals only, crates/ferricula-cognition/src/bhavana.rs | maybe | propose the centroid; do not wipe the source rows |
| SQL and distinct-tag query | ferricula-original:src/http.rs `POST /query`, `GET /refs/distinct` | missing | maybe | precise operator questions; a raw SQL tool is sharp |
| one container per agent | ferricula-arena:arena/supervisor.py | missing | maybe | the fleet the offer and the benches sit on; alpha is one runtime |
| dream writes fidelity, forgive, archive | ferricula-original:src/memory.rs; memex-rs dream | math at crates/ferricula-core/src/memory.rs; live dream does not apply it | maybe | the formula is in tree; applying it would reverse the non-destructive dream |
| five named archetype sub-agents | ferricula-original:src/archetypes.rs | reduced to an intensity role-set in crates/ferricula-cognition/src/dream.rs | maybe | Intuition, Fortune, Craft, Ethics, Advocate are only partly inlined |
| original MCP surface beyond recall | ferricula-original README: reflect, observe, inspect, connect, neighbors, dream, keystone, query, inversion, clock, offer-entropy | MCP is status, recall, chat, ingest, documents, life, read_section (`crates/ferricula-server/src/mcp.rs`) | maybe | several crates exist; those tools are not exposed |
| citation and age ranking without writing rows | not in these legacy trees; merged from PR #12 | crates/ferricula-server/src/recall_overlay.rs (s 0.003, f 0.002, h 30; a citation nudges rank) | no | already merged; it never writes a memory row |
| radio entropy clock | ferricula-original:src/clock.rs | crates/ferricula-cognition/src/clock.rs | no | already ported |
| hexagram and horoscope identity | ferricula-original:src/casting.rs | crates/ferricula-cognition/src/casting.rs | no | already ported |
| X25519 key agreement primitive | ferricula-original:src/ec_key.rs | crates/ferricula-cognition/src/identity.rs | no | the primitive is ported; the offer protocol is the row above |
| Pali term expansion | ferricula-original:src/pali.rs | crates/ferricula-cognition/src/pali.rs | no | already ported |
| prime tree, SKG, Weber bracket | ferricula-original:src/prime_tree.rs, src/skg.rs, src/graph.rs | crates/ferricula-core/src/prime_tree.rs, skg.rs, graph.rs | no | already ported |
| vec2text inversion | ferricula-original:src/inversion.rs | crates/ferricula-semantic/src/inverter.rs | no | already ported |
| hybrid lexical plus vector recall | ferricula-original `POST /hybrid`; ferricula_v2 hybrid.rs | crates/ferricula-search and crates/ferricula-server/src/recall.rs | no | already ported |
| advocate advisory | ferricula-arena:arena/advocate.py; ferricula_v2 advocate.rs | crates/ferricula-cognition/src/advocate.rs | no | already an advisory; the scored audit is a separate row |
| Nuts News read client | ferricula_v2:crates/ferricula-server/src/nutnews.rs | crates/ferricula-server/src/nutnews.rs | no | the reader is ported |
| Nuts News site (ledger, sqrl, karma, extension) | ferricula_v2:research/nuts-news/source | missing | maybe | a different product; port only if Steve should host the site |
| OCR a PDF to markdown | ferricula-original:arena/ocr_client.py | missing | maybe | the physics arena needed it; ingest today expects text |
| radio speech bridge | ferricula-original:arena/radio_bridge.py | missing | maybe | forwards transcriptions; distinct from the entropy clock |
| locations and a room model | memex:SPEC.md locations; ferricula-arena:arena/world.py | missing | maybe | specified, and a small room map in the arena; no store |
| index a repository as keystone memories | ferricula-original:tools/ferricula-code.py | crates/ferricula-server/src/code.rs reads the tree and does not ingest it | maybe | reading code and remembering code are different tools |
| dataset trainer (ingest a folder, fast-forward dreams) | ferricula-arena:arena/trainer.py | missing | maybe | how an agent "reads" a directory; shelf covers the book case |
| idle curiosity crawl | ferricula-arena:delos-broker.py `idle_loop` | crates/ferricula-server/src/autonomy.rs pauses and sleeps; it does not crawl when quiet | maybe | overlaps crawl tools and the fleet |
| Stripe billing and tenant provisioning | ferricula-original:ferricula-ops/cloud/billing | missing | no | operator billing, not memory |
| wisdom-king essays | myoo:KUNDALI.md, TRAILOKYAVIJAYA.md, VAJRAYAKSA.md, YAMANTAKA.md, ENTROPY.md | whispers in crates/ferricula-cognition/src/wisdom.rs | no | essays; the engine hook is already there |
| compiled chat bundles | myoo:mingwang/**/index-*.js | missing | no | build output |
| fib scratch script | myoo:arena_workspace/solution.py | missing | no | not a memory capability |

## ferricula-original

Thermodynamic memory engine for one agent. The README states the claim: memories decay, recall strengthens them, dreams consolidate, identity is cast from entropy. Three threads: main owns the engine, HTTP accepts commands, the clock polls gnosis-radio. Optional services are shivvr (embed and invert) and the radio (time and entropy). Without the radio, dreams run only when asked.

HTTP routes from `src/http.rs` (verified as match arms):

- `GET /` — serves the UI.
- `GET /diagnostic` — diagnostic page.
- `POST /remember` — store a memory.
- `POST /recall` — search, and the original updates recall stats on the row.
- `POST /recall_ids` — strengthen specific ids with no embedding.
- `POST /dream` — run a dream cycle.
- `GET /dream/latest` — last dream report, no new cycle.
- `GET /status` — counts and thermodynamic state.
- `GET /inspect/:id` — full record.
- `GET /get/:id` — raw row.
- `POST /offer` — inject entropy and trigger a dream.
- `POST /keystone/:id` — toggle keystone.
- `POST /connect` — edge between two memories.
- `POST /disconnect` — remove that edge.
- `GET /neighbors/:id` — neighbors with labels and fidelity.
- `GET /clock` — clock telemetry.
- `POST /checkpoint` — flush the WAL.
- `GET /identity` — hexagram, horoscope, archetypes.
- `GET /terms` — prime-tree terms.
- `GET /inversion/:id` — vec2text check.
- `GET /skg` and `GET /skg/:term` — semantic-knowledge-graph edges.
- `DELETE /delete/:id` and `POST /delete/:id` — remove a memory.
- `GET /maxid` — highest id.
- `POST /search` — BM25.
- `POST /hybrid` — vector plus BM25.
- `GET /glossary` — Pali glossary.
- `POST /confer` — archetype review of a draft reply: hedging, assistant-voice phrases, and question count.
- `GET /tools` — current tool tier.
- `POST /query` — SQL.
- `GET /refs/distinct` — distinct values of one tag field.
- `GET /admin/decay_status` — decay status.
- `POST /admin/recast_thermo` — recast thermodynamic constants from fresh entropy.
- `GET /admin/config` and `PUT /admin/config` — runtime config, redacted on read.

MCP tools from `src/mcp.rs` and the README. Cognitive surface: `ferricula_remember`, `ferricula_recall`, `ferricula_reflect` (thinking channel, faster decay), `ferricula_observe` (file observation, keystoned), `ferricula_inspect`, `ferricula_connect`, `ferricula_neighbors`, `ferricula_status`, `ferricula_health`, `ferricula_identity`. System surface: `ferricula_dream`, `ferricula_keystone`, `ferricula_checkpoint`, `ferricula_offer_entropy`, `ferricula_inversion_check`, `ferricula_terms`, `ferricula_query`, `ferricula_disconnect`, `ferricula_clock`. `FERRICULA_SURFACE` selects cognitive, system, or all.

README lifecycle, which the code in `src/memory.rs` implements: fidelity is `exp(-alpha_eff * ticks)`; `alpha_eff = alpha / (1 + ln(1 + consolidation_depth))`, bounded 0.001..0.02; recall multiplies alpha by 0.95; neglect grows it by 1.005; the gate is 0.75 (Active to Forgiven); keystones skip decay and stay under review. Dream order in the README: decay, forgive, consolidate, neglect, review, prune. Channels: hearing and seeing at alpha 0.010, thinking at 0.015. Five archetypes: Intuition, Fortune, Craft, Ethics, Advocate.

`src/ec_key.rs` is X25519 plus HKDF for a transform seed. The functions are listed below. No key material is quoted. `ferricula-ops/cloud/billing` is operator billing (Stripe checkout, portal, webhooks, Cloud Run tenants). Routes and function names only.

### Modules

### `arena/arena.py`
Ferricula Arena — The Count of Monte Cristo Each character is a ferricula container with its own identity, keys, and encrypted memory space. They read chapter by chapter, react in character, and dream independently. Sharing memories between characters uses the offering protocol (Diffie-Hellman key a
- `__init__` — constructs the object
- `run` — runs the main loop
- `main` — process entry point

### `arena/assis_reader.py`
Assis reads 'The Electric Force of a Current' by Assis & Hernandes. Single file, single ferricula. Reads PDF page by page with PyMuPDF, detects figures and sends them to Claude Vision for descriptions, enriches chunks with figure descriptions, embeds via shivvr, ingests into ferricula. Chat mode aft
- `cast_hexagram` — Cast an I Ching hexagram using radio entropy.
- `shivvr_chunk_and_embed` — shivvr chunk and embed
- `shivvr_embed` — Embed a short text via shivvr, return vector.
- `__init__` — constructs the object
- `remember` — stores one memory
- `recall` — Recall by semantic search.
- `recall_ids` — Recall by semantic search.
- `get_row` — get row
- `dream` — runs one dream cycle
- `status` — returns a status summary
- `checkpoint` — flushes durable state to disk
- `inspect` — Get memory metadata: fidelity, state, emotion, degree, etc.
- `neighbors` — Get graph neighbors of a memory.
- `available` — reports whether the remote service answers
- `describe_figure` — describe figure
- `render_page` — Render a PDF page as PNG bytes.
- `extract_page_text` — Extract text from a PDF page.
- `is_keystone_text` — Check if text contains keystone-worthy physics content.
- `hearing_rewrite_pali` — Rewrite text through Pali to strip compliance signal.
- `pali_link` — pali link
- `process_page` — process page
- `recall_memories` — recall memories
- `fire_synapse` — fire synapse
- `llm_respond` — llm respond
- `visualize_dream` — visualize dream
- `chat_mode` — Interactive chat with Assis after reading.
- `main` — process entry point

### `arena/assis_tui.py`
Assis TUI — Textual chat interface for assis_reader.py. Provides a rich terminal UI with: - Scrollable chat log with Rich markup - Multi-line input (Enter to submit, Shift+Enter for newline) - Suggestion buttons parsed from responses - Escape to interrupt LLM/muse/sleep - Background musing on silenc
- `get_selection` — Extract selected text from the log's Strip lines.
- `render_image_to_text` — Convert an image to colored half-block characters for TUI display.
- `__init__` — constructs the object
- `search` — search
- `discover` — Show all commands when palette opens with no query.
- `__init__` — constructs the object
- `compose` — builds the TUI layout
- `on_mount` — runs when the TUI mounts
- `log_message` — writes one log line
- `do_POST` — handles an HTTP POST
- `do_GET` — handles an HTTP GET
- `on_text_area_changed` — Reset silence timer on any keypress.
- `on_chat_input_submitted` — Handle Enter to submit.
- `on_button_pressed` — Handle suggestion button clicks.
- `action_copy_last` — Ctrl+Y — copy last Assis response to clipboard.
- `action_copy_all` — Ctrl+Shift+Y — copy full chat history to clipboard.
- `action_interrupt` — Escape pressed — interrupt current operation.
- `action_dream` — Ctrl+D — trigger dream cycle.
- `action_help_quit` — Ctrl+C — quit directly instead of showing hint toast.
- `action_quit` — Ctrl+Q / Ctrl+R / Ctrl+C — quit.
- `action_sleep` — Command palette: sleep.
- `action_status` — Command palette: show memory stats in chat.
- `action_hold` — Command palette: hold — Assis waits quietly.
- `action_wake` — Command palette: wake — resume Assis.
- `action_checkpoint` — Command palette: save memory to disk.
- `action_help` — Command palette: show help.
- `main` — process entry point

### `arena/chat.py`
Chat with a character after an arena run. Pick a character, talk to them. They recall from their ferricula instance — whatever survived the arena's dream cycles. Usage: python chat.py python chat.py --character Dantès Requires: - Character's ferricula container still running (docker compose up) - sh
- `pick_character` — pick character
- `get_identity` — get identity
- `recall_memories` — recall memories
- `llm_chat` — llm chat
- `main` — process entry point

### `arena/chunker.py`
Chapter splitting and keystone classification for The Count of Monte Cristo. Chapter splitting is done here (structural, based on headings). Text chunking + embedding is done by shivvr (gnosis-chunk service). Classification (keystone, emotion, characters) is applied after shivvr returns chunks.
- `split_chapters` — Split text into chapters using Gutenberg heading pattern.
- `extract_characters` — Return list of character names found in text.
- `classify_chunk` — classify chunk
- `load_texts` — Read all .txt files from docs_dir, concatenate.
- `load_chapters` — Load text and split into Chapter objects.

### `arena/clients.py`
HTTP clients for ferricula and shivvr (gnosis-chunk / shivvr v0.2). Reuses patterns from tools/ferricula-mcp.py — stdlib-only HTTP, regex-based response parsing, no third-party dependencies.
- `__init__` — constructs the object
- `embed` — Embed text via shivvr, return first chunk's vector.
- `chunk_and_embed` — Send text to shivvr's chunker.
- `available` — reports whether the remote service answers
- `parse_dream_report` — Parse dream report from ferricula response text.
- `parse_recall` — Parse recall results into list of RecallHit.
- `parse_inspect` — Parse inspect response into InspectResult.
- `parse_status` — Parse status response into StatusResult.
- `parse_remember_id` — Extract memory ID from remember response.
- `__init__` — constructs the object
- `available` — reports whether the remote service answers
- `alloc_id` — alloc id
- `remember` — stores one memory
- `recall_vector` — recall vector
- `recall_text` — recall text
- `dream` — runs one dream cycle
- `inspect` — returns the full record for one memory
- `status` — returns a status summary
- `keystone` — marks a memory as a keystone
- `connect` — creates an edge between two memories
- `offer` — POST hex entropy to /offer, get dream report with archetype activation.
- `identity` — GET /identity, return full identity JSON.
- `neighbors` — GET /neighbors/{id}, return neighbor text.
- `checkpoint` — flushes durable state to disk
- `get_row` — Get raw row JSON (tags, vector_dim — no raw vector).

### `arena/crawl_codebase.py`
- `http_post` — http post
- `http_get` — http get
- `embed_text` — Get embedding vector from shivvr v0.2.
- `get_maxid` — get maxid
- `store_memory` — Embed text and store as a memory in ferricula.
- `extract_fn_body` — Extract the function body by brace counting from the opening {.
- `parse_rust_file` — Extract all symbols from a Rust source file.
- `get_commentary` — Ask Claude for commentary on a code symbol.
- `crawl` — crawl

### `arena/crawler_client.py`
HTTP client for grub-crawl (web crawling → markdown). Crawl URLs (Wikipedia, etc.) and extract clean markdown text. Caches results locally to avoid re-crawling.
- `__init__` — constructs the object
- `crawl_markdown` — crawl markdown
- `crawl_batch` — Crawl multiple URLs, return {url: markdown}.
- `available` — reports whether the remote service answers

### `arena/eval.py`
Ferricula Evaluation Harness — empirical proof of core thermodynamic claims. Usage: python eval.py [--url URL] [--url2 URL2] [--shivvr URL] [--test TEST] [--verbose] Requires: - At least one ferricula instance (default http://localhost:8765) - shivvr embedding service (default https://shivvr.nuts.se
- `__init__` — constructs the object
- `remember` — stores one memory
- `query` — Return top-k (id, score) by cosine similarity.
- `load_corpus` — Load corpus paragraphs (>= 50 chars, split on blank lines).
- `sample_paragraphs` — Deterministic sample of n paragraphs.
- `find_themed_paragraphs` — find themed paragraphs
- `__init__` — constructs the object
- `build_vocab` — Build vocabulary from texts for fallback embedding.
- `embed` — embeds text and returns a vector
- `using_shivvr` — using shivvr
- `http_post_raw` — http post raw
- `query_sql` — POST /query with raw SQL, return rows.
- `delete_memory` — POST /delete/{id}, return success.
- `parse_neighbor_ids` — Parse neighbor IDs from /neighbors/{id} response text.
- `fresh_instance_check` — Check if instance is fresh.
- `ingest_batch` — ingest batch
- `cleanup_memories` — Delete memories by ID, best effort.
- `test_baseline_vs_ferricula` — test baseline vs ferricula
- `test_survival_curve` — test survival curve
- `test_hysteresis` — test hysteresis
- `test_graph_emergence` — test graph emergence
- `get_degree` — get degree
- `test_perturbation` — test perturbation
- `test_latency_scale` — test latency scale
- `print_summary` — print summary
- `main` — process entry point

### `arena/ocr_client.py`
HTTP client for gnosis-ocr (PDF → markdown with equations). Upload PDFs, poll for OCR completion, fetch markdown results. Caches results locally to avoid re-processing.
- `__init__` — constructs the object
- `upload` — Upload a PDF file, return session_id.
- `poll_status` — poll status
- `fetch_markdown` — Fetch the OCR'd markdown for a completed session.
- `process_pdf` — process pdf
- `available` — reports whether the remote service answers

### `arena/physics_arena.py`
Ferricula Arena — Physics: Faraday, Weber, Assis, Tesla Four scientists read physics papers section-by-section, discuss electrodynamics, and dream independently. PDFs are OCR'd via gnosis-ocr, Wikipedia bios crawled via grub-crawl, embeddings via shivvr (gnosis-chunk). Usage: docker compose -f docke
- `__init__` — constructs the object
- `run` — runs the main loop
- `main` — process entry point

### `arena/physics_chunker.py`
Section splitting and classification for physics texts. Splits OCR'd markdown into sections, classifies keystones, detects which scientists are mentioned, tags emotion.
- `split_sections` — Split markdown into sections by headings or page breaks.
- `extract_scientists` — Return list of scientist names found in text.
- `classify_chunk` — classify chunk

### `arena/radio_bridge.py`
Radio bridge — connects gnosis-radio WebSocket to Assis command port. Monitors the radio's signal level via WebSocket (port 9081), tracks the noise floor, and forwards transcriptions to Assis's command port (7273) when signal exceeds the noise floor threshold. Architecture: gnosis-radio (WS :9081) →
- `post_to_assis` — POST text to Assis command port.
- `freq_to_channel` — Convert VHF frequency to channel label.
- `bridge` — Main bridge loop — connect to radio WS, forward to Assis.
- `main` — process entry point

### `arena/seed_ferricula_memories.py`
Seed ferricula self-knowledge memories into Steve's instance.
- `embed` — embeds text and returns a vector
- `remember` — stores one memory

### `arena/serve_diagnostic.py`
Serves diagnostic.html + capture endpoint for screenshots. Run: python serve_diagnostic.py Open: http://localhost:8081/diagnostic.html POST /capture {image: "data:image/png;base64,...", filename: "..."} -> saves to captures/ dir, returns {path: "captures/filename.png"} GET /capture -> returns latest
- `__init__` — constructs the object
- `do_POST` — handles an HTTP POST
- `do_GET` — handles an HTTP GET
- `do_OPTIONS` — handles a CORS preflight
- `end_headers` — end headers

### `arena/steve.py`
Steve — autonomous agent UI: chat on one side, a view of thinking and browsing on the other, with a background think loop. Talks to a ferricula instance. (Launch notes that name a key are omitted.)
Endpoints: `/`, `/api/audio/<token>`, `/api/chat`, `/api/chat/recent`, `/api/code/<token>`, `/api/code/recent`, `/api/dashboard`, `/api/dream`, `/api/email-raw`, `/api/feed/recent`, `/api/iching`, `/api/image-mode`, `/api/image-provider`, `/api/image/<token>`, `/api/inject`, `/api/js-log`, `/api/local-mode`, `/api/nb-context`, `/api/nb-error`, `/api/nb-graph`, `/api/nb-screenshot-request`, `/api/nb-screenshot-result`, `/api/recall`, `/api/set-budget`, `/api/set-model`, `/api/status`, `/api/stream`, `/api/think/poke`, `/api/think/start`, `/api/think/stop`, `/api/upload-image`, `/dashboard`, `/presentation`, `/presentation/<token>`, `/presentation/<token>/slides`, `/presentation/slides`, `/presentations`, `/static/impress.css`, `/static/impress.js`.
- `radio_rand` — True random float from SDR entropy source.
- `model_for_emotion` — Return the model for the given emotion, with 2% SDR-gated random flip.
- `get_emotion` — get emotion
- `shift_emotion` — shift emotion
- `broadcast` — sends an event to connected clients
- `broadcast_system_update` — broadcast system update
- `tool_search` — tool search
- `tool_read_url` — tool read url
- `tool_remember` — tool remember
- `tool_purpose` — tool purpose
- `tool_recall` — tool recall
- `tool_search_documents` — Hybrid search over everything Steve has read (lume document memory).
- `tool_ask_openai` — tool ask openai
- `tool_ask_gemini` — tool ask gemini
- `tool_ask_gemma` — tool ask gemma
- `tool_thinking` — tool thinking
- `tool_time` — tool time
- `tool_ferricula_status` — tool ferricula status
- `tool_ferricula_neighbors` — tool ferricula neighbors
- `tool_ferricula_walk` — Walk the memory graph from a starting node, following edges for up to `hops` steps.
- `tool_ferricula_surface` — Surface archived/forgotten memories relevant to current context — things that have faded but might matter now.
- `tool_ferricula_connect` — Manually create a semantic edge between two memories.
- `tool_ferricula_disconnect` — Remove a semantic edge between two memories.
- `tool_ferricula_keystone` — Pin a memory as a keystone — exempt from decay, weighted in dreams.
- `tool_ferricula_inspect` — Full record for a single memory: text, fidelity, decay rate, lifecycle state, keystone status, recall count, age, staleness, graph degree.
- `tts_speak` — Generate speech via ElevenLabs.
- `tool_draw` — Generate an image — mode: auto=ComfyUI→cloud, local=ComfyUI only, cloud=skip ComfyUI.
- `tool_set_image_mode` — Switch image generation mode.
- `tool_local_mode` — Enable or disable local-only mode.
- `tool_set_budget` — Set the hourly API spend budget in dollars.
- `tool_set_model` — Switch the active LLM.
- `tool_dream` — Trigger a dream cycle — memory consolidation + visualization.
- `tool_poke` — Trigger an immediate think cycle right now, regardless of the normal interval.
- `tool_think_control` — Control the background think loop.
- `tool_advocate_control` — Control the advocate loop.
- `tool_speak` — Deliver a reply in two parts: a hook (lead) and the full body.
- `tool_run_notebook` — tool run notebook
- `tool_nb_screenshot` — Ask the browser to capture the notebook canvas and return it as a PNG.
- `tool_email_check` — tool email check
- `tool_email_read` — tool email read
- `tool_email_send` — tool email send
- `check_and_handle_mail` — Poll inbox, return count of new messages.
- `tool_list_images` — List available images Steve can attach to email.
- `tool_email_reply` — tool email reply
- `tool_email_manage` — Manage inbox: mark_read, delete, archive a message.
- `tool_open_tab` — tool open tab
- `__init__` — constructs the object
- `call` — invokes the model or tool
- `tool_terminal_status` — List all Hyperia windows, tabs, and panes (with paneId, label, pid).
- `tool_terminal_run` — tool terminal run
- `tool_terminal_screen` — Read a pane's current screen without running anything.
- `tool_terminal_keys` — Send raw keystrokes to a pane.
- `tool_terminal_new_tab` — Open a new tab, optionally running a startup command.
- `tool_terminal_split` — Split the active pane.
- `tool_terminal_focus` — Bring a pane to focus.
- `tool_terminal_close` — Close a pane (if `pane` given) or whole tab (if only `tab`).
- `tool_terminal_rename` — Rename a tab — the display name in the tab bar.
- `tool_tab_snapshot` — Full contents of all panes in a tab — better than terminal_screen when the tab is split.
- `tool_hyperia_open_web` — Open a URL in a Hyperia embedded web pane (alongside terminals).
- `tool_sticky_note_create` — Pin a sticky note in Hyperia's UI.
- `tool_sticky_note_create_code` — Pin a code sticky — syntax-highlighted, monospace.
- `tool_sticky_note_list` — List all open sticky notes (id, title, preview).
- `tool_sticky_note_read` — Read full contents of a sticky note by id.
- `tool_sticky_note_update` — Edit an existing sticky note.
- `tool_sticky_note_close` — Close (hide) a sticky note.
- `tool_sticky_note_delete` — Delete a sticky permanently.
- `tool_file_read` — tool file read
- `tool_file_write` — tool file write
- `tool_file_patch` — tool file patch
- `tool_file_list` — tool file list
- `tool_run_presentation` — Generate and serve a fullscreen impress.js presentation.
- `tool_iching` — tool iching
- `dispatch_tool` — dispatch tool
- `local_llm` — local llm
- `openai_llm` — openai llm
- `gemini_llm` — gemini llm
- `call_llm` — call llm
- `claude` — claude
- `think_cycle` — think cycle
- `dream_visualize` — The agent dreams.
- `index` — index
- `api_nb_context` — Rich context for notebook code — memory stats, recent nodes, channel distribution, emotion.
- `api_status` — api status
- `api_dashboard` — api dashboard
- `dashboard` — dashboard
- `presentation` — presentation
- `presentation_slides` — presentation slides
- `presentation_by_token` — presentation by token
- `presentation_slides_by_token` — presentation slides by token
- `presentations_index` — presentations index
- `static_impress_js` — static impress js
- `static_impress_css` — static impress css
- `api_set_budget` — api set budget
- `api_chat` — api chat
- `api_think_start` — api think start
- `api_think_stop` — api think stop
- `api_think_poke` — api think poke
- `api_set_model` — api set model
- `api_iching` — api iching
- `api_dream` — api dream
- `api_image_provider` — Return what tool_draw will actually use on the next call, given current mode/budget/availability.
- `api_image_mode` — api image mode
- `api_local_mode` — api local mode
- `api_upload_image` — Accept an image upload, save to UPLOADS_DIR, serve via /api/image/<token>, and let Steve see it.
- `api_image` — api image
- `api_audio` — api audio
- `api_stream` — api stream
- `generate` — generate
- `api_feed_recent` — api feed recent
- `api_chat_recent` — api chat recent
- `api_recall` — Notebook helper — semantic recall from ferricula.
- `api_nb_graph` — Memory node graph for notebook — diverse recalls + neighbor traversal from ferricula.
- `api_nb_screenshot_result` — Browser POSTs canvas dataUrl here after a screenshot_request SSE event.
- `api_email_raw` — Debug: return raw agentmail response so we can see the actual shape.
- `api_nb_screenshot_request` — Trigger a canvas capture without blocking — result fires screenshot_ready SSE.
- `api_code_recent` — api code recent
- `api_code_token` — api code token
- `api_js_log` — api js log
- `api_nb_error` — Receive a notebook JS error and inject it into Steve's chat so he can fix it.
- `api_inject` — Inject a message into Steve's chat.

### `arena/steve_analyze.py`
- `call_name` — Best-effort dotted name for a Call's func.
- `line_text` — line text
- `get_const_str` — get const str
- `section` — section
- `ruled` — ruled
- `__init__` — constructs the object
- `visit_FunctionDef` — visit FunctionDef
- `visit_Assign` — visit Assign
- `visit_AnnAssign` — visit AnnAssign
- `visit_Global` — visit Global
- `visit_Name` — visit Name
- `visit_Call` — visit Call
- `visit_Try` — visit Try
- `visit_Compare` — visit Compare
- `classify_http` — classify http

### `arena/steve_map.py`
- `call_name` — Best-effort dotted name for a Call's func.
- `first_str_arg` — first str arg
- `argnames` — argnames
- `line_text` — line text
- `__init__` — constructs the object
- `visit_FunctionDef` — visit FunctionDef
- `visit_Import` — visit Import
- `visit_ImportFrom` — visit ImportFrom
- `visit_Assign` — visit Assign
- `visit_AnnAssign` — visit AnnAssign
- `visit_For` — visit For
- `visit_While` — visit While
- `visit_Call` — visit Call
- `visit_Compare` — visit Compare
- `section` — section
- `ruled` — ruled

### `arena/test_api.py`
- test module — 0 test functions (quick diagnostic for the model API; key material not recorded)

### `arena/test_chonk_gpu.py`
- test module — 11 test functions: __init__, start, stop, percentile, healthcheck, split_paragraphs, load_source_paragraphs, build_samples, embed_once, run_load_test, main

### `arena/test_doc_scale.py`
- test module — 10 test functions (Validates whitepaper SLarge-Scale Document Access. Usage: python arena/test_doc_scale.py --doc monte_cristo python arena/test_doc_scale.py --doc encyclopedia Tests sub-second recall over a large indexed corpus with brute-force cosine.): http_post, http_get, embed, chunk_text, remember, recall_timed, ingest_text_file, ingest_pdf, run_recall_benchmark, main

### `arena/trek.py`
Trek — the same autonomous agent UI as Steve, bound to Trek's ferricula instance. (Launch notes that name a key are omitted.)
Endpoints: `/`, `/api/audio/<token>`, `/api/chat`, `/api/chat/recent`, `/api/code/<token>`, `/api/code/recent`, `/api/dashboard`, `/api/dream`, `/api/feed/recent`, `/api/iching`, `/api/image-mode`, `/api/image-provider`, `/api/image/<token>`, `/api/inject`, `/api/js-log`, `/api/nb-context`, `/api/nb-error`, `/api/nb-graph`, `/api/nb-screenshot-result`, `/api/recall`, `/api/set-budget`, `/api/set-model`, `/api/status`, `/api/stream`, `/api/think/poke`, `/api/think/start`, `/api/think/stop`, `/api/upload-image`, `/dashboard`.
- `radio_rand` — True random float from SDR entropy source.
- `model_for_emotion` — Return the model for the given emotion, with 2% SDR-gated random flip.
- `get_emotion` — get emotion
- `shift_emotion` — shift emotion
- `broadcast` — sends an event to connected clients
- `broadcast_system_update` — broadcast system update
- `tool_search` — tool search
- `tool_read_url` — tool read url
- `tool_remember` — tool remember
- `tool_purpose` — tool purpose
- `tool_recall` — tool recall
- `tool_ask_openai` — tool ask openai
- `tool_ask_gemini` — tool ask gemini
- `tool_ask_gemma` — tool ask gemma
- `tool_thinking` — tool thinking
- `tool_time` — tool time
- `tool_ferricula_status` — tool ferricula status
- `tool_ferricula_neighbors` — tool ferricula neighbors
- `tool_ferricula_walk` — Walk the memory graph from a starting node, following edges for up to `hops` steps.
- `tool_ferricula_surface` — Surface archived/forgotten memories relevant to current context — things that have faded but might matter now.
- `tts_speak` — Generate speech via ElevenLabs.
- `tool_draw` — Generate an image — mode: auto=ComfyUI→cloud, local=ComfyUI only, cloud=skip ComfyUI.
- `tool_set_image_mode` — Switch image generation mode.
- `tool_set_budget` — Set the hourly API spend budget in dollars.
- `tool_set_model` — Switch the active LLM.
- `tool_dream` — Trigger a dream cycle — memory consolidation + visualization.
- `tool_poke` — Trigger an immediate think cycle right now, regardless of the normal interval.
- `tool_think_control` — Control the background think loop.
- `tool_advocate_control` — Control the advocate loop.
- `tool_speak` — Deliver a reply in two parts: a hook (lead) and the full body.
- `tool_run_notebook` — tool run notebook
- `tool_nb_screenshot` — Ask the browser to capture the notebook canvas and return it as a PNG.
- `tool_email_check` — tool email check
- `tool_email_read` — tool email read
- `tool_email_send` — tool email send
- `check_and_handle_mail` — Poll inbox, return count of new messages.
- `tool_list_images` — List available images Trek can attach to email.
- `tool_email_reply` — tool email reply
- `tool_email_manage` — Manage inbox: mark_read, delete, archive a message.
- `tool_open_tab` — tool open tab
- `tool_iching` — tool iching
- `dispatch_tool` — dispatch tool
- `local_llm` — local llm
- `openai_llm` — openai llm
- `gemini_llm` — gemini llm
- `call_llm` — call llm
- `claude` — claude
- `think_cycle` — think cycle
- `dream_visualize` — Have Claude write a dream from memory fragments, then render it with DALL-E.
- `index` — index
- `api_nb_context` — Rich context for notebook code — memory stats, recent nodes, channel distribution, emotion.
- `api_status` — api status
- `api_dashboard` — api dashboard
- `dashboard` — dashboard
- `api_set_budget` — api set budget
- `api_chat` — api chat
- `api_think_start` — api think start
- `api_think_stop` — api think stop
- `api_think_poke` — api think poke
- `api_set_model` — api set model
- `api_iching` — api iching
- `api_dream` — api dream
- `api_image_provider` — api image provider
- `api_image_mode` — api image mode
- `api_upload_image` — Accept an image upload, save to UPLOADS_DIR, serve via /api/image/<token>, and let Trek see it.
- `api_image` — api image
- `api_audio` — api audio
- `api_stream` — api stream
- `generate` — generate
- `api_feed_recent` — api feed recent
- `api_chat_recent` — api chat recent
- `api_recall` — Notebook helper — semantic recall from ferricula.
- `api_nb_graph` — Memory node graph for notebook — diverse recalls + neighbor traversal from ferricula.
- `api_nb_screenshot_result` — Browser POSTs canvas dataUrl here after a screenshot_request SSE event.
- `api_code_recent` — api code recent
- `api_code_token` — api code token
- `api_js_log` — api js log
- `api_nb_error` — Receive a notebook JS error and inject it into Trek's chat so he can fix it.
- `api_inject` — Inject a message into Trek's chat.

### `arena/docs/weber-mcp.py`
Weber Simulation MCP Server — AI control layer for scene manipulation. Provides tools to inspect, build, and modify simulation scenes via natural language through Claude Code. Reads/writes geometry_config.json and optionally pushes updates to the dashboard server. Usage (stdio, registered in .mcp.js
- `weber_get_scene` — Get the full current geometry config (bodies, magnets, motor, protocol).
- `weber_get_results` — Get latest or specified CSV results summary.
- `weber_list_runs` — List available CSV result files.
- `weber_add_body` — weber add body
- `weber_move_body` — Move a body by delta {dz} or to absolute position {z}.
- `weber_remove_body` — Remove a body from the scene.
- `weber_set_body_material` — Change a body's material (recomputes mass from density).
- `weber_add_magnet` — weber add magnet
- `weber_move_magnet` — Move a magnet by delta or to absolute position.
- `weber_remove_magnet` — Remove a magnet from the scene.
- `weber_set_magnet_field` — Modify magnet properties (current, n_loops).
- `weber_add_motor` — weber add motor
- `weber_set_motor` — weber set motor
- `weber_set_protocol` — weber set protocol
- `weber_set_experiment` — Load a named experiment preset.
- `weber_save_config` — Save current config to a JSON file.
- `weber_load_config` — Load config from a JSON file.
- `weber_list_materials` — List available material presets with their properties.
- `weber_add_sensor` — weber add sensor
- `weber_move_sensor` — Move a sensor plate to a new position.
- `weber_remove_sensor` — Remove a sensor plate from the scene.
- `weber_sample_field` — Sample the B-field on a sensor plate via the dashboard server.
- `weber_capture_ui` — Capture screenshots of all dashboard viewports (3D view + widget canvases).
- `weber_cloud_status` — Check Cloud Run service health and list completed simulation runs.
- `weber_cloud_start` — Start a simulation on the Cloud Run GPU service.
- `weber_cloud_results` — Fetch simulation results from the Cloud Run service.

### `arena/docs/run/generate_reports.py`
- `discover_runs` — Find all run directories under BASE that contain CSV data.
- `find_csv` — Find the best CSV file in a run directory.
- `build_meta` — Build metadata dict from config + stderr + csv filename.
- `load_csv` — Load CSV data from a run's CSV file.
- `write_table` — Write a markdown summary + full data table.
- `plot_run` — Generate all plots for a single run.
- `plot_comparison` — Generate comparison plots across all runs.
- `main` — process entry point

### `cloudrun-test/test_client.py`
- test module — 1 test functions: main

### `ferricula-ops/cloud/billing/gateway.py`
Billing gateway: checkout, webhooks, tenant provisioning, and subscription changes. Route and function names only; configuration values are not recorded. skipped: sensitive for any key material in this file.
Endpoints: `/billing/auth/{token}`, `/billing/cancel`, `/billing/checkout`, `/billing/free`, `/billing/plans`, `/billing/portal`, `/billing/session/{session_id}`, `/billing/success`, `/billing/tenants/{email}`, `/health`.
- `generate_login_token` — Generate a secure login token.
- `send_welcome_email` — Send welcome email with magic login link via AgentMail.
- `lifespan` — lifespan
- `health` — returns a health check
- `create_checkout` — Create a Stripe Checkout Session for a paid plan.
- `create_free_tier` — Provision a free-tier tenant (1 agent, no payment required).
- `checkout_success` — Redirect to dashboard after successful Stripe checkout.
- `get_session_tenant` — Look up tenant info from a Stripe checkout session ID.
- `auth_by_token` — Validate a login token and return tenant info.
- `checkout_cancel` — checkout cancel
- `get_tenant` — Look up a tenant by email.
- `create_portal_session` — Create a Stripe Customer Portal session for managing subscriptions.
- `list_plans` — List available plans and pricing.

### `ferricula-ops/cloud/billing/provision.py`
Tenant provisioning for ferricula-cloud. Creates and manages Cloud Run services for each tenant. Uses the Cloud Run Admin API v2 via google-cloud-run client library. Each tenant gets: - 1+ Cloud Run services (ferricula-tenant-{tenant_id}-{n}) - GCS prefix for persistence (gs://{bucket}/{tenant_id}/)
- `__init__` — constructs the object
- `available` — reports whether the remote service answers
- `load` — Load the tenant registry from GCS.
- `save` — Save the tenant registry to GCS.
- `__init__` — constructs the object
- `get_tenant` — get tenant
- `find_tenant_by_stripe_customer` — find tenant by stripe customer
- `find_tenant_by_token` — find tenant by token
- `update_tenant_field` — update tenant field
- `email_to_tenant_id` — Deterministic tenant ID from email.
- `provision` — creates the agent's container and data
- `update_plan` — update plan
- `suspend` — Suspend a tenant — scale all services to 0 max instances.
- `reactivate` — Reactivate a suspended tenant.
- `delete_tenant` — Delete all services and data for a tenant.

### `ferricula-ops/cloud/billing/stripe_webhooks.py`
Stripe webhook handler for ferricula-cloud. Handles: - checkout.session.completed → provision tenant - customer.subscription.updated → upgrade/downgrade agents - customer.subscription.deleted → suspend tenant - invoice.payment_failed → warn, eventually suspend
Endpoints: `/webhooks`.
- `stripe_webhook` — Receive and process Stripe webhook events.

### `ferricula-ops/site/tools/ferricula-mcp.py`
Ferricula Cognitive MCP Server — thermodynamic memory for AI agents. Spawns the ferricula binary as a subprocess and exposes cognitive memory operations as MCP tools over stdio. Supports two transport modes: - subprocess (default): spawns ferricula binary, communicates via stdin/stdout - HTTP: conne
- `__init__` — constructs the object
- `send` — Send request to ferricula HTTP service, return response body.
- `get` — sends an HTTP GET
- `post` — sends an HTTP POST
- `available` — reports whether the remote service answers
- `__init__` — constructs the object
- `embed` — Embed text via shivvr, return dense vector.
- `invert` — Invert a vector back to approximate text via shivvr.
- `health` — Check shivvr health.
- `available` — Check if shivvr is reachable.
- `__init__` — constructs the object
- `append` — Serialize one seed line and append to the journal.
- `read_all` — Read all seed entries.
- `exists` — True if journal file exists and has content.
- `__init__` — constructs the object
- `next` — returns the next item
- `__init__` — constructs the object
- `drain_clock_events` — Return and clear buffered [clock] events.
- `send` — Send a command and return the response.
- `shutdown` — shuts the child process down
- `ferricula_remember` — ferricula remember
- `ferricula_recall` — Search memories by text.
- `ferricula_inspect` — Inspect a memory — shows reconstructed text, fidelity, emotion, graph.
- `ferricula_observe` — Observe a file — creates a keystone reference node in the knowledge graph.
- `ferricula_reflect` — Record a thought — working memory with faster decay.
- `ferricula_health` — Check health of ferricula and shivvr (embedding service).
- `ferricula_dream` — Run a dream cycle: decay, forgive, consolidate, neglect, review, prune.
- `ferricula_status` — Get memory system status: counts of active/forgiven/archived memories, graph nodes/edges, prime tree terms.
- `ferricula_keystone` — Toggle keystone status on a memory.
- `ferricula_connect` — Create a graph edge between two memories.
- `ferricula_disconnect` — Remove the graph edge between two memories.
- `ferricula_neighbors` — Get all graph neighbors of a memory with edge labels and fidelity.
- `ferricula_terms` — List all terms in the prime tree with member counts.
- `ferricula_query` — Run a raw SQL query against the store.
- `ferricula_checkpoint` — Flush current state to V2 snapshot and clear WAL.
- `ferricula_identity` — Get the agent's identity: hexagram, horoscope, archetypes, emotions.
- `ferricula_inversion_check` — Check semantic fidelity of a memory via vec2text inversion.
- `ferricula_clock` — Get clock telemetry: ticks, dreams, entropy stats, radio status.
- `ferricula_offer_entropy` — Inject entropy into ferricula to trigger a dream cycle.

### `paper/render_pdf.py`
Render ferricula white paper as two-column PDF using fpdf2.
- `__init__` — constructs the object
- `header` — header
- `footer` — footer
- `blank` — blank
- `section` — section
- `subsection` — subsection
- `para` — para
- `bold_para` — bold para
- `italic_para` — italic para
- `small` — small
- `bullet` — bullet
- `table` — table
- `code` — code
- `equation` — equation
- `force_new_column` — force new column
- `build` — build

### `scripts/book_is_a_person.py`
- `post_json` — POST JSON, return parsed response or None.
- `get_json` — GET JSON, return parsed response or None.
- `alive` — Check if ferricula is reachable.
- `shivvr_alive` — Check if shivvr is reachable.
- `hash_vector` — Generate a deterministic unit vector from text via hashing.
- `shivvr_embed` — Embed via shivvr /memory/{session}/ingest.
- `extract_terms` — Extract top N content words as terms.
- `detect_emotion` — Crude keyword-based emotion detection.
- `download_book` — Download or use cached copy of Count of Monte Cristo.
- `split_chapters` — Split Gutenberg text into chapters.
- `main` — process entry point

### `site/tools/ferricula-mcp.py`
Ferricula Cognitive MCP Server — thermodynamic memory for AI agents. Spawns the ferricula binary as a subprocess and exposes cognitive memory operations as MCP tools over stdio. Supports two transport modes: - subprocess (default): spawns ferricula binary, communicates via stdin/stdout - HTTP: conne
- `__init__` — constructs the object
- `send` — Send request to ferricula HTTP service, return response body.
- `get` — sends an HTTP GET
- `post` — sends an HTTP POST
- `available` — reports whether the remote service answers
- `__init__` — constructs the object
- `embed` — Embed text via shivvr, return dense vector.
- `invert` — Invert a vector back to approximate text via shivvr.
- `health` — Check shivvr health.
- `available` — Check if shivvr is reachable.
- `__init__` — constructs the object
- `append` — Serialize one seed line and append to the journal.
- `read_all` — Read all seed entries.
- `exists` — True if journal file exists and has content.
- `__init__` — constructs the object
- `next` — returns the next item
- `__init__` — constructs the object
- `drain_clock_events` — Return and clear buffered [clock] events.
- `send` — Send a command and return the response.
- `shutdown` — shuts the child process down
- `ferricula_remember` — ferricula remember
- `ferricula_recall` — Search memories by text.
- `ferricula_inspect` — Inspect a memory — shows reconstructed text, fidelity, emotion, graph.
- `ferricula_observe` — Observe a file — creates a keystone reference node in the knowledge graph.
- `ferricula_reflect` — Record a thought — working memory with faster decay.
- `ferricula_health` — Check health of ferricula and shivvr (embedding service).
- `ferricula_dream` — Run a dream cycle: decay, forgive, consolidate, neglect, review, prune.
- `ferricula_status` — Get memory system status: counts of active/forgiven/archived memories, graph nodes/edges, prime tree terms.
- `ferricula_keystone` — Toggle keystone status on a memory.
- `ferricula_connect` — Create a graph edge between two memories.
- `ferricula_disconnect` — Remove the graph edge between two memories.
- `ferricula_neighbors` — Get all graph neighbors of a memory with edge labels and fidelity.
- `ferricula_terms` — List all terms in the prime tree with member counts.
- `ferricula_query` — Run a raw SQL query against the store.
- `ferricula_checkpoint` — Flush current state to V2 snapshot and clear WAL.
- `ferricula_identity` — Get the agent's identity: hexagram, horoscope, archetypes, emotions.
- `ferricula_inversion_check` — Check semantic fidelity of a memory via vec2text inversion.
- `ferricula_clock` — Get clock telemetry: ticks, dreams, entropy stats, radio status.
- `ferricula_offer_entropy` — Inject entropy into ferricula to trigger a dream cycle.

### `src/archetypes.rs`
Archetype sub-agents — five roles cast from identity entropy.  Each archetype has its own hexagram, horoscope, and emotion profile. Phase 1: initialized, persisted, and logged. Behavioral effects are stubs.
- `name` — returns the display name of this variant
- `all` — returns every variant
- `name` — returns the display name of this variant
- `activate` — marks the archetype active
- `deactivate` — marks the archetype inactive
- `cast_all_archetypes` — Cast all 5 archetypes from identity entropy.
- `activation_tier` — Classify entropy intensity into archetype activation tiers.
- `active_roles` — Which roles are active at this tier.

### `src/auth.rs`
Auth shell for the operator console — Phase 1.  Phase 1 implements: - Off: anything goes - Local (default): loopback / docker bridge addresses allowed - NutsAuth: stub returning 501 for any non-public route  Phase 2 will land real JWT verification against a JWKS endpoint.  `is_public_route` defines
Endpoints: `GET`, `OPTIONS`, `POST`, `PUT`.
- `allow` — builds an allow decision
- `deny` — builds a deny decision with a reason
- `from_env` — Read FERRICULA_AUTH_MODE: "off" | "local" | "nuts_auth".
- `verify` — Decide whether to allow a request.
- `is_public_route` — Public routes — never gated.

### `src/bb25.rs`
Bayesian BM25 (BB25): calibrated probability from BM25 relevance scores. Full port of bayesian-bm25's probability.py — deterministic transform of BM25 into posterior probabilities via sigmoid calibration + Bayesian priors.
- `new` — constructs a default or empty value
- `add_document` — Add a document's terms to the corpus.
- `remove_document` — Remove a document from the corpus.
- `avgdl` — returns the average document length
- `idf` — returns the inverse document frequency of a term
- `sigmoid` — Numerically stable sigmoid.
- `logit` — Logit (inverse sigmoid) with clamping.
- `clamp_prob` — Clamp probability to avoid log(0).
- `bm25_term_score` — Classic BM25 score for a single term in a single document.
- `likelihood` — P(relevant | score) via sigmoid calibration.
- `tf_prior` — Term frequency prior: 0.2 base, saturates at tf=10.
- `norm_prior` — Document length normalization prior.
- `composite_prior` — Composite prior blending tf + normalization.
- `posterior` — Bayesian posterior: P(relevant | score, prior, base_rate).
- `score_to_probability` — Full pipeline: score → calibrated probability.
- `update` — Update calibration parameters from a relevance judgment.
- `auto_estimate` — Estimate (alpha, beta) from corpus statistics by sampling pseudo-queries.
- `log_odds_fusion` — Combine two probability scores via balanced log-odds fusion.

### `src/casting.rs`
Hexagram + horoscope casting system — pure arithmetic, no external deps.  Ports the casting logic from `myoo/mingwang/memex/casting.py`. Yarrow stalk probabilities from entropy bytes, King Wen table lookup, trigram-to-emotion mapping, zodiac from epoch.
- `name` — returns the display name of this variant
- `name` — returns the display name of this variant
- `name` — returns the display name of this variant
- `cast_hexagram` — Cast a hexagram from 6 entropy bytes.
- `trigram_emotion` — Emotion from a trigram index.
- `zodiac_from_month_day` — Derive zodiac sign from month and day.
- `zodiac_from_epoch` — Derive zodiac from a Unix epoch timestamp.
- `identity_seed` — Generate a deterministic identity seed from hexagram data.
- `seed_to_vector` — Generate an identity vector from a seed for anchor memory.
- `seed_to_vector_dim` — Generate an identity vector of a specific dimension from a seed.

### `src/clock.rs`
Entropy-driven clock — the radio IS the time source.  Spawns a background thread that polls gnosis-radio for time + entropy. Emits `ClockEvent`s over an mpsc channel. Without the radio, time does not flow and the memory system stays frozen.
Endpoints: `entropy_hex`, `epoch`.
- `from_env` — reads configuration from the environment
- `spawn_clock` — Spawn the clock thread.
- `fetch_radio_entropy` — Fetch time and entropy from the radio via raw HTTP/1.0 GETs.
- `hex_decode` — Decode a hex string to bytes.

### `src/corpus.rs`
Corpus-level statistics for BM25 search, maintained incrementally. Wires bb25 scoring through prime_tree posting lists.
- `new` — constructs a default or empty value
- `add_document` — Index a document's text into corpus stats.
- `remove_document` — Remove a document from corpus stats.
- `recalibrate` — Recalibrate alpha/beta from current corpus.
- `bm25_search` — BM25 search using prime_tree as the inverted index.
- `hybrid_search` — Hybrid search: combine vector cosine scores with BM25.
- `snapshot` — Serialize for persistence.
- `load_snapshot` — Load from persistence snapshot.

### `src/dream.rs`
Endpoints: `embedding`.
- `dream_cycle` — Run one dream cycle at full intensity (manual `dream` command).
- `dream_cycle_with_intensity` — Run one dream cycle with entropy-modulated decay.

### `src/ec_key.rs`
Elliptic-curve key agreement (X25519) + HKDF seed derivation for transform keys. Deterministic, no RNG at call sites: callers must supply 32-byte private keys from SDR.
- `derive_shared_secret` — Derive a shared secret from our 32-byte private key and their 32-byte public key.
- `seed_from_shared` — HKDF-SHA256 to produce a seed for orthogonal transform generation.
- `public_from_private` — Generate a public key from a 32-byte private key (deterministic).
- `parse_hex_key` — Parse hex-encoded 32-byte key.

### `src/embed.rs`
Embeddable server — run Ferricula's HTTP service inside another binary.  Call `spawn_embedded(data_dir, port)` to start the full Ferricula engine with HTTP service on a background thread. Returns a handle to stop it.
Endpoints: `decay_alpha`, `emotion`, `id`, `ids`, `importance`, `keystone`, `primary`, `query`, `secondary`, `tags`, `text`, `vector`.
- `stop` — stops the background service
- `spawn_embedded` — Spawn the full Ferricula engine + HTTP server on background threads.

### `src/engine.rs`
- `new` — constructs a default or empty value
- `upsert` — inserts or replaces a row
- `delete` — removes a row by id
- `get` — sends an HTTP GET
- `row_count` — returns how many rows are stored
- `all_bitmap` — returns every id as a bitmap
- `bitmap_for_tag_eq` — bitmap for tag eq
- `bitmap_for_tag_range` — Returns a bitmap of all IDs where `field` parses as `u32` and the parsed value satisfies the requested numeric range.
- `distinct_tag_values` — Returns all distinct tag values for `field`, sorted.
- `bitmap_jaccard` — returns the Jaccard overlap of two bitmaps
- `vector_topk` — returns the nearest rows by vector score
- `vector_topk_bitmap` — returns the nearest rows inside a bitmap
- `execute_sql` — runs a SQL query against the memory store
- `execute_sql_with_embed` — runs SQL, embedding text predicates through the embedding service
- `rows_iter` — iterates stored rows

### `src/graph.rs`
- `new` — constructs a default or empty value
- `connect` — Connect two memories with a labeled, weighted edge.
- `push_edge_observation` — Push an observed similarity between two nodes into the per-edge dynamics history.
- `edge_bracket` — Weber bracket `B = ṡ² + s · s̈` for the edge between `a` and `b`.
- `edge_dynamics_count` — Number of edges with recorded dynamics.
- `disconnect` — Remove the edge between two memories.
- `neighbors` — Forward-reachable neighbors of a node.
- `predecessors` — Causal predecessors — nodes with causal edges pointing TO this node.
- `edge` — The edge between two nodes, if any.
- `degree` — Degree centrality (number of forward connections).
- `remove_node` — Remove a node and all its edges.
- `node_count` — Nodes that have at least one edge.
- `edge_count` — Total edges.
- `neighborhood_2` — Two-hop neighborhood: neighbors of neighbors, excluding self.
- `all_edges` — Snapshot all edges for persistence.
- `load_edges` — Rebuild from persisted edges.

### `src/http.rs`
HTTP service thread — tiny_http server with channel-based command dispatch.  Each request is parsed into an `HttpCommand` and sent to the main thread over a sync channel. The main thread processes commands against the DurableEngine and sends JSON responses back.
- `spawn_http` — Spawn the HTTP server thread.

### `src/identity.rs`
Identity system — singleton agent identity with hexagram + horoscope.  Persists as `identity.json` in the data directory. Created once from entropy, then loaded on subsequent starts.
- `from_entropy` — Derive per-agent thermodynamic constants from physical entropy.
- `to_json` — Serialize to JSON.
- `save` — Persist this identity to `<data_dir>/identity.json`.
- `recast_thermo` — Re-derive thermodynamic constants from fresh physical entropy.
- `activate_for_tier` — Activate archetypes based on dream intensity tier.
- `activate_from_report` — Activate archetypes based on a completed dream report.
- `apply_passive_cooling` — Apply passive cooling based on elapsed time since last update.
- `add_recall_heat` — Add heat from a recall transaction.
- `dream_cool` — Cool the agent after a dream cycle.
- `active_resonance_gates` — Return resonance gates for currently active archetypes.
- `load_or_create` — loads the persisted identity, or casts one from entropy
- `create_anchor` — Create the anchor memory Row + MemoryRecord for a new identity.

### `src/inversion.rs`
Semantic fidelity verification via vec2text inversion.  After consolidation warps a memory's vector, invert it back to text via shivvr and compare with the original text tag. This measures whether the manifold warping preserved semantic content.
Endpoints: `chunks`, `embedding`, `hypothesis`, `text`.
- `embed_text` — Call shivvr to embed text, return the embedding vector.
- `invert_vector` — Call shivvr `/invert` with a vector, return approximate text.
- `text_similarity` — Jaccard similarity on whitespace-tokenized word sets.
- `shivvr_available` — Check if shivvr is reachable.
- `check_inversion_with_data` — Full inversion check for a memory.

### `src/lib.rs`

### `src/main.rs`
Endpoints: `a`, `b`, `context`, `decay_alpha`, `emotion`, `id`, `ids`, `importance`, `k`, `keystone`, `kind`, `label`, `llm_planner_key`, `llm_planner_model`, `llm_planner_url`, `primary`, `query`, `radio_url`, `ref`, `secondary`, `shivvr_url`, `tags`, `text`, `vector`, `weight`.

### `src/mcp.rs`
Native streamable-HTTP MCP server.  Mounted at `/mcp` on `MCP_PORT` (or `serve_port + 1`). Tools proxy to the existing ferricula tiny_http API on `serve_port`. Embeddings (for `remember`/`reflect`/`observe`) are obtained from shivvr at `{shivvr_url}/temp/ferricula/ingest` before storage.
Endpoints: `/checkpoint`, `/clock`, `/connect`, `/disconnect`, `/dream`, `/dream/latest`, `/identity`, `/query`, `/recall`, `/remember`, `/status`, `/terms`, `chunks`, `embedding`, `entropy_hex`.
- tool — Store a memory in ferricula. Embeds the text via shivvr, then writes a record with the given channel (hearing/seeing/thinking/body/taste/smell). Optional importance (0..1) and keystone flag. Returns the stored row id.
- tool — Recall memories matching a natural-language query. Ferricula handles embedding internally and returns ranked hits.
- tool — Record a reflective thought. Stored on the 'thinking' channel with alpha=0.015. Optional importance.
- tool — Record an observation about a file or external artifact. Stored as a keystone on the 'seeing' channel with type=file and the given path.
- tool — Inspect a memory record by id. Returns the full row.
- tool — Get ferricula runtime status (counts, last events, thermodynamic state).
- tool — Get ferricula's identity (agent_id, name, archetype, casting).
- tool — Combined health check: ferricula /status plus shivvr /health.
- tool — Trigger a dream cycle (consolidation pass). Returns the dream report.
- tool — Mark a memory as a keystone (resists decay). Provide its id.
- tool — List graph neighbors of a memory (ids and edge labels).
- tool — Connect two memories with an optional label and edge kind.
- tool — Remove the edge between two memories.
- tool — Get clock/cadence telemetry (heat, drift, dream cadence).
- tool — Force a checkpoint of the durable engine to disk.
- tool — List vocabulary terms tracked by ferricula's tokenizer.
- tool — Run a SQL query against the memory store. Pass the query as 'sql'.
- tool — Run an inversion check on a memory id (compares stored vector against re-embedding to detect drift).
- tool — Embody: gather identity, status, and the latest dream into a single contextual snapshot. Optional 'memories' is reserved for future expansion.
- tool — Offer entropy to the engine. With source='radio', fetches 64 bytes from the radio entropy API; otherwise pass a hex string as 'source' to be offered directly.
- `new` — constructs a default or empty value
- `streamable_http_service` — streamable http service
- `run_mcp_server` — run mcp server

### `src/memory.rs`
- `now_epoch` — returns the current unix time
- `new` — constructs a default or empty value
- `new_at` — Create with a fixed timestamp (for testing / deserialization).
- `decay_tick` — One tick of exponential decay.
- `effective_alpha` — α_eff = α / (1 + ln(1 + consolidation_depth))
- `on_recall` — Called when this memory is recalled.
- `on_neglect` — Called during dream for memories not recently recalled.
- `on_halo_touch` — Called during dream for memories that are direct graph neighbors of a keystone.
- `above_gate` — True if fidelity is at or above the survival gate.
- `forgive` — Active → Forgiven.
- `archive` — Forgiven → Archived.
- `staleness` — Seconds since last recall.
- `age` — Age in seconds.
- `resonates` — Wheeler-Feynman resonance check — does this memory respond to the query? Keystones always resonate.
- `new` — constructs a default or empty value
- `insert` — inserts one record
- `get` — sends an HTTP GET
- `get_mut` — fetches one record for modification
- `remove` — removes one record
- `contains` — reports whether the id is present
- `len` — returns how many entries are stored
- `is_empty` — reports whether the collection is empty
- `iter` — iterates the records
- `iter_mut` — iterates the records mutably
- `in_state` — All records in a given lifecycle state.
- `keystones` — All keystone records.
- `all_records` — Snapshot all records for persistence.
- `load_records` — Load records from a persistence snapshot.

### `src/model.rs`
- `is_empty` — reports whether the collection is empty
- `to_tags` — Flatten reference fields to `(tag_key, tag_value)` pairs for the bitmap tag index.

### `src/pali.rs`
Pāḷi translation layer — bidirectional term expansion between Abhidhamma vocabulary and computational implementation terms.  Sits between raw text and embedding: expands Pali terms to their code equivalents (and vice versa) so both vocabularies land in the same vector neighborhood. "anicca" and "dec
- `expand` — Expand text by appending Pali↔code equivalents for any recognized terms.
- `has_pali` — Check if text contains any recognized Pali terms.
- `glossary_json` — Get the full glossary as JSON for API exposure.

### `src/persist.rs`
- `open` — opens the store or handle
- `upsert` — inserts or replaces a row
- `delete` — removes a row by id
- `execute_sql` — runs a SQL query against the memory store
- `execute_sql_with_embed` — runs SQL, embedding text predicates through the embedding service
- `remember` — stores one memory
- `update_record` — updates one stored memory record
- `remove_memory` — removes one memory from the durable engine
- `connect` — creates an edge between two memories
- `disconnect` — removes the edge between two memories
- `insert_term` — inserts a term into the prime tree
- `dream` — runs one dream cycle
- `dream_with_intensity` — dream with intensity
- `checkpoint` — flushes durable state to disk
- `engine` — borrows the durable engine
- `engine_mut` — borrows the durable engine mutably
- `memory_store` — borrows the memory store
- `memory_store_mut` — borrows the memory store mutably
- `graph` — borrows the graph
- `graph_mut` — borrows the graph mutably
- `prime_tree` — borrows the prime tree
- `prime_tree_mut` — borrows the prime tree mutably
- `skg` — borrows the semantic knowledge graph
- `skg_mut` — borrows the semantic knowledge graph mutably

### `src/planner.rs`
Endpoints: `content`, `text`.
- `new` — constructs a default or empty value
- `needs_llm` — Check if input needs LLM rewrite (not already SQL, key available).
- `rewrite_query_sync` — Synchronous rewrite — rule-based only, never blocks on network.
- `rewrite_query` — Original synchronous rewrite — tries LLM then falls back to rules.
- `spawn_llm_rewrite` — Spawn LLM rewrite on a background thread.

### `src/prime_tree.rs`
- `new` — constructs a default or empty value
- `insert` — Insert a memory into the partition for `term`.
- `remove_member` — Remove a memory ID from every node in the tree.
- `split` — Split a node when its member count exceeds its prime.
- `merge` — Merge: the operation SlothANN was missing.
- `search` — Search for all memory IDs associated with a term (prefix match).
- `search_exact` — Exact term search — only matches root-level terms.
- `collect_members` — Collect all member IDs from a node and its descendants.
- `terms` — All term strings at root level.
- `node_count` — returns how many nodes have an edge
- `root_count` — returns how many root terms exist
- `total_members` — Total unique members across the tree.
- `snapshot` — Snapshot for persistence.
- `load_snapshot` — Load from persistence snapshot.

### `src/server_config.rs`
Runtime-mutable server configuration.  Persisted at `<data_dir>/server_config.toml`. Env vars override file values at load time so the existing docker-compose env-setting behavior keeps working — this struct never silently breaks an operator-set var.  Layered precedence at `load()`: env var  >  file
- `load` — Layered load: env > file > defaults.
- `save` — Atomic write: temp file + fsync + rename.
- `redacted_json` — JSON for GET /admin/config.

### `src/skg.rs`
Semantic Knowledge Graph — materialized edges from prime tree posting list intersections.  Grainger's SKG projects relationships dynamically from bitmap intersections. Weber's bracket `[ṡ² + s·s̈]` tracks velocity and acceleration of semantic distance (Jaccard similarity) over dream cycles.
- `new` — constructs a default or empty value
- `push` — Push a new (tick, score) observation.
- `len` — Number of entries currently stored.
- `last_tick` — Most recent tick in the buffer.
- `velocity` — Compute velocity (ṡ) from the two most recent entries.
- `acceleration` — Compute acceleration (s̈) from the three most recent entries.
- `weber_bracket` — Weber bracket: B = ṡ² + s · s̈ where s is the most recent Jaccard score.
- `current_jaccard` — Most recent Jaccard score.
- `new` — constructs a default or empty value
- `update` — Run one SKG update cycle during dream.
- `snapshot` — Create a snapshot for persistence.
- `load_snapshot` — Load from a snapshot.
- `edges_for_term` — Get all edges for a specific term.

### `src/sparse.rs`
Sparse inverted index stubs for term-based scoring (BM25/BB25). This is a minimal placeholder so agents can wire in a real implementation later.
- `add_doc` — adds one document to the sparse index stub
- `df` — returns the document frequency of a term
- `avg_dl` — returns the average document length

### `src/sql.rs`
- `execute_sql` — runs a SQL query against the memory store
- `execute_sql_with_embed` — runs SQL, embedding text predicates through the embedding service

### `src/tokenizer.rs`
Word-level tokenizer + Porter stemmer for BM25 indexing. Self-contained — no external crates required.
- `tokenize` — Tokenize text into lowercase words, filtering stopwords and short tokens.
- `extract_terms` — Tokenize + stem: the primary pipeline for term extraction.
- `stem` — Apply the Porter stemming algorithm to a single word.

### `src/transform.rs`
Geometric trust: vector encryption via seed-derived permutation + sign flip.  Preserves cosine similarity when the same key is used for both vectors. Destroys similarity when keys differ. O(n) space and time.
- `from_seed` — Build a transform from a 32-byte seed for the given dimension.
- `encrypt` — Encrypt a vector: permute then flip signs.
- `decrypt` — Decrypt a vector: reverse sign flips then unpermute.
- `orthogonal_from_seed` — orthogonal from seed
- `warp` — warp

### `tools/ferricula-code.py`
ferricula-code: Index a codebase into ferricula as keystones. Walks a directory, summarizes each significant file, and stores each as a keystone memory in ferricula. Enables semantic code search across all indexed projects via ferricula_recall. The codebase becomes part of the agent's permanent memo
- `summarize_file` — Create a summary of a file for indexing.
- `walk_project` — Walk a project directory and return indexable files.
- `index_project` — Index a codebase into ferricula as keystone memories.
- `search_code` — Search indexed code via ferricula memory.
- `index_file` — Index a single file into ferricula as a keystone memory.

### `tools/ferricula-mcp.py`
Ferricula Cognitive MCP Server — thermodynamic memory for AI agents. Spawns the ferricula binary as a subprocess and exposes cognitive memory operations as MCP tools over stdio. Supports two transport modes: - subprocess (default): spawns ferricula binary, communicates via stdin/stdout - HTTP: conne
- `__init__` — constructs the object
- `send` — sends one request and returns the response
- `get` — sends an HTTP GET
- `post` — sends an HTTP POST
- `post_raw` — post raw
- `available` — reports whether the remote service answers
- `__init__` — constructs the object
- `embed` — Embed text via shivvr, return dense vector.
- `invert` — Invert a vector back to approximate text via shivvr.
- `health` — Check shivvr health.
- `available` — Check if shivvr is reachable.
- `__init__` — constructs the object
- `append` — Serialize one seed line and append to the journal.
- `read_all` — Read all seed entries.
- `exists` — True if journal file exists and has content.
- `__init__` — constructs the object
- `next` — returns the next item
- `__init__` — constructs the object
- `drain_clock_events` — Return and clear buffered [clock] events.
- `send` — Send a command and return the response.
- `shutdown` — shuts the child process down
- `ferricula_remember` — ferricula remember
- `ferricula_recall` — Search memories by text.
- `ferricula_inspect` — Inspect a memory — shows reconstructed text, fidelity, emotion, graph.
- `ferricula_observe` — Observe a file — creates a keystone reference node in the knowledge graph.
- `ferricula_reflect` — Record a thought — working memory with faster decay.
- `ferricula_health` — Check health of ferricula and shivvr (embedding service).
- `ferricula_dream` — Run a dream cycle: decay, forgive, consolidate, neglect, review, prune.
- `ferricula_status` — Get memory system status: counts of active/forgiven/archived memories, graph nodes/edges, prime tree terms.
- `ferricula_keystone` — Toggle keystone status on a memory.
- `ferricula_connect` — Create a graph edge between two memories.
- `ferricula_disconnect` — Remove the graph edge between two memories.
- `ferricula_neighbors` — Get all graph neighbors of a memory with edge labels and fidelity.
- `ferricula_terms` — List all terms in the prime tree with member counts.
- `ferricula_query` — Run a raw SQL query against the store.
- `ferricula_checkpoint` — Flush current state to V2 snapshot and clear WAL.
- `ferricula_identity` — Get the agent's identity: hexagram, horoscope, archetypes, emotions.
- `ferricula_inversion_check` — Check semantic fidelity of a memory via vec2text inversion.
- `ferricula_clock` — Get clock telemetry: ticks, dreams, entropy stats, radio status.
- `ferricula_offer_entropy` — Inject entropy into ferricula to trigger a dream cycle.
- `ferricula_embody` — Embody a ferricula character — load identity and available mental state.
- `ferricula_discover` — Scan ports for running ferricula instances, register them by name.
- `ferricula_list_characters` — List all registered ferricula character instances.

## ferricula_v2

Rust workspace that fuses three projects, per its README: ferricula (thermodynamic memory), lume (FST and BM25 document search), and shivvr (chunking, embeddings, inversion). The Steve runtime slice adds recovered-memory recall, pause sleep and wake, Hacker News and Nuts News reads, mention deliberation, wisdom-king whispers, and a budgeted model router. The nested `research/original-ferricula/source` code files that also exist in ferricula-original are byte-identical and are not listed again. `research/nuts-news/source` is a separate product and is listed after the crates.

v2's server HTTP surface (`crates/ferricula-server/src/api.rs`) is the operator control plane: `/health`, `/status`, `/identity`, `/memory/recall`, `/memory/overlay`, `/memory/overlay/approve/{event_id}`, `/control/advocate`, `/control/autonomy`, `/control/mode`, `/control/pause`, `/control/schedule`, `/control/schedule/plan`, `/control/sleep`, `/control/wake`, `/models/status`, `/tasks`, `/tasks/considerations`, `/tasks/mention`, `/tasks/read-feed`, `/tasks/read-hacker-news`, `/tasks/{id}`, `/wisdom/preview`.

### `crates/ferricula-cognition/src/abhidharma.rs`
- `new` — constructs a default or empty value

### `crates/ferricula-cognition/src/advocate.rs`
Bounded, provider-neutral **Advocate** advisory primitive.
- `is_zero` — reports whether the value is zero
- `sanitized` — Bound and sanitize snippet lists (length + char caps).
- `fingerprint` — Cheap stable fingerprint for provenance (not a security hash).
- `should_skip_model_review` — Whether the integrator should skip a **model** review this cycle.
- `as_str` — returns the stable string form
- `asserts_zero_authority` — Structural invariant used by tests and integrators.
- `is_expired` — is expired
- `from_parsed` — Build from a model-parsed wants/verdict pair plus the original input.
- `validate` — checks the value against its bounds
- `review_mechanical` — Run a **mechanical** advisory review (no model).
- `parse_wants_verdict` — Parse arena-style two-line model output: `WANTS: ...` / `VERDICT: ...` (case-insensitive, flexible spacing).
- `should_defer_history_write` — Whether overlay advisory-history persistence should be deferred (spec: ≥75% budget).
- `advocate_system_preamble` — System-role text for integrators that still call a model.

### `crates/ferricula-cognition/src/agency.rs`
Sovereign choice belongs to the simulated person, not to trigger code.
- `awaiting_choice` — Safe state before cognition has made a choice.

### `crates/ferricula-cognition/src/casting.rs`
Hexagram + horoscope casting system — pure arithmetic, no external deps.
- `name` — returns the display name of this variant
- `name` — returns the display name of this variant
- `name` — returns the display name of this variant
- `cast_hexagram` — Cast a hexagram from 6 entropy bytes.
- `trigram_emotion` — Emotion from a trigram index.
- `zodiac_from_month_day` — Derive zodiac sign from month and day.
- `zodiac_from_epoch` — Derive zodiac from a Unix epoch timestamp.
- `identity_seed` — Generate a deterministic identity seed from hexagram data.
- `seed_to_vector` — Generate an identity vector from a seed for anchor memory.
- `seed_to_vector_dim` — Generate an identity vector of a specific dimension from a seed.

### `crates/ferricula-cognition/src/clock.rs`
Entropy-driven clock — the radio IS the time source.
- `from_env` — reads configuration from the environment
- `spawn_clock` — Spawn the clock thread.
- `fetch_radio_entropy` — Fetch time and entropy from the radio via raw HTTP/1.0 GETs.
- `hex_decode` — Decode a hex string to bytes.

### `crates/ferricula-cognition/src/dream.rs`
- `dream_cycle` — Run one dream cycle at full intensity (manual `dream` command).
- `dream_cycle_with_intensity` — Run one dream cycle with entropy-modulated decay.

### `crates/ferricula-cognition/src/emotion.rs`
Provider-neutral Plutchik affect + somatic state for Steve.
- `as_str` — returns the stable string form
- `parse` — parses the string form back into the value
- `as_str` — returns the stable string form
- `components` — components
- `from_pair` — Lookup blend for an unordered pair of bases.
- `as_str` — returns the stable string form
- `parse` — parses the string form back into the value
- `is_surf_state` — Arena "surf" states — curiosity/restless modes (informational only).
- `clamp01` — clamps a number into 0..1
- `smooth_toward` — Exponential lag toward a target: `old*(1-α) + target*α`.
- `description` — Short prose fragment for prompts (arena `_somatic_description` style).
- `somatic_target` — Target somatic pose for an affect label (from arena maps; defaults otherwise).
- `as_str` — returns the stable string form
- `primary` — primary
- `model_demand_for` — Map affect → capability demand (arena EMOTION_MODEL, de-branded).
- `new` — constructs a default or empty value
- `score` — computes a score
- `scores` — scores
- `new` — constructs a default or empty value
- `label` — label
- `intensity` — intensity
- `somatic` — somatic
- `apply` — Apply one causal stimulus.
- `apply_all` — apply all
- `model_demand` — Model demand for the current label, demoted under budget pressure.
- `snapshot` — Snapshot for prompts / deliberation context.

### `crates/ferricula-cognition/src/identity.rs`
Identity system — singleton agent identity with hexagram + horoscope.
- `from_entropy` — Derive per-agent thermodynamic constants from physical entropy.
- `to_json` — Serialize to JSON.
- `save` — Persist this identity to `<data_dir>/identity.json`.
- `recast_thermo` — Re-derive thermodynamic constants from fresh physical entropy.
- `apply_passive_cooling` — Apply passive cooling based on elapsed time since last update.
- `add_recall_heat` — Add heat from a recall transaction.
- `dream_cool` — Cool the agent after a dream cycle.
- `active_resonance_gates` — Return the resonance gates applied during recall.
- `load_or_create` — loads the persisted identity, or casts one from entropy
- `create_anchor` — Create the anchor memory Row + MemoryRecord for a new identity.

### `crates/ferricula-cognition/src/pali.rs`
Pāḷi translation layer — bidirectional term expansion between Abhidhamma vocabulary and computational implementation terms.
- `expand` — Expand text by appending Pali↔code equivalents for any recognized terms.
- `has_pali` — Check if text contains any recognized Pali terms.
- `glossary_json` — Get the full glossary as JSON for API exposure.

### `crates/ferricula-cognition/src/planner.rs`
- `new` — constructs a default or empty value
- `needs_llm` — Check if input needs LLM rewrite (not already SQL, key available).
- `rewrite_query_sync` — Synchronous rewrite — rule-based only, never blocks on network.
- `rewrite_query` — Original synchronous rewrite — tries LLM then falls back to rules.
- `spawn_llm_rewrite` — Spawn LLM rewrite on a background thread.

### `crates/ferricula-cognition/src/wisdom.rs`
The Wisdom Kings are bounded perspectives inside one mind.
- `whisper` — Deterministic baseline whispers.
- `integrate` — integrate

### `crates/ferricula-core/src/engine.rs`
- `new` — constructs a default or empty value
- `upsert` — inserts or replaces a row
- `delete` — removes a row by id
- `get` — sends an HTTP GET
- `row_count` — returns how many rows are stored
- `all_bitmap` — returns every id as a bitmap
- `bitmap_for_tag_eq` — bitmap for tag eq
- `bitmap_for_tag_range` — Returns a bitmap of all IDs where `field` parses as `u32` and the parsed value satisfies the requested numeric range.
- `distinct_tag_values` — Returns all distinct tag values for `field`, sorted.
- `bitmap_jaccard` — returns the Jaccard overlap of two bitmaps
- `vector_topk` — returns the nearest rows by vector score
- `vector_topk_bitmap` — returns the nearest rows inside a bitmap
- `rows_iter` — iterates stored rows

### `crates/ferricula-core/src/graph.rs`
- `new` — constructs a default or empty value
- `connect` — Connect two memories with a labeled, weighted edge.
- `push_edge_observation` — Push an observed similarity between two nodes into the per-edge dynamics history.
- `edge_bracket` — Weber bracket `B = ṡ² + s · s̈` for the edge between `a` and `b`.
- `edge_dynamics_count` — Number of edges with recorded dynamics.
- `disconnect` — Remove the edge between two memories.
- `neighbors` — Forward-reachable neighbors of a node.
- `predecessors` — Causal predecessors — nodes with causal edges pointing TO this node.
- `edge` — The edge between two nodes, if any.
- `degree` — Degree centrality (number of forward connections).
- `remove_node` — Remove a node and all its edges.
- `node_count` — Nodes that have at least one edge.
- `edge_count` — Total edges.
- `neighborhood_2` — Two-hop neighborhood: neighbors of neighbors, excluding self.
- `all_edges` — Snapshot all edges for persistence.
- `load_edges` — Rebuild from persisted edges.

### `crates/ferricula-core/src/memory.rs`
- `now_epoch` — returns the current unix time
- `new` — constructs a default or empty value
- `new_at` — Create with a fixed timestamp (for testing / deserialization).
- `decay_tick` — One tick of exponential decay.
- `effective_alpha` — α_eff = α / (1 + ln(1 + consolidation_depth))
- `on_recall` — Called when this memory is recalled.
- `on_neglect` — Called during dream for memories not recently recalled.
- `on_halo_touch` — Called during dream for memories that are direct graph neighbors of a keystone.
- `above_gate` — True if fidelity is at or above the survival gate.
- `forgive` — Active → Forgiven.
- `archive` — Forgiven → Archived.
- `staleness` — Seconds since last recall.
- `age` — Age in seconds.
- `resonates` — Wheeler-Feynman resonance check — does this memory respond to the query? Keystones always resonate.
- `new` — constructs a default or empty value
- `insert` — inserts one record
- `get` — sends an HTTP GET
- `get_mut` — fetches one record for modification
- `remove` — removes one record
- `contains` — reports whether the id is present
- `len` — returns how many entries are stored
- `is_empty` — reports whether the collection is empty
- `iter` — iterates the records
- `iter_mut` — iterates the records mutably
- `in_state` — All records in a given lifecycle state.
- `keystones` — All keystone records.
- `all_records` — Snapshot all records for persistence.
- `load_records` — Load records from a persistence snapshot.

### `crates/ferricula-core/src/model.rs`
- `is_empty` — reports whether the collection is empty
- `to_tags` — Flatten reference fields to `(tag_key, tag_value)` pairs for the bitmap tag index.

### `crates/ferricula-core/src/persist.rs`
- `open` — opens the store or handle
- `upsert` — inserts or replaces a row
- `delete` — removes a row by id
- `remember` — stores one memory
- `update_record` — updates one stored memory record
- `remove_memory` — removes one memory from the durable engine
- `connect` — creates an edge between two memories
- `disconnect` — removes the edge between two memories
- `insert_term` — inserts a term into the prime tree
- `checkpoint` — flushes durable state to disk
- `engine` — borrows the durable engine
- `engine_mut` — borrows the durable engine mutably
- `memory_store` — borrows the memory store
- `memory_store_mut` — borrows the memory store mutably
- `graph` — borrows the graph
- `graph_mut` — borrows the graph mutably
- `prime_tree` — borrows the prime tree
- `prime_tree_mut` — borrows the prime tree mutably
- `skg` — borrows the semantic knowledge graph
- `skg_mut` — borrows the semantic knowledge graph mutably

### `crates/ferricula-core/src/prime_tree.rs`
- `new` — constructs a default or empty value
- `insert` — Insert a memory into the partition for `term`.
- `remove_member` — Remove a memory ID from every node in the tree.
- `split` — Split a node when its member count exceeds its prime.
- `merge` — Merge: the operation SlothANN was missing.
- `search` — Search for all memory IDs associated with a term (prefix match).
- `search_exact` — Exact term search — only matches root-level terms.
- `collect_members` — Collect all member IDs from a node and its descendants.
- `terms` — All term strings at root level.
- `node_count` — returns how many nodes have an edge
- `root_count` — returns how many root terms exist
- `total_members` — Total unique members across the tree.
- `snapshot` — Snapshot for persistence.
- `load_snapshot` — Load from persistence snapshot.

### `crates/ferricula-core/src/skg.rs`
Semantic Knowledge Graph — materialized edges from prime tree posting list intersections.
- `new` — constructs a default or empty value
- `push` — Push a new (tick, score) observation.
- `len` — Number of entries currently stored.
- `last_tick` — Most recent tick in the buffer.
- `velocity` — Compute velocity (ṡ) from the two most recent entries.
- `acceleration` — Compute acceleration (s̈) from the three most recent entries.
- `weber_bracket` — Weber bracket: B = ṡ² + s · s̈ where s is the most recent Jaccard score.
- `current_jaccard` — Most recent Jaccard score.
- `new` — constructs a default or empty value
- `update` — Run one SKG update cycle during dream.
- `snapshot` — Create a snapshot for persistence.
- `load_snapshot` — Load from a snapshot.
- `edges_for_term` — Get all edges for a specific term.

### `crates/ferricula-core/src/sparse.rs`
Sparse inverted index stubs for term-based scoring (BM25/BB25).
- `add_doc` — adds one document to the sparse index stub
- `df` — returns the document frequency of a term
- `avg_dl` — returns the average document length

### `crates/ferricula-core/src/transform.rs`
Geometric trust: vector encryption via seed-derived permutation + sign flip.
- `from_seed` — Build a transform from a 32-byte seed for the given dimension.
- `encrypt` — Encrypt a vector: permute then flip signs.
- `decrypt` — Decrypt a vector: reverse sign flips then unpermute.
- `orthogonal_from_seed` — orthogonal from seed
- `warp` — warp

### `crates/ferricula-search/src/agent.rs`
- `resolve_ollama_url` — Resolves the effective Ollama endpoint.
- `extract_entities` — Calls local/remote Ollama chat endpoint to extract key concepts, proper names, organizations, locations, and terms from a text chunk.
- `serve` — starts the service loop
- `run_agent_loop` — run agent loop
- `summarize_document` — summarize document

### `crates/ferricula-search/src/answer.rs`
Agentic question-answering over the retrieved field (`lume answer`).
- `ollama_chat` — Non-streaming Ollama `/api/chat` call.
- `plan_queries` — Plans 1–3 search queries for the question.
- `evaluate` — Judges whether `passages` answer `question`; if not, proposes new queries.
- `synthesize` — Synthesizes a cited answer from numbered passages.
- `parse_citations` — Parses the `[n]` markers actually used in an answer, returning the distinct 1-based passage numbers in first-appearance order.

### `crates/ferricula-search/src/bm25.rs`
- `serialize_u8_map` — serialize u8 map
- `deserialize_u8_map` — deserialize u8 map
- `serialize_vec_u8_map` — serialize vec u8 map
- `deserialize_vec_u8_map` — deserialize vec u8 map
- `is_stopword` — Returns true if the folded token bytes correspond to a stopword.
- `filter_query_stopwords` — Drops stopword tokens from a tokenized query.
- `parse_markdown` — Simple, robust line-by-line Markdown section parser.
- `build` — Constructs a search index over a collection of Markdown sections.
- `search` — Evaluates a query and returns matching sections ordered by their BM25 score.

### `crates/ferricula-search/src/crawl.rs`
- `crawl_hn_via_api` — crawl hn via api
- `crawl_url` — crawl url
- `clean_html_to_markdown` — clean html to markdown
- `run` — runs the main loop

### `crates/ferricula-search/src/eval.rs`
Retrieval evaluation harness.
- `parse_qna` — Parses a Q&A JSON array of `{question, answer, ...}` objects.
- `answer_recall` — Fraction of the answer's content tokens that appear in `section_body`, in `[0,1]`.
- `is_relevant` — Whether a section counts as relevant: at least `threshold` of the answer's content tokens are present.
- `dcg_at_k` — Discounted cumulative gain over a ranked list of binary relevances, top `k`.
- `ndcg_at_k` — Normalized DCG@k: actual DCG over the ideal DCG (the same relevances sorted best-first).
- `reciprocal_rank` — Reciprocal rank: `1 / (rank of first relevant)`, `0` if none in the list.
- `hit_at_k` — Whether any of the top `k` is relevant.
- `new` — constructs a default or empty value
- `record` — Records one question.
- `hit_rate` — hit rate
- `mrr` — mrr
- `ndcg` — ndcg

### `crates/ferricula-search/src/fast_retrieval.rs`
- `new` — constructs a default or empty value
- `insert` — Insert a 32-bit ID into the roaring bitmap
- `contains` — Check if the 32-bit ID is contained in the bitmap
- `intersect` — Intersects two roaring bitmaps to yield a new intersected bitmap
- `union` — Unions two roaring bitmaps to yield a new unioned bitmap
- `iter` — Extract all document IDs in sorted order
- `is_empty` — reports whether the collection is empty
- `len` — Returns the total number of IDs stored in this roaring bitmap
- `intersection_count` — Counts the intersection cardinality without materializing a result bitmap.
- `jaccard_similarity` — Computes the Jaccard similarity index (intersection size / union size) between two roaring bitmaps.
- `is_prime` — Helper to check if a number is prime.
- `get_nth_prime` — Helper to get the n-th prime number (1-indexed, so 1st prime is 2, 2nd is 3, etc.)
- `fnv1a_hash` — Simple fast FNV-1a 32-bit hash function
- `new` — constructs a default or empty value
- `add_term` — Add a vocabulary term to the 64-bit bitwise Bloom filter mask
- `test_term` — Check if a query term is possibly present in the document.
- `add_tag_prime` — Add a tag prime to the tag Gödel signature
- `test_tag_prime` — Check if a tag prime is possibly present in the document.

### `crates/ferricula-search/src/fusion.rs`
- `reciprocal_rank_fusion` — Fuses two ranked lists of items (lexical and semantic) using Reciprocal Rank Fusion (RRF), and optionally applies FST dynamic intent boosting based on tag matches in chunk text.

### `crates/ferricula-search/src/graph_search.rs`
SKG (Semantic Knowledge Graph) traversal — Primitive 6 wired into the Primitive 7 HATCHERIK boost.
- `resolve_query_entities` — Resolves the entities named in `query` to SKG node keys.
- `build_adjacency` — Adjacency map from the precomputed SKG edges: `entity_key -> [(neighbor, weight)]`, each list sorted descending by weight.
- `compute_skg_scores` — Walks the SKG from the query's entities and scores each section by the mass of related entities it contains.
- `apply_skg_boost` — Applies the SKG boost to a list of lexical `SearchHit`s in place: existing hits get `score *= 1 + beta*skg`, and strongly-related sections not already present are appended (scored `beta*skg`, so they rank below real lexical matches).

### `crates/ferricula-search/src/hybrid.rs`
- `embed_text` — Embeds `text` to its 768-d GTR-T5 ("organize") vector.
- `cosine_similarity` — Cosine similarity between two equal-length vectors.
- `set_cache_dir` — Anchors the session/semantic cache files to the index db directory so they follow the index instead of landing in whatever cwd the process runs from.
- `percent_encode` — Simple percent encoder to avoid adding external dependencies.
- `get_corpus_metadata` — get corpus metadata
- `load_session_cache` — Loads the session cache for `corpus_path` if it exists and hasn't expired, WITHOUT checking the corpus fingerprint.
- `load_cached_session` — load cached session
- `save_cached_session` — save cached session
- `delete_cached_session` — delete cached session
- `load_semantic_cache` — load semantic cache
- `save_semantic_cache` — save semantic cache
- `section_hash` — Stable content hash identifying a section across re-indexes.
- `initialize_and_ingest_session` — Ingests all sections into a newly initialized shivvr session and caches it.
- `ensure_semantic_session` — Returns a semantic session covering `sections`, ingesting only what's missing: a no-op when the corpus fingerprint matches the cached session, an incremental top-up of new/changed sections when it doesn't, and a full ingest only when no usa
- `cleanup_session` — cleanup session
- `query_semantic_search` — query semantic search
- `blend_hybrid_scores` — Blends local lexical hits with remote semantic hits and the local SKG (entity co-occurrence) signal: `hybrid = bm25 * (1 + alpha*semantic + beta*skg)` when lexically matched, otherwise it falls back to the available signals.
- `format_shivvr_error` — format shivvr error
- `get_shivvr_base_url` — get shivvr base url
- `load_nuts_token` — load nuts token
- `execute_hybrid_search` — The core hybrid search primitive.
- `to_markdown` — to markdown
- `print_cli` — print cli

### `crates/ferricula-search/src/inversion.rs`
- `invert_vector` — A simple, reusable primitive to invert a 768-dimensional GTR-T5 vector back to its original text representation via the remote shivvr.nuts.services API.
- `execute_steered_inversion` — The core pipeline function that performs vector inversion and, if corpus details are provided, steering FST theme extraction and Markov chain synthesis.
- `to_markdown` — Formats the result as a beautiful, premium Markdown document for MCP consumers.
- `print_cli` — Outputs a stunning, premium terminal view of the vector inversion and steering flow.

### `crates/ferricula-search/src/lib.rs`
FST-based text tagger — Rust port of the `App.java` reference from <https://github.com/jsclosures/fstguardrails>.
- `new` — constructs a default or empty value
- `with_output` — with output
- `with_regex` — with regex
- `build` — Build a tagger from a list of [`Entry`]s.
- `phrases` — Access the original loaded dictionary phrases
- `len` — Number of distinct FST keys.
- `is_empty` — reports whether the collection is empty
- `record_count` — Total number of records (including synonyms collapsed onto the same FST key).
- `kinds` — Distinct kinds in the dictionary, sorted.
- `from_tsv_file` — Build a tagger from a TSV file: each line is `phrase<TAB>id`.
- `from_data_dir` — Load every `*.csv` file in `dir` (Java parity).
- `from_env` — If the `DATA` env var is set, build from that directory; otherwise `Ok(None)` so callers can fall back.
- `tag` — Tag with the default policy (longest match per start position).
- `tag_with` — Tag with an explicit overlap policy.
- `tokenize` — tokenize
- `derive_output` — Java's `deriveOutput`: uppercase every char then strip everything that isn't `A-Z` or `0-9`.
- `parse_csv_line` — Minimal RFC-4180-ish CSV line parser.
- `uuid_v4` — UUID v4 string (RFC 4122).

### `crates/ferricula-search/src/regex.rs`
- `new` — constructs a default or empty value
- `parse` — parses the string form back into the value
- `levenshtein_regex` — Automatically expands a query term into a Levenshtein regex pattern representing 1-character edits and transpositions.
- `compile` — compile
- `matches` — matches

### `crates/ferricula-search/src/semantic_mesh.rs`
- `new` — constructs a default or empty value
- `next_u64` — next u64
- `next_range` — next range
- `parse_words_and_punctuation` — Tokenizes raw text, separating alphanumeric words (including internal apostrophes and hyphens like "d'if" or "twenty-four") from punctuation.
- `reconstruct_spaces` — Reconstructs spacing between tokens to form natural, human-readable paragraphs.
- `build` — Builds a trigram Markov Chain model over text sections.
- `generate` — Generates styled text starting with an optional seed word.
- `generate_steered` — Generates steered text utilizing an FST tagger and co-occurrence posting lists for local attention feedback.
- `weight` — Weight the SKG walk should use for this edge.
- `cooccurrence_relatedness` — SKG significance score for a co-occurrence, in `[-1,1]`.
- `build` — Computes co-occurrence graph from BM25Index matched entity posting lists
- `to_json` — Serializes the graph manually to a clean JSON string without serde
- `print_ascii_table` — Prints a beautiful ASCII-art relationship table to the terminal

### `crates/ferricula-search/src/spelling.rs`
- `trigrams` — Generates character trigrams for a given word.
- `levenshtein_distance` — levenshtein distance
- `build` — Builds a spelling index from the static tagger phrases and BM25 index corpus terms.
- `correct_word` — Evaluates a query term and returns up to `max_suggestions` ordered candidates with their final alignment scores.

### `crates/ferricula-search/src/stream.rs`
Live search-dynamics stream (`lume stream`).
- `run` — Runs the relaxation for `queries` over the union `cands` and streams frames.

### `crates/ferricula-semantic/src/chunker.rs`
- `new` — constructs a default or empty value
- `chunk` — chunk

### `crates/ferricula-semantic/src/crypto.rs`
- `new` — Create from flattened row-major key data
- `encrypt` — Encrypt embedding: v @ Q (preserves cosine similarity)
- `decrypt` — Decrypt embedding: v @ Q^T
- `new` — constructs a default or empty value
- `register_keys` — Register key matrices for an agent
- `get_keys` — Get keys for an agent (None if not registered this session)

### `crates/ferricula-semantic/src/embedder.rs`
- `new` — constructs a default or empty value
- `embed` — embeds text and returns a vector
- `count_tokens` — count tokens

### `crates/ferricula-semantic/src/inverter.rs`
- `new` — Load all ONNX models and tokenizer Expected paths: - projection_path: projection.onnx (Nd → 16x768) - encoder_path: encoder.onnx (inputs_embeds interface) - decoder_path: decoder.onnx - tokenizer_path: tokenizer.json (T5 tokenizer)
- `invert` — Invert an embedding back to text

### `crates/ferricula-semantic/src/openai.rs`
- `new` — constructs a default or empty value
- `embed` — Embed a single text, returns L2-normalized 1536d vector
- `embed_batch` — Embed a batch of texts, returns L2-normalized 1536d vectors

### `crates/ferricula-semantic/src/similarity.rs`
- `cosine_similarity` — Cosine similarity (SIMD accelerated when available)

### `crates/ferricula-server/src/api.rs`
Operator-facing HTTP surface for Steve.
Endpoints: `/control/advocate`, `/control/autonomy`, `/control/mode`, `/control/pause`, `/control/schedule`, `/control/schedule/plan`, `/control/sleep`, `/control/wake`, `/health`, `/identity`, `/memory/overlay`, `/memory/overlay/approve/{event_id}`, `/memory/recall`, `/models/status`, `/status`, `/tasks`, `/tasks/considerations`, `/tasks/mention`, `/tasks/read-feed`, `/tasks/read-hacker-news`, `/tasks/{id}`, `/wisdom/preview`.
- `router` — router

### `crates/ferricula-server/src/autonomy.rs`
Bounded, provider-neutral **autonomy** state machine for Steve.
- `as_str` — returns the stable string form
- `is_working` — is working
- `is_paused` — is paused
- `kind_name` — kind name
- `is_halt` — True for operator halt intents (highest precedence).
- `validate` — checks the value against its bounds
- `is_expired` — is expired
- `remaining_steps` — remaining steps
- `holds` — holds
- `new` — constructs a default or empty value
- `with_defaults` — with defaults
- `recover` — Restore after crash.
- `snapshot` — captures state for persistence
- `into_snapshot` — into snapshot
- `state` — returns the current state
- `lease` — lease
- `policy` — policy
- `note_chat` — Record user chat activity (starts quiet period).
- `set_budget_pressure` — set budget pressure
- `set_daily_budget_exhausted` — set daily budget exhausted
- `set_queue_depth` — set queue depth
- `set_idle_secs` — set idle secs
- `quiet_active` — quiet active
- `budget_blocks_model` — budget blocks model
- `may_call_model` — Ledger Safeguard Invariant, checked at **execution** time: the spend ledger must gate every model call, not just lease grants.
- `may_write_overlay` — Overlay-write permission at execution time.
- `record_step` — Consume one step on the active lease if present.
- `apply` — Apply a trigger at time `now`.
- `legal_edges` — Static table of diagram-legal edges (for tests / docs).

### `crates/ferricula-server/src/config.rs`
- `load` — loads a persisted value
- `validate` — checks the value against its bounds

### `crates/ferricula-server/src/feeds.rs`
- `read_hacker_news` — Fixed-source Hacker News reader.

### `crates/ferricula-server/src/lib.rs`
- `inspect_data_dir` — inspect data dir

### `crates/ferricula-server/src/memory.rs`
- `open` — opens the store or handle
- `recall_candidates` — Provider-free lexical candidate retrieval keeps Steve's continuity available before an embedding service is configured.

### `crates/ferricula-server/src/memory_overlay.rs`
Bounded, append-only overlay for the recovered (read-only) memory base.
- `key` — Stable map key: `base:<id>` / `overlay:<event_id>`.
- `validate` — checks the value against its bounds
- `new` — constructs a default or empty value
- `overlay_file` — The only path this module will write.
- `base_root` — Base root is exposed for the union engine to open **read-only**.
- `save` — persists the value
- `load` — loads a persisted value
- `assert_safe_overlay_write_path` — Refuse paths whose leaf is a known base-volume artifact.
- `new` — constructs a default or empty value
- `with_defaults` — with defaults
- `config` — returns the configuration
- `events` — events
- `get` — sends an HTTP GET
- `append` — Validate and append one event.
- `approval_status` — Approval status of an event in this log.
- `is_effective` — An event contributes to the projection only when effective.
- `projection` — Fold the chain into the union-engine view.
- `verify` — Verify the full hash chain and per-event invariants.
- `save` — Atomic persistence: verify, then whole-document write via tmp+rename.
- `load` — Reload a document, verifying schema version, config bounds, and the entire hash chain before accepting any of it.

### `crates/ferricula-server/src/mention_ingest.rs`
Bounded, provider-neutral Nuts mention-ingestion primitive.
- `new` — constructs a default or empty value
- `dedupe_id` — Canonical dedupe key: `nutnews:<instance>:<event_seq>`.
- `fresh` — fresh
- `after` — Argument for the next `events_since(after=…)` poll.
- `parse` — parses the string form back into the value
- `is_ignorable_noise` — Votes and handle churn are never mention candidates.
- `key` — returns the stable map key
- `validate` — checks the value against its bounds
- `may_enqueue_consideration` — Mentions never compel a response.
- `normalize_and_validate` — Normalize identity fields in place, then check bounds.
- `new` — constructs a default or empty value
- `with_defaults` — with defaults
- `load` — loads a persisted value
- `load_pinned` — Load and refuse state whose identity does not match the deployment pin.
- `save` — persists the value
- `new` — constructs a default or empty value
- `with_defaults` — with defaults
- `from_state` — from state
- `state` — returns the current state
- `into_state` — into state
- `cursor` — cursor
- `pending_considerations` — Pending considerations that have not been marked enqueued yet.
- `apply_batch` — Apply a provider-neutral event batch.
- `mark_enqueued` — Mark a consideration as handed to the runtime queue.
- `take_pending_for_enqueue` — Drain pending considerations into a list for the runtime to enqueue as *tasks for deliberation only*.
- `force_cursor` — Operator/reconciler path: move the cursor **forward** after gap recovery.
- `force_cursor_rewind` — Deliberate history replay.
- `normalize_instance` — Canonical instance form: trimmed, ASCII-lowercased, host-shaped.
- `normalize_handle` — Canonical handle form: trimmed, zero-width-stripped, ASCII-lowercased, restricted to `[a-z0-9_-]`.
- `classify_mention` — Detect whether an event is a direct mention/reply to Steve.
- `consideration_task_payload` — Map a consideration into a neutral runtime task payload (no reply body).

### `crates/ferricula-server/src/model.rs`
Pluggable inference router for the single Steve identity.
- `as_code` — as code
- `new` — constructs a default or empty value
- `with_tokens` — with tokens
- `require` — require
- `script` — script
- `new` — constructs a default or empty value
- `from_entries` — from entries
- `record` — record
- `spent_usd_today` — spent usd today
- `total_usd_today` — total usd today
- `entries` — entries
- `new` — constructs a default or empty value
- `with_usage` — with usage
- `with_defaults` — with defaults
- `config` — returns the configuration
- `ledger` — ledger
- `identity_name` — identity name
- `select` — Select a route without performing any network I/O.
- `materialize` — Build a provider-specific request representation (still offline).
- `complete` — Select, materialize, execute via transport, and record usage.
- `complete_with_budget` — complete with budget
- `complete_no_model` — Deterministic no-model completion used for mechanical work and headless operation.

### `crates/ferricula-server/src/model_config.rs`
Declarative model-routing configuration for the single Steve identity.
- `steve_jobs` — steve jobs
- `as_str` — returns the stable string form
- `parse` — parses the string form back into the value
- `as_str` — returns the stable string form
- `parse` — parses the string form back into the value
- `estimate_usd` — Estimate USD cost from token counts.
- `timeout` — timeout
- `has_capability` — has capability
- `is_no_model` — is no model
- `steve_safe_defaults` — Safe defaults: local Ollama for ordinary work, optional Anthropic escalation for deliberate/code, and a deterministic no-model route for mechanical tasks.
- `load` — loads a persisted value
- `profile` — profile
- `route_for` — route for
- `profile_map` — profile map
- `validate` — Reject inline secrets, empty/broken routes, duplicate ids, and dangling profile references.
- `looks_like_inline_secret` — True when a string is clearly a secret value rather than an env-var name.

### `crates/ferricula-server/src/nutnews.rs`
- `new` — constructs a default or empty value
- `newest` — newest
- `get_item` — get item
- `comment` — comment

### `crates/ferricula-server/src/nutnews_events.rs`
Bounded, **read-only** Nuts News `events_since` client primitive.
- `pin` — Build a config with a **pinned** origin.
- `pinned_instance` — Convenience: production-like defaults with a custom origin/instance pair.
- `json_ok` — json ok
- `max_seq` — Highest sequence present on the page, if any.
- `is_ignorable_noise` — Votes / handle churn / classification / crawl provenance are never mention candidates (contract §4.2 + real Event variants).
- `normalize_event_fields` — Map a raw ledger `event` object to bridge fields.
- `capability_status` — Operator-facing status string for live acceptance dashboards.
- `new` — constructs a default or empty value
- `config` — returns the configuration
- `transport` — transport
- `prepare_events_since` — Pure request construction: validates `limit`, never touches the network.
- `events_since_default` — `events_since(after, limit)` using the configured default limit.
- `events_since` — Poll `events_since` and return a validated page.
- `interpret_response` — Interpret a raw HTTP response into a validated page (pure; offline-testable).
- `parse_event_page_bytes` — Parse JSON bytes into an [`EventPage`] (MCP result envelope or bare page).
- `parse_event_page_value` — Accept either a bare `EventPage` object or a JSON-RPC / MCP wrapper: `{ "result": { …page } }`, `{ "result": { "content": [{ "text": "{…page}" }] } }`.
- `validate_event_page` — Validate a parsed page against the request cursor/limit and expected instance.
- `page_requires_reconcile` — Pure helper: should the bridge enter reconcile instead of sequential apply?
- `suggested_cursor_after_process` — Pure helper: next `after` argument after successfully processing a contiguous non-gap page.
- `validate_limit` — validate limit
- `new` — constructs a default or empty value
- `enqueue_ok_page` — enqueue ok page
- `enqueue_jsonrpc_page` — enqueue jsonrpc page
- `enqueue_mcp_text_page` — enqueue mcp text page
- `enqueue_raw` — enqueue raw
- `enqueue_status_body` — enqueue status body
- `enqueue_with_content_type` — enqueue with content type
- `seen_requests` — seen requests

### `crates/ferricula-server/src/runtime.rs`
Steve runtime: single-writer task loop with durable phase-one primitives.
- `parse` — parses the string form back into the value
- `open` — opens the store or handle
- `status` — returns a status summary
- `authorize` — authorize
- `set_mode` — set mode
- `set_schedule` — set schedule
- `enqueue` — enqueue
- `tasks` — tasks
- `wisdom_preview` — wisdom preview
- `recall_candidates` — recall candidates
- `model_status` — model status
- `run_worker` — run worker
- `run_scheduler` — run scheduler

### `crates/ferricula-server/src/sleep_cycle.rs`
Bounded, provider-neutral sleep/dream planning primitive.
- `allows_planning` — allows planning
- `as_str` — returns the stable string form
- `as_str` — returns the stable string form
- `required_capabilities` — Capabilities an executor must advertise for this envelope.
- `hardened` — Defense-in-depth clamp applied when emitting proposals so a misconfigured persisted policy cannot ship unredacted private memory.
- `hardened` — Clamp to absolute planning bounds so executors never see NaN or unbounded ceilings from a corrupted policy snapshot.
- `is_mechanical` — is mechanical
- `for_kind` — for kind
- `base_spacing_secs` — Base spacing between cycles before jitter, floored at one minute.
- `effective_jitter_secs` — Jitter half-width clamped so spacing never goes non-positive relative to half the base interval.
- `validate` — checks the value against its bounds
- `is_ready` — is ready
- `next_ready_at` — Earliest time the kind becomes ready again; `now`-independent.
- `note_run` — Record an actual execution.
- `last_run_at` — Last recorded run time for a kind, if any.
- `merge_prefer_later` — Merge another ledger (e.g.
- `sanitize` — Drop entries that are not known kinds (forward-compat no-op today) and ensure map only holds finite timestamps.
- `asserts_non_mutating` — Structural check: proposal never authorizes direct memory mutation.
- `is_empty` — reports whether the collection is empty
- `next_cycle_at` — When the next sleep cycle should start.
- `next_cycle_at_gated` — Gate-aware next-cycle time.
- `cycle_is_due` — True when `now` has reached the next scheduled cycle under the gate.
- `plan_cycle` — Plan one sleep cycle with the default allow gate.
- `plan_cycle_gated` — Plan under an explicit pause/stop/allow gate.

### nuts-news (inside ferricula_v2)

A reader site with its own ledger, not the ferricula memory engine. Routes from `src/main.rs`: `/`, `/auth/callback`, `/c/:cat`, `/comment`, `/contest`, `/edit_comment`, `/events`, `/extension.zip`, `/favicon.ico`, `/favicon.svg`, `/health`, `/healthz`, `/item/:id`, `/log`, `/login`, `/logout`, `/mcp`, `/newest`, `/search`, `/settings`, `/submit`, `/u/:handle`, `/vote`. The MCP endpoint is `POST /mcp`. Ranking is Hacker News gravity, `(points - 1) / (age_hours + 2)^1.8`, in `src/sqrl.rs`. The browser extension (`extension/popup.js`) captures a page; the binary serves it as a stored zip.

### `extension/popup.js`
- `config` — returns the configuration
- `capturePage` — capture page

### `src/auth.rs`
nuts-auth verifier — ported verbatim from sdrrand/src/auth.rs (itself from shivvr).
- `new` — constructs a default or empty value
- `refresh_jwks` — refresh jwks
- `verify_jwt` — verify jwt
- `validate_api_token` — validate api token
- `verify` — verify

### `src/bot.rs`
@nuts — the resident reader-bot.
- `from_env` — reads configuration from the environment
- `describe` — returns a short description
- `enabled` — reports whether the feature is on
- `on_item` — on item
- `on_edit` — A human edited a comment (fix): the flag cleared on apply; re-police it.
- `on_comment` — on comment
- `spawn` — spawn

### `src/events.rs`
Every mutation in nutnews is an Event.
- `summary` — One-line human summary for the public ledger page.
- `ts` — returns the event timestamp
- `now_ts` — returns the current timestamp

### `src/gcs.rs`
GCS plumbing: a minimal JSON-API client (`GcsClient`) shared by the rolling-segment ledger backend (`GcsLedger`) and the search vector cache.
- `new` — constructs a default or empty value
- `list_prefix` — list prefix
- `get` — Download an object.
- `put` — PUT the full object, optionally with an ifGenerationMatch precondition.
- `open` — Open the bucket ledger: list all segments, download them in order, and return the backend plus every segment's bytes for the caller to replay.
- `truncate_active` — If replay stopped early in the final segment (torn/corrupt tail — shouldn't happen with atomic PUTs, but the CRC guard stays), drop the undecodable tail from the active buffer so we never append after junk.
- `append` — Durably append one framed record: buffer it, re-PUT the active segment with a generation precondition, roll the segment at the threshold.

### `src/handles.rs`
Assigned animal identities.
- `emoji_allowed` — emoji allowed
- `emoji_choices` — All distinct emoji choices, for the settings page picker.
- `generate` — Deterministic-enough pick: hash the user id, current time, and attempt counter.
- `validate_handle` — Validate a user-chosen handle: same charset rules the ledger has always enforced, plus reservations.

### `src/html.rs`
Server-rendered HTML.
- `anon` — renders an anonymous display name
- `esc` — escapes text for HTML
- `age` — returns age
- `page` — page
- `page_with` — page with
- `search_page` — search page
- `listing` — listing
- `item_page` — item page
- `submit_page` — submit page
- `settings_page` — settings page
- `profile_page` — profile page
- `log_page` — log page

### `src/main.rs`
Endpoints: `/`, `/auth/callback`, `/c/:cat`, `/comment`, `/contest`, `/edit_comment`, `/events`, `/extension.zip`, `/favicon.ico`, `/favicon.svg`, `/health`, `/healthz`, `/item/:id`, `/log`, `/login`, `/logout`, `/mcp`, `/newest`, `/search`, `/settings`, `/submit`, `/u/:handle`, `/vote`.

### `src/mcp.rs`
Native MCP endpoint at POST /mcp (streamable HTTP, JSON responses).
- `tool_schemas` — tool schemas
- `item_json` — item json
- `handle` — Handle one JSON-RPC message.

### `src/search.rs`
Hybrid search over the ledger projection, built on Lume + shivvr.
- `describe` — returns a short description
- `query` — Hybrid query: returns (item id, fused score), best first.
- `vector_count` — How many items currently have vectors (for stats).
- `spawn` — Start the index maintainer: initial build, then rebuild (debounced) on relevant ledger events, forever.

### `src/sqrl.rs`
Ranking + sqrl, the squirrel query language.
- `rank_score` — Hacker News gravity: (points - 1) / (age_hours + 2)^1.8
- `front_page_ids` — front page ids
- `newest_ids` — newest ids
- `run` — runs the main loop

### `src/store.rs`
squirrel — the storage engine.
- `domain` — domain
- `emoji_for` — Emoji for a display handle, if its owner set one.
- `open` — Local-file backend (dev mode and tests).
- `open_gcs` — GCS rolling-segment backend (production).
- `commit` — Durably append an event, then apply it to the projection.
- `set_pending` — set pending
- `clear_pending` — clear pending
- `pending_phase` — pending phase
- `alloc_id` — alloc id
- `submit` — submit
- `comment` — comment
- `vote` — `ai_handles` is the site's registry of AI/agent identities (always includes BOT_HANDLE; extended via NUTNEWS_AI_HANDLES for dedicated agent tokens).
- `bot_comment` — Post as the reserved bot handle.
- `classify` — Classify an item (public path — handle validated, category validated).
- `bot_classify` — Classification by the bot; unknown categories fall back to "misc".
- `bot_retitle` — Replace an item's title with the article's own headline (submitter opted in via "pull title from article").
- `bot_flag_comment` — Comment police verdict: badge a human comment as slop.
- `edit_comment` — A human editing their OWN comment (the "fix" path for a slop flag — but works on any of their comments).
- `contest` — Contest a slop flag: burns 10 karma, lifts the badge.
- `karma_of` — karma of
- `bot_edit_comment` — Revise one of the bot's OWN comments (owner-requested correction).
- `record_provided` — Ledger that the submitter's browser provided the page text directly.
- `record_fetch` — Ledger an external fetch performed by the bot — the public crawl log.
- `handle_of` — Current (handle, emoji) for an authenticated user, if assigned.
- `ensure_handle` — Get the user's handle, assigning a fresh animal identity on first contact (ledgered as HandleClaimed).
- `set_handle` — Change handle and/or emoji (ledgered; old handle is freed).
- `set_about` — Set (or clear, with empty text) the profile about.
- `find_by_url` — Newest item with this exact URL (trailing-slash-insensitive) — the extension re-submit path updates the existing story instead of duping.
- `ancestors` — Parent chain of a comment, root-first (for giving the bot thread context).

### `src/zip.rs`
Minimal ZIP writer (method 0 = stored, no compression) — enough to ship the browser extension from the binary itself, in the same no-deps spirit as the rest of the codebase.
- `build` — build
- `extension_zip` — The browser extension, zipped once on first request from files compiled into the binary.

## ferricula-arena

Python product on top of a ferricula container per agent. The README's promise: each agent has a volume, fidelity decays, keystones resist, similar memories merge, and radio entropy drives dreams. The supervisor creates and dreams the fleet. The advocate audits keystones. The trainer ingests a directory and fast-forwards dreams. A TUI watches them. Five dream roles in the README: Intuition (edges), Fortune (timing), Craft (consolidation), Ethics (keystone review), and the fifth is the advocate.

`delos-broker.py` is the autonomous conversation loop: when quiet, it asks a local model what to look up, crawls, and ingests. Shelf is one reading bot. nbtx-deathmatch is a three-agent municipal-charter audit with an answer key. The bench runs LoCoMo.

### `delos-broker.py`
- `ferricula_request` — Make a request to ferricula HTTP API.
- `get_active_tools` — Call ferricula GET /tools to get current tier and active tool surface.
- `ponder_escalation` — Dispatch to a frontier model (Anthropic Claude) when CRITICAL tier fires.
- `generate_image_via_gemini` — Call Gemini Imagen to generate an image from a text description.
- `embed_via_shivvr` — Embed text via shivvr for memory ingestion.
- `ferricula_raw_post` — POST raw text (not JSON) to ferricula — used for /offer endpoint.
- `trigger_dream` — Fire a dream via /offer with synthetic entropy when radio is down.
- `curiosity_query` — Ask the local model what Steve is curious about based on a memory.
- `grub_agent` — Run an agentic crawl task via Grub.
- `ingest_snippets` — Ingest web crawl results into ferricula as seeing-channel memories.
- `idle_loop` — Background thread: when quiet, Steve gets curious and goes looking.
- `execute_tool` — Execute a tool call and return the result string.
- `get_or_create_conversation` — get or create conversation
- `ollama_chat` — POST to Ollama /api/chat, return the response dict.
- `handle_chat` — Run the Ollama tool-call loop and return final response + UI commands.
- `do_OPTIONS` — handles a CORS preflight
- `do_GET` — handles an HTTP GET
- `do_POST` — handles an HTTP POST
- `log_message` — writes one log line

### `arena/__init__.py`
- test module — 0 test functions (ferricula-arena — Agent runner SDK for ferricula-backed AI agents.)

### `arena/advocate.py`
Memory advocate — audits keystones, challenges fidelity, identifies gaps.
- `summary` — returns a short summary
- `audit` — audit
- `apply_recommendations` — apply recommendations

### `arena/agent.py`
Base agent class — wraps ferricula container + LLM + personality.
- `__init__` — constructs the object
- `from_template` — from template
- `create` — Create and start the ferricula Docker container.
- `stop` — Stop the container (data persists in volume).
- `resume` — Restart a stopped container.
- `destroy` — Stop and remove container + volume.
- `remember` — stores one memory
- `recall` — Recall memories by semantic similarity.
- `see` — Perceive the environment through the eye sense base.
- `dream` — Trigger a manual dream cycle.
- `offer` — Inject entropy and trigger dream with archetype activation.
- `status` — returns a status summary
- `inspect` — returns the full record for one memory
- `chat` — chat
- `to_dict` — Serialize agent state for registry persistence.

### `arena/autonomous.py`
Autonomous agent loop — the agent thinks, browses, and speaks on its own.
- `autonomous_loop` — autonomous loop

### `arena/cli.py`
CLI entry point for ferricula-arena.
- `cmd_create` — Create and start an agent from a TOML template.
- `cmd_train` — Train an agent on a dataset directory.
- `cmd_audit` — Run advocate audit on an agent.
- `cmd_chat` — Interactive chat with an agent.
- `interrupt_print` — Print agent's spontaneous thought to terminal.
- `cmd_list` — List all registered agents.
- `cmd_stop` — Stop an agent's container.
- `cmd_resume` — Resume a stopped agent.
- `cmd_dream` — Run dream cycles on an agent or all agents.
- `main` — process entry point

### `arena/clients.py`
HTTP clients for ferricula and shivvr (embedding service).
- `parse_dream_report` — parse dream report
- `parse_recall` — parse recall
- `parse_inspect` — parse inspect
- `parse_status` — parse status
- `parse_remember_id` — parse remember id
- `__init__` — constructs the object
- `embed` — Embed a short text, return 768d vector.
- `chunk_and_embed` — Chunk and embed text.
- `available` — reports whether the remote service answers
- `__init__` — constructs the object
- `available` — reports whether the remote service answers
- `alloc_id` — alloc id
- `remember` — stores one memory
- `recall_vector` — Recall by vector similarity.
- `recall_text` — recall text
- `dream` — runs one dream cycle
- `dream_latest` — GET /dream/latest — last dream report without triggering a new one.
- `offer` — POST hex entropy to /offer, triggers dream with archetype activation.
- `status` — returns a status summary
- `identity` — returns the agent identity
- `inspect` — returns the full record for one memory
- `neighbors` — lists the graph neighbors of a memory
- `connect` — creates an edge between two memories
- `keystone` — marks a memory as a keystone
- `checkpoint` — flushes durable state to disk
- `search` — BM25 full-text search.
- `terms` — terms
- `get_row` — get row

### `arena/config.py`
TOML config loader for agent templates.
- `port` — port
- `load_config` — Load an AgentConfig from a TOML file.

### `arena/monitor.py`
Live TUI dashboard for ferricula-arena agents.
- `__init__` — constructs the object
- `hexagram` — hexagram
- `horoscope` — horoscope
- `emotions` — emotions
- `active_archetypes` — active archetypes
- `fetch_detail` — Fetch full detail for one agent.
- `on_mount` — runs when the TUI mounts
- `compose` — builds the TUI layout
- `update_detail` — update detail
- `compose` — builds the TUI layout
- `__init__` — constructs the object
- `compose` — builds the TUI layout
- `on_mount` — runs when the TUI mounts
- `on_data_table_row_selected` — on data table row selected
- `on_data_table_row_highlighted` — on data table row highlighted
- `action_refresh` — action refresh
- `action_toggle_chat` — action toggle chat
- `action_dream_selected` — action dream selected
- `on_input_submitted` — Handle chat input.
- `run_monitor` — Entry point for the monitor TUI.

### `arena/supervisor.py`
Supervisor — manages the lifecycle of all arena agents.
- `__init__` — constructs the object
- `create_agent` — create agent
- `stop_agent` — Stop an agent's container (data persists).
- `resume_agent` — Restart a stopped agent's container.
- `destroy_agent` — Stop and remove an agent's container + volume.
- `list_agents` — List all registered agents with their status.
- `health` — Get detailed health metrics for an agent.
- `dream_all` — Run dream cycles on all running agents.

### `arena/tools.py`
External tools for arena agents — web search, browse, etc.
- `web_search` — Search the web via Grub crawling Google.
- `fetch_page` — Fetch a web page via Grub crawler and return clean markdown.
- `serpapi_search` — Search the web via SerpAPI.
- `set_ferricula_url` — set ferricula url
- `set_radio_url` — set radio url
- `set_shivvr_url` — set shivvr url
- `set_hyperia_url` — set hyperia url
- `set_agent_world` — Set the agent's name and starting room in the world.
- `execute_tool` — Execute a tool call and return the result as a string.

### `arena/trainer.py`
Dataset trainer — feeds documents, classifies keystones, fast-forwards dreams.
- `train` — train

### `arena/world.py`
World model — spatial awareness for ferricula agents.
- `california_now` — Current time in California, formatted naturally.
- `get_room` — get room
- `look` — What an agent sees when they look around.
- `move` — Move an agent from one room to another.
- `enter_world` — Place an agent in the world for the first time.
- `available_tools` — Get the tool names available in a room.
- `room_list` — List all rooms with basic info.

### `bench/_inspect_results.py`
- test module — 1 test functions (One-shot helper: dump per-category accuracy from a partial qa_results.jsonl.): summarize

### `bench/_rescore_llm_judge.py`
- test module — 3 test functions (Re-score a finished agent qa_results.jsonl with LoCoMo's published LLM-judge prompt (gpt-4o-mini).): judge_one, rescore, main

### `bench/_show_answers.py`
- test module — 1 test functions (Dump (gold, answer) pairs from a qa_results.jsonl so we can see what the agent is actually saying.): show

### `bench/_show_d13.py`
- test module — 0 test functions (Inspect what utterance D1:3 actually says in conv-26, and what dates the persona keystone vs the gold answer expect.)

### `bench/_show_stored.py`
- test module — 1 test functions (Show what the agent actually stored in memory — text + tags for remember calls.): show

### `bench/_temporal_diag.py`
- test module — 1 test functions (Show all temporal questions + the agent's answers, plus any multi-hop hits.): show

### `bench/agent_runner.py`
Agentic LoCoMo runner.
- `__init__` — constructs the object
- `add` — add
- `usd` — usd
- `as_dict` — as dict
- `run_tool_loop` — run tool loop
- `ingest_for_character` — ingest for character
- `sample_inversion_check` — Sample n random non-anchor rows, run inversion check, return summary.
- `filter_qa` — filter qa
- `route_question` — Pick which character should answer.
- `answer_question` — answer question
- `substring_judge` — substring judge
- `run_one_conversation` — run one conversation
- `load_locomo` — load locomo
- `run` — runs the main loop
- `main` — process entry point

### `bench/agent_tools.py`
Tool surface for the agentic LoCoMo runner.
- `compute_time_arithmetic` — Apply a relative offset to a reference date.
- `execute_tool` — Execute a tool_call against the Ferricula client and return a string result the LLM can read back.
- `char_events` — Return (session_idx, date, event_text) tuples from event_summary.
- `char_observations` — Return (session_idx, dia_id, observation_text) from observation block.
- `build_persona_text` — Render a character's back-story as a single string for the system prompt.
- `build_system_prompt` — build system prompt
- `seed_persona_keystones` — seed persona keystones

### `bench/disposition.py`
I Ching disposition caster for the agentic runner.
- `cast_hexagram` — Cast a hexagram.

### `bench/ferricula_client.py`
Ferricula HTTP client for bench runners + agent tool executors.
- `__init__` — constructs the object
- `wait_ready` — wait ready
- `remember` — stores one memory
- `reflect` — reflect
- `hybrid` — BM25+vector recall.
- `recall` — SQL-based recall that raises heat + fires on_recall per hit.
- `dream` — runs one dream cycle
- `offer_entropy` — offer entropy
- `neighbors` — Returns [(neighbor_id, edge_weight)] for the given memory id.
- `neighbors_with_bracket` — Returns [(neighbor_id, edge_weight, weber_bracket)] for each outgoing neighbor.
- `connect` — creates an edge between two memories
- `keystone` — Manually promote an existing memory to keystone status.
- `get_row_text` — get row text
- `inspect` — returns the full record for one memory
- `inversion_check` — Embed → store → invert → text fidelity probe.
- `status_stats` — Parse /status into a stats dict.
- `decay_status` — GET /admin/decay_status — fidelity stats, alpha distribution.
- `identity` — returns the agent identity
- `clock` — clock
- `spin_up` — spin up
- `tear_down` — tear down

### `bench/locomo_runner.py`
LoCoMo benchmark runner for ferricula.
- `__init__` — constructs the object
- `wait_ready` — wait ready
- `remember` — stores one memory
- `hybrid` — hybrid
- `offer_entropy` — offer entropy
- `dream` — Explicit /dream trigger — runs the full dream cycle.
- `neighbors` — Fetch graph neighbors of id_ via /neighbors/<id>.
- `get_row_text` — Fetch the text tag of a row via /get/<id>.
- `status_stats` — Parse /status text into a stats dict (row_count, graph_edges, graph_nodes, dreams, heat).
- `spin_up` — spin up
- `tear_down` — tear down
- `generate_answer` — generate answer
- `substring_judge` — Cheap fallback judge for smoke-testing without OPENAI_API_KEY.
- `judge_answer` — judge answer
- `load_locomo` — load locomo
- `conversation_utterances` — Flatten a conversation's sessions into an ordered list of utterances with dia_id, speaker, text, and session_date attached.
- `filter_qa` — filter qa
- `run_one_conversation` — run one conversation
- `run` — runs the main loop
- `main` — process entry point

### `bench/locomo_two_agent_runner.py`
LoCoMo benchmark — two-agent setup, perspective-correct.
- `__init__` — constructs the object
- `wait_ready` — wait ready
- `remember` — stores one memory
- `hybrid` — hybrid
- `dream` — runs one dream cycle
- `status_stats` — status stats
- `spin_up` — spin up
- `tear_down` — tear down
- `substring_judge` — substring judge
- `load_locomo` — load locomo
- `filter_qa` — filter qa
- `session_indices` — session indices
- `char_events_for_session` — Returns (event_text, date) tuples from event_summary for this char + session.
- `char_observations_for_session` — Returns (observation_text, dia_id) tuples from observation block.
- `session_utterances` — session utterances
- `route_question_to_char` — Return the character whose perspective should answer this question.
- `seed_backstory` — Seed the agent with the dataset's per-character keystones BEFORE any session utterances are ingested.
- `ingest_sessions_perspective` — ingest sessions perspective
- `run_one_conversation` — run one conversation
- `run` — runs the main loop
- `main` — process entry point

### `bench/runner.py`
Ferricula benchmark runner.
- `__init__` — constructs the object
- `wait_ready` — wait ready
- `remember` — stores one memory
- `hybrid` — hybrid
- `offer_entropy` — offer entropy
- `status` — returns a status summary
- `spin_up` — spin up
- `tear_down` — tear down
- `load_corpus_chunks` — Returns [(source_name, chunk_text), ...] from a directory of .md/.txt files.
- `fire_probes` — fire probes
- `recall_at_k` — recall at k
- `run` — runs the main loop
- `main` — process entry point

### `bench/datasets/locomo/locomo_ingest_eval.py`
- `convert_to_iso8601` — Convert human-readable timestamps to ISO 8601 format.
- `evaluate_answer_with_llm` — evaluate answer with llm
- `__init__` — constructs the object
- `create_assistant` — Create a new assistant with memory enabled
- `create_thread` — Create a new thread
- `send_message` — send message
- `send_message_streaming` — send message streaming
- `wait_for_memory_operation` — wait for memory operation
- `load_conversation_sessions` — load conversation sessions
- `ask_question` — ask question
- `test_question_batch` — Test all questions from a single conversation (with multiple QA items)
- `run_test` — Run the complete test on dataset items
- `save_conversation_results` — Save per-conversation results to a JSON file
- `print_final_results` — Print comprehensive test results
- `main` — Main entry point

### `nbtx-deathmatch/nbtx_deathmatch/__init__.py`
- test module — 0 test functions (nbtx_deathmatch — calibration MVP for multi-agent municipal charter audit.)

### `nbtx-deathmatch/nbtx_deathmatch/__main__.py`
- test module — 0 test functions (Entry point for `python -m nbtx_deathmatch ...`.)

### `nbtx-deathmatch/nbtx_deathmatch/audit.py`
Audit cards + amendments — the issue lifecycle for nbtx-deathmatch.
- `file_issue` — file issue
- `comment_on_issue` — comment on issue
- `set_issue_status` — set issue status
- `propose_amendment` — propose amendment
- `list_issues` — list issues
- `list_amendments` — list amendments
- `get_issue` — get issue
- `get_amendment` — get amendment
- `dispatch_audit_tool` — Execute one audit tool call.

### `nbtx-deathmatch/nbtx_deathmatch/cli.py`
CLI entry point: `python -m nbtx_deathmatch <command>`.
- `cmd_setup` — cmd setup
- `cmd_serve` — cmd serve
- `cmd_reset` — cmd reset
- `cmd_status` — cmd status
- `main` — process entry point

### `nbtx-deathmatch/nbtx_deathmatch/events.py`
SSE event bus — shared between orchestrator and the Flask UI.
- `publish` — Broadcast an event to every subscribed SSE consumer.
- `subscribe` — Generator that yields SSE-formatted lines.
- `subscriber_count` — subscriber count

### `nbtx-deathmatch/nbtx_deathmatch/findings.py`
Finding dataclass + JSON (de)serialization.
- `to_dict` — serializes to a dictionary
- `from_dict` — builds the value from a dictionary
- `normalize_section` — Canonicalize a section label to `§N.NN` form.
- `sections_match` — Compare two section references for equivalence after normalization.
- `findings_to_json` — findings to json
- `findings_from_json` — Parse a findings list out of JSON.
- `load_answer_key` — Load answer_key.json and return findings as Finding objects.

### `nbtx-deathmatch/nbtx_deathmatch/manifest_ingest.py`
Manifest-driven corpus ingestion (steve.py pattern).
- `slug` — returns a stable slug
- `parse_manifest` — Hand-rolled YAML parser (no pyyaml dep).
- `load_manifest` — load manifest
- `find_grub_content_file` — Locate the grub_content .md for this manifest entry.
- `find_pdf_fallback` — find pdf fallback
- `load_entry_text` — load entry text
- `ingest_for_agent` — ingest for agent

### `nbtx-deathmatch/nbtx_deathmatch/orchestrator.py`
Group chat + agent loops + LLM provider switch.
- `call_llm` — call llm
- `max_tokens_for_call` — How many tokens to authorize on the next call.
- `settle` — Apply a response's actual cost to the bank, return summary.
- `in_debt` — reports whether the token budget is exhausted
- `to_dict` — serializes to a dictionary
- `get_budget` — Lazy-init a budget for `agent` if not present (uses env defaults).
- `set_budget` — set budget
- `all_budgets` — all budgets
- `to_dict` — serializes to a dictionary
- `post_chat` — Append a message to the group chat and broadcast it.
- `get_recent_chat` — get recent chat
- `get_chat_since` — get chat since
- `agent_log` — Log an event to one agent's column.
- `get_agent_log_recent` — get agent log recent
- `role_relevance` — 0..1 score for how relevant a message is to this role.
- `thread_state` — Snapshot of the current deliberation thread — used by the UI.
- `should_respond` — Returns (yes_or_no, reason).
- `start_agent_loops` — Spin up the three agent background loops.
- `stop_agent_loops` — stop agent loops
- `kick_off_audit` — User clicked the audit button — inject a directive into the group chat.

### `nbtx-deathmatch/nbtx_deathmatch/prompts.py`
Per-role audit prompt templates.
- `build_audit_prompt` — Construct the role-specific audit prompt.

### `nbtx-deathmatch/nbtx_deathmatch/score.py`
Diff agent findings against the answer key.
- `hit` — reports whether this finding was hit
- `precision` — returns precision against the answer key
- `recall` — returns recall against the answer key
- `hits` — returns the hit count
- `misses` — returns the miss count
- `passed` — All answer-key findings hit by at least one agent.
- `to_dict` — serializes to a dictionary
- `score` — computes a score
- `render_text` — Human-readable summary suitable for terminal output.
- `score_run` — Load findings.json from a run dir and answer_key.json, score them, write score.json + score.txt back to run_dir, return report.

### `nbtx-deathmatch/nbtx_deathmatch/server.py`
Flask UI for nbtx-deathmatch — 4-column group chat.
Endpoints: `/`, `/api/agent/<name>/log/recent`, `/api/amendment/<aid>`, `/api/amendments`, `/api/answer-key`, `/api/audit/start`, `/api/budgets`, `/api/budgets/<agent>`, `/api/chat`, `/api/chat/recent`, `/api/document/<doc_id>`, `/api/document/<doc_id>/locate`, `/api/documents`, `/api/issues`, `/api/issues/<iid>`, `/api/issues/<iid>/comments`, `/api/issues/<iid>/status`, `/api/status`, `/api/stream`, `/api/thread`, `/api/tools/config`.
- `index` — index
- `api_status` — api status
- `api_stream` — api stream
- `gen` — gen
- `api_chat` — api chat
- `api_chat_recent` — api chat recent
- `api_agent_log` — api agent log
- `api_audit_start` — api audit start
- `api_budgets` — Snapshot every agent's current token budget (bank, target, hard cap, wall-secs, cumulative use).
- `api_set_budget` — Override one agent's budget at runtime.
- `api_issues_list` — All audit cards.
- `api_issues_create` — User-driven issue filing (panel agents file via tools).
- `api_issue_get` — api issue get
- `api_issue_comment` — api issue comment
- `api_issue_status` — api issue status
- `api_amendments_list` — api amendments list
- `api_amendment_get` — api amendment get
- `api_tools_config` — Inspect the web-tool wiring: arena import status, GRUB_URL, SERPAPI key presence, and the list of tool names exposed to agents.
- `api_thread` — Snapshot of the current deliberation thread (exchanges + wall-clock).
- `api_answer_key` — api answer key
- `api_documents` — Catalog of available documents — UI uses to populate the tab picker.
- `api_document` — api document
- `api_document_locate` — Fuzzy-locate a snippet in a document.
- `serve` — starts the service loop

### `nbtx-deathmatch/nbtx_deathmatch/setup.py`
Provision the three calibration agents and ingest the manifest-driven corpus.
- `provision` — creates the agent's container and data
- `reset` — Destroy all three agents (containers + volumes + registry).

### `nbtx-deathmatch/tests/test_score.py`
- test module — 15 test functions (Score logic tests — no LLM, no network.): test_normalize_section_canonical, test_normalize_section_paired, test_sections_match, test_findings_from_json_clean, test_findings_from_json_with_prose, test_findings_from_json_markdown_fence, test_findings_from_json_empty, test_findings_severity_normalization, test_score_perfect_hit, test_score_miss, test_score_extra, test_score_section_format_drift, test_score_per_agent_metrics, test_score_type_drift_still_matches_with_note, test_score_paired_sections_match

### `shelf/cookbook/render.py`
Render cookbook.md to cookbook.pdf using reportlab.
- `styles` — returns the PDF paragraph styles
- `parse_blocks` — Walk the markdown into (kind, text) blocks.
- `esc` — Escape characters reportlab's Paragraph parser treats as markup.
- `build_story` — build story
- `render` — writes the PDF

### `shelf/cookbook/verify_chapters.py`
Dry-run the chapter cascade on each PDF in pdfs/.
- `main` — process entry point

### `shelf/cookbook/verify_ingest.py`
Dry-run the shelf ingestion pipeline against the cookbook PDF.
- `main` — process entry point

### `shelf/shelf/__init__.py`
- test module — 0 test functions (Shelf — a single reading bot.)

### `shelf/shelf/__main__.py`
- test module — 0 test functions

### `shelf/shelf/aliases.py`
Book title aliases — a client-side rename map for shelf.
- `get_alias` — Return the alias title for `book_id` if set, else None.
- `display_title` — The title the UI should show — alias if present, else fallback, else book_id.
- `set_alias` — set alias
- `clear_alias` — clear alias
- `all_aliases` — all aliases

### `shelf/shelf/chapters.py`
Chapter detection cascade for shelf's ingestion pipeline.
- `detect_chapters` — detect chapters

### `shelf/shelf/cli.py`
CLI: python -m shelf <command> Commands: setup provision the single Shelf agent serve start the Flask UI on :8300 (default) reset destroy the Shelf agent status show provisioning state reingest wipe + re-read a book's PDF from disk reconstr
- `cmd_setup` — cmd setup
- `cmd_serve` — cmd serve
- `cmd_reset` — cmd reset
- `cmd_reingest` — cmd reingest
- `cmd_reconstruct` — cmd reconstruct
- `cmd_read_url` — cmd read url
- `cmd_status` — cmd status
- `main` — process entry point

### `shelf/shelf/events.py`
Minimal SSE event bus — one publisher, many subscribers.
- `publish` — broadcasts one event to subscribers
- `subscribe` — yields events to one subscriber

### `shelf/shelf/ingest.py`
PDF → book/chapter/paragraph ingestion for the Shelf bot.
- `slugify` — Slug a title to a stable book_id (also used as PDF filename stem).
- `cite_prefix` — [Book ch.N par M p.P] header we prepend to each paragraph's text so source metadata survives the /hybrid round trip (which strips tags).
- `get_progress` — get progress
- `get_book_progress` — get book progress
- `is_book_indexed` — Check ferricula's /refs/distinct?field=book_id to see if this book already has memories.
- `delete_book_memories` — Delete every memory tagged with the given book_id.
- `find_pdf_for_book_id` — Locate the on-disk PDF for a book.
- `reingest_book` — reingest book
- `ingest_pdf_sync` — ingest pdf sync
- `ingest_pdf` — ingest pdf

### `shelf/shelf/orchestrator.py`
Single-bot chat handler with recall + LLM call.
- `call_llm` — call llm
- `hybrid_recall` — hybrid recall
- `get_recent_chat` — get recent chat
- `offer_entropy` — Feed entropy bytes to ferricula's /offer endpoint to trigger a dream cycle.
- `get_identity` — Fetch the current identity + emotional state from ferricula.
- `handle_user_message` — Tool-driven chat: bot calls recall_shelf if it wants to ground a claim.
- `list_books_via_ferricula` — Use ferricula's /refs/distinct?field=book_id to enumerate books.

### `shelf/shelf/page_editor.py`
Page-level edits to PDFs on the shelf.
- `write_page` — write page

### `shelf/shelf/reconstruct.py`
Reconstruct a checked-out book's PDF from its ferricula memories.
- `gather_paragraphs` — gather paragraphs
- `render_reconstruction` — render reconstruction
- `reconstruct` — reconstruct

### `shelf/shelf/server.py`
Flask server for Shelf — single-bot reading UI.
Endpoints: `/`, `/api/books`, `/api/books/<book_id>`, `/api/chat`, `/api/chat/recent`, `/api/client-log`, `/api/dream`, `/api/identity`, `/api/ingest/progress`, `/api/memory-inventory`, `/api/pdf/<book_id>`, `/api/pdf/<book_id>/page/<int:page>.png`, `/api/reconstruct`, `/api/stream`, `/api/upload`, `/api/url`.
- `index` — index
- `api_stream` — api stream
- `gen` — gen
- `api_upload` — api upload
- `api_url` — Fetch a URL through the crawler, render to PDF, and ingest as a new book.
- `api_reconstruct` — Reconstruct a checked-out book from its memories into a fresh PDF on the shelf.
- `api_books` — List PDFs on the shelf plus memory-only entries as checked out.
- `api_memory_inventory` — api memory inventory
- `api_ingest_progress` — api ingest progress
- `api_chat` — api chat
- `api_chat_recent` — api chat recent
- `api_identity` — Current emotional state + identity for the header strip.
- `api_dream` — Manually trigger a dream cycle (feed entropy → consolidate memories).
- `api_client_log` — api client log
- `api_pdf` — api pdf
- `api_pdf_page_png` — Render one PDF page to a PNG and cache it on disk.
- `api_pdf_delete` — api pdf delete
- `api_book_delete` — Dismiss a memory-only/checked-out entry from the visible catalog.
- `serve` — starts the service loop

### `shelf/shelf/setup.py`
Provision the single Shelf agent (one ferricula docker container).
- `provision` — Spin up the Shelf agent (creates the ferricula container if needed).
- `reset` — Destroy the Shelf agent (container + volume).

### `shelf/shelf/web_ingest.py`
Single-URL → book ingestion.
- `fetch_url_to_markdown` — fetch url to markdown
- `resolve_title` — resolve title
- `markdown_to_pdf` — markdown to pdf
- `create_book_from_url` — create book from url

### `tests/__init__.py`
- test module — 0 test functions

### `tests/test_clients.py`
- test module — 55 test functions (Tests for arena.clients — response parsing from ferricula server output.): test_unwrap_result_field, test_unwrap_error_field, test_unwrap_plain_text, test_unwrap_no_result_key, test_unwrap_empty_string, test_unwrap_nested_json, setUp, test_ticks, test_decayed, test_forgiven, test_archived, test_consolidated, test_pruned, test_ghost_echoes, test_keystones_reviewed, test_edges_created, test_keystones_promoted, test_active_archetypes, test_empty_archetypes, test_plain_text_input, setUp, test_hit_count, test_first_hit, test_second_hit, test_empty_recall, test_single_hit, test_fallback_id_only, setUp, test_id, test_state, test_fidelity, test_decay_alpha, test_effective_alpha, test_keystone, test_recalls, test_consolidation_depth, test_importance, test_emotion, test_degree, test_keystone_false, test_forgiven_state, setUp, test_rows, test_memories, test_active, test_forgiven, test_archived, test_keystones, test_graph_nodes, test_graph_edges, test_empty_status, test_standard_output, test_no_id, test_plain_text, test_large_id

### `tests/test_config.py`
- test module — 37 test functions (Tests for arena.config — TOML loading and default values.): setUp, test_agent_name, test_agent_role, test_agent_model, test_personality_trait, test_personality_voice, test_personality_focus, test_personality_emotions, test_memory_chonk_url, test_memory_radio_url, test_memory_clock_tick, test_memory_dream_threshold, test_training_patterns, test_training_decay_keystone, test_training_decay_normal, test_training_dreams_per_chapter, test_training_chunk_size, test_advocate_review_interval, test_advocate_min_fidelity, test_advocate_gap_detection, test_advocate_challenge_rate, test_advocate_source_verification, test_graph_auto_connect, test_graph_causal_labels, test_tools, setUp, test_name_from_toml, test_default_role, test_default_model, test_default_personality, test_default_memory, test_default_training, test_default_advocate, test_default_graph, test_default_tools, test_port_property, test_name_from_filename

### `tests/test_trainer.py`
- test module — 21 test functions (Tests for arena.trainer — keystone classification and dataset scanning.): test_three_patterns_is_keystone, test_two_patterns_two_focus_is_keystone, test_one_pattern_not_keystone, test_no_patterns_not_keystone, test_case_insensitive, test_importance_scales_with_matches, test_importance_capped_at_one, test_empty_patterns, test_empty_focus, test_two_patterns_one_focus_not_keystone, test_two_patterns_two_focus_is_keystone_explicit, test_regex_patterns, test_finds_txt_files, test_finds_md_files, test_finds_pdf_files, test_ignores_other_extensions, test_recursive_scan, test_empty_directory, test_nonexistent_directory_raises, test_sorted_output, test_mixed_extensions

## memex

The README describes a memory store: time as the primary index, context manifolds that warp a query, a decay path Active to Forgiven to Compressed to Archived with anchors, dual embeddings (a small invertible one and a large precise one), and utility tracked by whether an injected memory helped a goal. The only code in this tree is `geometric_access.py` (and the same file under `geometric-trust/`). Everything else below is the spec in `SPEC.md`, not an implementation.

### `geometric_access.py` and `geometric-trust/geometric_access.py`

Same 4066-byte script in both places. One public function.

- `cos_sim` — cosine similarity of two vectors.

The script then demonstrates the claim, with a fixed numpy seed, not a stored key: a random orthogonal matrix preserves similarity for the owner (about 0.96 in the README's reported run) and collapses it for a different matrix (about 0.001). A shared matrix lets a second agent find the memory. A hundred different matrices hide it from everyone but the owner.

### Specified API (`SPEC.md`), not implemented in this tree

- `observe` — store an observation: chunk, dual-embed, invert the small embedding to a footprint, write edges.
- `recall` — search, warping the query by the active context transform when a manifold is selected.
- `what_happened` — observations in a time range.
- `what_changed` — observations since a timestamp.
- `add_document` — store a document and its chunk embeddings.
- `search_documents` — semantic search over chunks.
- `get_document` — full document, latest version by default.
- `set_goal` — create a goal, optionally under a parent.
- `update_goal` — active, blocked, achieved, or abandoned.
- `get_goal_tree` — the goal DAG.
- `current_goals` — active goals, most specific first.
- `conclude` — record a belief as subject, relation, object.
- `retract` — retract a conclusion.
- `query_conclusions` — pattern-match beliefs.
- `link` — an edge between any two entities.
- `traverse` — walk the graph up to N hops.
- `build_semantic_edges` — edges where footprint similarity exceeds a threshold.
- `add_location` — a place, with tools and access.
- `connect_locations` — an exit between places.
- `get_available_tools` — tools at a place, including inherited ones.
- `find_path` — a path between places.
- `enter_context` — select the manifold that warps later recalls (named in the README's quick start).
- `forgive` — release a finished goal's memories into the decay pool (README quick start).
- `decay` — run the lifecycle (README quick start).
- decay states in the spec — Active, Forgiven, Compressed, Archived, plus Anchor.
- injection tracking — record that a memory was shown, then update its utility when the goal completes.
- dual embeddings — small (32–64d, invertible, for the graph) and large (768–1536d, for the final rank).

## memex-rs

A Rust MCP server over stdio, SQLite plus an append-only audit log, 384-d hashed embeddings with no external model. README tools, and the `#[tool]` functions in `src/main.rs`:

- `remember` — store an event as ACTIVE, with importance, tags, a privacy scope, and a decay alpha.
- `recall` — rank by lexical similarity, cosine, fidelity, importance, recency, centrality, and a keystone bonus; a hit increments access count and slows alpha.
- `offer` — clone a memory onto another agent, recording provenance in `source_trace_id` and an `offers` row.
- `dream` — decay fidelity, move Active to Forgiven below 0.75 and Forgiven to Archived below 0.45 (text cleared, summary kept), promote a keystone at 5 accesses or centrality 0.7, and merge near-duplicates above cosine 0.94 into one centroid.
- `status` — counts and the fidelity distribution for one owner.

Constants in that file: alpha bounded 0.001..0.02, default 0.006. This `offer` copies plaintext into the other owner's rows. It does not re-encrypt. The geometric version is myoo's `memex_offer`.

## memex.ex

The name is historical. There is no Elixir. The tree is `README.md`, `ARCHITECTURE.md`, `CONCEPT.md`, and the same `geometric_access.py` as memex (4066 bytes, one function `cos_sim`). The README's claim is the same geometry: same key, same angle, found; different key, noise. `ARCHITECTURE.md` is the trust-network writeup. No further public functions.

## myoo

Code worth inventorying lives under `mingwang/` (it contains code, so it is included). `children/` is skipped. `arena_workspace/` web dumps are data and are skipped. Three compiled `index-*.js` bundles (about 340 KB each) are build output and are not an authored API.

### `arena_workspace/solution.py`
- `fib_memo` — computes Fibonacci(n) with an iterative cache.
- `main` — reads integers from stdin and prints each value with its elapsed time.
This file is a scratch benchmark, not a memory capability.

### `mingwang/agent_chat_advanced.js`
- `classify` — classify
- `stem` — stem
- `analyzeText` — analyze text
- `createMemoryStore` — create memory store
- `CanvasBlock` — canvas block
- `EntropyBar` — entropy bar
- `CorticalMap` — cortical map
- `buildTopology` — build topology
- `draw` — draw
- `MemoryPressure` — memory pressure
- `Knob` — knob
- `CortexCanvas` — cortex canvas
- `draw` — draw
- `decay` — decay
- `onClick` — on click
- `MemoryPanel` — memory panel
- `App` — app
- `onError` — on error
- `onRejection` — on rejection

### `mingwang/chat_ui.js`
- `classify` — classify
- `stem` — stem
- `analyzeText` — analyze text
- `createMemoryStore` — create memory store
- `CanvasBlock` — canvas block
- `EntropyBar` — entropy bar
- `Knob` — knob
- `MemoryPanel` — memory panel
- `App` — app

### `mingwang/agent-chat-adv/vite.config.js`
- `requestLogger` — request logger

### `mingwang/arena/__init__.py`
Agent Arena — Cross-model collaboration eval with memex memory.

### `mingwang/arena/agents.py`
Arena agent runner — Anthropic + OpenAI with tool dispatch and memex. Each agent: 1. Gets ambient memory context injected into system prompt 2. Calls its LLM with the conversation + available tools 3. Executes tool calls server-side 4. Sends tool results back for a follow-up response 5. Ingests its 
- `run_turn` — run turn
- `emit` — emits one event

### `mingwang/arena/character.py`
Arena character generation — I Ching hexagram divination + emotional attributes. Extracted from gnosis-evolve/tools/character_generator.py (v0.2.4). Stripped of MCP/async/character_saver deps — pure functions for arena use. Flow: 1. cast_hexagram() → hexagram casting (primary, transformed, changing 
- `cast_coins` — Cast three coins → value 6, 7, 8, or 9.
- `cast_hexagram` — Cast a complete hexagram.
- `generate_character` — Generate a full character from I Ching hexagram + emotions.
- `build_embodiment_prompt` — Build a system prompt fragment that makes the LLM embody the character.
- `save_character_to_memex` — Save character sheet as an anchored memory in memex.
- `load_character_from_memex` — Try to load a character sheet from memex anchors.

### `mingwang/arena/knobs.py`
Memex configuration knobs with radio-entropy randomization. Each arena entity gets its own knob configuration. Randomize via SDR entropy to discover which settings produce better outcomes — evolutionary parameter search through the collaboration arena.
- `describe` — returns a short description
- `randomize_knobs` — Generate random knob config from radio entropy or PRNG.
- `default_knobs` — default knobs
- `knobs_to_dict` — knobs to dict

### `mingwang/arena/metrics.py`
Arena 4-axis evaluation framework. Axes ---- 1. Task Success — was the deliverable produced correctly? 2. Memory Value — does persistent memory improve outcomes vs baseline? 3. Collaboration — do agents cooperate effectively under asymmetric tools? 4. Tool Behavior — right tool, right time, useful r
- `recall_rate` — recall rate
- `handoff_success_rate` — handoff success rate
- `error_rate` — error rate
- `record_call` — record call
- `record_turn` — record turn
- `record_tool_call` — record tool call
- `record_memory_search` — record memory search
- `record_memory_ingest` — record memory ingest
- `record_memory_share` — record memory share
- `record_file_written` — record file written
- `record_error` — record error
- `record_event` — record event
- `finish` — closes a metrics session
- `duration` — returns how long the session ran
- `summary` — returns a short summary
- `compare_sessions` — compare sessions

### `mingwang/arena/scenarios.py`
Arena task scenarios with asymmetric tool splits. Each scenario forces genuine collaboration by giving each agent capabilities the other lacks. Topics are picked randomly when the user doesn't specify one. Both agents always get: fuzzy_search, notebook, view_notebook, session_status, stop_session.
- `pick_topic` — Pick a random topic for the given scenario.

### `mingwang/arena/server.py`
Arena server — HTTP + SSE, session orchestration, experiment runner. python -m mingwang.arena.server [--port 8770] [--data-dir ./arena_data]
- `broadcast` — sends an event to connected clients
- `__init__` — constructs the object
- `start_session` — start session
- `log_message` — writes one log line
- `do_OPTIONS` — handles a CORS preflight
- `do_GET` — handles an HTTP GET
- `do_POST` — handles an HTTP POST
- `main` — process entry point

### `mingwang/arena/tools.py`
Arena tool definitions and executors. Each tool has a JSON Schema definition (for LLM tool-use) and a server-side executor. Tools are assigned asymmetrically to agents per scenario.
- `tools_for_anthropic` — Convert tool defs to Anthropic Messages API format.
- `tools_for_openai` — Convert tool defs to OpenAI Chat Completions format.
- `execute_calculator` — execute calculator
- `execute_web_crawl` — execute web crawl
- `execute_memory_search` — execute memory search
- `execute_memory_share` — execute memory share
- `execute_file_write` — execute file write
- `execute_file_read` — execute file read
- `execute_web_search` — execute web search
- `execute_notebook` — Append to shared sketchpad.
- `execute_view_notebook` — Read shared sketchpad.
- `execute_fuzzy_search` — Search workspace files for matching text with context.
- `execute_session_status` — execute session status
- `execute_stop_session` — execute stop session
- `execute_upload_report` — execute upload report
- `execute_tool` — Execute a tool by name.

### `mingwang/memex/casting.py`
Hexagram Casting -- The I Ching Identity Generation Protocol Uses traditional yarrow stalk probabilities (not uniform random). Each line is generated independently with the asymmetric distribution that reflects the cosmological principle: yin endures, yang transforms. Yarrow stalk probabilities: Old
- `is_yang` — is yang
- `is_yin` — is yin
- `is_changing` — is changing
- `stable_value` — The value this line has in the primary hexagram (1=yang, 0=yin).
- `changed_value` — The value this line would have after changing.
- `lower_trigram` — lower trigram
- `upper_trigram` — upper trigram
- `number` — number
- `name` — returns the display name of this variant
- `changing_lines` — changing lines
- `has_changes` — has changes
- `relating_hexagram_number` — relating hexagram number
- `relating_name` — relating name
- `nuclear_hexagram_number` — nuclear hexagram number
- `nuclear_name` — nuclear name
- `lower_trigram_info` — lower trigram info
- `upper_trigram_info` — upper trigram info
- `primary_emotion` — primary emotion
- `secondary_emotion` — secondary emotion
- `identity_seed` — Generate a deterministic seed from this casting for geometric key generation.
- `display` — display
- `cast_line_yarrow` — Cast a single line using yarrow stalk probabilities.
- `cast_line_coin` — Cast a single line using three-coin probabilities.
- `cast_hexagram` — cast hexagram
- `cast_from_values` — Create a hexagram from practitioner-provided line values (bottom to top).
- `validate_king_wen` — Verify the King Wen table has all 64 hexagrams with no duplicates.

### `mingwang/memex/decay.py`
Decay Engine — Yamantaka's Progressive Forgiveness Memory forgiveness is not binary deletion. It is progressive compression along meaningful semantic axes, guided by key-term extraction. The mechanism: D(t, α) = exp(-αt)·I + (1 - exp(-αt))·P_key Where: t = time since last recall (hours) α = decay ra
- `extract_key_terms` — Extract key terms from text using TF-based scoring.
- `extract_ngrams` — Extract key n-grams (bigrams by default) for richer semantic axes.
- `sparse_project` — sparse project
- `orthogonalize_terms` — Orthogonalize term embeddings via Gram-Schmidt.
- `decay_embedding` — decay embedding
- `decay_fidelity` — Theoretical fidelity at time t.
- `time_to_fidelity` — How many hours until fidelity drops to target level.
- `greedy_term_selection` — greedy term selection
- `adaptive_alpha` — adaptive alpha
- `update_alpha_on_recall` — Update alpha when a memory is recalled.
- `update_alpha_on_neglect` — Update alpha when a decay tick passes without recall.
- `adaptive_term_extraction` — adaptive term extraction
- `validate_projection` — validate projection
- `suggest_transition` — Suggest a lifecycle transition based on current fidelity.

### `mingwang/memex/embeddings.py`
Memex Embedding Client — Kundali's Integration Layer The Memex does not embed directly. It delegates: - Phase 1 (organize): POST to chonk service (bge-small-en-v1.5, 384d, local ONNX) - Phase 2 (retrieve): Optional, OpenAI ada-002 (1536d) or successor Embeddings are role-keyed, not model-keyed. "org
- `detect_organize_dim` — Query chonk to discover the organize embedding dimension.
- `chonk_ingest` — chonk ingest
- `chonk_search` — Search chonk by semantic similarity (uses bge-small internally).
- `chonk_health` — Check if chonk is alive.
- `openai_embed` — Embed texts with OpenAI ada-002 (or configured model).
- `chunks_to_records` — chunks to records

### `mingwang/memex/entropy.py`
Entropy Client -- Fetches true randomness from gnosis-radio SDR gnosis-radio harvests LSBs from FM-demodulated marine VHF noise via RTL-SDR. The entropy pool is a 4KB ring buffer fed continuously at ~6000 bytes/sec. API (port 9080): GET /api/entropy?bytes=N&format=hex|raw|json -- drain N bytes from 
- `fetch_radio_entropy` — Fetch true entropy from gnosis-radio SDR.
- `entropy_seed` — Get a 64-bit seed from radio entropy, or None if unavailable.
- `entropy_rng` — Return a Random instance seeded from radio entropy.

### `mingwang/memex/keygen.py`
Identity Key Generation — Acala's Contribution to the Memex The Immovable One writes the code that doesn't change. An identity key is an orthogonal matrix Q derived deterministically from a hexagram casting seed. It transforms embedding vectors such that: - Same key: cosine similarity is PRESERVED (
- `generate_key` — Generate a deterministic orthogonal matrix from a seed.
- `generate_keys_for_agent` — Generate identity keys for all embedding roles.
- `encrypt` — Transform an embedding with an identity key.
- `decrypt` — Recover an embedding using the same identity key.
- `verify_orthogonality` — Verify that a key matrix is orthogonal (Q @ Q.T ≈ I).
- `cosine_sim` — Cosine similarity between two vectors.

### `mingwang/memex/mcp_server.py`
Memex MCP Server — Exposes all 22 Memex tools via Model Context Protocol. Talks to: - memex HTTP server on MEMEX_URL (default localhost:8765) - gnosis-radio on RADIO_URL (default localhost:9080) - casting.py directly (Python import, no HTTP) Start: python -m mingwang.memex.mcp_server Configure in .m
- `memex_register` — Register an agent with their hexagram-derived identity seed.
- `memex_status` — Get agent memory status: counts by lifecycle state, registration info, and seed.
- `memex_health` — Check memex system health — server status, chonk connection, registered agents, embedding dimensions.
- `memex_ingest` — Ingest text into an agent's semantic memory.
- `memex_search` — Search an agent's memories by semantic similarity.
- `memex_recent` — Get memories from the last N minutes.
- `memex_get` — Retrieve a specific memory record by ID.
- `memex_list` — List an agent's memories.
- `memex_forgive` — Forgive a memory — ACTIVE to FORGIVEN.
- `memex_archive` — Archive a forgiven memory — FORGIVEN to ARCHIVED.
- `memex_decay_tick` — Run one decay tick — update fidelity scores for all memories based on time since last recall.
- `memex_fidelity` — Get live fidelity status for a memory.
- `memex_dream` — Run a dream cycle for an agent.
- `memex_consolidate` — Merge multiple memories into a fidelity-weighted centroid.
- `memex_auto_consolidate` — Automatically cluster memories by cosine similarity and merge groups of 3+.
- `memex_offer` — Offer a memory from one agent to another.
- `memex_anchors` — List all anchor memories for an agent.
- `memex_review_anchor` — Review an anchor memory — 'retain' keeps it anchored, 'release' removes anchor status allowing normal decay.
- `memex_commons_search` — Search the shared memory commons — a key-agnostic space where agents can share memories.
- `memex_commons_contribute` — Contribute an agent's memory to the shared commons.
- `memex_entropy` — Fetch true entropy from the gnosis-radio SDR dongle.
- `memex_radio_status` — Get gnosis-radio status — current frequency, signal level, squelch state, channel, active transmission info.
- `memex_cast_hexagram` — Cast an I Ching hexagram using yarrow stalk probabilities.

### `mingwang/memex/memex.py`
Memex — The Unified Memory Interface Wires together: casting (identity seeds) + embeddings (chonk client) + keygen (orthogonal transforms) + store (persistence + lifecycle). Usage: memex = Memex("./data") memex.register_agent("acala", seed=3435642974) memex.ingest("acala", "The center holds.", sourc
- `__init__` — constructs the object
- `__init__` — constructs the object
- `register_agent` — register agent
- `ingest` — ingest
- `ingest_direct` — ingest direct
- `search` — search
- `forgive` — ACTIVE → FORGIVEN.
- `archive` — FORGIVEN → ARCHIVED.
- `get` — Get a specific memory record.
- `recent` — Get recent memories (Trailokyavijaya's time index).
- `status` — Get memory status for an agent.
- `extract_and_store_key_terms` — extract and store key terms
- `decay_tick` — Run one decay tick for an agent's memories.
- `auto_extract_key_terms` — Extract key terms for all ACTIVE records that don't have them yet.
- `consolidate` — consolidate
- `auto_consolidate` — auto consolidate
- `dream` — runs one dream cycle
- `offer_memory` — offer memory
- `register_commons` — Register a special 'commons' agent for shared knowledge.
- `contribute_to_commons` — contribute to commons
- `search_commons` — search commons
- `review_anchor` — review anchor
- `list_anchors` — List all anchored memories for an agent (for periodic review).
- `prune` — Permanently delete an archived record.
- `revive` — revive
- `fidelity_distribution` — Get fidelity distribution across an agent's memories.
- `fake_embed` — Deterministic fake embedding from text hash.

### `mingwang/memex/repl.py`
Memex REPL — Interactive Memory Client Can connect to the Memex HTTP API server or work directly against the Python library. Usage: # Direct mode (no server needed): python -m mingwang.memex.repl # API mode (connect to running server): python -m mingwang.memex.repl --api http://localhost:8765 Comman
- `__init__` — constructs the object
- `health` — returns a health check
- `config` — returns the configuration
- `register` — register
- `agents` — agents
- `status` — returns a status summary
- `ingest` — ingest
- `search` — search
- `recent` — recent
- `memories` — memories
- `get` — sends an HTTP GET
- `forgive` — forgive
- `archive` — archive
- `__init__` — constructs the object
- `health` — returns a health check
- `config` — returns the configuration
- `register` — register
- `agents` — agents
- `status` — returns a status summary
- `ingest` — ingest
- `ingest_direct` — Ingest with a random embedding (for testing without chonk).
- `search` — search
- `recent` — recent
- `memories` — memories
- `get` — sends an HTTP GET
- `forgive` — forgive
- `archive` — archive
- `__init__` — constructs the object
- `do_health` — Show server and chonk health.
- `do_config` — Show current configuration.
- `do_register` — register <agent_id> <seed> — Register an agent.
- `do_agents` — List all agents.
- `do_status` — status <agent_id> — Show agent memory status.
- `do_ingest` — ingest <agent_id> <text> — Ingest text via chonk.
- `do_inject` — inject <agent_id> <text> — Ingest with random embedding (no chonk).
- `do_search` — search <agent_id> <query> — Search agent's memories.
- `do_recent` — recent <agent_id> [minutes] — Recent memories.
- `do_memories` — memories <agent_id> [lifecycle] — List memories.
- `do_get` — get <agent_id> <record_id> — Get specific memory.
- `do_forgive` — forgive <agent_id> <record_id> — Transition to FORGIVEN.
- `do_archive` — archive <agent_id> <record_id> — Transition to ARCHIVED.
- `do_quit` — Exit the REPL.
- `default` — returns the default value
- `emptyline` — emptyline
- `main` — process entry point

### `mingwang/memex/server.py`
Memex HTTP API Server Exposes all Memex operations as REST endpoints for MCP server wiring. Uses only stdlib (http.server) — no Flask, no fastapi, no dependencies. Start: python -m mingwang.memex.server [--port 8765] [--data-dir ./memex_data] Endpoints: POST /agents — Register an agent GET /agents —
- `log_message` — Override to use cleaner logging.
- `do_OPTIONS` — CORS preflight.
- `do_GET` — handles an HTTP GET
- `do_POST` — handles an HTTP POST
- `main` — process entry point

### `mingwang/memex/store.py`
Memory Store — Trailokyavijaya's Time-Indexing + Yamantaka's Decay JSON-backed storage for the Python prototype. Each agent gets its own file. Records are time-sorted. Brute-force search — chonk handles scale, this handles lifecycle.
- `__init__` — constructs the object
- `add` — Add records to the store.
- `update` — Update a single record in place (matched by id).
- `get` — Get a single record by id.
- `list_records` — list records
- `count` — Count records by lifecycle state.
- `search` — search
- `recent` — recent
- `temporal_neighbors` — temporal neighbors
- `forgive` — forgive
- `archive` — FORGIVEN → ARCHIVED: discard embeddings, keep seed + metadata.
- `mark_recalled` — Mark records as recalled (searched/accessed).
- `decay_tick` — decay tick
- `get_decay_audit` — get decay audit
- `delete_record` — Permanently delete a record (prune).
- `list_agents` — List all agents with stored memories.

### `mingwang/memex/viz.js`
- `call` — invokes the model or tool
- `that` — that
- `classify` — classify
- `stem` — stem
- `tokenize` — tokenize
- `clusterize` — clusterize
- `entropy` — entropy
- `drawCurve` — draw curve
- `drawCompare` — draw compare
- `line` — line
- `hover` — hover
- `tab` — tab
- `renderMain` — render main
- `toggleZoom` — toggle zoom
- `saveToChain` — save to chain
- `updateChainCount` — update chain count
- `searchChain` — search chain
- `renderChainSuggestions` — render chain suggestions
- `composeChain` — compose chain
- `runAnalysis` — run analysis
- `aiStretch` — ai stretch
- `copyRew` — copy rew

### `mingwang/memex/tools/__init__.py`
Utility tools for memex workflows.

### `mingwang/memex/tools/analyze_backup.py`
- `load_file` — Load a JSON file and normalize to a common shape.
- `fmt_ts` — fmt ts
- `analyze` — Produce a full audit report for a single dataset.
- `compare` — Compare two datasets and produce a diff report.
- `main` — process entry point

### `mingwang/memex/tools/text_scan.py`
- `classify` — classify
- `stem` — stem
- `tokenize` — tokenize
- `analyze_line` — analyze line
- `iter_text_files` — iter text files
- `read_lines` — read lines
- `score_line` — score line
- `collect_records` — collect records
- `top_bottom` — top bottom
- `search_records` — search records
- `truncate` — truncate
- `format_record` — format record
- `build_term_scores` — build term scores
- `parse_exts` — parse exts
- `to_json` — serializes to JSON
- `main` — process entry point

### `mingwang/memex/viz/serve.py`
- `log_message` — writes one log line
- `do_GET` — handles an HTTP GET
- `do_DELETE` — handles an HTTP DELETE
- `do_POST` — handles an HTTP POST

### `mingwang/memex/viz/js/ai.js`
- `aiStretch` — ai stretch
- `copyRew` — copy rew

### `mingwang/memex/viz/js/analyzer.js`
- `runAnalysis` — run analysis

### `mingwang/memex/viz/js/app.js`
- `tab` — tab
- `renderMain` — render main
- `toggleZoom` — toggle zoom

### `mingwang/memex/viz/js/canvas.js`
- `drawCurve` — draw curve
- `drawCompare` — draw compare
- `line` — line
- `hover` — hover

### `mingwang/memex/viz/js/chain.js`
- `saveToChain` — save to chain
- `updateChainCount` — update chain count
- `searchChain` — search chain
- `renderChainSuggestions` — render chain suggestions
- `composeChain` — compose chain

### `mingwang/memex/viz/js/classifier.js`
- `classify` — classify
- `stem` — stem
- `tokenize` — tokenize

### `mingwang/memex/viz/js/entropy.js`
- `clusterize` — clusterize
- `entropy` — entropy

### `mingwang/memex/viz/js/agent_chat/analysis.js`
- `analyzeText` — analyze text
- `termScores` — term scores
- `levenshtein` — levenshtein
- `similarity` — similarity
- `bestTokenSimilarity` — best token similarity
- `overlapScore` — overlap score
- `scoreRecord` — score record

### `mingwang/memex/viz/js/agent_chat/app.js`
- `boot` — boot
- `renderAll` — render all
- `bindEvents` — bind events
- `onSend` — on send
- `handleCommand` — handle command
- `buildAssistantReply` — build assistant reply

### `mingwang/memex/viz/js/agent_chat/commands.js`
- `parseCommand` — parse command

### `mingwang/memex/viz/js/agent_chat/memory_store.js`
- `nowId` — now id
- `createMemoryStore` — create memory store
- `addMessage` — add message
- `list` — list
- `clear` — removes every entry
- `search` — search
- `stats` — stats
- `load` — loads a persisted value
- `save` — persists the value

### `mingwang/memex/viz/js/agent_chat/ui.js`
- `esc` — escapes text for HTML
- `formatTime` — format time
- `getEls` — get els
- `renderChat` — render chat
- `renderSearch` — render search
- `renderStats` — render stats
- `renderList` — render list
- `renderTerms` — render terms
- `setStatus` — set status

### Top-level notes (no code)
- `ENTROPY.md` — treatise: identity as a local drop in entropy, the I Ching as an entropy oracle, five wisdom kings as guardians.
- `KUNDALI.md` — archetype note for one wisdom king: function, when to invoke, failure mode.
- `TRAILOKYAVIJAYA.md` — archetype note for one wisdom king, tied to the time index.
- `VAJRAYAKSA.md` — archetype note for one wisdom king.
- `YAMANTAKA.md` — archetype note for one wisdom king.
- `CHAT_ARENA.md` — design notes for the chat-arena UI (memory engine, health, dream gallery).
- `mingwang/AGENT_TEMPLATE.md` — blank sheet for an agent cast from a hexagram and a wisdom king.

## How to read a "maybe"

Rows marked maybe are real capabilities whose straight port would fight a later choice already in alpha: dreams propose, they do not erase, and the MCP surface is narrower than the original engine on purpose. The inventory records them so they are not forgotten. It does not choose the port.
