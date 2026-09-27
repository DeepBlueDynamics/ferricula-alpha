# Spec: Internal Advocate Advisory Loop

This specification analyzes the background value-assessment loop (the Advocate) from the original Ferricula codebase, separates its advisory properties from its implementation side-effects, and defines requirements for a non-commanding self-review cycle in Ferricula V2.

---

## 1. Original Mechanics & Source References

All references are mapped from the original source file [steve.py](file:///workspace/ferricula_v2/research/original-ferricula/source/arena/steve.py).

### A. Cadence & Triggering
* **Execution Interval**: The background loop is managed by `_advocate_loop()` (**Lines 4065–4099**), which sleeps for `ADVOCATE_INTERVAL = 180` seconds (**Lines 85, 4079**) before running.
* **Think-Loop Coupling**: The advocate loop is coupled to the main reasoning cycle: it remains dormant and skips evaluations if `_thinking` is False (**Line 4084**).
* **Budget-Pressure Exits**: The cycle is skipped if hourly cost limits are reached (`_budget_pressure() >= 0.90`) unless local gemma mode is active (**Lines 4088–4092**).

### B. Inputs Gathered
In `_run_advocate_cycle()` (**Lines 4102–4130**), the loop retrieves a specific context package:
* **Stated Core Values**: Queries the Ferricula client via `hybrid` search (**Line 4106**) for `"what Steve wants values goals drives"` ($k=8$).
* **Recent Activities**: Queries the Ferricula client (**Line 4112**) for `"recent actions decisions thoughts"` ($k=8$).
* **Conversational History**: Gathers the last 6 entries of the in-memory `_chat_log` (**Line 4117**).
* **Current Mood & Horoscope**: Reads the active `emotion` (**Line 4120**) and the identity's `hexagram` (**Line 4122**).

### C. Prompting & Output Verification
* **Sovereign System Role**: The prompt (**Lines 4131–4139**) enforces an internal monitoring role:
  > *"You are Steve's internal advocate — not his assistant. Your job is to hold his actual values and ask whether what's happening serves them."*
* **Verification Checks**: The prompt demands brief, non-hedged outputs formatted into exactly two lines (**Lines 4136–4138**):
  * `WANTS`: Steve's primary current drive.
  * `VERDICT`: Assessment of whether the trajectory aligns with his values (and why/why not).

### D. Memory Writeback & Wisdom King Links
* **Overlay Writes**: The advocate writes the verdict back to the memory engine via `tool_remember()` (**Line 4161**) under the `thinking` channel.
* **Budget Gating**: To avoid creating downstream cost pressure, writebacks are blocked if `_budget_pressure() >= 0.75` (**Line 4160**).
* **Wisdom King Archetype**: The advocate loop is a separate thread, but its cognitive weight aligns with the `WisdomKing::Advocate` archetype in [wisdom.rs](file:///workspace/ferricula_v2/crates/ferricula-cognition/src/wisdom.rs#L21), which deters impulsive publication by increasing `AudienceSimulation` (**Line 15** of [wisdom.rs](file:///workspace/ferricula_v2/crates/ferricula-cognition/src/wisdom.rs)).

### E. Action Authority (None)
* **Command Capability**: The advocate cycle has **zero** action capabilities. It is not connected to a tool executor, cannot execute code, and cannot send messages to public channels. It functions strictly as a self-reflective warning system that updates internal memory.

---

## 2. Redesign Evaluation: Preserve vs. Retire

### Worth Preserving
1. **Separation of Concerns**: Decoupling value audits from tool execution is a solid safety design. The advocate observes and warns but never commands.
2. **Goal-Trajectory Contrast**: Querying long-term goals vs. short-term thoughts exposes cognitive deviations.
3. **Budget-Bounded Ingestion**: Preventing writing reviews during budget spikes keeps the database footprint lean.

### Needs Redesign
1. **Raw Text Index Pollution**: Ingesting unstructured advocate thoughts directly into the primary memory graph clutters the search space. Reflections should reside in a separate operational database.
2. **Explicit Thread Loop**: Background OS thread loops should be replaced by structured event-driven tasks.
3. **Moral/Policy Scoring**: Simple prompt-based verdicts should be structured into concrete metrics for operator auditability.

---

## 3. V2 Provider-Neutral Requirements

To implement the advocate's value check in V2 without modifying the read-only volume, the following architecture is required:

1. **Advisory Task Definition**:
   * Implement `TaskKind::AdvisoryReview` in the task-worker queue. The scheduler inserts this task on an interval.
2. **Decoupled Model Execution**:
   * Execute the review using standard `TaskClass::Deliberate` routing parameters, sending the request to the configured local/cheap profile.
3. **Structured Review Store**:
   * Save WANTS and VERDICT lines into a structured runtime JSON file `/data/steve-runtime/advisory-history.json` on the read-write overlay volume instead of writing to the read-only memory base.
4. **Wisdom Council Integration**:
   * Read the `AudienceSimulation` delta from `wisdom.rs` to determine the depth of values queries.
