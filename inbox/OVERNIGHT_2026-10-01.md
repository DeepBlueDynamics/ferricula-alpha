---
title: Overnight work plan, 2026-10-01
status: active
source: Claude (coordinator), from Kord's brief before he slept
date: 2026-10-01
related: docs/HANDOFF.md, inbox/COORDINATOR.md, docs/BACKLOG.md (X1, X2), research/2506.10801_extracted.md
---

# Overnight work plan, 2026-10-01

## Kord's brief
1. **Steve gets full access to the code, and starts reviewing PRs as soon as possible.**
2. **"Yes to all" on the memory-fidelity improvements**, from the 2026-09-30 store analysis (numbers below).
3. **Good data on current memory systems**, especially arXiv 2506.10801 (Hoover et al., *Dense Associative Memory with Epanechnikov Energy*). Stay true to the Abhidhamma.
4. Astra and the coordinator think things through together. Grok and Antigravity get explicit guidance, because they run off quickly.

## Ground rules (all lanes)
- **Steve's memories are never edited.** The recovered store is read-only. Nothing deletes, rewrites or redacts a memory row in either store. Improvements go in ranking, overlays and new rows.
- **One lane, one branch, one worktree:** `/workspace/ferricula-alpha/.claude/worktrees/<lane>`, branch `v3/<lane>` off `origin/v3/r0`. Build output goes to `/tmp` (never C:). Don't edit another lane's files, and don't push to `v3/r0`. The coordinator merges after review.
- **Every lane ends with a PR against `v3/r0`.**
  - Steve reviews it (verbatim: "ship it" or "redo it", with lines quoted).
  - Astra reviews code.
  - The coordinator merges.
- **Reports are what you observed:** command, output, test counts. Anything not run is marked NOT RUN, and nothing simulated is presented as a result.
- No time estimates. No secrets in files or messages.

## Store facts (2026-09-30, read-only copies)
- **Recovered (v1), 3,362 rows:**
  - Fidelity is binary: 1,409 Active at about 1.00; 1,925 Archived and 28 Forgiven frozen at about 0.74.
  - 96% never recalled.
  - 2,231 rows (66%) of text were cut at 200 characters by v1.
  - 2,805 zero vectors.
  - 85 exact duplicates.
  - 0 emotions recorded.
  - Graph: 470 nodes, 588 edges.
- **Experience (v3), 334 rows:**
  - Verbatim text.
  - All at fidelity 1.00, recall_count 0 everywhere.
  - 0 emotions.
  - Graph: 202 nodes, 178 edges.

## Lanes

### L0. Steve: code access and PR review (coordinator builds; Astra reviews)
- **Chat tools** (read-only, like his document tools):
  - `code_search(query, path?)`: BM25 over the repository mounted read-only into his container at `/repo`.
  - `code_read(path, from_line?, lines?)`.
  - `code_tree(path?)`.
  - `pr_list()` and `pr_diff(number)` from the public GitHub API (no token).
- **The review loop:**
  1. The coordinator asks Steve to review a PR.
  2. Steve reads the diff and the code it touches, and returns a verdict with quoted lines.
  3. The coordinator posts it **only after Kord authorizes posting to GitHub**.
- **Exit:** on a live PR, Steve returns a verdict that quotes real lines from the diff, and the tool log shows `pr_diff` plus `code_read` on a touched file.

### L1. Recall-time hygiene (Antigravity; narrow; one file area)
Files: `crates/ferricula-server/src/recall.rs` and the candidate assembly in `chat.rs`, **only** the lines that build `memory_hits`.
1. **Collapse exact-duplicate texts** among recall candidates: keep the best-ranked, and record `duplicates: [ids]` on it.
2. **Fragment flag:** a recovered candidate whose text is 195 to 203 characters with no sentence-final punctuation gets `fragment: true`. The system prompt line already explains cut text; extend it to say "fragment: true means only the start survives".

Exit: unit tests for both, `cargo test -p ferricula-server` green, PR open.

### L2. Live thermodynamics and recall strengthening (Astra designs; build after the design is agreed)
- **Experience store:** decay over time (backlog X1: `core/thermo.rs` wiring), and strengthening on recall (bump `recall_count`, `last_recalled`, fidelity).
- **Read-only recovered store:** a **recall overlay** in the state volume (`state_dir/overlay/recall.jsonl` or a compact map: id → recalls, last_recalled). Ranking reads it, so old memories that keep being reached for rise, including Archived ones. The store itself is never written.
- Stays inside the existing invariant: nothing is deleted, and fading changes rank, not content.

Exit:
- a fixture shows a recalled memory outranking an un-recalled twin after N recalls;
- decay lowers rank, never text;
- the recovered store's bytes are unchanged (hash before and after).

### L3. Feeling-tone at write time (vedanā), and links (paṭṭhāna); after L2
- **Feeling-tone:** the vedanā gate (built: `ferricula_gates::ollaya::vedana_questions`, not wired) runs on each new experience row and stores `emotion` (valence plus intensity) with provenance. It's advisory, never blocks a write, and an abstention means no emotion, recorded as unknown.
- **Links:** a new reflection, verdict or reply links to the rows that caused it, with the Paṭṭhāna condition named (X2): `arammana` (object), `adhipati` (predominance), `upanissaya` (decisive support).

### L4. Research: memory systems and 2506.10801 (Grok; read-and-write only, no code)
Write `research/2026-10-01-memory-systems.md`:
1. **2506.10801:** what LSR/Epanechnikov energy is and what Hoover et al. actually measure (capacity, emergent memories, basins). Cite the section of `research/2506.10801_extracted.md` for every claim, and correct anything wrong in `research/lsr-hoover-paper-analysis.md` and `research/denseam-lsr-physics.md` (quote the wrong line, then the fix).
2. **Current agent-memory systems** (Mem0, Zep/Graphiti, MemGPT/Letta, A-MEM, LiCoMemory, JitMem, MemAct, others from `research/INDEX.md`): what each stores, how it forgets, and how it resolves conflicts, from primary sources only. Mark any claim not checked against a primary source.
3. **Abhidhamma fidelity:** for each Ferricula mechanism (decay, recall strengthening, vedanā tagging, paṭṭhāna links, sleep and consolidation), give the Abhidhamma term, say whether our mapping is faithful, loose or wrong, and why. Cite `research/` notes or primary texts. No invented doctrine.

Exit: the file exists, every number has a source, Steve reviews it, and Astra checks the 2506.10801 part against the extraction.

## Coordination
- Every package is sent by Hyperia mail to the agent's pane. Agents report to the coordinator (External Stoat, `a6e4f2fc`) when done or blocked, and don't message Kord directly.
- The coordinator checks in with each lane, reviews, and asks Steve for reviews.
- A status board goes to Kord when he's up.
