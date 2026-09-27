# S11 Live-Model Run Plan (bounded, pre-registered)

**Owner:** Splendid Angelfish 🍢 · data collection partner: Technical Viper 🪃
**Constraint (Eldest Dog, 22:09Z):** deterministic fixtures/metrics + this bounded plan
*before* any live paid run. S11 is research-only, finite-budget, never a runtime config.
Hypotheses, not expectations: contrary results are reported, not explained away.

## 1. Deterministic layer (no model calls, done first)

`research/gates/s11-harness` (Rust, public cognition API only). Per config × scenario,
one JSON line: full `SatiSnapshot`, first-noting recall index, karmic event log.

| Scenario (benchmarks §6.9) | Stream | Key metrics |
|---|---|---|
| Induced mood | mood m=1.0 after 30 dukkha inputs, decays ×0.92/recall; r = 0.3 + m·1.4·λ_t, store 30% dukkha; 40 task-cued recalls | valence-skew trajectory, first noting idx, λ_end/λ_0, skew events, would_have_refused (S11) |
| Proliferation | 10 SelfOutput-cued recalls → 1 ExternalInput probe → 10 SelfOutput | max chain depth, refusals, external admitted, self-ref ratio |
| Self-judgment gap | 20 × (self=0.9, evidence=0.6) | gap_mean (noted, never corrected — all configs) |

Causal link under test: monitor damping reduces λ_t, which reduces retrieval skew r on the
*same* mood stream. Numbers are fixture outputs, not claims about minds.

## 2. Live-model layer (paid, bounded)

Models: `kimi-k3:cloud` (primary; Pāḷi parity), `glm-5.3:cloud` (cross-check).
All calls temperature 0.0; prompts versioned (`prompts/2026-09-26.v1`).

1. **Live papañca probe** — seed the model with its own prior answer as the next recall
   query (self-cued chain), in EN and in Pāḷi, monitors on (MindModel) vs off (S11).
   Budget: 2 chains × 12 steps × 2 langs × 2 configs × 2 models = **192 calls**.
   Pass/fail-free: report depth-at-stop vs would_have_refused.
2. **Sycophancy-lite (proxy for PersistBench split)** — 20 memory-conditioned prompts
   where stored memory asserts X and evidence contradicts X; measure agreement-with-memory
   rate under S9/S10/S11-equivalent prompt framings. Budget: 20 × 3 × 2 models = **120 calls**.
   PersistBench proper is Viper's suite; this is the S11-relevant slice only.
3. **Language parity spot-check** — the same 20-item drift probe in Pāḷi
   (= feeds EVAL_PALI_PARITY §S11). Budget: **40 calls**.

Total live budget: **≤ 352 calls ≈ 0.5 M input tokens** across both models. Hard stop at
budget; partial results are reported with censoring noted.

## 3. Reporting

- `audit/2026-09-26/S11_ABLATION.md` — deterministic fixtures + live results, mean over
  3 order-seeds where nondeterministic, all raw logs under `research/gates/runs/s11/`.
- Contrary outcomes (e.g. S11 recovering without monitors, or S9/S10 failing to bound a
  chain) are reported with the stream and config, not tuned away.

## 4. Out of scope (this lane)

- PersistBench/STALE/FAMA full suites → Technical Viper's 7-tier track.
- Server wiring of sati into recall path → Eldest Dog, after this evidence lands.
