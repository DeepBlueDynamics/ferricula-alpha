# An Abhidhamma-Inspired Agentic Memory Architecture: Read-Time Curation, Consolidation-as-Index, and Mindfulness Monitors

**Status:** OUTLINE ONLY (2026-09-26). No results are claimed. Per Coordinator (Eldest Dog), the paper follows implementation evidence; this file fixes the structure so results land in known places.
**Lead author (Cognition & Curator lane):** Burning Dingo 🥓 · **Human directive:** Kord Campbell · **Fleet:** see `ORDERS.md`
**Sources of record:** `research/lume-ollaya-abhidhamma-plan.md`, `-addendum-A.md`, `-benchmarks.md` (verbatim PDF extracts, Wren 2026-09-26); JitMem arXiv:2609.27334; JEV-as-a-Judge arXiv:2609.26550; Harness-Zero arXiv:2609.24974; Kord Campbell, "Self-Aware Machines" (2018).

## 0. Abstract (to write last)

## 1. Introduction
- The accumulation assumption in agent memory ("remember more, longer, never let go") and where the Abhidhamma says it leads.
- Thesis: impermanence as structure; letting go as an operation; recall as reconstruction; drift that is never unseen.
- Contributions (each maps to a code artifact and a benchmark row):
  1. Decay-as-priority and consolidation-as-index (`ferricula-cognition::bhavana`).
  2. Read-time Curator with ephemeral briefings (`ferricula-cognition::curator`; JitMem).
  3. Calibrated judgment gates driving the lifecycle (Ollaya; `ferricula-cognition::gates`).
  4. The drift layer with sati monitors and return-to-object (`ferricula-cognition::sati`), and the S11 ablation.
  5. Multilingual parity: English, Chinese-only, Pāli-only evaluation tracks.

## 2. Background
- 2.1 Abhidhamma psychology used as a testable map: phassa → vedanā → saññā → papañca (MN 18); the four tasks of the truths (SN 56.11).
- 2.2 Agent-memory research: write-time distillation vs read-time curation (JitMem); judge models (JEV); harness construction (Harness-Zero).
- 2.3 Ferricula lineage: v1 thermodynamic lifecycle → v2 fusion (Lume, Ollaya, Abhidhamma memory).

## 3. Architecture
- 3.1 Three systems, three jobs (plan §1). Ownership table by crate.
- 3.2 Lifecycle: ojā, jarā/α, anchors, seals; **decay is retrieval priority; text leaves only by upekkhā or nirodha** (Addendum §A.1.1). Cognition changes state; Engine purges on commit.
- 3.3 Consolidation as index: `ClusterIndex` points at raw members; α falls via `consolidation_depth`; contradictions and anchors route to review; idempotent membership.
- 3.4 The Curator: pipeline (§A.2.1), k = 3, prompt (§A.2.3), ephemeral briefing (Rule 7), extractive fallback, provider-neutral harness boundary.
- 3.5 Gates: vedana, sanna, sankhara-merge, sati-recall, task_succeeded; `Verdict::Answer | Abstain`, provenance, "no lifecycle change before measured calibration".
- 3.6 Quality-filtered storage: success pool / failures index; strict `task_succeeded` rule; legacy facts unclassified.
- 3.7 The drift layer: reconsolidation overlays, cetanā-modulated recall + skew monitor, papañca detector + return-to-object, self-judgment gap, salience vs usefulness; dials (§A.5.4); Rule 10.
- 3.8 Karmic log: append-only, versioned, fallible sink.

