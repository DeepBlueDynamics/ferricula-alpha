# Ferricula research run (live)

Status: running in Crusade Spicy Meatball / Difficult Stork (antigravity).
Wisdom Kings / five archetypes: out of scope unless a paper forces a one-line mention.
Write findings here under `research/`.

## Corpus already on disk (`ferricula/research/`)

| File | What it is |
|---|---|
| `AI Agent Memory and Thermodynamic Architectures_ Mid-2026 Frontier Briefing for Ferricula and LUME.pdf` | Primary map. Agent-memory SOTA + thermodynamic/DenseAM papers + Lume hybrid retrieval. |
| `Furicula Project AI Memory Research.pdf` | SOTA in agentic memory: Ferricula + Loom/MSC, DenseAM, LSE vs LSR energy. |
| `citta-vithi.md` | Local 17-moment → Ferricula mapping. Use as homology notes, not a workstream. |
| `gemini_paper.txt` | Local Ferricula engine writeup (decay, WAL, arena numbers). |
| `MappingtheMindAModelBasedonTheravadaBuddhistTextsandPractices.pdf` | Walpola et al. 2017. Abhidharma paper is already here — do not hunt a monk. |
| `Wu_Bowen.pdf` | ETH thesis: GPU-accelerated vectorized analytics (TPC-H). Metal/GPU recall, not identity. |
| `2512.01797v2.pdf` | arXiv 2512.01797 H-Neurons (hallucination neurons). Secondary. |
| `2605.13438v3.pdf` | Local arXiv PDF; title not resolved from filename. |
| `weber_bracket_paper.pdf` + Weber errata | Physics/Weber, not agent memory. |

Host also cloned `lume/` at workspace root (Vicious Piranha).

## Extract from the frontier briefing (verified from the PDF)

- Split: **Lume = document index**, **Ferricula = lifecycle/consolidation memory**.
- Agent-memory SOTA: hybrid vector+graph+lexical with explicit consolidation/decay. Production: Mem0, Letta/MemGPT, Zep/Graphiti, A-MEM, Cognee.
- Benchmarks: LoCoMo, LongMemEval, BEAM (1M/10M).
- Thermodynamic frontier (priority):
  - DenseAM / modern Hopfield (Krotov & Hopfield); one-step update ≈ transformer attention.
  - **Stochastic Thermodynamics of Associative Memory** — Rooke, Krotov, Balasubramanian, Wolpert, arXiv:2601.01253 (Jan 2026). Entropy production vs retrieval vs speed.
  - Companion: Geometric Entropy and Retrieval Phase Transitions, arXiv:2604.07401.
  - LSE (Gaussian/infinite support, noise floor) vs **LSR / Log-Sum-ReLU** (Epanechnikov/finite support, noise-free below α_th).
- Lume upgrades named: Qwen3-Embedding, late-interaction (ColBERT/ColPali), RAPTOR/HippoRAG hierarchical index.
- Hardware named: Extropic TSU, Normal Computing CN101 — early-stage, not a blocker.

## What to produce (this run)

1. One markdown note per priority paper/system: claim, method, number, what it implies for Ferricula vs Lume.
2. A short gap list vs current `ferricula/` + the v2 workspace (no crate edits yet).
3. Do not create Wisdom King / archetype designs.
