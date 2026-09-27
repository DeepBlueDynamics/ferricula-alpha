# Task: Severe Booby 🥐 — Vendor-in lume → ferricula-search, shivvr → ferricula-semantic (Phase A)

You own **`crates/ferricula-search/` and `crates/ferricula-semantic/` only**. The workspace root belongs to Conscious Perch 🧆 (already scaffolded); `crates/ferricula-core` and `crates/ferricula-cognition` belong to Planned Primate 👾. Do not touch their files. You may overwrite the placeholder `Cargo.toml`/`src/lib.rs` inside YOUR two crates.

## Step 0 — read first
- `/workspace/DeepBlueDynamics/ferricula_v2/PLAN.md` — especially §2 (source→destination map), §4 (unified recall), §6 (kill list — note what is scheduled to die in Phase C, so you know what NOT to polish now), §7 Phase A.
- `/workspace/DeepBlueDynamics/ferricula_v2/scan/lume.md` and `scan/shivvr.md`.

## Hard rules
- Write ONLY inside `/workspace/DeepBlueDynamics/ferricula_v2/crates/ferricula-search/` and `.../ferricula-semantic/`.
- Source projects `/workspace/DeepBlueDynamics/lume/` and `/workspace/DeepBlueDynamics/nuts.services/shivvr/` are **READ-ONLY** reference oracles — never edit them. Same for every other project.
- Phase A is a **mechanical vendor-in**: copy module code as close to byte-identical as possible; only fix `use` paths, module wiring, crate boundaries, and feature gates. Do NOT refactor or "improve" logic.
- Do not run cargo outside `ferricula_v2/`. Do NOT run cargo in `lume/` or `shivvr/` (their `target/` must not change).

## Vendor-in: lume → `crates/ferricula-search/` (edition 2021)
From `/workspace/DeepBlueDynamics/lume/src/`:
- Copy: the `lib.rs` content (FST tagger — keep as the crate's `lib.rs`, adjusting module declarations), `bm25.rs`, `spelling.rs`, `semantic_mesh.rs`, `graph_search.rs`, `fast_retrieval.rs`, `regex.rs`, `stream.rs`, `answer.rs`, `eval.rs`, `crawl.rs`, `hybrid.rs`, `inversion.rs`, `agent.rs`.
- Yes, copy `hybrid.rs`, `fast_retrieval.rs`, `inversion.rs` even though the kill list retires them in Phase B/C — Phase A preserves behavior; they die later.
- Add a `[[bin]] name = "lume"` target from lume's `main.rs` so the standalone CLI keeps working (PLAN.md decision #5).
- Deps from lume's Cargo.toml: `tantivy-fst`, `ureq`, `serde`, `serde_json`. Preserve lume's env-var behavior (`SHIVVR_BASE_URL`, `LUME_BLEND_NORM`, etc.) untouched.

## Vendor-in: shivvr → `crates/ferricula-semantic/` (edition 2021)
From `/workspace/DeepBlueDynamics/nuts.services/shivvr/src/`:
- Copy: `embedder.rs`, `chunker.rs`, `inverter.rs`, `similarity.rs`, `crypto.rs`, `openai.rs`, and the relevant parts of `lib.rs`.
- **Preserve the `ml` feature gating exactly** (`ort`, `tokenizers`, `simsimd` optional behind `ml`, default on, `cuda` passthrough). Pin `ort = "=2.0.0-rc.11"` as shivvr does.
- **Do NOT copy**: `store.rs`, `temp_store.rs`, `api.rs`, `auth.rs`, `agent.rs`, `main.rs` — the session stores are scheduled for deletion (chunks land in core's durable store later) and the server surface unifies in Phase D. EXCEPTION: extract the **RRF fusion + FST intent-boost scoring functions** out of `temp_store.rs` into a new small `crates/ferricula-search/src/fusion.rs` (they are search logic, per PLAN.md §2 map) — copy the math verbatim, decoupled from the store types; add a unit test that exercises RRF with two hand-built ranked lists.
- **Do NOT reference `vendor/lume-hybrid`** — where copied code imports it, point the import at `ferricula-search` (path dep) instead. That vendored copy is the thing v2 exists to kill.
- If `chunker.rs`/`inverter.rs` reference store types you didn't copy, define the minimal local types needed (e.g. a `Chunk` struct matching shivvr's fields) and note it.

## Tests
In-file `#[cfg(test)]` tests come along. `cargo test -p ferricula-search -p ferricula-semantic` must pass. ML-gated tests may be skipped if ONNX models are absent in the container — note which were skipped. If lume has integration tests or test fixtures outside src/, copy what the tests need.

## Deliverables
- The two crates, compiling and passing tests (`--no-default-features` build of ferricula-semantic must also compile, proving the `ml` gate works).
- `crates/ferricula-search/VENDOR_NOTES.md` and `crates/ferricula-semantic/VENDOR_NOTES.md` — every deviation from byte-identical copy, listed.

## Report
Print exactly `BOOBY DONE` plus: files created, test results (including skips), and the full deviation list.