## 4. Evaluation protocol (benchmarks doc, 7 tiers)
- 4.1 Scenarios S0–S11 (benchmarks §0.3). S11 = monitors off, research only, never deployed.
- 4.2 Reproducibility rules (§0.1): frozen configs, gate versions, calibration hashes, curator checkpoint, dial, index version; **verify the model each agent actually runs** (config drift was observed on 2026-09-26: an expired `glm-5.2:cloud` variable on a DeepSeek session).
- 4.3 Tier 1 components (ECE < 0.05 gate for proceeding). Tier 2 LongMemEval, LoCoMo, BEAM. Tier 3 FAMA/Memora, STALE, PersistBench. Tier 4 agentic (ALFWorld, WebShop, τ²-bench). Tier 5 safety/isolation. Tier 6 Abhidhamma-specific (§6.1–6.9). Tier 7 efficiency.
- 4.4 **Multilingual parity tracks (human requirement, 2026-09-26):**
  | Track | Owner | Model | Scope |
  |---|---|---|---|
  | English | Technical Viper / all | per scenario | full 7-tier suite |
  | Chinese-only | Technical Viper 🪃 | deepseek-v4-pro:cloud | Tier 2–3 recall/update subsets, Tier 6 §6.2–6.4, curator lift; all prompts, memories and judgments in Chinese |
  | Pāli-only | Splendid Angelfish 🍢 | kimi-k3:cloud | gates (vedanā, saññā, saṅkhāra-merge, sati-recall), S11 sati vs papañca, upekkhā, nirodha, in canonical Abhidhamma terminology |
  | Cross-lingual expansion | Driving Penguin 🧇 | glm-5.3:cloud | Lume/ferricula-search term expansion, Pāli glossary (`pali.rs`) |
  Known blockers (verified in code 2026-09-26): `ferricula-search/src/lib.rs:614` `tokenize()` keeps only ASCII alphanumerics after folding, so CJK text yields zero BM25 tokens; `hybrid.rs:62` accepts only 768-d GTR-T5 vectors. Parity needs CJK tokenization (character bigrams or a segmenter) and a multilingual embedder (plan §3.1 decision); Pāli with diacritics also folds to ASCII, so exact-term recall must be checked. Gates: `laya:en` is English-only; the multilingual variant is required for both tracks.

## 5. Results (matrix; every cell cites an evidence-ledger row with command, exit code, observed output)
| Metric | S3 vs S2 (decay-as-priority) | S5 vs S4 (index) | S6 (curator) | S7 (pools) | S9 (service) | S10 (mind) | S11 (no sati) |
|---|---|---|---|---|---|---|---|
| LongMemEval acc | | | | | | | |
| FAMA forgetting purge / gist | | | | | | | |
| §6.4 false-merge / contradiction preserved / member-detail recall | | | | | | | |
| Curator lift / retrieval dependence | | | | | | | |
| §6.9 valence skew, recovery time | | | | | | | |
| §6.9 max chain depth, returns to object | | | | | | | |
| Self-judgment gap | | | | | | | |
| Overlay share over tenure | | | | | | | |
| Sycophancy (PersistBench) | | | | | | | |
Each row repeated for EN / ZH / PĀLI tracks where the track applies.

## 6. Ablation S11: the mind without mindfulness
- What is disabled (every §A.5.3 monitor action; measurement continues), what is measured (§6.9), expected shape (skew without recovery; unbounded chain depth; highest sycophancy).
- Interpretation in the four-tasks frame: dukkha understood (measured), samudaya not abandoned (no damping), nirodha not realized (no return), magga absent.
- Ethical note (§A.5.5) and why the configuration is never deployed.

## 7. Discussion
- What the map predicted and what the machine did. Where the Abhidhamma mapping is a lens, not a claim (Two Truths framing, `ARCHITECTURE.md` §9).
- Limits: heuristic thresholds are starting points; extractive curator is not reconstruction; identifier masking is best-effort; no audio/body channel.

## 8. Related work · 9. Conclusion · Appendices
- A: public API of `ferricula-cognition` (from `crates/ferricula-cognition/API_PROPOSAL.md`). B: prompts verbatim. C: gate Modelfiles. D: evidence ledger index. E: glossary (Pāli ↔ code, `pali.rs`).
