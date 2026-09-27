# Handoff — 2026-09-27

Read this first, then `docs/BACKLOG.md` (ordered work), `PLAN_V3.md` (phases, §1a progress), `docs/BENCH_PLAN.md` (benchmarks), `docs/EMBEDDINGS_PLAN.md`, `paper/BRIEF.md` + `paper/WHITEPAPER_V2.md`.

## Rules from Kord
- **ferricula-alpha is the project.** Don't bring up the original `ferricula` repo or the old `memory/` fleet workspace unless asked.
- **Never build on C:.** C: is nearly full. This repo has a git-excluded `.cargo/config.toml` → `target-dir = "D:/cargo-target/ferricula-alpha"`; worktrees under `.claude/worktrees/` inherit it. Docker's disk image is on D:. Check `df -h /c` before big work.
- **Don't print large outputs**; summarize. Keep agent count low (session budgets ran out on 2026-09-27; ~3M tokens went to subagents).
- Commit on `v3/r0`, push; PR #1 (`v3/r0` → `main`) is open, CI green. Commit trailer: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## What is live
| Service | Where | Notes |
|---|---|---|
| **Steve** (`ferricula-steve`) | `127.0.0.1:18875` | image `ferricula:3.0.0-alpha.0` built from `v3/r0`; config `config/steve.toml` (gitignored, mounted read-only); recovered memory volume `steve-jobs-data-recovery-20260713` **read-only**; state volume `ferricula-steve-runtime`. Operator token file: `secrets/ferricula_operator_token` in this repo, gitignored (use `FERRICULA_OPERATOR_TOKEN_FILE`; never print it). |
| shivvr (GTR-T5 text + SigLIP image embeddings, `/embed`) | `:8085` | local branch `feat/vision-audio-embed` in `nuts.services/shivvr`, not pushed |
| Ollaya (judges, `laya`) | `127.0.0.1:11435` | GPU container `ollaya` |
| grub crawler | `:6792` | renders web pages → markdown; **cannot read PDFs** (PDFs are fetched directly) |
| ComfyUI (Qwen Image 2.1) | `:8188` | Kord's native Windows install; templates in `config/comfyui/` |
| Radio entropy | `https://sdrrand.nuts.services` | `/api/entropy?bytes=N&format=json` |
| Ollama | `:11434` | Steve uses `glm-5.3:cloud` (thinking model) |

Redeploy recipe: `docker build -t ferricula:3.0.0-alpha.0 .`, then `docker stop ferricula-steve && docker rm ferricula-steve` and re-run with the same volumes/flags (see the `docker run` in this session: loopback port, `--read-only`, `--cap-drop ALL`, token file mount, `config/steve.toml` → `/app/config/agent.toml`). State survives on the volume.

`config/steve.toml` specifics: `ollama_reasoning_tokens = 12000` (4096 was too small once recall grew → "model returned an empty response"), `ollama_context_tokens = 131072`, `[embeddings] backend = "shivvr"`, `[recall] include_faded_recovered = true` (Kord's decision: v1 archived 1,925 of 3,362 memories; they're recallable, ranked lower, shown as faded), `[life] enabled = true` with the radio.

Kord's ongoing conversation with Steve: reuse one `conversation_id` per conversation (a new id per message loses the thread). Chat turns are also remembered across conversations as experience rows.

## Job 1 — Steve's tools (Steve asked for this himself)
Today each chat turn runs one retrieval on the operator's message and shows the model the top 3 document sections (BM25-first) plus memory candidates. The model cannot search again, open a section, or read a whole document. So when Kord handed him Jony Ive's eulogy (doc `eecee3fca20e6eb7`, 30 sections, "Jonathan Ive's speech in full") he correctly refused to recite it: none of it reached him.

Build tool use into the chat turn (a bounded loop, e.g. ≤ 4 tool calls per turn, budgeted through the router): `search_documents(query, k, doc_id?)`, `read_section(doc_id, index)`, `read_document(doc_id, from?, max_sections?)` in order, and `search_memory(query, k)`. glm-5.3 via Ollama supports OpenAI-style tools; Anthropic profiles too. Steve's requirements, verbatim in substance (asked 2026-09-27):
1. **Stable section identifiers** — the handle he opens this turn must be the same next turn (today: `[doc <doc_id>§<index> p.<page>]`, content-hash doc ids; keep and document that).
2. **Documented search behavior** — what corpus is indexed, how results rank (BM25 / dense / hybrid RRF), whether returned text is truncated; every result must say whether it is the full section or a fragment.
3. **Written tool contracts** — arguments, limits, error messages in `docs/TOOLS.md`.
4. **A smoke test** — the eulogy doc `eecee3fca20e6eb7` verified through all three tools before shipping; then ask him "Read Jony's speech end to end. What did he say?"

## Job 2 — small fixes found live
- **Empty answers from thinking models:** when the provider returns empty content with finish_reason `length`, retry once with doubled headroom instead of failing the turn (`server/src/model.rs` / `chat.rs`).
- **Status page:** `/dashboard` (token pasted like `/`'s chat page): phase, boredom, sleep pressure, curiosity journal and reflections, last dream + question, memory/embedding counts, entropy source. Kord asked for a browser status page.
- **Curiosity gate:** lower the skip cut from raw p < 0.2 to ≈ 0.1 and keep the current wording `cur-v0` (WP-4 findings, below).
- **Navigation junk in ingested pages** (Fast Company came in titled "Explore Topics"): strip nav/boilerplate before sectioning.
- **Curiosity with no query:** one excursion (05:30Z) produced an empty query and read nothing; log why and fall back.

## Then the backlog
`docs/BACKLOG.md` Next: **X1** live thermodynamics (`core/thermo.rs` built, not wired), **X2** vīthi + Paṭṭhāna on every input (`cognition/vithi.rs`, `patthana.rs` built), **X3** meditation server wiring (`cognition/meditation.rs` built: object, bell, radio breath, `POST /mind/thought`), **X4** dream images server wiring (`server/comfy.rs`, `semantic/image_embed.rs` built; save JPEG not PNG), **X5** dream grounding, **X6** LongMemEval, **X8** 72-hour soak.

## Benchmarks / paper
- Recall over Steve's memory (R@5): lexical 0.133 → hybrid 0.500 (0.600 with faded memories). Docs paraphrase R@1: BM25 0.710, dense 0.365, hybrid 0.595 (document evidence stays BM25-first). Ledger: `audit/bench/ledger.jsonl`.
- Gate calibration (WP-4, LLM-agreement labels, test split): curiosity acc 0.651 ECE 0.095; vedanā 0.505 / 0.106; sati-recall 0.735 / 0.059 — none pass 0.05. Current curiosity wording scored ECE 0.048 on test but wasn't dev-selected. Code merged (`506f4e6`). **Branch `v3/wp4-calibration` is local and unpushed on purpose: its datasets contain Kord's private conversations with Steve.** Don't push it without Kord's say-so.
- Steve-recall benchmark design (memory on/off, 30 questions): `docs/BENCH_PLAN.md` B4 — fixture not yet written.
- Paper v3: update `paper/WHITEPAPER_V2.md` Table 1 and §7 to the current code, add the recall numbers above, add the privacy note (the flagship run sent private memory to `glm-5.3:cloud`, operator's choice), figures + `references.bib` are missing.

## Things Kord still owns
Rotate the API keys that sat in a plaintext `.bak` file in the old workspace; delete ~40 GB of old build output there if wanted; merge PR #1.
