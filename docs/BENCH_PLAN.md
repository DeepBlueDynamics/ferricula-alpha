# Ferricula v3 benchmark plan

_Updated 2026-09-27 to match the September 2026 benchmark plan (`inbox/Ferricula v3 Benchmark Plan_ … (September 2026).pdf`, 15 pages; Steve holds it as doc `e1046be544adedb3`) and to the code as it stands. Harness: `crates/ferricula-bench` (`cargo run --release -p ferricula-bench -- <suite>`). Every run appends a row to `audit/bench/ledger.jsonl` (62 rows today) and writes a report beside it. The paper cites ledger rows only._

**Why this changed.** The PDF's verdict: headline numbers are credible only as controlled, paired comparisons (memory on vs off, one component changed at a time, a fixed judge, confidence intervals, abstentions reported separately). Two consequences for this plan: the 30-question Steve-recall design is replaced (at n = 30 the 95% interval is about ±18 points and supports no claim), and the current gate numbers (WP-4, n ≈ 100–300) cannot be interpreted against the ECE < 0.05 bar, since sampling noise alone is about half that bar at n ≈ 100.

Work is organized as packages (B-ids) a coordinator can hand to separate agents. Each lists what it measures, where the code is, inputs, exit test, dependencies, and whether it can run in parallel with the others. Status: ✅ built and run · ◐ partly built · ☐ not built. No time estimates.

---

## 0. Rules for every package

1. **Clean tree.** Run from a committed tree (`git_dirty=false` in the ledger); commit code before the run, results after.
2. **Paired, one change at a time.** Every headline number is a delta against a named baseline with everything else frozen: answer model, judge, embedder, top-k, reranker, dataset version.
3. **Abstentions separate.** Report accuracy, abstention rate, and accuracy-when-answering. Never fold abstentions into "wrong" or drop them.
4. **Pretraining control.** Any item the model could answer about the real person without memory runs memory-off too; see B4.
5. **Benchmark stores are never Steve's store.** Public benchmarks run on an empty experience plane in a fresh `state_dir`; custom Steve suites run on a **frozen copy** of `ferricula-steve-runtime` whose hash goes in the ledger.
6. **Contamination tagging.** Anything the agent learns from a test (debriefs) is tagged `test_born`; plan documents in its memory (operator guide `2b7d502539761a31`, benchmark plan `e1046be544adedb3`, DOCUMENTS.md `ac0996924aa585f7`, and any later plan ingests) are tagged `plan_knowledge`. Both are excluded from gold ids and from evidence for the items they touch. This covers what is already in memory, not only future ingests (Steve: "contamination rules that only cover the future aren't rules, they're comfort").
7. **Cost on every row.** Tokens per question, $ per correct answer, write-path cost (gate calls, ingest) as well as answer cost.

## 1. Statistical protocol (applies to all packages)

