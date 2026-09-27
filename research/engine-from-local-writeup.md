# Ferricula Engine Analysis: Local Writeup Synthesis

**Source Document**: [`ferricula/research/gemini_paper.txt`](gemini_paper.txt)  
**Context**: Research run synthesis for engine architecture, decay mathematics, persistence/WAL mechanics, dream metabolic cycles, and multi-agent Arena empirical results.  
**Scope**: Core storage and runtime physics only. (Archetype state machines and Wisdom King designs are intentionally excluded per research directives).

---

## 1. Thermodynamic Decay Mathematics

Ferricula models memory retention not as a static table or immutable vector index, but as a biological-analog metabolic process governed by continuous decay and reinforcement.

### Core Decay Equations

1. **Fidelity State**:
   Every memory possesses a scalar fidelity metric:
   $$f \in [0.0, 1.0]$$
   where $f = 1.0$ represents pristine initial capture and $f \to 0.0$ triggers terminal eviction/inversion.

2. **Exponential Decay Update**:
   For any non-keystone memory, fidelity decays over time according to:
   $$f \leftarrow f \cdot e^{-\alpha_{\mathrm{eff}}}$$
   where $\alpha_{\mathrm{eff}}$ is the effective decay rate.

3. **Consolidation Depth Mitigation (Long-Term Potentiation)**:
   The effective decay rate decreases logarithmically with the memory's consolidation depth $d$ (the count of dream cycles in which this memory was merged with semantic relatives):
   $$\alpha_{\mathrm{eff}} = \frac{\alpha}{1 + \ln(1 + d)}$$
   *Implication*: Frequently reinforced memories become progressively more resilient to decay, approaching semi-permanence without freezing into static entries.

4. **Sensory Channel Parameters**:
   Initial decay rates ($\alpha_0$) and default keystone protections vary by modality:

| Sensory Channel | Initial Decay ($\alpha_0$) | Keystone Default | Operational Context |
|---|---|---|---|
| `hearing` | $0.010$ | No | Standard external conversational inputs and interaction turns. |
| `seeing` | $0.010$ | Yes | File observations, code snippets, and long-term reference documents. |
| `thinking` | $0.015$ | No | Ephemeral internal working memory and agent self-reflection. |

*Keystones*: Memories designated as keystones are completely immune to natural time decay ($\alpha = 0$), reserved for core invariants, foundational identity anchors, and critical reference docs.

---

## 2. Systems Architecture, WAL, and Persistence

Ferricula's storage engine (`DurableEngine`) is implemented in pure Rust without external database dependencies, optimized for deterministic single-node sovereignty and sub-20ms writes.

### Tri-Threaded Model

Coordination is partitioned across three dedicated OS threads communicating over typed `mpsc` channels to prevent data races and eliminate lock contention:

1. **Main Thread**:
   * Exclusively owns `DurableEngine`, holding mutable state for all rows, the knowledge graph (`MemoryGraph`), prime-partitioned term hierarchy (`PrimeTree`), semantic knowledge graph (`SkgState`), and persistence mechanisms.
   * Synchronously drains `HttpCommand` requests and `DreamTrigger` events.
2. **HTTP Thread**:
   * Serves REST API endpoints (16 endpoints in v0.4.0) via `tiny_http` / axum.
   * Serializes incoming HTTP calls into strongly typed command envelopes dispatched to the Main Thread.
3. **Clock Thread**:
   * Maintains wall-clock time synchronization.
   * Samples physical entropy from an external software-defined radio (`gnosis-radio`).
   * Accumulates entropy into a reservoir and emits `DreamTrigger` pulses when thresholds are satisfied.

### Write-Ahead Log (WAL) & Postcard Snapshots

* **Streaming WAL**:
  * Every state mutation (`Remember`, `UpdateRecord`, `RemoveMemory`, `Connect`, `InsertTerm`) is written to `wal.log` using length-prefixed `postcard` binary serialization.
  * Append-only writes guarantee fast durablity before acknowledging operations to the client (sub-20ms write latency).
  * System recovery streams and replays the WAL from the last valid snapshot.
* **Atomic Checkpointing**:
  * Periodically (e.g., at 64 MB WAL boundary or explicit trigger), the engine writes an atomic checkpoint snapshot (`snapshot_v4.bin.tmp` renamed to `snapshot_v4.bin` / v5 envelope `[b"FERR", version, 3 reserved bytes, payload]`).
  * Snapshots serialize all rows, roaring bitmap indices, thermodynamic envelopes, graph edges, and prime tree partitions into a single compact postcard payload.

---

## 3. The Dream Cycle: Metabolic Maintenance

The dream cycle is the autonomous maintenance routine driven by environmental entropy that maintains cognitive bounded capacity.

### Entropy Harvesting & Stochastic Gating

