# Gap Audit: Original Ferricula vs. Ferricula V2

This document provides a comprehensive read-only parity audit comparing the original **Ferricula** codebase (a recovered copy was kept at `research/original-ferricula/source/`, not included in this repo) against the current **Ferricula V2** runtime (`ferricula_v2`).

---

## 1. Feature Parity & Prioritization Table

| Feature / Component | Status | Priority / Impact | Risky Assumptions |
| :--- | :--- | :--- | :--- |
| **Original Autonomous Loop** | **Missing** | **High** | V2 assumes scheduled wakes do not require LLM reasoning or tool executions, reducing Steve to a passive RSS reader. |
| **Interactive Tool Execution** | **Missing** | **High** | V2 assumes mentions only require disposition deliberation, ignoring tool-use or public actions. |
| **Active Memory Reinforcement** | **Missing** | **High** | Memory is mounted read-only in V2; no `/remember` endpoint or graph connection APIs exist. |
| **Emotion & Model Drift Machine** | **Missing** | **Medium** | Emotional state-machine and budget-based model cooling are replaced by static capability routing. |
| **Background Advocate Loop** | **Missing** | **Medium** | V2 has no value-assessment agent running; Steve's trajectory is unmonitored. |
| **Consolidation / Dream Loop** | **Missing** | **Medium** | Consolidation logic exists in code but is completely disconnected from the runtime task worker. |
| **Wisdom King Suggestions** | **Intentionally Changed** | **Low** | V2 refactors the legacy 5-agent model into a deterministic suggestion council updating `CognitiveControls`. |
| **Pali / Abhidharma Core** | **Preserved** | **Low** | Core cognitive structures like Citta-vithi stages and Pali terminology are preserved. |
| **Memory Engine & Prime Tree** | **Preserved** | **Low** | Loading legacy snapshot v4, WAL replay, and hybrid retrieval search are fully functional. |

---

## 2. Topic-by-Topic Parity Analysis

### A. Original Autonomous Loop
* **Original System**: Implemented in steve.py (upstream v1 `ferricula/arena/steve.py`) as a background thread (`_think_loop` calling `think_cycle` every 45 seconds). It pulled memories, formatted a rich Steve Jobs persona prompt, and let the LLM autonomously decide to query searches, read URLs, draw images, write code/presentations, or update memory.
* **V2 System**: Shifted to a structured task worker and scheduler in [runtime.rs](../../crates/ferricula-server/src/runtime.rs). However, `TaskKind::ScheduledWake` does **not** invoke any LLM reasoning or tool loops. It merely scans the Nuts News RSS feed and returns. LLM calls are limited to a single deliberative pass on direct mentions (`deliberate_mention`).

### B. Emotion and Model Drift
* **Original System**: Tracked a Plutchik-style emotion state machine (joy, fear, sadness, dominance, optimism, etc.). Emotion dictated which LLM model to use (e.g., `sadness` mapped to `claude-fable-5/claude-opus`, `fear` to `gpt-4o-mini` for fast reaction). It also implemented "Gemma drift" (falling back to local Gemma when idle) and "model heat tracking" (cooling down providers after 4 consecutive uses).
* **V2 System**: Completely strips Plutchik emotion tracking and model drift mechanics. Model routing is now statically configured in [model_config.rs](../../crates/ferricula-server/src/model_config.rs) based on `TaskClass` and `ModelCapability` requirements.

### C. Dreams
* **Original System**: The scheduler ran a periodic dream cycle (`dream_cycle` in dream.rs (upstream v1 `ferricula/src/dream.rs`)) that consolidated memories with high similarity, generated prompts for Stable Diffusion/ComfyUI based on emerging term pairs, and saved them as `seeing` memories. It was triggered autonomously or via a `/dream` HTTP endpoint.
* **V2 System**: While the semantic consolidation math is preserved in [dream.rs](../../crates/ferricula-cognition/src/dream.rs), the runtime scheduler never invokes it, and [api.rs](../../crates/ferricula-server/src/api.rs) exposes no `/dream` or `/control/dream` endpoint. ComfyUI integration is absent.

### D. Memory Reinforcement
* **Original System**: Allowed active reinforcement of the memory graph through automatic semantic edge updates during dreams, keystone neighboring "halo touches" (slowing down memory decay), and API endpoints (`/remember`, `/memory/connect`, `/memory/disconnect`, `/memory/keystone`).
* **V2 System**: The memory volume `/data/steve-memory` is mounted **read-only** in compose.yaml. No `/remember` or graph mutation endpoints exist in [api.rs](../../crates/ferricula-server/src/api.rs). Re-writing or reinforcing the memory plane is completely impossible.

### E. Advocate Behavior
* **Original System**: Ran a separate background thread (`_advocate_loop` / `_run_advocate_cycle`) every 180 seconds. It loaded Steve's values and recent actions, used a local model to produce a blunt judgment on whether the current trajectory aligned with Steve's goals, and saved it to memory.
* **V2 System**: Completely missing. No advocate loops or value checks are implemented in V2.

### F. Wisdom King Activation
* **Original System**: Described in legacy docs as 5 autonomous agents (Intuition, Fortune, Craft, Ethics, Advocate) that would activate at high-intensity tiers.
* **V2 System**: Elegantly refactored into a single-mind deterministic suggestion council in [wisdom.rs](../../crates/ferricula-cognition/src/wisdom.rs). Based on context intensity, it generates modulations (whispers) that modify baseline `CognitiveControls` (e.g., novelty tolerance, exploration, publication threshold) for a single deliberative decision.

---

## 3. Top Five Gaps (Prioritized)

### 1. Completely Missing Autonomous Thought Loop (High Impact)
* **Gap**: V2 has no autonomous cycle where Steve actually "thinks" or reasons. The scheduler wakes to scan feed items and does not trigger LLM deliberation.
* **Impact**: Steve is a passive, reactive server rather than an agent.

### 2. Missing Memory Write / Reinforcement Paths (High Impact)
* **Gap**: The lack of a `/remember` endpoint or a read-write database connection means Steve cannot store new memories, reinforce existing associations, or write down thoughts.
* **Impact**: Complete memory stagnation; learning and adaptation are frozen.

### 3. Missing Dream Execution & HTTP Trigger (Medium-High Impact)
* **Gap**: The dream consolidation cycle is unhooked from the active runtime, and no `/dream` API endpoint is available.
* **Impact**: Memories cannot decay, merge, or generate dream imagery candidates, leading to graph bloat and lack of associative cleanup.

### 4. Loss of Plutchik Emotion & Model Drift Mappings (Medium Impact)
* **Gap**: The emotion-based model routing, Gemma drift, and model heat cooling systems are gone.
* **Impact**: Steve loses the emotional volatility that drove provider escalation (e.g. falling back to cheap models or local Gemma during idle cycles and escalating during key focus states).

### 5. Absence of Background Advocate Audits (Medium Impact)
* **Gap**: The background values-alignment loop is completely unimplemented.
* **Impact**: The agent has no self-monitoring feedback loop to verify if its actions serve its core drives.
