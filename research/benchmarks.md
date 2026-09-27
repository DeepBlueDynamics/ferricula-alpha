# Benchmark numbers — LoCoMo, LongMemEval, BEAM

Source: `ferricula/research/AI Agent Memory and Thermodynamic Architectures_ Mid-2026 Frontier Briefing for Ferricula and LUME.pdf` (pages 2–3, 8). Every number below is quoted from that PDF verbatim. Vendor-reported scores conflict across sources; treat as indicative, not settled (see caveats at foot).

## Mem0

> "Mem0's new token-efficient memory algorithm hits 92.5 on LoCoMo, 94.4 on LongMemEval, and 64.1/48.6 on BEAM (1M/10M) while averaging under 7,000 tokens per retrieval call. Full-context approaches on the same benchmarks use 25,000+."

- LoCoMo: **92.5**
- LongMemEval: **94.4**
- BEAM: **64.1** (1M) / **48.6** (10M)
- Retrieval cost: avg **< 7,000 tokens/call** vs **25,000+** for full-context approaches.
- Largest gains: **+29.6 points on temporal queries, +23.1 on multi-hop** (same paragraph, PDF page 2).

## Letta (MemGPT)

> "It is now productized as Letta; independent evals put Letta around ~83% on LoCoMo."

- LoCoMo: **~83%**.

## Zep / Graphiti

> "on the DMR benchmark 'Zep demonstrates superior performance (94.8% vs 93.4%),' and on LongMemEval (GPT-4o) Zep scores 63.8% vs Mem0's 49.0% — a ~15-point advantage on temporal retrieval."

- DMR: **94.8%** (vs 93.4% — Zep's comparison).
- LongMemEval (GPT-4o): **63.8%** (Zep) vs **49.0%** (Mem0).

## BEAM background

> "BEAM, which per Mem0's State of AI Agent Memory 2026 report 'evaluates memory systems at 1M and 10M token scales across ten task categories, including preference following, temporal reasoning, and contradiction resolution… the only public benchmark that operates at context volumes production AI agents actually encounter.'"

> "BEAM is deliberately designed so no current system saturates it (best scores ~64%/49% at 1M/10M tokens)."

- Scale: **1M and 10M tokens**, ten task categories.
- Ceiling: best scores **~64% / ~49%** at 1M/10M — no system saturates it.

## Benchmark definitions (PDF page 3)

- **LoCoMo**: 1,540 questions across single/multi-hop/temporal/open-domain, ~600 turns and ~16K tokens over up to 32 sessions.
- **LongMemEval**: 500 questions across five abilities (information extraction, multi-session reasoning, temporal reasoning, knowledge updates, abstention); ~115K-token histories.
- EpBench (adjacent): episodic memory at 100K–1M tokens.

## Caveats

- **Vendor-reported scores conflict.** > "Mem0 and Zep report materially different LongMemEval numbers for each other (e.g., Mem0 at 49.0% per Zep vs Mem0's own higher self-reported figures); treat all single-source scores as indicative, not settled, and re-run evals on your own workload." (PDF page 9.)
- > "most benchmarks test chat history rather than task execution." (PDF page 3.)
- Recommendation 5 (PDF page 9) compresses the target set: "Mem0 (92.5/94.4), Zep (63.8 LongMemEval), and Letta (~83 LoCoMo)" as comparison points for Ferricula/Lume evals.

## Implication for Ferricula vs Lume

- BEAM (1M/10M) is the only benchmark at production context volumes — the one that can differentiate Ferricula's lifecycle/consolidation layer. LoCoMo/LongMemEval are chat-history tests better suited to the Lume index + agent memory framing.
- No system saturates BEAM; a correct Ferricula run can produce the project's only independent positioning number relative to Mem0/Letta/Zep.