* **Physical Entropy Source**: Polled from `gnosis-radio` (monitoring FM receiver noise or marine VHF channels) into an entropy reservoir.
* **Trigger Threshold**: When the reservoir accumulates $\ge 16$ bytes, a `DreamTrigger` event is emitted with intensity:
  $$I \in [0.0, 1.0]$$
* **Stochastic Decay Filter (Phase 1)**:
  Instead of applying deterministic decay across all records uniformly, individual bits of the harvested entropy seed probabilistically filter target memories:
  $$\text{include}(i) = \text{true} \iff \texttt{seed}[i \bmod n] < \lfloor 255 \cdot I \rfloor$$
  *Implication*: Forgetting is fundamentally non-deterministic and bound to physical environmental chaos rather than clock regularity.

### Semantic Consolidation (Phase 3)

* **Clustering**: The engine computes pairwise cosine similarity across all active memory vectors.
* **Merge Threshold**: Clusters with similarity $\ge 0.85$ are collapsed into a single consolidated record.
* **Weighted Centroid**: The surviving record is formed as the weighted centroid of ancestor vectors. It inherits merged graph adjacency edges and provenance records (`Provenance::Consolidated { from: Vec<u32> }`).
* **Depth Increment**: Consolidation depth $d$ increments ($d \leftarrow d + 1$), hardening the survivor against subsequent decay.

### Ghost Echo Inversion (Phase 4)

When a memory's fidelity drops below threshold and reaches terminal decay, the engine executes vec2text inversion before deleting the vector:
1. **Hypothesis Generation**: `gnosis-chunk` (T5-base pipeline) produces an initial text hypothesis directly from the dying vector.
2. **Iterative Correction**: The system re-embeds the text hypothesis, measures the residual error vector against the original, and adjusts the decoded text.
3. **Fidelity Gate**: If the round-trip cosine similarity between original vector and reconstructed text exceeds $0.5$, the text is accepted.
4. **Graph Attachment**: The recovered text is attached as a low-weight ($0.1$) self-edge ("ghost echo") to the dying node's former graph neighbors.
5. **Vector Purge**: The 768-d float vector is deleted, freeing memory while preserving historic semantic gist in graph topology.

---

## 4. Empirical Validation: The Monte Cristo Arena

To validate multi-agent memory dynamics under continuous narrative load, an 8-instance simulation was executed using Alexandre Dumas's *The Count of Monte Cristo*.

### Simulation Configuration

* **Corpus**: 2.79 MB full novel text spanning 117 chapters.
* **Ingestion**: Chapters sequentially chunked and embedded via `gnosis-chunk`; identical memory payloads ingested across 8 independent Ferricula instances simultaneously.
* **Agent Nodes**:
  * Edmond Dantès (Port 8765): Focus on justice and transformation; persistent prison keystones.
  * Mercédès (Port 8766): Focus on relationship and emotional consolidation; political decay.
  * Danglars (Port 8768): Financial/tactical focus; frequent consolidation.
  * Villefort (Port 8769): Conflict between public law and private guilt.
* **Controllers**: Anthropic Claude Haiku calling `recall()` and `neighbors()` over the REST/MCP interface.

### Narrative Divergence Findings

Despite starting from identical text inputs, character memory landscapes diverged completely within **5 chapters**:
1. **Recall Bias**: Frequent recall of specific events (e.g., Dantès querying betrayal details) lowered their effective decay rate ($\alpha_{\mathrm{eff}}$), potently preserving them.
2. **Stochastic Dream Variance**: Unique local entropy seeds caused characters to prune disparate noise chunks.
3. **Consolidation Variance**: Graph weighting differences led agents to merge distinct semantic clusters.
*Final Divergence Metric*: At simulation conclusion, Dantès retained **69 active memories out of 90 total**, maintaining "prison" and "betrayal" keystones at near-1.0 fidelity, whereas peripheral characters decayed identical chapter events to negligible fidelity or ghost echoes.

### Performance & Latency Benchmarks

Measured under active operational load in the Arena:

| Operation | Scale (Dataset Size) | Average Latency | Throughput | Notes |
|---|---|---|---|---|
| **Remember** (Ingest) | N/A | **18.7 ms** | 53 records/sec | Dominated by synchronous WAL append. |
| **Recall** | ~10,000 records | **57.4 ms** | 17 queries/sec | Pre-filtered RoaringBitmap + brute-force cosine. |
| **Dream Cycle** | ~1,000 records | **10.8 ms** | 92 cycles/sec | In-memory clustering, graph rewiring, decay. |
| **Large-Volume Search** | ~100,000 records (163 MB *Encyclopaedia of Religion & Ethics*) | **< 1.0 s** | — | Bitmap tag-filtering + brute-force scan; outperforms HNSW indexing overhead at agent scale. |

*Bottleneck Observation*: Internal thermodynamic engine operations run in 10–50 ms; external model embedding calls remain the primary wall-clock bottleneck (~100× slower than engine operations).
