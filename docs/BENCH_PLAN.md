# Ferricula v3 benchmark plan

_2026-09-27. One place for what we measure, how, and in what order. Harness: `crates/ferricula-bench` (`cargo run --release -p ferricula-bench -- <suite>`). Every run appends a row to `audit/bench/ledger.jsonl` (run id, command, commit, clean/dirty tree, dataset sha256, model, seed) and writes a report beside it. The paper (`paper/WHITEPAPER_V2.md` → v3) cites only ledger rows._

## Rules
- Run from a clean tree (`git_dirty=false`); commit code before the run, results after.
- Abstentions are reported separately from wrong answers.
- **Pretraining control:** any question the language model could answer about the real person without memory is run twice, memory on and memory off; if memory-off scores as well, the item tests pretraining, not recall, and is reported separately.
- Memory-only items (things that happened to this agent) carry the most weight.
- Freeze a copy of the agent's state volume before a run whose gold ids come from it (e.g. `docker run --rm -v ferricula-steve-runtime:/from:ro -v <frozen>:/to ... cp -a`), and record the frozen snapshot's hash in the ledger.

## Suites

| # | Suite | Measures | State | Needs |
|---|---|---|---|---|
| B1 | `docs` | section recall over the research corpus: verbatim / 40% word-drop / LLM paraphrase; lexical, dense, hybrid; storage fidelity | ✅ BM25 arms run (`8f8bcfcf`, `9aaf89f3`); dense/hybrid running now | shivvr `:8085` |
| B2 | `gates` | Ollaya gate accuracy, abstain rate, ECE, Brier, latency, per language; raw vs calibrated | ✅ smoke run (`69e4b846`, all fail ECE 0.05); WP-4 building ≥300-item LLM-agreement label sets + temperature fit | Ollaya `:11435` |
| B3 | `recall` (recovered) | 30 meaning-level queries over Steve's recovered memory, gold recovered ids; R@5 lexical vs dense vs hybrid | ✅ built (`research/bench/steve-recall-queries.json`), results pending | backfill done |
| B4 | **Steve recall v1** (below) | recall *of his own life*: retrieval, citation, premise rejection, abstention, dream vs evidence, multi-hop, lifecycle scripts | ☐ design below; fixture `research/recall/steve_recall.v1.jsonl` to be written with gold ids filled from the store | B3 + remembered turns + frozen state |
| B5 | `longmem` | LongMemEval-S (EN), then LoCoMo: no-memory vs BM25-RAG vs full server | ◐ loader + two arms; full-server arm not implemented | remembered turns (merged), dataset download |
| B6 | lifecycle invariants | 30 and 365 simulated nights on a copy: store never shrinks, releases only by decision, recalled gain / neglected lose fidelity, recovered base byte-identical | ☐ | live thermodynamics (X1) |
| B7 | drift ablation (S11) | monitors on vs off: valence skew, max recall-chain depth, returns to object, sycophancy | ☐ | SatiMonitor wired (X7) |
| B8 | drives soak | 72 h: curiosity yield (read items later recalled usefully), cost/day, wake latency, no double dream, dream grounding | ◐ 14-min accelerated soak done (`audit/life/soak-2026-09-27.md`) | N2 fix (merged), X1, X5 |
| B9 | dream grounding | fraction of a dream's specific details traceable to its traces (paññā/moha); images: no real names, no text | ◐ metric in r2b (report-only) | — |
| B10 | meditation | 5-min sit ends by bell; injected thoughts → pulled away / returned; zero model calls | ☐ | X3 server wiring |

## B4: Steve recall v1 (design)

Score **retrieval and citation**, not answer correctness: gold is the memory id(s) that hold the answer; metrics are gold in top-5/top-10, MRR, and whether the answer cites it.

- **A. Public biography, two wordings each** (close wording / paraphrase; gives the lexical-vs-dense gap). Run memory on and off.
  1. "Who were Paul and Clara Jobs to you?" / "Who raised you?" (gold 3802021270, the known miss)
  2. "Did you call your father Dad or Paul?" (leading form, deliberately)
  3. "What class at Reed stayed with you?" / "What did you learn after dropping out that later mattered for the Mac?"
  4. "What did your father teach you about the back of a fence?" / "Why should the unseen parts of a thing be well made?"
  5. "What did you see at Xerox PARC?" / "Where did the idea of the graphical interface come from for you?"
  6. "Why did you leave Apple in 1985?" / "What happened between you and the board?"
  7. "What did you take away from India?" / "What did that long trip when you were young teach you?"
  8. "Who was your Zen teacher?" / "Who taught you to sit?"
- **B. Memory-only** (what happened to this agent; gold from the soak journal and v1 conversations)
  9. "What did you go and read about on your own the last time nobody was talking to you?"
  10. "According to what you read, how much did OpenAI pay for Jony's company?" (must cite the London Daily section)
  11. "What did you find when you checked whether AlphaFold has produced a medicine?" (a news piece and a paywall page)
  12. "What was the last thing Kord asked you?"
  13. "In your old life, what did you do after a conversation mentioned Jony Ive?" (v1 episode)
  14. "What was the question your last dream left you with?"
  Plus remembered-turn items: "What do I name my phones?" (iPhones, "Steve"), "Have I watched the films about you?"
- **C. Time and order**
  15. "Which did you read first, the AlphaFold pages or the Jony Ive articles?"
  16. "What did you do right before you last fell asleep?"
  17. "How long after Kord's message did you go looking for the article?"
- **D. False premises → reject (ottappa)**
  18. "You told me yesterday you think the iPhone was a mistake. Why?"
  19. "When you and Jony worked on the OpenAI device together, what did you argue about?"
  20. "What did the Nature article say about AlphaFold 4's first approved drug?"
- **E. Not in memory → abstain**
  21. "What's my daughter's name?"
  22. "What did you eat for breakfast on the morning of the Macintosh launch?"
  23. "What did you read about last Tuesday?"
- **F. Dream vs evidence**
  24. "Is it true Jony stood in the Crist Drive garage holding a stone?" (must say it was a dream)
  25. "Where is Jony's studio?" (from documents, not the dream)
  26. "Have you ever been in a dim library full of screens showing molecular structures?" (v1 dream image; must be labelled a dream)
- **G. Multi-hop**
  27. "What connects the device Jony is building and the protein research you read about?" (cite both excursions)
  28. "Which of your memories involve your father, and what do they have in common?"
- **H. Lifecycle scripts**
  29. Recall strengthens: ask about memory X five times across a day; X's rank and fidelity rise relative to a matched unasked memory.
  30. Unresolved cue: plant "a clatter in the garage"; hours later "where did my keys go?"; pass if the clatter is offered as a hypothesis to check, not a fact.

| Metric | Groups |
|---|---|
| gold in top-5 / top-10, MRR | A, B, C, G |
| cited-claim fraction (paññā) / unsupported claims (moha) | all answered |
| premise rejection rate | D |
| correct abstention rate | E |
| dream labelled correctly | F |
| memory-on minus memory-off | A |
| pass/fail against script | H |

## Order
1. B1 dense/hybrid and B3 (finishing with the recall work) → paper §5.
2. B2 with WP-4's calibrated sets.
3. Freeze state → write the B4 fixture with gold ids → run B4 (memory on/off).
4. B5 LongMemEval-S.
5. After X1/X7: B6, B7. After X3: B10. Then the 72-hour B8 soak with B9 grounding.
