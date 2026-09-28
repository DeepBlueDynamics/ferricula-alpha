# Brief: finishing the Ferricula v3 white paper (PDF)

**Goal.** Turn `paper/WHITEPAPER.md` into a finished white paper and a PDF. Author: Kord Campbell, DeepBlue Dynamics. Ask Kord when unsure; he will answer questions.

**Repo.** github.com/DeepBlueDynamics/ferricula-alpha, branch `v3/r0` (PR #1 into `main`). Local: `C:\Users\kordl\Code\DeepBlueDynamics\ferricula-alpha`.

## What Ferricula is (one paragraph)
A Rust runtime that gives one language-model agent a persistent identity, a memory that behaves like one, and drives. Two planes: **evidence** (documents kept verbatim in a lume BM25 index, never decayed, cited exactly) and **experience** (thermodynamic memory records: fidelity decays as retrieval priority, recall strengthens, consolidation indexes without deleting). Inputs pass through a cognitive process modeled on the Abhidhamma vīthi (contact → feeling-tone → recognition → investigating → **determining by a fast local judge (Ollaya)** → impulsion (LLM, only if worth it) → registration). **Drives**: boredom → curiosity (reads the web via the grub crawler) → reflection; sleep pressure → sleep → consolidation → dream → rested wake; meditation holds the drives. Randomness comes from a software-defined radio (sdrrand.nuts.services) with OS fallback. The flagship deployment is "Steve", a simulation built from a recovered v1 memory of Steve Jobs; the engine is persona-neutral.

## Status of the draft
| Section | State | What to do |
|---|---|---|
| Abstract, §1 Intro, §2 Background | written | tighten; keep claims to what is built |
| §3 Architecture | written but dated | add: dense recall via shivvr GTR-T5 embeddings (in progress), live thermodynamics (`ThermoLayer`, radio-clock decay), vīthi record with 52 cetasikas (`vithi.rs`), Paṭṭhāna typed edges (`patthana.rs`), dream images (ComfyUI Qwen Image 2.1, SigLIP-embedded). Mark each **built / wired / planned** honestly |
| §4 Abhidhamma terms: what they mean and don't | written | extend the table with cetasikas (e.g. judge abstention = vicikicchā/doubt, papañca detector = uddhacca, guardrail refusal = ottappa, grounded answer = paññā) and Paṭṭhāna conditions |
| §5 Evaluation | partial | only numbers from `audit/bench/ledger.jsonl`, each citing its run id; see below |
| Case study (new) | missing | the live soak: Kord mentions Jony Ive → 2.5 min later Steve searches, reads two articles, reflects → curiosity chain to AlphaFold (seeded by an old v1 dream image, radio entropy) → sleep → dream → question "Was the shape designed, or was it always in the sequence?" → rested wake. Source: `audit/life/soak-2026-09-27.md` |
| §6 Limitations | written | add the findings below |
| §7 Conclusion | missing | write after §5 |
| Author's note | placeholder | Kord's words: the idea came from a 7-day silent (noble silence) retreat in San Jose, ~8 h sitting and walking meditation a day; ask him for the rest |
| Figures | none | architecture diagram (§3), vīthi stages with gates, drives state machine, one dream image (ComfyUI output) if Kord approves |

## Measured results so far (ledger `audit/bench/ledger.jsonl`)
- Document recall, 58 docs / 1,037 sections, n=200 each, BM25 only: verbatim R@1 0.945 / R@5 1.000 / MRR 0.970; 40%-word-drop R@1 0.850 / R@5 0.945; LLM paraphrase (qwen2.5:7b) R@1 0.710 / R@5 0.910 / MRR 0.791 (run `9aaf89f3`); 1,037/1,037 sections byte-identical to source (run `8f8bcfcf`).
- Gate calibration, Ollaya `laya`, held-out: vedanā acc 0.444, ECE 0.283; saññā acc 0.455, ECE 0.147; yes/no judge acc 0.778, ECE 0.195. **All fail the ECE < 0.05 bar**; Pāli near chance; labels are small and self-authored (run `69e4b846`). Latency ~10–18 ms/decision on an RTX 3060.
- Coming soon (don't invent): dense and hybrid recall numbers, a 30-query recall benchmark over Steve's memory, LongMemEval, lifecycle invariants, 72-hour drives soak.

## Findings that belong in the paper (honestly)
- Waking recall by words fails on meaning: asked a leading question about what he called his father, Steve refused to confirm it (good), but his memory *did* contain the answer (memory 3802021270) and lexical recall missed it. 2,805 of 3,362 recovered vectors turned out to be all-zero. Dense recall is the fix.
- The dream's most vivid details (a childhood place, a parent, a craftsman's maxim) came from the language model's pretraining, not from the traces supplied; grounding must be measured.
- Judges are advisory until calibrated; the "worth researching?" gate (p=0.33) did not stop curiosity. The calibration rule is why this is safe.
- Old v1 dream images leaked into curiosity seeds; a paywall page was ingested as a document.

## Rules
- Every number cites a ledger run id. No vendor or literature numbers presented as ours.
- The Abhidhamma is an architecture and a vocabulary for testable components, **not** a claim about consciousness, experience or physics. Quote Steve's outputs as model outputs ("the agent wrote"), never as feelings.
- Steve is a simulation of a real person who has died: present him as a configured example built from recovered memory, with the guardrail that it never claims to be the living person. No real person's likeness in generated figures.
- Keep the thesis: impermanence as structure; letting go as an operation; recall as reconstruction; drift that is never unseen.

## Sources
`paper/WHITEPAPER.md` (draft), `PLAN_V3.md`, `docs/EMBEDDINGS_PLAN.md`, `README.md`, `audit/bench/*.md`, `audit/life/soak-2026-09-27.md`, `research/INDEX.md`, `research/REFERENCES.md` (arXiv links; PDFs are not redistributed), `research/PAPER_DRAFT.md` (the fleet's older outline and 7-tier protocol), `research/lume-ollaya-abhidhamma-*.md`, `research/07_*`, `10_*`, `12_*` (vīthi, Paṭṭhāna, sense doors), key papers: JEV-as-a-Judge arXiv 2609.26550, JitMem arXiv 2609.27334, MemGPT arXiv 2310.08560.

## Output
`paper/WHITEPAPER.md` updated in place, plus `paper/ferricula-v3.pdf` (e.g. pandoc or a LaTeX template; figures in `paper/figures/`). Keep it about 10–14 pages. Leave a TODO marker (not invented text) wherever a number is pending.
