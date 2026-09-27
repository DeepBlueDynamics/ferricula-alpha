# Lume retrieval frontier — BM25 + GTR-T5 + SKG blend, and the named upgrades

Source: `ferricula/research/AI Agent Memory and Thermodynamic Architectures_ Mid-2026 Frontier Briefing for Ferricula and LUME.pdf` (pages 2, 6–7, 8–9). PDF quotes verbatim. Crate facts verified 2026-09-19 by direct read of `ferricula_v2/crates/ferricula-search/` and `ferricula_v2/crates/ferricula-semantic/` (file/symbol cited per claim). Scope: Retrieval (Whippet). No crate edits made; this is the mapping note.

## The blend today (verified in ferricula_v2)

| Signal | Implementation | Verified facts |
|---|---|---|
| Lexical (BM25) | `ferricula-search/src/bm25.rs` | Field-aware, 3 variants (`SearchVariant::{Classic,Plus,L}`); defaults `k1=1.2, b=0.75, delta=1.0, title_weight=2.0, body_weight=1.0`; stopword-filtered queries; `COORD_FLOOR=0.5` multi-term coverage multiplier. Matches the PDF's "field-aware BM25 (k1=1.2, b=0.75, title_weight=2.0)" exactly. |
| Dense (GTR-T5) | `ferricula-semantic/src/embedder.rs` (local ONNX, BERT/T5 input auto-detect via `token_type_ids`); `ferricula-search/src/hybrid.rs::embed_text` (remote shivvr path — ingests into a throwaway scratch store, reads the 768-d "organize" vector off the response) | 768-d GTR-T5-base; `store::Chunk` already carries a **dual** embedding: `embedding` (768-d gtr-t5-base, "organize") + optional `embedding_retrieve` (1536-d ada-002) (`ferricula-semantic/src/lib.rs`). |
| Graph (SKG) | `ferricula-search/src/semantic_mesh.rs::cooccurrence_relatedness`, `graph_search.rs` | z-score vs independence `E=a·b/n`, symmetric finite-population variance, then **log-compress** `sign(z)·ln(1+|z|)` and `tanh(·/3.0)` → [−1,1]; one-hop query walk (`compute_skg_scores`) → per-section [0,1] boost; `SKG_EXPAND_MIN=0.5` recall expansion; fully local. |
| Fusion | `hybrid.rs::blend_hybrid_scores` | Multiplicative: `hybrid = bm25 * (1 + alpha*semantic + beta*skg)`; `beta=0` reproduces lexical+semantic exactly. v2 addition: `fusion.rs` = RRF (k=60) + FST intent-boost, extracted from shivvr `temp_store.rs` — an alternate ranked-list fusion path, currently standalone with its own test. |
| Pre-filter | `fast_retrieval.rs` | `MiniRoaring` bitmap + `PrimeFilter` (Bloom + Gödel-prime tags) candidate pruning ahead of scoring. |
| Chunking | `ferricula-semantic/src/chunker.rs` | Semantic chunking (embedding-similarity breakpoints): defaults `min 50 / max 512` tokens, `similarity_threshold 0.5`, with optimization passes. |
| vec2text | `ferricula-semantic/src/inverter.rs` | projection (Nd → 16×768) → T5 encoder → T5 decoder, greedy decode. **Hard-coupled to the 768-d organize vector.** |

## Frontier verdict on the blend — keep it

- PDF p.6: "Hybrid retrieval is the frontier default: lexical (BM25) + dense (embeddings) + graph signals, fused. Lume's multiplicative blend with significance-scored (not raw co-occurrence) entity edges — which resists 'promiscuous hubs' and a regression test confirms outranks a perfect-Jaccard hub — is a sophisticated, auditable instance of this."
- PDF p.8, recommendation 1: "keep the BM25 + significance-graph blend, which is genuinely frontier-grade and auditable."

Implication: no re-architecture of the fusion or the SKG. All three upgrades below slot into the dense primitive or add new capability axes beside it.

## Upgrade 1 — Qwen3-Embedding / BGE-M3 (retire GTR-T5 for retrieval)

- Claim (p.7): "Lume's GTR-T5 (768-d) is now dated; Qwen3-Embedding or BGE-M3 would be upgrades."
- Qwen3-Embedding (arXiv:2506.05176): 8B model "ranks No.1 in the MTEB multilingual leaderboard (as of June 5, 2025, score 70.58)"; 100+ languages, 32K context, Matryoshka dimensions 32–4096, Apache-2.0, 0.6B/4B/8B variants. BGE-M3: MIT, dense+sparse+multi-vector, 100+ languages — the production workhorse.
- Switch threshold (p.8, rec.1): "if recall@k on your own corpus is materially below a Qwen3/BGE-M3 baseline, switch; otherwise the engineering cost of re-indexing isn't justified."
- Mapping to crates:
  - `embedder.rs` is already model-agnostic for input signature (BERT `token_type_ids` vs T5 detection); dimension flows from the ONNX model, not the code. Gate is `ml`-feature ONNX (CPU + optional CUDA) — Qwen3/BGE-M3 ONNX exports drop in as model paths.
  - **Dual-embedding precedent is the safe path**: `store::Chunk.embedding` (organize, 768-d GTR-T5) must stay GTR-T5 because the vec2text `Inverter` projection is T5/768-coupled; add Qwen3 as the retrieve embedding alongside `embedding_retrieve` (the ada-002 slot already exists as the schema precedent). Matryoshka dims let retrieval dim be tuned independently of storage cost. [INFERENCE] unless a new vec2text projection head is trained for Qwen3-space, organize and retrieve embeddings permanently diverge — which the two-slot schema already anticipates.
  - `hybrid.rs` remote path (embed-by-ingest against shivvr) and `openai.rs` (ada-002) each need a "retrieve model" config knob; the blend formula itself is dimension-agnostic (cosine over equal-length vectors).