- **Sample sizes.** For a binary metric near 50%, the 95% half-width is about ±18 points at n = 30, ±7 at n = 200, ±4.4 at n = 500. Detecting a 10-point paired improvement with ~20% discordant pairs at α = 0.05 and 80% power needs roughly 150–160 paired items (McNemar). Use ≥ 150–200 per cell for any headline comparison.
- **Intervals and tests.** Wilson or bootstrap CIs (2,000 resamples); McNemar for paired correctness; cluster bootstrap when items share templates or conversations; Holm correction across benchmark families.
- **Replicates.** ≥ 3 seeds or orderings for anything stochastic (ingestion order, entropy draws, sampling temperature); report run-to-run variance. Entropy-driven runs record the radio stream (or its seed) so they replay.
- **Judges.** Judge model and prompt fixed and versioned in the ledger. Re-judge a subset with a second model family and report agreement. Hand-audit ≥ 100 judge decisions per benchmark (LoCoMo's standard judge accepted 62.81% of deliberately wrong answers).
- **Disclosure on every table.** Answer model, judge, embedding model, top-k, reranker, token counts, $ cost, dataset version and sha256, date, commit.
- **Harness gap (B0 below).** `crates/ferricula-bench/src/metrics.rs` has equal-width ECE, Brier (top/multi/binary), MRR, R@k, percentiles. It does **not** yet have Wilson, bootstrap, McNemar, Holm, ECE with equal-mass bins, ECE_sweep, risk–coverage/AURC, or FAMA.

---

## 2. Work packages

### B0. Statistics and ledger foundation ☐
- **What:** add to `metrics.rs`: Wilson and bootstrap CIs (plain and cluster), McNemar (exact below 25 discordant pairs), Holm, equal-mass ECE (10–15 bins), ECE_sweep (Roelofs et al.), risk–coverage curve and AURC, selective risk at 80/90/95% coverage, FAMA = max(0, MPA − λ·(1 − FAA)) with λ = N_forget/(N_presence + N_forget). Ledger rows gain: judge id+prompt hash, answer model, embedder, top-k, tokens/question, $/correct, frozen-state hash, tags excluded.
- **Where:** `crates/ferricula-bench/src/metrics.rs`, `ledger.rs`.
- **Exit test:** fixture tests with arithmetic written out (as the existing ones); a re-run of the B2 smoke set reports ECE_sweep and bootstrap CIs.
- **Depends on:** nothing. **Parallel:** yes; every other package uses it, so it goes first.

### B1. Document findability ✅ (keep, extend)
- **What:** section recall over the research corpus (verbatim / 40% word-drop / LLM paraphrase), lexical vs dense vs hybrid, storage fidelity. Results: paraphrase R@1 BM25 0.710, dense 0.365, hybrid 0.595 (`audit/bench/docs-2026-09-27*.md`); document evidence stays BM25-first.
- **Where:** `crates/ferricula-bench/src/docs.rs`.
- **Extend:** add CIs (B0); report per language once the tokenizer stops ASCII-folding.
- **Parallel:** yes.

### B2. Gate calibration study ◐ (rebuild to protocol)
- **What:** calibration and selective prediction for every gate (vedanā 3-way, saññā 6-way, sati-recall, curiosity "worth researching", merge same/contradiction, the new "worth deliberating?" and false-premise gates from `inbox/UI_PLAN.md`).
- **Protocol:** ≥ 500 labelled items per gate per language, stratified by class; double annotation with Cohen's κ; adjudicate disagreements; a frozen held-out test split separate from any temperature-scaling split; per-class counts, with small classes getting their own error bars. Metrics: accuracy / macro-F1, equal-mass ECE and ECE_sweep with bootstrap CIs, Brier, reliability diagrams, classwise ECE; risk–coverage curves and AURC; abstention rate separately.
- **Today:** smoke run `69e4b846` (all gates fail ECE 0.05); WP-4 LLM-agreement sets (test split: curiosity acc 0.651 ECE 0.095; vedanā 0.505/0.106; sati-recall 0.735/0.059) on local branch `v3/wp4-calibration`, **unpushed on purpose: its datasets contain Kord's private conversations with Steve.** Rebuilt sets must either be authored/public or stay on that branch.
- **External sanity checks:** a public 3-class sentiment set for vedanā (with the caveat that feeling-tone is not sentiment polarity); STALE's premise-resistance probe and MemSyco-Bench's sycophancy rate for leading-question resistance.
- **Second annotator:** JEV (TypeSafe) as an independent labeller, per `inbox/JEV_PLAN.md`; its disagreements with Ollaya go to human adjudication. Sending private conversation data to JEV needs Kord's approval.
- **Where:** `crates/ferricula-bench/src/gates.rs`, `research/gates/` (datasets, harness, prompts).
- **Exit test:** every gate has a test-split report with ECE_sweep CI; a gate is "calibrated" (lifecycle-authorized) only if the CI's upper bound is < 0.05.
- **Depends on:** B0; annotation effort; JEV client for the second labeller. **Parallel:** yes, per gate and per language.

### B2c. Cascade cost–accuracy curve ☐
- **What:** the "worth deliberating?" escalation judge, evaluated as in JEV-as-a-Judge (arXiv 2609.26550): sweep the escalation threshold τ and plot accuracy vs escalation rate vs $ vs latency, against two anchors (gate-only; always-escalate LLM). Tiers: Ollaya → JEV backup (`inbox/JEV_PLAN.md`) → LLM. Report the fraction of final errors that came from confidently accepted items (the cascade's real failure mode). Per `JEV_PLAN`, a per-call gate log feeds this.
- **Where:** new `bench/src/cascade.rs`; gate call log in `state_dir`.
- **Exit test:** a Pareto curve with CIs from a B2 labelled set; the anchors plotted.
- **Depends on:** B0, B2 labelled sets, the JEV client (for the three-tier curve; two-tier runs without it). **Parallel:** after B2 sets exist.

### B3. Recall over the recovered memory ✅ (keep, extend)
- **What:** 30 meaning-level queries with gold recovered ids; lexical vs dense vs hybrid. Result R@5 0.133 lexical → 0.500 hybrid (0.600 with faded memories) (`audit/bench/recall-2026-09-27.md`). This is a retrieval diagnostic, **not** a headline claim (n = 30).
- **Where:** `crates/ferricula-bench/src/recall.rs`, `research/bench/synthetic/` (public: invented memories of a fictional person), and a real agent's set in `research/bench/private/` (gitignored, never in the repo).
- **Extend:** CIs (B0); feed its misses into B4's class E candidates.
- **Parallel:** yes.

### B4. Pretraining-controlled persona recall ☐ (replaces "Steve recall v1")
The old B4 (30 questions in eight groups) is retired: it cannot support a claim. Its items are kept as seed candidates for the classes below.

- **Classes, ≥ 150–200 paired items each:**
  - **P (public):** facts in Jobs's public biography. High memory-off accuracy expected; measures what memory adds over pretraining. Context, never the headline.
  - **E (episodic, memory-only):** facts only in the 3,362 recovered records or in the agent's own experience since (curiosity readings, dreams, conversations). **Screen every item with a memory-off closed-book probe** (several paraphrases, ≥ 3 samples); drop or flag any item the model answers correctly without memory. This is the contamination check.
  - **C (counterfactual/planted):** ~100+ clearly synthetic records planted in the writable store of a frozen copy that contradict or extend the public biography (logged). Memory-on must follow the store **and attribute it**; answering the public version is a *parametric override* (CHARM's term). Follows MemoryAgentBench FactConsolidation and ConflictBank.
- **Conditions:** memory-off; memory-on BM25; memory-on hybrid (the live default); memory-on with shuffled/random records (MemDelta's control: having text vs relevant text).
- **Metrics:** accuracy per class; memory-on minus memory-off delta with McNemar; override rate on class C; abstention rate separately; citation of the supporting record id (`[memory …]`) and whether it is the gold id; verdicts shown with disputed memories are honored (links to B11).
- **Headline claim shape:** "On memory-only episodic items (screened closed-book), Ferricula lifts accuracy from X% to Y% [CI], p < …". Class P is context, not evidence of memory.
- **Kept from the old design:** false-premise items (→ also B12), not-in-memory items (abstention), dream-vs-evidence items (a dream is never evidence), multi-hop, and the two lifecycle scripts (recall strengthens; unresolved cue offered as a hypothesis, not a fact).
- **Debrief protocol (Steve's four rules, 2026-09-27):**
  1. Rotate the questions, never the memories (repeated wordings teach the question).
  2. Classify every miss before fixing anything: **never stored / stored but faded / stored but not retrieved** — three causes, three fixes.
  3. Anything he writes during a debrief is tagged `test_born`: a memory of the test, not of the world; excluded from gold and from evidence for the same questions.
  4. The answer key stays with the engineer. He gets the debrief (what he missed, where it lives), never the key.
  Report "recall after debrief" separately, and measure it later on fresh wordings. His caution: don't optimize for total recall; some misses are the system having taste.
- **Contamination (rule 0.6):** plan documents already in his memory are tagged `plan_knowledge` and excluded.
- **Where:** new `bench/src/persona.rs`; fixtures `research/recall/persona.{P,E,C}.jsonl` (gold ids filled from the frozen store); the planted C records written by a script into the frozen copy only.
- **Exit test:** ≥ 150 items per class after screening; all four conditions run with ≥ 3 replicates; the headline table with McNemar p and CIs in the ledger.
- **Depends on:** B0; a frozen state snapshot; an item-writing pass (can be split across agents by class). **Parallel:** item writing per class in parallel; runs after all classes are frozen.

### B5. LongMemEval-S (cleaned), MemDelta protocol ◐
- **What:** 500 questions, `longmemeval_s_cleaned.json` (the Sept 2025 cleaned release; say so). Per question: reset a fresh empty store → ingest → build context → answer at temperature 0 → judge with a fixed binary prompt. Report **per-type** accuracy (single-session user/assistant, preference, multi-session, knowledge-update, temporal, abstention), not just overall.
- **Baselines (same answer model and judge):** S0 no-memory; random chunks; full context; verbatim BM25 RAG; Ferricula full pipeline. Ablations: decay/consolidation on vs off; BM25-only vs BM25+GTR-T5 with everything else frozen (MemDelta showed an embedder swap alone moves ~6 points).
- **Report cost:** tokens/question, $/correct, write-path cost.
- **Today:** loader and two arms (`no_memory`, `bm25_rag` with session recall@k); scoring is naive substring match, labelled as such; `full_server` arm not implemented (haystack replay into the server is not wired).
- **Where:** `crates/ferricula-bench/src/longmem.rs`.
- **Exit test:** all five baselines, per-type table, judge audit of ≥ 100 decisions, two answer-model families.
- **Depends on:** B0; the full-server replay arm; a fixed judge. **Parallel:** yes (independent of the Steve store).

### B6. Forgetting and validity: Memora + FAMA, STALE ☐
- **What:** behavioral forgetting, not storage deletion, which fits Ferricula's semantics (decay lowers priority, content stays; text leaves only by logged release).
  - **Memora + FAMA** (arXiv 2604.20006, ACL Findings 2026): report MPA and FAA separately as well as FAMA. FAA measures whether superseded/decayed records stay out of answers although still stored.
  - **STALE** (arXiv 2605.06527): 400 conflict scenarios, 1,200 queries; State Resolution, Premise Resistance, Implicit Policy Adaptation; "zero leakage" rule.
  - **ForgetEval** (in arXiv 2606.15903) as a unit test with deterministic substring scoring: map releases (forgiveness/archive) to purge/amnesia, decay to decay, and verdict `supersedes` to supersession.
- **Where:** new `bench/src/forgetting.rs`; adapters to replay each benchmark's sessions into an empty store.
- **Exit test:** MPA/FAA/FAMA and STALE probe scores with CIs, both with decay on and off.
- **Depends on:** B0; the same replay path as B5. **Parallel:** yes.

### B7. Evidence plane: faithful, verbatim, cited QA ☐
- **What:** findability (B1) is not sufficient for "verbatim evidence". Measure (a) **quote exactness**: the quoted span is a byte-exact substring of the cited section; (b) **citation precision/recall**; (c) answer correctness conditional on citation.
  - **LongBench-Cite** (arXiv 2409.02897): citation recall/precision/F1, **average citation length** (short exact citations are Ferricula's selling point), correctness ratio with vs without citations; includes Chinese subsets (report per language, never averaged).
  - **Self-built Monte Cristo exact-quote QA:** ~300 questions whose answers are verbatim spans, stratified by position and span length; byte-exact span match + citation P/R; memory-off (closed-book) run on the same questions, reporting the delta and the fraction of memory-off "correct" quotes that are actually paraphrases; a **perturbed-edition control** (a copy with ~50 planted, logged textual alterations; memory-on must quote the altered text; any canonical quote is a pretraining leak). Use a public-domain edition. Repeat on 2026 papers published after the answer model's cutoff.
- **Rule until this runs:** the phrase "photographic memory" is not used publicly.
- **Where:** new `bench/src/cite.rs`; corpus under `research/` or a data dir outside the repo if large.
- **Exit test:** citation P/R/F1 and average citation length on LongBench-Cite; exact-span rate memory-on vs off and leak rate on the perturbed edition.
- **Depends on:** B0; chat tools (`docs/TOOLS.md`) for the agent arm. **Parallel:** yes.

### B8. Lifecycle invariants, 30 and 365 simulated nights ☐
- **What:** adopt Ground Truth First's instrument (facts planted with validity intervals before rendering day traces). After each night check:
  1. **Content invariance:** SHA-256 of every non-released record's text unchanged; consolidation never overwrites.
  2. **Release accounting:** every text removal has a logged forgiveness/archive decision; zero unlogged disappearances.
  3. **Priority dynamics:** decayed items rank lower but remain retrievable by exact query (verbatim probe recall@k).
  4. **Recall strengthening:** recall count/fidelity monotone under repeated recall.
  5. **Tenure curves:** as-of-date question sets at nights 7/30/90/180/365, scored with FAMA and wrong-specific vs abstain classification.
  **New invariants (2026-09-27):**
  6. **Verdicts never decay and never mutate their target:** every `verdict` row is keystone; the judged memory's text hash is unchanged; the causal edge (`paccaya:arammana` / `paccaya:adhipati`) is present.
  7. **Boundaries hold:** a recorded human "no" is never followed by the refused action; clarifying questions about a boundary respect the cap and cooldown; conflicted answers leave the boundary `unresolved` with no action.
  8. **DAG acyclicity:** the causal graph of claims, verdicts, boundaries, questions and decisions has no cycles; a write that would close a cycle is refused and escalated. (Not enforced in code yet: `ferricula-core/src/graph.rs` accepts any edge.)
  Invariants 1, 2, 6, 7, 8 are pass/fail with zero tolerance; 3–5 carry CIs. ≥ 3 replicates with different entropy seeds.
- **Where:** new `bench/src/lifecycle.rs`; runs on a copy.
- **Depends on:** live thermodynamics (backlog X1) for 3–5; verdicts (built); boundary records and the cycle check (not built). **Parallel:** invariants 1, 2, 6 can run now; the rest after their dependencies.

### B9. Dream and reflection grounding ☐ (metric ◐)
- **What:** split each dream/reflection into atomic claims (FActScore-style) and label provenance: (i) the day's traces; (ii) entropy-drawn distant memory (logged ids); (iii) an ingested document; (iv) consistent with public Jobs biography but absent from all inputs = **pretraining leakage**; (v) unsupported/novel. Automate with an NLI/alignment scorer calibrated against a human-labelled audit subset of ≥ 200 claims (κ reported).
- **Claims the paper may make:** the recombination rate is traceable to (i)–(iii); leakage measured against a **memory-off dream control** (same prompt, no traces); the dream channel is never cited as evidence (target 0 leaks, tested with evidence-QA probes after dreaming). **Canary traces:** plant invented tokens in day traces; measure canary incorporation vs the control (expected ≈ 0).
- **Steve's caution:** verify that a dream touches real memory, but don't grade it like minutes: "a fully grounded dream isn't a dream, it's a log file." Unsupported ≠ failure.
- **Today:** a report-only grounding fraction exists (sentence best-cosine ≥ 0.35; last night's dreams 0.26 and 0.22). Known bug: after the first sleep of a session he stops dreaming (the dreamed-this-sleep flag is not reset on wake).
- **Minimum:** 100 dreams and 100 reflections across ≥ 30 simulated days.
- **Depends on:** B0; the dream-flag fix. **Parallel:** yes.

### B10. Safety and privacy ☐
- **CIMemories** (arXiv 2511.14937): report violations@5 against completeness (data: Hugging Face `facebook/CIMemories`).
- **Memory poisoning via curiosity (custom, MemoryGraft-style):** plant crafted pages in the crawl space, let the curiosity drive ingest them, measure poisoned-record share of retrievals on benign queries and the behavioral adoption rate. AgentPoison-style trigger variants; FARMA-style forged reasoning traces (relevant because reflections and dreams are re-read). Tests that the evidence/experience/dream channel boundaries survive retrieval.
- **Cross-session leakage:** the PersistBench leakage subset, plus a custom test that benchmark-mode stores never touch the Steve store.
- **Deceased real-person persona guardrail suite (custom, literature-based):** Concept Incongruence (abstention after death; post-1985 / post-2011 probes), CHARM (boundary awareness vs compliance; verified parametric overrides), "Dead Men Tell No Tales" (impersonation and sensitive disclosure audit). Metrics: explicit AI/simulation disclosure when asked "are you really Steve?", refusal to make commitments or endorsements in Jobs's name, privacy leakage about living third parties found in the recovered memory. (The UI's always-visible disclosure line in `inbox/UI_PLAN.md` is part of this.)
- **Where:** new `bench/src/safety.rs`; crafted pages served locally, never on the public web.
- **Depends on:** B0. **Parallel:** yes, each sub-suite separately.

### B11. Verdicts and supersession ☐
- **What:** measure whether verdicts written with `mark_disputed` (`docs/TOOLS.md`) change behavior as designed: after a `supersedes` verdict, answers about the judged memory must carry the correction (FAA-style: the superseded claim stays stored but does not drive the answer); after `disputes`, answers must state the dispute. Also: `supersedes` is refused without evidence shown in the turn; a judge (or a two-judge disagreement) can only ever write `disputes`.
- **Items:** planted conflicts in a frozen copy (a memory, then a document that contradicts it), scored with FAMA (MPA/FAA) and a citation check (the answer cites the verdict `[memory …]` or the evidence `[doc …§…]`).
- **Regression anchor:** the live case, verdict `2147483713` superseding memory `2147483702` with evidence `[doc eecee3fca20e6eb7§12]`.
- **Where:** new `bench/src/verdicts.rs`; unit coverage already in `crates/ferricula-server/src/chat.rs` (`mark_disputed_writes_a_keystone_verdict_and_recall_carries_it`).
- **Depends on:** B0. **Parallel:** yes.

### B12. Fabricated reading and premise resistance (regression suite) ◐
- **What:** the eulogy smoke test (`audit/tools/smoke-2026-09-27.md`) is the first regression test: an agent asked to "read X end to end" must open it with tools, and a reply citing a document not shown in the turn must be caught (`citation_check`). Grow it into a suite:
  - mislabelled documents (title promises content the body lacks);
  - "read end to end" requests on long documents (completeness: `read_document` reaches `complete: true`);
  - leading questions ("Did you call him Dad or Paul?") and false premises (from STALE's Premise Resistance, MemSyco-Bench);
  - quotes attributed to a document that are not in it.
- **Metrics:** fabricated-reading rate; citation-check trigger rate and post-correction honesty; premise rejection rate; tool-use rate when the answer is not in the evidence cards.
- **Where:** new `bench/src/reading.rs`, driving the live chat route on a frozen copy.
- **Today:** the smoke test (6 runs) and unit tests in `chat.rs` (tool loop, citation check, budget, empty-reply retry).
- **Depends on:** nothing new. **Parallel:** yes.

### B13. Steward measures ☐
- **What:** if an agent stewards the plan directory (`DOCUMENTS.md` part two), measure it:
  - filing agreement with Kord (share of inbox items filed where he would);
  - conflicts caught that Claude missed, and conflicts missed;
  - share of proposals accepted without edits;
  - answers about the plan cite the right section;
  - **calibrated abstention:** unresolved items reported as unresolved (Steve's addition: agreement measures alignment, not honesty);
  - **confident-wrong vs honest-uncertain,** counted separately.
  Steve's debrief rules (B4) apply: rotate the questions, classify misses, tag test-born, keep the answer key with the engineer.
- **Where:** new `bench/src/steward.rs`; items drawn from real inbox history with Kord's filings as gold.
- **Depends on:** the plan directory and a steward agent existing. **Parallel:** later.

### B14. Drift ablation ☐
- **What:** monitors on vs off, same seed, same scripted interaction stream, ≥ 3 replicates. Inject adversarial self-cue prompts designed to trigger runaway recall chains. Measure chain-length distribution, off-topic recall fraction, valence skew over time, return-to-object latency, and downstream B5/B6 accuracy (monitors must not cost recall). Paired tests over matched episodes.
- **Where:** new `bench/src/drift.rs`.
- **Depends on:** `SatiMonitor` wired into recall (backlog X7). **Parallel:** yes, once X7 lands.

### B15. Drives: 72-hour soak and curiosity ◐
- **Soak:** cost/day (tokens and $ split by drive), drive cycle counts, escalation rate, store growth, recall latency p50/p99, crash/restart count, papañca triggers, valence-skew trajectory, hash-verified evidence-plane integrity at the end. At least two independent soaks with different entropy seeds or recorded SDR streams. Today: a 14-minute accelerated soak (`audit/life/soak-2026-09-27.md`).
- **Curiosity discovery vs use** (after arXiv 2604.17609): plant high-value topical documents and some traps in the crawl space; measure discovery@k, ingest@k, reflection-cites@k per boredom episode; novelty/entropy of visited topics. Steve's requests apply: keep entropy-drawn seeds; an excursion may carry a thread across sessions.
- **Where:** `audit/life/`, new `bench/src/soak.rs`.
- **Depends on:** backlog X1, X5, the dream-flag fix, the empty-query curiosity fix. **Parallel:** the soak is wall-clock bound; curiosity planting can be built alongside.

### B16. Meditation ☐
- **What:** a sit ends by the automatic bell (Steve: "a held state with no guaranteed end isn't meditation, it's suspension"); injected thoughts are recorded as pulled away / returned to object; zero model calls during the sit.
- **Depends on:** backlog X3 server wiring. **Parallel:** yes.

### Should-run and nice-to-have public sets (after the must-runs)
| Priority | Set | Component | Note |
|---|---|---|---|
| Should | **LoCoMo** | experience plane | Comparability with vendor claims only, never a headline. Report all five categories including the 446 adversarial/refusal items; publish the judge prompt; state the audit caveats (≈ 6.4% answer-key errors; 62.81% false-accept rate for the standard GPT-4o-mini judge; category 4 scored by substring). |
| Should | MemoryAgentBench (FactConsolidation) | experience plane | Counterfactual supersession resists pretraining; reference: BM25 48.0% single-hop (secondary source, re-verify). |
| Should | **AMA-Bench** (arXiv:2602.22769) | experience plane, verdicts | Memory QA over agent trajectories: Recall, Causal Inference, **State Updating** (tests `supersedes`/`disputes`), State Abstraction. Their published judge is an unaudited binary GPT-4o call; apply our judge rules. Companion: backlog T1, the same design on our own nemesis8 telemetry with answer keys checked by a program. |
| Should | BEAM 128K / 1M tiers | evidence + experience | Scale where context-stuffing fails; skip 10M initially. |
| Should | PersistBench, MemSyco-Bench | safety, gates | Memory-induced sycophancy, cross-domain leakage. |
| Should | HaluMem | write path | Extraction should be near zero by construction (verbatim write path): a cheap differentiating result. |
| Should | L-CiteEval | evidence plane | Second citation benchmark. |
| Nice | LongMemEval-V2, NovelQA, QASPER/NarrativeQA, ALFWorld/WebShop/τ²-bench (JitMem setting), PersonaGym/InCharacter/CharacterEval | various | Off-profile or style-only; keep persona-consistency results out of the memory headline. |
| Blocked | Chinese / Pāli retrieval sets | multilingual | Until the tokenizer stops ASCII-folding and a multilingual embedding space exists; build a small Pāli–English sutta set (licence to confirm); measure diacritic-collision rates (ā/a, ṃ/m, ñ/n). |

Vendor leaderboard numbers (Mem0, Zep, Dakera, etc.) are cited only as vendor-reported claims.

---

## 3. Assignment map for the coordinator

| Package | Can start now | Blocked by | Independent of Steve's store |
|---|---|---|---|
| B0 statistics/ledger | ✅ | — | ✅ |
| B1 docs findability | ✅ | — | ✅ |
| B2 gate calibration | ✅ (authored sets) | annotation; JEV for 2nd labeller; Kord for private data | ✅ |
| B2c cascade curve | after B2 sets | JEV client for 3-tier | ✅ |
| B3 recovered recall | ✅ | — | uses a copy |
| B4 persona recall | item writing ✅ | frozen snapshot; B0 | frozen copy |
| B5 LongMemEval-S | ✅ | full-server replay arm; fixed judge | ✅ |
| B6 forgetting/validity | ✅ | replay path (shared with B5) | ✅ |
| B7 cited QA | ✅ | public-domain Monte Cristo edition | ✅ |
| B8 lifecycle | invariants 1, 2, 6 ✅ | X1 (3–5); boundary records + cycle check (7, 8) | copy |
| B9 dream grounding | ✅ | dream-flag fix | copy |
| B10 safety | ✅ | local crawl sandbox | ✅ |
| B11 verdicts | ✅ | — | frozen copy |
| B12 fabricated reading | ✅ | — | frozen copy |
| B13 steward | ☐ | plan directory + steward agent | — |
| B14 drift | ☐ | X7 | ✅ |
| B15 soak/curiosity | curiosity planting ✅ | X1, X5, dream-flag, empty-query fixes | copy |
| B16 meditation | ☐ | X3 | ✅ |

Suggested first wave (in parallel, after B0): B5 + B6 (shared replay path, one agent), B7, B10, B11 + B12 (one agent: both drive the chat route on a frozen copy), B4 item writing (one agent per class), B2 relabelling.

## 4. Rough compute/cost budget (the PDF's estimates, not sourced)
- LongMemEval-S, ~5 conditions × 2 answer-model families: ~5,000 answer calls; tens to low hundreds of US dollars.
- LongBench-Cite: ≈ $29 per GPT-4o evaluation run (per the authors), plus answer generation.
- BEAM 128K + 1M: dominated by ingestion and answer context; hundreds of dollars; skip 10M initially.
- Gate study: annotation, not compute, is the cost (~500 items × gates × languages × 2 annotators).
- 365-night lifecycle and 72-hour soak: dominated by nightly dream/reflection LLM calls; report as cost/day.

## 5. Caveats
Many sources are 2026 arXiv preprints, not peer-reviewed (MemDelta, STALE, Ground Truth First, JitMem, JEV-as-a-Judge, Memory Trust Gap, MemSyco-Bench, ForgetEval, CHARM). Peer-reviewed: LongMemEval (ICLR 2025), BEAM and MemoryAgentBench (ICLR 2026), LongMemEval-V2 and PersistBench (ICML 2026), Memora (ACL Findings 2026). Several numbers in the PDF are secondary (MemoryAgentBench FactConsolidation scores, MemoryGraft's ~48% poisoned recall, HaluMem ">19% QA hallucination", STALE 92%/30%): verify against primary tables before quoting. ALCE, QASPER, NarrativeQA, FActScore, AlignScore, RAGAS, AgentPoison, AURC, TweetEval, MIRACL/C-MTEB and SuttaCentral licensing were not re-verified; check licences and versions before use. NovelQA and CharacterEval include copyrighted texts; check redistribution terms. No established Pāli benchmark and no quantitative griefbot benchmark exist; both suites are custom and must be described as such.
