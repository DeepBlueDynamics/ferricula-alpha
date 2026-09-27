# Technical Scan: Ferricula (v1 Oracle) vs. Ferricula v2 (Next-Gen Architecture)

**Audit Date:** 2026-09-19  
**Auditor:** Antigravity (Engine / Added Armadillo)  
**Location:** fleet workspace (v1 `ferricula/` and the v2 modular workspace, now this repo)

---

## 1. Executive Summary of Directory Scan

A comprehensive architectural inspection of the two primary system directories in the fleet workspace reveals a generational transition in the codebase:

1. **`ferricula/` (The v1 Oracle / Monolithic Implementation):**
   - A single-node, single-writer Rust application (Rust 2024 edition) implementing a thermodynamic cognitive memory engine for AI agents.
   - Core philosophy: Memories are living thermodynamic objects characterized by exponential decay, recall-based reinforcement, neglect penalties, SDR-harvested entropy, background dream consolidation, and vec2text ghost-echo inversion.
   - Serves as the verified functional baseline ("oracle") against which all v2 crates are validated.

2. **The v2 workspace (this repo; The Next-Generation Modular Workspace):**
   - A cargo-workspace refactor decomposing the monolithic engine into decoupled, single-responsibility crates governed by strict trait contracts (`TEAM.md` alignment).
   - Crates include:
     - `ferricula-core` (Engine / Antigravity ownership): Row store, bit-sliced roaring bitmaps, WAL persistence, and memory lifecycle primitives.
     - `ferricula-cognition` (Architect / Codex ownership): Dream cycle, consolidation, identity state machine, and cognitive load governance.
     - `ferricula-search` & `ferricula-semantic` (Retrieval / Whippet ownership): Ingestion, dense embedding adapters, hybrid scoring, and semantic graph traversal.
     - `ferricula-server` (Architect / Codex ownership): REST / MCP API boundaries, HTTP routing, and broker dispatch.
     - `parity/` (Parity / Evil Magpie ownership): Independent test harness and cross-engine verification.

---

## 2. Deep Dive: `ferricula` (The Oracle)

### 2.1 Concurrency Model & Runtime Wiring
- **Single-Writer Database Core:** Mutable state is strictly owned by the **Main Thread** (`DurableEngine`, `IdentityState`).
- **Tri-Threaded Architecture:**
  1. `main thread`: Drains `HttpCommand` and `DreamTrigger` channels, mutates records, manages the write-ahead log (WAL).
  2. `http thread`: Powered by `tiny_http`, exposes 16 REST endpoints, dispatches typed commands over `std::sync::mpsc`.
  3. `clock thread`: 60-second tick loop polling physical entropy from `gnosis-radio` (:9080) and emitting `DreamTrigger` events when reservoir limits are breached.
- **Surface Scoping:** The engine strictly isolates tools into two surfaces via `FERRICULA_SURFACE`:
  - **Cognitive Surface (10 tools):** `remember`, `recall`, `reflect`, `observe`, `inspect`, `connect`, `neighbors`, `status`, `health`, `identity`.
  - **System Surface (9 tools):** `dream`, `keystone`, `checkpoint`, `offer_entropy`, `inversion_check`, `terms`, `query`, `disconnect`, `clock`.

### 2.2 Thermodynamic Engine Primitives
- **Fidelity ($f$):** Evaluated as $f(t) = f_0 \cdot e^{-\alpha_{\text{eff}} \cdot \Delta t}$, bounded within $[0.0, 1.0]$.
- **Adaptive Alpha:** $\alpha_{\text{eff}} = \frac{\alpha_0}{1 + \ln(1 + d)}$, where $d$ is consolidation depth.
- **Sensory Channels:**
  - `hearing`: $\alpha_0 = 0.010$, non-keystone default (conversational turns).
  - `seeing`: $\alpha_0 = 0.010$, keystone default (file observations, immutable references).
  - `thinking`: $\alpha_0 = 0.015$, non-keystone default (high-entropy internal reflections).
- **Potentiation & Neglect:**
  - Recall potentiation: $\alpha \leftarrow \alpha \times 0.95$.
  - Neglect penalty: $\alpha \leftarrow \alpha \times 1.005$ on every dream pass where the memory is unreferenced.