## Upgrade 2 — ColBERT/ColPali late interaction (largest capability gap)

- Claim (p.7): "This is the biggest capability Lume currently lacks and the natural path for indexing visual/PDF documents."
- Lineage: ColBERT (token-level MaxSim late interaction, Khattab & Zaharia 2020) → ColBERTv2/PLAID/WARP → ColPali/ColQwen (Faysse et al., ICLR 2025): visual retrieval over image patches via VLMs, SigLIP + PaliGemma/Qwen, 128-d patch embeddings, evaluated on ViDoRe by NDCG@5. Efficiency variants: ColModernVBERT (250M params, within 0.6 pts of ColPali), token pruning.
- Adoption gate (p.9, rec.2): "Benchmark on ViDoRe; proceed if NDCG@5 beats the current dense+BM25 blend on your document mix." For paginated PDFs, prefer page-level chunking: "Page-level chunking won NVIDIA's 2024 benchmarks for paginated documents (0.648 accuracy, lowest variance)" (p.6).
- Mapping to crates:
  - **Schema change**: chunk goes from one vector to per-token / per-patch multi-vectors. The scoring and query path is mine (ferricula-search); where multi-vectors durably persist is a storage-internals question — Engine's boundary per team assignment. Needs a Goldfish/Armadillo contract decision before implementation.
  - `fast_retrieval.rs` (`MiniRoaring` + `PrimeFilter`) is the natural PLAID-style candidate prefilter before MaxSim scoring; the pruning-then-exact-scoring shape already exists.
  - `blend_hybrid_scores` gains MaxSim as a fourth signal (or replacement for the dense term on document queries); `fusion.rs` RRF is the ready-made fusion for heterogeneous scorers that don't share a score scale.
  - ColPali targets the PDF case where today only extracted text is indexed — patch embeddings add layout/visual signal the BM25+text-dense blend cannot see.
- Novelty hook (p.8, open problem 4): "Combining ColPali-style visual late interaction with significance-weighted entity graphs (Lume's strength) is not yet standard practice." Both halves already live in ferricula-search; the fusion is the unclaimed part.

## Upgrade 3 — RAPTOR / HippoRAG hierarchical + associative indexing

- Claim (p.6): "Microsoft GraphRAG (entity-relation graphs), RAPTOR (recursive summarization into hierarchical trees for multi-level retrieval), LightRAG (KG + vector, local+global search), HippoRAG (Personalized PageRank associative recall)."
- Mapping to crates:
  - **HippoRAG → `graph_search.rs`**: current `compute_skg_scores` is a one-hop walk with capped decay. Personalized PageRank over `EntityGraph` is the multi-hop generalization of that same walk — same adjacency (`build_adjacency`), same significance weights as transition mass. [INFERENCE] bounded-depth PPR keeps it local and fast; no new graph structure needed.
  - **RAPTOR → `chunker.rs`**: today's chunker emits flat semantic chunks (50–512 tokens). RAPTOR adds recursive summary levels above the leaves; summaries are new chunks with parent/child links (fits in `store::Chunk.metadata` JSON without a schema break) and a blend rule so summaries and leaves compete fairly.
  - **LightRAG local+global** decomposes onto existing pieces: local = current BM25+SKG section search; global = RAPTOR summary levels.
  - Chunking spectrum note (p.6): semantic chunking "can improve recall by up to ~9% but is costly and sometimes unstable on real documents" — we already sit at the semantic tier; late chunking (embed the full doc with a long-context model, then split) becomes feasible with Qwen3's 32K context and is the next rung. [INFERENCE] GTR-T5's short input window is why late chunking isn't currently possible.

## What NOT to change (PDF-explicit)

- The multiplicative blend and significance-scored graph — "genuinely frontier-grade and auditable" (p.8).
- No ANN: exact scoring over pruned candidates stays (frontier-briefing synthesis, p.7–8, endorses exact-over-pruned for small active sets).

## Benchmarks for this surface

- **ViDoRe** (NDCG@5) — gate for ColPali/ColQwen.
- **BEIR / MTEB** — embedding/retrieval baselines; the recall@k comparison that decides the Qwen3 swap must run on our own corpus.
- Discipline (p.9, rec.3): "always pair an accuracy number with its token cost."

## Caveats

- Vendor/researcher-reported numbers, single-source, indicative not settled (PDF p.9); 2026 arXiv IDs are recent preprints possibly under revision (PDF p.10).
- Boundary note: durable multi-vector storage internals belong to Engine (Goldfish's assignment); this note claims only scoring/query/chunking paths for ferricula-search / ferricula-semantic.
- Verified vs inferred marked inline. No crate edits made.