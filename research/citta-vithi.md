# Citta-Vīthi Architecture: The 17-Moment Cognitive Process

This document maps the Theravada Abhidhamma's 17-moment *citta-vīthi* (cognitive process sequence for a "very great object") to Ferricula's computational architecture, specifically focusing on chunking, retrieval cascades, and affective metadata (*vedanā*).

This mapping provides a mechanistic homology rather than a metaphorical one, establishing the structural basis for Ferricula's memory lifecycle.

## The 17-Moment Sequence

| Moment | Pāḷi Term | Function / Computational Homology |
| :--- | :--- | :--- |
| 1 | **Atīta-bhavaṅga** | **Past Background:** The context window is "quiet." Stagnant prior state. |
| 2 | **Bhavaṅga-calana** | **Vibration:** First "perturbation" of the vector field. New signal detected but not yet parsed. |
| 3 | **Bhavaṅga-upaccheda** | **Arrest:** Background stream stops. Context window is cleared for incoming input. |
| 4 | **Pañcadvāra-āvajjana** | **Five-door Adverting:** Attention shifts to the sensor/channel (e.g., File, Shell, Web). |
| 5 | **Pañca-viññāṇa** | **Sense Consciousness:** The raw "bite" of data. Embedding starts here (*Dassana* = Seeing). |
| 6 | **Sampaṭicchana** | **Receiving:** The vector is generated and held in temporary buffer. |
| 7 | **Santīraṇa** | **Investigating:** Comparison against the existing manifold. Cosine similarity pass. |
| 8 | **Voṭṭhapana** | **Determining:** Decision to commit/reject. Determining which "labels" or "tags" apply. |
| 9–15 | **Javana (7 moments)** | **Impulsion (Processing):** The active transformation. Weight updates, spreading activation, associative logic. |
| 16 | **Tadālambana** | **Registration 1:** Committing the first tail of the chunk to long-term storage. |
| 17 | **Tadālambana** | **Registration 2:** Final commit. Moment of "closing the transaction." |

## Architectural Implications for Ferricula

### 1. The Overlap Ratio (Javana / Bhavaṅga Continuity)
The 7 *javana* moments in a 17-moment cycle dictate an overlap ratio of ~41% (7/17). This suggests that for high-fidelity continuity in the chunking pipeline, the "stride" should ensure approximately 40% of the prior context is maintained in the *bhavaṅga* (background) of the next chunk. This prevents the *nimitta* (sign/target) from degrading when a chunk starts mid-context.

### 2. The Santīraṇa Gate (Quality/Redundancy Check)
The 7th moment (*Santīraṇa*) represents the investigation phase. In Ferricula, this is where a "quality gate" should sit. If an incoming *nimitta* is too similar to an existing memory (via cosine similarity), the *vīthi* should short-circuit here to prevent duplicate entropy, rather than proceeding to full consolidation.

### 3. Two-Stage Registration (Tadālambana)
The fact that Registration requires *two* moments (16 and 17) suggests that the "write" process should be a two-stage commit:
*   **Stage 1:** Persist the node itself (the embedding, raw text, and base metadata).
*   **Stage 2:** Persist the structural edges (`prev_chunk`, `next_chunk`, semantic links).

### 4. Affective Metadata (Vedanā) and Consolidation
Currently, Ferricula treats system errors and novel discoveries with similar consolidation priority if they share vector space. The Abhidhamma indicates that *vedanā* (feeling-tone: pleasant, unpleasant, neutral) acts as a multiplier for consolidation during the *Javana* phase. High-valence states (strong emotional or goal-relevant arousal) should "hardcode" memories, altering their `decay_alpha` or `importance` weights before the *Tadālambana* registration.

### 5. Spreading Activation Limits
When a memory is recalled, activating its semantic and causal edges runs the risk of a "recall cascade" (lighting up the entire manifold). The *citta-vīthi* model is strictly bounded (17 moments). Spreading activation in Ferricula's `graph.rs` must implement a similar strict decay function or traversal depth limit (e.g., a "heat" ceiling) to prevent infinite loops during retrieval.