# Spec: Emotion & Model Drift System

This specification analyzes the Plutchik-style emotion system, model routing, drift mechanics, and advocate interactions in the original Ferricula codebase, and outlines provider-neutral requirements for integration into Ferricula V2.

---

## 1. Original Mechanics & Source References

All references are mapped from the original source file [steve.py](file:///workspace/ferricula_v2/research/original-ferricula/source/arena/steve.py).

### A. Plutchik Emotional State Space
* **Base Emotions**: Eight primary emotions are defined in the array `EMOTIONS` at **Line 185**: `joy`, `trust`, `fear`, `surprise`, `sadness`, `boredom`, `anger`, and `interest`.
* **Dyads (Blends)**: At **Lines 187–200**, the `BLENDS` dictionary maps unique sets of two base emotions to unified dyadic emotional states:
  * `joy` + `trust` $\rightarrow$ `love`
  * `joy` + `interest` $\rightarrow$ `optimism`
  * `joy` + `surprise` $\rightarrow$ `delight`
  * `trust` + `fear` $\rightarrow$ `submission`
  * `trust` + `anger` $\rightarrow$ `dominance`
  * `fear` + `surprise` $\rightarrow$ `awe`
  * `sadness` + `interest` $\rightarrow$ `melancholy`
  * `sadness` + `surprise` $\rightarrow$ `disapproval`
  * `boredom` + `sadness` $\rightarrow$ `withdrawal`
  * `anger` + `interest` $\rightarrow$ `agitation`
  * `anger` + `surprise` $\rightarrow$ `outrage`
  * `interest` + `surprise` $\rightarrow$ `alertness`

### B. Emotional Transitions & Somatic Mapping
* **Transitions**: In `shift_emotion()` (**Lines 341–354**), emotional states are updated at each cycle:
  * **25% chance**: Two base emotions are selected at random. If a blend exists in `BLENDS` for that pair, it becomes the active state; otherwise, the second emotion is selected.
  * **75% chance**: A single base emotion is selected at random, clearing any blend.
* **Somatic Mapping**: The active emotion is mapped to somatic variables (`tension`, `activation`, `groundedness`) at **Lines 356–382** (`_SOMATIC_TENSION`, `_SOMATIC_ACTIVATION`, `_SOMATIC_GROUND`).
* **Somatic Smoothing**: In `_update_somatic()` (**Lines 384–390**), somatic variables are updated using an exponential lag coefficient $\alpha = 0.35$:
  $$\text{state}_{\text{new}} = \text{state}_{\text{old}} \times 0.65 + \text{target} \times 0.35$$
* **Somatic Prose Synthesis**: In `_somatic_description()` (**Lines 391–407**), somatic values are converted into a descriptive string (e.g. *"tight in the chest, something braced, still, low current"*) and injected into the prompt.

### C. Model Selection, Drift, and Escalation
* **Emotion Mappings**: At **Lines 217–240** (`EMOTION_MODEL`), each emotional state is mapped to a preferred model tier (e.g., `optimism` $\rightarrow$ `claude_opus`, `fear` $\rightarrow$ `openai_fast`).
* **SDR-Entropy Jitter**: In `model_for_emotion()` (**Lines 281–288**), there is a **2%** chance (gated by true random entropy from a hardware radio bridge via `radio_rand()`) that a random alternative model is selected instead of the mapped default.
* **Gemma Idle Drift**: In `_drifted_model()` (**Lines 4184–4193**), if the agent remains idle (seconds since last user chat message $\ge 90$s):
  * **$\ge 4$ idle cycles**: 60% chance to force model selection to `gemma`.
  * **$\ge 8$ idle cycles**: 75% chance to force model selection to local `gemma`.
* **Budget Pressure & Nudges**: In `_budget_pressure()` (**Lines 155–159**) and `_budget_nudge()` (**Lines 160–180**), a rolling 1-hour expenditure ledger (`_hourly_log` at **Line 317**) evaluates cost limits against the hourly budget:
  * **$\ge 50\%$ spend**: expensive "frontier" models (`claude_opus`) are blocked.
  * **$\ge 75\%$ spend**: only cheap cloud models are permitted.
  * **$\ge 90\%$ spend**: forced fallback to local, free `gemma`.
* **Forced Cooling / Heat Limits**: In `_check_model_heat()` (**Lines 322–336**), if the same model is selected $\ge 4$ consecutive cycles (`_MODEL_OVERHEAT = 4`), it is marked as overheated and a random alternative cloud provider is forced to run instead.

### D. Advocate Interactions
* **Advocate Evaluation**: The background `_advocate_loop()` (**Lines 4065–4099**) triggers `_run_advocate_cycle()` (**Lines 4102–4164**) every 180 seconds.
* **Emotional Context**: It reads the active emotion via `get_emotion()` and injects it along with I Ching hexagrams and values into a local LLM prompt. The local model produces a verdict on trajectory alignment, which is stored back into the memory database via `tool_remember()`.

---

## 2. Redesign Evaluation: Preserve vs. Retire

### Worth Preserving
1. **Somatic Smoothing Loop**: The $\alpha = 0.35$ somatic dampening loop creates smooth, realistic physiological transitions that prevent the prompt from shifting abruptly.
2. **Budget-Ledger Escalation**: Restricting cloud usage dynamically based on real-time rolling transaction costs protects the host from api spending spikes.
3. **Anti-Repetitive Cooling**: Tracking model usage and forcing a switch after 4 consecutive rounds prevents prompt-reinforcement feedback loops.

### Needs Redesign
1. **Hardcoded Model Identifiers**: Mappings like `claude_opus` and `openai_fast` are hardcoded. These should be decoupled from specific model names and instead mapped to abstract capabilities (`Frontier`, `Standard`, `Lite`, `Local`).
2. **Deterministic Random Transitions**: Emotion shifts are purely random (25% change). Transitions should be driven by causal triggers (e.g., getting a response increases `joy` or `anger`; long idle states increase `boredom`).
3. **Memory Ingestion side-effects**: The advocate loop writes directly to the memory database, which fails under read-only mounting and clutters the core semantic index with short-term self-reflections.

---

## 3. V2 Provider-Neutral Requirements

To bring this system into the modern, Rust-based Ferricula V2 architecture, the following requirements must be implemented:

1. **Abstract Capability Mapping**:
   * Map emotions to model classes (`Frontier`, `Standard`, `Lite`, `Local`) defined in the declarative `ModelRoutingConfig`.
2. **Causal Emotional transitions**:
   * Implement emotional decay (valence/arousal decaying toward baseline over time) and emotional changes responding to tasks (e.g. `direct_mention` inputs or successful task completions).
3. **Somatic State Struct**:
   * Implement a Rust-native somatic calculation struct within the `ferricula-cognition` crate that computes `tension`, `activation`, and `groundedness` using somatic transition math.
4. **Non-Blocking Spend Ledger**:
   * Implement a token-ledger tracking system inside the Axum HTTP routing server that tracks actual cost per task and scales back model routes dynamically.
