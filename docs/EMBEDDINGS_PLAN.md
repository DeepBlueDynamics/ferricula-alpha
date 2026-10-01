# Meaning in the loop: embeddings through the gates

_Plan, 2026-09-27. Status: items 1–3 done (shivvr `/embed` on branch `feat/vision-audio-embed`, `TextEmbedder`/`ShivvrEmbedder`, `[embeddings]` + startup probe); 4–5 done and 6 partly done on `v3/r2b-recall` (see **Landed** below). Owner: Ferricula v3 (PLAN_V3 phase R2b)._

## Why

Today the agent recalls by **words**: lexical token coverage over memories and BM25 over document sections. It finds a paraphrase of something it read only 71% of the time at rank 1 (bench run `9aaf89f3`). It cannot tell that "Jony Ive's new device with OpenAI" and "the designer who left Apple is building hardware for an AI lab" are the same thought. Every judgment that should be about meaning (is this new? have I seen this before? do these two memories say the same thing? what is far from everything I know?) is currently either a word match or not made at all.

The original design (Kord): a vector store whose near neighbors live together in a graph, indexed by the words they contain (roaring bitmaps), where **outliers** (things that don't fit any neighborhood yet) are kept in the graph until something later connects them. This plan puts that into the running cognitive process.

## What we have (verified 2026-09-27)

| Fact | Evidence |
|---|---|
| All 3,362 recovered memories carry a 768-d vector, but **2,805 are all-zero placeholders** (ids 730222..4294945545; none below 100k); only 557 are non-zero. The zero rows need a backfill (embed their text) before dense recall can see them | probe of a copy of the recovery volume: `dims={768: 3362}`; zero-vector count 2026-09-27 (the first R2b startup probe picked one and measured cosine 0.0) |
| Those vectors are GTR-T5-base, exactly what the running shivvr produces | re-embedding three short memories through shivvr `:8085`: cosine **1.0000** each |
| The recovered graph has 470 nodes / 588 edges | same probe |
| shivvr `:8085` serves GTR-T5 (text, 768), SigLIP (image, 768), vec2text inversion, GPU | `GET /health` |
| shivvr has no plain `/embed`; only session/temp ingest (the v2 "scratch-store hack") and `/image/embed` | `src/api.rs` routes |
| shivvr's vision/audio work is **uncommitted** in `nuts.services/shivvr` | `git status` |
| Core already has cosine top-k with a roaring-bitmap candidate filter and graph neighborhoods | `Engine::vector_topk`, `MemoryGraph::neighbors`, `neighborhood_2` |
| Experience rows and document sections are stored **without** vectors | R1 report |

So the agent's past is already in one embedding space, and a service that speaks that space is running. Nothing asks it anything.

## The space

- **One text space for now: `gtr-t5-base@768`**, because it is the space of the entire recovered past and it supports vec2text inversion ("ghost echoes"). Every stored vector gets a `space` tag; vectors from different spaces are never compared.
- **Image space `siglip-base-patch16-224@768`** (shivvr `/image/embed`) for rendered PDF pages and, later, what the agent "sees". Text↔image comparison only through SigLIP's own text tower, never GTR↔SigLIP.
- A multilingual text space (for Chinese/Pāli parity) is a later migration: a second space alongside, re-embedded in the background, switched per query by language. Not part of this plan.

## Architecture

```
            ┌──────────────── Embedder trait (ferricula-semantic) ─────────────────┐
            │ space() -> "gtr-t5-base@768"   embed(&[&str]) -> Vec<Vec<f32>>        │
            │ impls: ShivvrHttp (default)  ·  LocalOnnx (feature ml)  ·  None        │
            └──────────────────────────────┬───────────────────────────────────────┘
                                           │ one call per input, batched
  sense door ─► phassa ─► EMBED ─► santīraṇa ─────────► saññā ─────────► gates ─► javana ─► commit (vector stored)
                                    novelty = 1 − max cos      neighbors' tags     sati-recall on
                                    dup if cos ≥ τ_dup         → recognition       dense candidates
                                    outlier if max cos < τ_out
```

`MeaningIndex` (server): one in-process view over three vector sets, all brute-force cosine behind roaring-bitmap filters (adequate to ~10⁵ rows; ANN only when it hurts):
- recovered memories (read-only, vectors already present),
- experience rows (vectors written at commit),
- document sections (vectors in a sidecar `documents/vectors/<doc_id>.f32` keyed by section index, written at ingest).

## Where meaning enters each gate and activity

| Moment / activity | Today | With embeddings |
|---|---|---|
| **Ingest (phassa)** | sections stored, BM25 indexed | each section embedded (batched); the "I read X" experience row gets the mean section vector; PDF pages optionally rendered and SigLIP-embedded |
| **Investigating (santīraṇa): known or new?** | none | `novelty = 1 − max cos` against experience + recovered; `cos ≥ 0.92` → "I've seen this" (short-circuit, reinforce the existing memory instead of writing a new one); `max cos < τ_out` → **outlier** |
| **Recognition (saññā)** | Ollaya 6-way choice (uncalibrated) | neighbors' tags (roaring bitmaps over the top-k) propose channel/topic tags; the saññā gate confirms. "Vectors near each other live in the graph by their words." |
| **Outliers** | not represented | an outlier gets an `unresolved` tag and a graph node with no edges; it is kept, never decayed below the exploration floor, and offered to curiosity and dreams. When a later input lands near it (cos ≥ τ_link) the runtime proposes a graph edge labeled as a *hypothesis* (Paṭṭhāna condition label), which the merge/link gate accepts or leaves pending |
| **Recall (chat, MCP `ferricula_recall`)** | lexical + BM25, RRF | adds a **dense arm** over all three sets (filter by channel/state bitmaps first), plus **one graph hop** from the top dense hits; RRF over lexical, BM25, dense, graph. The query is embedded once per turn |
| **Sati-recall gate** | not run in chat | the fused top-20 go through the Ollaya sati-recall judge; advisory until calibrated (it reorders but does not drop), then filtering once ECE < 0.05 |
| **Operator message → drives** | novelty fixed at 0.5 | `Stimulus::Operator{ novelty }` from the santīraṇa novelty of the message; a message about something the agent already knows well relieves less boredom |
| **Curiosity** | open thread from chat, else a random memory | open thread first; otherwise prefer **outliers and sparse regions** (memories with low neighbor density), entropy-weighted. After reading, novelty of what it read vs. what it knew is journaled: *did I learn something?* |
| **Consolidation (bhāvanā)** | clusters by what? | cosine clusters (single-link at cos ≥ 0.85) over the day's experience become `ClusterIndex` entries pointing at raw members (never replaced); the saṅkhāra-merge gate labels same-truth / contradiction |
| **Dreams** | "distant" = random older memories | residue = today's strongest; **distant** = sampled from far regions (low cosine to the residue centroid), entropy-picked (radio); **unresolved** = outliers nearest to today's residue (the ones most likely to be about to make sense) |
| **Ghost echoes** | none in v3 | for faded memories whose text was released, vec2text inversion through shivvr `/invert` renders what the vector still "remembers", labeled as reconstruction |
| **Visualization (later)** | none | `GET /meaning/projection` exports a 3D PCA of vectors with cluster and outlier labels for the Hyperia UI |

## Work items

1. **shivvr** (in `DeepBlueDynamics/shivvr`): commit the vision/audio WIP to a branch; add `POST /embed {texts: [..], model?: "gtr-t5-base"} -> {model, dim, vectors}` (batched, no session/store side effects) and keep `/image/embed`; health reports model ids. Rebuild the container.
2. **ferricula-semantic**: `Embedder` trait with `space()`; `ShivvrEmbedder` (HTTP, batching, timeout, retries off) and `NoEmbedder`; `LocalOnnx` stays behind `ml`.
3. **Server config `[embeddings]`**: `backend = "shivvr" | "none"`, `url`, `space = "gtr-t5-base@768"`, `batch`, `timeout_secs`. Startup checks the backend's space matches the recovered memory's (probe: re-embed one short memory, require cos > 0.999, else refuse dense recall and log why).
4. **Write path**: ingest embeds sections (sidecar vectors) and the reading memory; every experience commit (chat turn summary, thought, dream) stores its vector with `space` tag. Backfill command `ferricula-server embed-backfill` for existing experience rows and documents.
5. **`MeaningIndex`**: dense top-k over the three sets with bitmap filters; novelty; neighbor density; outlier test; farthest-from-centroid sampling.
6. **Wire into**: santīraṇa (novelty, dup short-circuit, outlier tag), saññā (neighbor tags), recall (dense arm + graph hop + sati-recall advisory rerank), drives (operator novelty; curiosity from outliers), bhāvanā (cosine clusters), dreams (far + unresolved sampling).
7. **Degraded mode**: embedder down → dense arm absent, novelty unknown (0.5), everything else works; `/status` shows `embeddings: degraded`.
8. **Benchmarks** (ledger rows): docs suite gains `dense` and `hybrid` arms (paraphrase R@1 is the number to move, currently 0.71 BM25-only); a recall suite over Steve's memories with paraphrased queries; novelty calibration (does novelty predict "the operator says this is new to me"?); latency p50/p95 per embed call and per recall.

## Exit test

Over MCP, on Steve's live memory: (a) "what did you think about the designer who left Apple?" recalls the Jony Ive memories with **no word overlap** in the query; (b) handing him the MemGPT paper twice reports the second as already read (dup short-circuit) and a related paper as novel-but-near; (c) a deliberately odd observation is kept as an outlier and shows up in the next dream's *Unexplained* list; (d) the paraphrase benchmark's hybrid R@1 beats 0.71 with the ledger row to prove it; (e) with shivvr stopped, chat still works and `/status` says `embeddings: degraded`.

## Order

After R3 (drives) merges: 1 → 2 → 3 → 4 → 5 → recall and santīraṇa in 6 → benchmark → the rest of 6. Items 1–3 can start now in parallel since they touch shivvr and `ferricula-semantic`, not the server files R3 is editing.

## Landed (v3/r2b-recall)

- **`MeaningIndex`** (`crates/ferricula-server/src/meaning.rs`): recovered (stored vector when non-zero, else a sidecar vector from the `text` tag, flagged `tag_text_truncated` when v1 cut it at 200 chars), experience rows, document sections; one space. Sidecars `state_dir/meaning/{recovered,experience,sections}.fmv` (magic `FMEANV01`, set, space, dim, entries of key + FNV-1a text hash + source + chars + f32×dim, FNV-1a checksum), written tmp+fsync+rename; a file in another space or with a bad checksum is ignored and re-embedded. Brute-force cosine with evidence (dreams, v1 `[dream image]` rows), lifecycle and roaring-bitmap id filters.
- **Backfill**: automatic after a passing probe (`[embeddings] backfill = true`), `POST /meaning/backfill`, and offline `ferricula-server embed-backfill --config <file>`; resumable (sidecars persisted every 8 batches). Progress in `/status` → `meaning` and `GET /meaning`. Steve: 2,802 rows in 11.5 s against shivvr on the GPU.
- **Write time**: new experience rows (reading, curiosity reflections, dreams, chat turns) and document sections are embedded right after they are committed; failures leave them pending for the backfill, never failing the write.
- **Recall**: hybrid recall adds a **dense** arm (each sentence of the message embedded separately, segment lists fused weighted by segment novelty, top-k over all three sets, dreams excluded) and a **graph** arm (one hop from the top 3 dense recovered hits, weight 0.5), fused with the lexical and BM25 arms by RRF; lexical memory arms weigh `[recall] lexical_weight` (0.5) while dense is present; the best 2 dense hits are kept at fused ranks 2 and 4 (`dense_guarantee`). Candidates list their `arms` and `dense_score`; responses add `dense_hits` and `dense_novelty`. No embedder: exactly the lexical recall.
- **Faded memories**: 1,925 Archived + 28 Forgiven recovered rows are excluded from ranking by the v3 release policy; `[recall] include_faded_recovered = true` admits them (the parents memory 3802021270 is Archived). Operator decision.
- **Conversation memory**: every completed chat turn becomes a `hearing` row ("<operator_name> said: ...") and a `thinking` row ("I said: ..."), linked by causal `paccaya:anantara` edges within the conversation, recallable from any later conversation.
- **Santīraṇa**: operator-message novelty = 1 − max cosine (most novel sentence); ingest records `near_duplicate_of` at mean-section cosine ≥ 0.97; curiosity prefers outliers (lowest mean cosine to 5 nearest memories); dream `distant` traces come from the third of candidates farthest from the residue centroid; the dream prompt demands traces-only material and the journal reports `grounding` (share of dream sentences with best trace cosine ≥ 0.35).
- **Benchmarks**: `ferricula-bench docs` gains `dense`, `hybrid_rrf`, `hybrid` arms; `ferricula-bench recall` runs 30 hand-written meaning-level queries (`research/bench/private/steve-recall-queries.json`) over a copy of the recovered memory.

Still open from this plan: sati-recall advisory rerank, saññā neighbor tags, outlier `unresolved` tags and hypothesis edges, cosine consolidation clusters, ghost echoes, `/meaning/projection`, a BM25-with-IDF lexical arm for memories (the substring scorer is the weakest arm).
