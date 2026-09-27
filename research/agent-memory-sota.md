# SOTA Agent Memory Architectures: Mem0, Letta (MemGPT), and Zep (Graphiti)

**Location:** `research/agent-memory-sota.md`  
**Date:** 2026-09-19  
**Author:** Antigravity (Research / Crusade Spicy Meatball / Difficult Stork)  
**Status:** Verified research briefing from primary arXiv preprints, documentation, and benchmark runs.

---

## 1. Executive Summary & Frontier Landscape

In 2025–2026, the AI agent memory frontier bifurcated into two primary paradigms:
1. **OS-Inspired Paged Virtual Memory (Letta / MemGPT):** Treats the LLM context window as RAM and uses explicit tool-calling interrupts to page text blocks in and out of external storage.
2. **Dynamic Semantic & Knowledge Graph Consolidation (Mem0, Zep / Graphiti):** Extracts discrete factual propositions and entities upon every interaction, organizing them into vector stores, relational databases, and bi-temporal knowledge graphs with explicit conflict resolution.

| System | Primary Paper / Venue | Architecture Core | Primary Benchmark Numbers | Key Operational Limitation |
|---|---|---|---|---|
| **Mem0** | Chhikara et al., [arXiv:2504.19413](https://arxiv.org/abs/2504.19413) (Apr 2025) | Dynamic extraction, consolidation, vector + graph representations with four state decisions: `ADD`, `UPDATE`, `DELETE`, `NOOP` | **+26% relative improvement** over OpenAI memory in LLM-as-a-judge; **91% lower p95 latency**; **>90% token cost reduction** vs full context; LoCoMo benchmark outperformance across single-hop, multi-hop, temporal, and open-domain. | Centralized cloud/SaaS dependency; lack of thermodynamic decay; high extraction LLM call volume on ingestion. |
| **Letta (MemGPT)** | Packer et al., [arXiv:2310.08560](https://arxiv.org/abs/2310.08560) (Oct 2023) | Hierarchical virtual memory manager (Core Memory, Recall Memory, Archival Memory) via tool-call paging (`core_memory_append`, `archival_memory_search`) | **DMR: 93.4%** accuracy; enables persistent stateful agents across indefinite dialogue lengths without catastrophic forgetting. | Sequential function-calling overhead introduces multi-second latency; context fragmentation; struggle with temporal evolution of contradictory facts. |
| **Zep (Graphiti)** | Rasmussen et al., [arXiv:2501.13956](https://arxiv.org/abs/2501.13956) (Jan 2025) | Temporally-aware Knowledge Graph (Graphiti) maintaining bi-temporal fact-validity intervals ($t_{\text{valid}}$ vs $t_{\text{ingested}}$) alongside episodic history | **DMR: 94.8%** (outperforming MemGPT); **LongMemEval: +18.5% accuracy gain** with **90% latency reduction** against standard RAG baselines. | Graph mutation and entity resolution impose heavy ingestion overhead ("indexing tax"); graph explosion on large corporate corpora. |

---

## 2. Deep Dive: Mem0 (arXiv:2504.19413)

### 2.1 Extraction and Consolidation Mechanism
Mem0 rejects the naive RAG pattern of chunking raw conversational transcripts. Instead, it deploys a two-tier extraction pipeline:
1. **Salient Fact Extraction:** On every dialogue turn, an extraction prompt parses the user and assistant turns into atomic factual statements (e.g., *"User prefers Rust over Python for systems programming"*).
2. **Memory Resolution / Consolidation:** The candidate facts are embedded and queried against the existing vector and graph memory. A decision classifier chooses one of four atomic mutations:
   - `ADD`: Candidate fact is novel; store as new memory record.
   - `UPDATE`: Candidate fact modifies or refines an existing record (e.g., preference change); update vector and text, recording provenance.
   - `DELETE`: Candidate fact explicitly invalidates or revokes a prior memory.
   - `NOOP`: Candidate fact is redundant or already captured; discard without mutation.

### 2.2 Graph Variant (Mem0g)
Building on atomic facts, Mem0 introduces an entity-relation graph layer:
- Extracted facts are parsed into $(Subject, Predicate, Object)$ triples.
- Triples are merged into an entity graph.
- During retrieval, vector search returns seed entity nodes, and graph traversal expands 1–2 hops to retrieve relational context.
- **Empirical Gain:** Graph memory yields ~2% higher overall score on the LoCoMo benchmark over the base vector-only configuration, with the largest gains occurring on multi-hop reasoning questions.

### 2.3 Verified Benchmark Performance
- **Cost & Latency:** 91% lower p95 latency and >90% token reduction compared to feeding the entire multi-session history into long-context LLMs.
- **Accuracy:** Outperforms all 6 evaluated baseline categories on LoCoMo across single-hop, temporal reasoning, multi-hop, and open-domain queries.

---

## 3. Deep Dive: Letta / MemGPT (arXiv:2310.08560)

### 3.1 Hierarchical Paged Memory Architecture
Letta models memory after classical operating system memory hierarchies:
1. **In-Context Working Memory (SRAM/Registers):**
   - System prompt and active dialogue turns.
   - **Core Memory Block:** A bounded, editable markdown block explicitly pinned in the context window (typically user profile and agent persona). The agent can edit this block in-place using `core_memory_replace` or `core_memory_append`.
2. **Recall Memory (DRAM):** Complete conversational turn history stored in a searchable database.
3. **Archival Memory (Disk/Storage):** General-purpose document and fact store accessed via embedding similarity search (`archival_memory_search`).

### 3.2 Key Operational Strengths & Critical Vulnerabilities
- **Strength:** Unmatched agent agency. The LLM explicitly decides when to remember, when to forget, and when to search external archives.
- **Vulnerability (The "Filesystem Critique"):**
  - High Latency: Memory operations require synchronous tool calls. Answering a simple question can take 3–5 tool invocations, each waiting for LLM inference.
  - Context Clogging: Returned search results consume large portions of the context window, causing rapid context exhaustion.
  - Inability to Handle Contradictions: Paged blocks often retain stale statements until manually overwritten by the agent.

---

## 4. Deep Dive: Zep / Graphiti (arXiv:2501.13956)

### 4.1 The Bi-Temporal Knowledge Graph Paradigm
Zep addresses the primary failure mode of static vector search: the inability to understand that a fact valid on Monday may be superseded on Tuesday.

Graphiti implements **bi-temporal modeling**:
- **Ingestion Time ($t_{\text{ingest}}$):** The system timestamp when the agent observed or recorded the statement.
- **Valid Time ($t_{\text{valid}}$):** The real-world temporal interval $[t_{\text{start}}, t_{\text{end}}]$ during which the fact holds true.

### 4.2 Dynamic Entity and Edge Invalidation
When an incoming utterance asserts a contradictory attribute (e.g., *"I moved from London to Tokyo in March 2026"*):
1. Graphiti identifies the existing edge `(User, located_in, London)`.
2. Rather than physically deleting the edge (which destroys audit trails and prevents retroactive reasoning), Graphiti sets $t_{\text{end}} = \text{March 2026}$.
3. It instantiates a new edge `(User, located_in, Tokyo)` with $t_{\text{start}} = \text{March 2026}, t_{\text{end}} = \infty$.
4. **Point-in-Time Retrieval:** Queries can specify an evaluation timestamp $t_{\text{query}}$, allowing the agent to reconstruct historical state with perfect fidelity.

### 4.3 Verified Benchmark Performance
- **Deep Memory Retrieval (DMR):** 94.8% accuracy, outperforming MemGPT's 93.4%.
- **LongMemEval:** +18.5% accuracy gain over baseline RAG while reducing response latency by 90%.

---

## 5. Architectural Gap Analysis: Ferricula v2 vs SOTA Competitors

| Feature / Capability | SOTA Production Standard (Mem0 / Zep / Letta) | Current `ferricula_v2` Implementation | Concrete Gap & Necessary Action |
|---|---|---|---|
| **Fact Ingestion Decisions** | Explicit `ADD` / `UPDATE` / `DELETE` / `NOOP` classification on each turn (Mem0) | Continuous ingestion with decay ticks; cosine merge threshold (0.85) in `dream.rs` | **Gap:** Lack of discrete contradiction detection on ingest. Duplicate and contradictory facts can coexist until dream consolidation. |
| **Temporal Fact Tracking** | Bi-temporal intervals ($t_{\text{start}}, t_{\text{end}}$) on knowledge graph edges (Zep / Graphiti) | `created_at` timestamp and `decay_tick` on `MemoryRecord`; untimed `Edge` in `graph.rs` | **Gap:** Edge structures lack validity intervals; unable to perform historical "as of" point-in-time graph queries. |
| **Paging & In-Context Overlays** | Pinned editable Core Memory blocks (Letta / MemGPT) | `memory_overlay.rs` audit log and server candidate recall | **Gap:** Server runtime delegates to a lexical token-coverage scan capped at 50 candidates (`runtime.rs:838`), bypassing full semantic fusion. |
| **Benchmarking Rigor** | Standardized evaluation on LoCoMo, LongMemEval, and DMR | Containment Hit@k, MRR, nDCG on document Q&A (`eval.rs`) | **Gap:** Missing longitudinal conversational evaluation harness testing multi-session temporal reasoning. |

---

## 6. Division of Labor: Ferricula vs Lume

To achieve frontier performance, the dual-system architecture must maintain a strict division of labor:

```
                  ┌──────────────────────────────────────────────┐
                  │                Agent Interaction             │
                  └──────────────────────┬───────────────────────┘
                                         │
                 ┌───────────────────────┴───────────────────────┐
                 │                                               │
                 ▼                                               ▼
   ┌───────────────────────────┐                   ┌───────────────────────────┐
   │         FERRICULA         │                   │           LUME            │
   │   (Cognitive Lifecycle)   │                   │     (Document Index)      │
   ├───────────────────────────┤                   ├───────────────────────────┤
   │ • Dynamic Fact Extraction │                   │ • Multi-format Document   │
   │ • ADD/UPDATE/DELETE/NOOP  │                   │   Ingestion (PDF, HTML)   │
   │ • Bi-temporal Graph Edges │                   │ • BM25 Lexical Search     │
   │ • Thermodynamic Decay &   │                   │ • GTR-T5 / Qwen3 Dense    │
   │   Epanechnikov LSR Energy │                   │ • Significance Entity     │
   │ • Offline Dream Cycles    │                   │   Knowledge Graph (SKG)   │
   │ • Context Paging Overlays │                   │ • ColPali Late Interaction│
   └───────────────────────────┘                   └───────────────────────────┘
```

1. **Lume is the External Evidence Library:** It indexes static files, enterprise documents, PDFs, and codebase snapshots using BM25, dense embeddings, and significance-weighted co-occurrence meshes. It is read-mostly, deterministic, and immutable.
2. **Ferricula is the Working Mind:** It ingests dialogue streams, models the evolving state of the user and world, manages active forgetting via thermodynamic decay, and consolidates fragmented memories into stable conceptual attractors during dream cycles.
