# Task: Conscious Perch 🧆 — Workspace scaffold + license

You own the **workspace root and crates/ferricula-server only**. Two other agents will replace the contents of the other crates after you:
- Planned Primate 👾 → `crates/ferricula-core`, `crates/ferricula-cognition`
- Severe Booby 🥐 → `crates/ferricula-search`, `crates/ferricula-semantic`

## Step 0 — read first
- `/workspace/DeepBlueDynamics/ferricula_v2/PLAN.md` in full — especially §2 (workspace layout), §5a (internal Abhidharma agent), decisions #8 and #9.
- `/workspace/DeepBlueDynamics/ferricula/LICENSE.md` (the Gnosis AI-Sovereign license).

## Hard rules
- Write ONLY inside `/workspace/DeepBlueDynamics/ferricula_v2/`.
- Everything else is READ-ONLY. Do not modify `lume/`, `ferricula/`, `nuts.services/`, or any other project in the workspace.
- Do not run cargo anywhere except inside `ferricula_v2/` (its `target/` is fine to create).

## Deliverables
1. **`ferricula_v2/LICENSE.md`** — the Gnosis AI-Sovereign license adapted from `ferricula/LICENSE.md`: project name **ferricula_v2**, copyright **2026 Kord Campbell / DeepBlue Dynamics**. Keep the license terms verbatim; only adapt the identifying header.
2. **`ferricula_v2/THIRD_PARTY_NOTICES.md`** — standard BSD-3-Clause attribution block for code derived from the **lume** project (copyright Kord Campbell), stating that `crates/ferricula-search` contains lume-derived code. Note that ferricula- and shivvr-derived code is same-author Gnosis AI-Sovereign.
3. **`ferricula_v2/Cargo.toml`** — `[workspace]`, `resolver = "2"`, members = the five crates under `crates/`; `[workspace.package]` with `version = "2.0.0-alpha.0"`, `license-file = "LICENSE.md"`.
4. **All five `crates/<name>/` dirs** with minimal placeholder `Cargo.toml` + `src/lib.rs` (ferricula-server gets `src/main.rs` that prints name + version). Editions: `ferricula-core`, `ferricula-cognition`, `ferricula-server` = **2024** (ferricula heritage); `ferricula-search`, `ferricula-semantic` = **2021** (lume/shivvr heritage). Placeholder dependency lists stay **empty** — the crate owners bring their own.
5. Update **`ferricula_v2/README.md`** with a License section reflecting the above.

## Verify
`cargo build --workspace` inside `ferricula_v2`. If cargo is not available in your container, say so explicitly in your report — do not silently skip.

## Report
Print exactly `SCAFFOLD DONE` plus the list of files created and the build result.