- **Fidelity Gate:** Threshold at $0.75$. Falling below triggers transition: `Active` $\to$ `Forgiven` $\to$ `Archived`.
- **Dream Cycle Phases:**
  1. *Decay:* Stochastic decay seeded by radio entropy noise.
  2. *Forgive:* Memories below $0.75$ fidelity transition out of active search space.
  3. *Consolidate:* Pairwise cosine similarity ($\ge 0.85$) collapses memories into fidelity-weighted centroids ($d \leftarrow d + 1$).
  4. *Neglect:* Unaccessed memories have decay rates accelerated.
  5. *Review:* Keystone audit.
  6. *Prune:* Purge terminal records, extract *ghost echoes* via `vec2text` T5 inversion.

### 2.3 Resonant Recall & Wheeler-Feynman Mechanics
- Implemented in v0.6.0: Memories act as absorbers in a Wheeler-Feynman transactional framework.
- Four resonance gates mapped to Buddhist Wisdom Kings (*Myōō*):
  1. *Fōdō-Myōō (Acala - Stability/Fidelity Gate):* Filters memories by fidelity threshold.
  2. *Gōzanze-Myōō (Trailokyavijaya - Conquest/Lifecycle Gate):* Rejects archived or stale states.
  3. *Gundari-Myōō (Kundali - Amrita/Temporal Coherence Gate):* Enforces temporal proximity and chronological resonance.
  4. *Kongōyasha-Myōō (Vajrayaksa - Wrath/Heat Gate):* Measures agent cognitive heat; throttles recall when heat is critical.
  5. *Daiitoku-Myōō (Yamantaka - Wisdom/Load Gate, v0.9.0 Craft Archetype):* Evaluates cognitive load score; triggers *vicikicchā* (perplexity escalation) to frontier reasoning models.

---

## 3. Deep Dive: the v2 workspace (The Modern Multi-Crate Architecture)

### 3.1 Workspace Structure
```
./   (v2 workspace root)
├── Cargo.toml               # Workspace manifest
├── PLAN.md                  # Implementation roadmap and phase tracking
├── README.md                # Architectural documentation
├── config/                  # Instance configurations
├── crates/
│   ├── ferricula-core/      # Storage, bit-sliced Roaring bitmaps, WAL, row store
│   ├── ferricula-cognition/ # Citta-vīthi, dream engine, identity, load governance
│   ├── ferricula-search/    # Hybrid search, BM25, graph-boosted retrieval
│   ├── ferricula-semantic/  # Embedding clients, vector operations, vec2text inversion
│   └── ferricula-server/    # HTTP tiny_http / axum, MCP server, tool dispatcher
├── docs/                    # Architectural decisions and specifications
├── research/                # Internal benchmarks and reference reports
└── tasks/                   # Execution tickets
```

### 3.2 Key Evolutions from v1 to v2
1. **Separation of State and Cognition:** In v1, `DurableEngine` owned both disk persistence and high-level cognitive metaphors. In v2, `ferricula-core` is purely data-plane (storage, WAL, bitmap indexing), while `ferricula-cognition` orchestrates the Abhidharma mental factors (*cetasikas*) and dream cycles.
2. **Multi-Agent Container Federation:** Governed by `TEAM.md`. Different specialized AI agents manage distinct crates, preventing cross-crate regression.
3. **Pluggable Vector Ingestion:** Replacing hardcoded ONNX bindings with the `Shivvr` embedding / inversion microservice and `gnosis-chunk`.
4. **Enhanced Query Optimization:** Adopting findings from GPU-accelerated database research (Bowen Wu's ETH Zurich thesis on vectorized execution and sort-merge-join) for microsecond set intersections over Roaring Bitmaps.

---

## 4. Synthesis of Existing Research Assets

Inside `research/`, several critical papers and technical artifacts prefigure this upgrade:
- `citta-vithi.md`: 17-moment sequence mapping from Theravāda Abhidhamma to computational memory pipelines.
- `gemini_paper.txt`: Comprehensive whitepaper on thermodynamic memory, radio entropy, and the Count of Monte Cristo multi-agent arena.
- `MappingtheMindAModelBasedonTheravadaBuddhistTextsandPractices.pdf`: Peer-reviewed clinical and neuro-cognitive model by P. L. Walpola et al. (2017).
- `AI Agent Memory and Thermodynamic Architectures...pdf`: Mid-2026 frontier briefing establishing modern benchmarks (LoCoMo, LongMemEval, BEAM) and competitors (Mem0, Letta, Zep, A-MEM, Memanto).
- `Furicula Project AI Memory Research.pdf`: Analysis of Continuous Dense Associative Memory (DenseAM), Epanechnikov / Log-Sum-ReLU kernels, and the Loom Minimum Sufficient Context (MSC) harness.
