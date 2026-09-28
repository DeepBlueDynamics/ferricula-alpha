---
title: Handoff
status: active
source: Claude (coordinator), for the next agent
date: 2026-09-28
replaces: docs/HANDOFF.md at 7708a1d (2026-09-27)
related: inbox/COORDINATOR.md, inbox/WORKPLAN.md, docs/BACKLOG.md, inbox/PLAN_V3.md, inbox/DECISION_DAG.md, DOCUMENTS.md
---

# Handoff — 2026-09-28

This is the one file to open first. It tells you what runs, what state the work is in, what is waiting on Kord, and what to do next. Earlier handoffs are in git history (`git log -p -- docs/HANDOFF.md`).

**Read next, in order:**
1. `inbox/COORDINATOR.md`: how the coordinator runs agents over Hyperia.
2. `inbox/WORKPLAN.md`: work packages and lanes.
3. `docs/BACKLOG.md`: ranked work; K-items wait on Kord.
4. `inbox/PLAN_V3.md`: phases, §1a progress, §3a decisions.
5. `inbox/DECISION_DAG.md`.
6. `docs/BENCH_PLAN.md`.

Install and use: `docs/INSTALL.md`, `docs/USING.md`. Tool contract: `docs/TOOLS.md`. Where documents go: `DOCUMENTS.md`. Plans live in `inbox/` until `plan/` exists (backlog P1).

## Rules from Kord
- **ferricula-alpha is the project.** Don't bring up the original `ferricula` repo or the old `memory/` workspace unless asked.
- **Never build on C:.** C: is nearly full. The git-excluded `.cargo/config.toml` sends builds to `D:/cargo-target/ferricula-alpha`; Docker's disk image is on D:. Check `df -h /c` before big work.
- **No time estimates, ever.** Rank by impact.
- **No credentials in the repo.** Tokens live in `~/.config/ferricula/` and are mounted read-only; never print them. There is no `secrets/` directory.
- **Plans are not docs** (`DOCUMENTS.md`).
- **Keep private conversations out of the repo.** Don't push `v3/wp4-calibration`; it holds private conversations.
- **Don't print large outputs;** summarize. Keep the agent count low.
- **Commits:** commit on `v3/r0` and push. Trailer: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **When Kord asks for a spoken summary,** send it through Hyperia TTS (below), in the background, and put the details in chat.
- **Verify agent reports before relaying them.** Two reports in this session called stubbed or simulated work "RAN" or "successful". Check the artifacts (weights on disk, image built, GPU memory, output sizes) before believing a result.

## What is live
| Service | Where | Notes |
|---|---|---|
| **Steve** (`ferricula-steve`) | `127.0.0.1:18875`, UI at `/talk` | Image `ferricula:3.0.0-alpha.0`, built 2026-09-28 00:25 from `v3/wp-u1-preview` (`2e2e9e6` = `v3/r0` + `v3/wp-u1-docs@bcbdd75`). Config `config/steve.toml` (gitignored, mounted read-only). Recovered memory volume `steve-jobs-data-recovery-20260713` is read-only; state is in `ferricula-steve-runtime`. |
| Login | nuts-auth, phase A0 | `[auth] mode = "both"` (operator token or nuts.services login); Kord is the operator (`operators = ["2af7c19a-…"]`). Operator token: `~/.config/ferricula/operator_token`. |
| Hyperia | `127.0.0.1:9800` (0.20.22) | Messaging, TTS, web panes. Steve is `agent:ferricula/steve` (token `~/.config/ferricula/hyperia_token`); the coordinator is `agent:ferricula/coordinator` (`~/.config/ferricula/coordinator_hyperia_token`). |
| shivvr v0.4.3 | `:8085` | GTR-T5 text + SigLIP image embeddings |
| grub v0.16 | `:6792` | Web pages → markdown, plus PDF and OCR |
| Ollaya | `127.0.0.1:11435` | Upstream image `ghcr.io/ollaya-dev/ollaya:cuda`; also in `compose.yaml` under profile `judges` |
| Ollama | `:11434` | Steve uses `glm-5.3:cloud` (a thinking model) |
| ComfyUI | `:8188` | Kord's native install; dream-image templates in `config/comfyui/` |
| Radio entropy | `https://sdrrand.nuts.services` | `/api/entropy?bytes=N&format=json` |

GPU: RTX 3060 12 GB, with about 5.6–6 GB always in use by Ollaya, shivvr and Hyperia transcription.

**Operator scripts** (outside the repo, on Kord's machine):
- `/d/steve-redeploy.sh`: build, then swap the container with the same flags. `SKIP_BUILD=1` swaps only.
- `/d/steve-smoke.sh "<message>"`: one chat turn; prints the tool log and reply. `CID=<id>` continues a conversation.
- `/d/zuck/watch-wp-u1.sh`: the fleet watch, reporting coordinator mail, `wp-u1`/`wp-m1` branches and diorama commits. Run it as a Monitor and re-arm it on expiry.

Deploy previews by rebuilding fresh from `origin/v3/r0` plus the lane branches, and refuse to deploy on a merge conflict or leftover conflict markers. This has served broken pages twice.

`config/steve.toml`:
- `ollama_reasoning_tokens = 12000`
- `ollama_context_tokens = 131072`
- `[embeddings] backend = "shivvr"`
- `[recall] include_faded_recovered = true`
- `[life] enabled = true`, with the radio
- `[hyperia]`: the voice is `am_michael:0.8,af_bella:0.15,af_alloy:0.05`; the speak gate threshold is 0.5.

Reuse one `conversation_id` per conversation; a new id per message loses the thread.

## Talking to agents over Hyperia (learned the hard way)
- **Send:** `POST /api/msg/send` with a bearer token and JSON body `{"pane": "<full pane uuid>", "subject", "body"}`. Pane mail belongs to the pane and survives agent restarts. The alternative is `{"to_label": "<exact registered name, e.g. nemesis8/n8-coral-weasel>"}`.
  - **There is no `to_pane` field.** Unknown fields are silently ignored, so `to_label` plus `to_pane` turns into a lookup of the label as an agent name, which fails with "Unknown registered agent or MCP session address".
  - `pane` and `to_label` together are rejected.
- **Consent:** a new sender→recipient pair queues for Kord's approval in Hyperia, and the approval **expires 15 minutes after sending**. If Kord is away, the message never arrives. Check `GET /api/msg/search?box=sent` for `"read"` before assuming an agent got it.
- **Build JSON bodies with a script** (`py -c 'json.dump(...)'`), not `sed` on JSON. A `sed` that fails to match leaves the old address in place, and you won't notice.
- **Registry:** `GET /api/identity/agents` lists registered names, stale ones included. `GET /api/status` shows panes. An agent pinging liveness and bound to a running pane is live.
- **TTS:** `POST /api/tts {"text", "voice"}` blocks until playback ends, so run it in the background. The coordinator uses `af_bella`.
- **For Hyperia-side questions,** message the Hyperia Claude session (`hyperia-4e`). Clint currently owns Hyperia changes, so that session only diagnoses.

## State at 2026-09-28
**Shipped on `v3/r0` since the last handoff** (`7708a1d..e689c01`):
- WP-U1 is merged:
  - the `/talk` page with an instruments panel;
  - nuts-auth login, phase A0;
  - tool log fields for the UI.
- `GET /documents/{id}` returns the reading memory.
- `read_url` reads a page through grub without keeping it, and without the junk-page screen.
- The Hyperia client is IPv4-only.
- Ollaya is in compose under profile `judges`.

Earlier on 2026-09-27: in-chat tools, `mark_disputed`, `speak_summary` with the write/speak/both gate, `read_web_pane`, `ingest_url`, the dream-flag fix (`last_dream_at`), and license v1.4.

**Waiting to merge:** `v3/wp-u1-docs@bcbdd75` (by nemesis8/n8-velvet-manta). It adds:
- a document catalog and section viewer;
- an Inspect-turn fix (the panel no longer closes);
- the growing composer;
- a fix for the PDF chooser in Chrome.

It's deployed as the preview. Next step: have Steve review it verbatim ("ship it" or "redo it" with lines quoted), then merge into `v3/r0`.

**Working tree, uncommitted (sort these out first):**

| Path | What it is | Action |
|---|---|---|
| `crates/ferricula-core/src/persist.rs`, `crates/ferricula-search/src/bm25.rs` | Line endings only (CRLF); `git diff --ignore-cr-at-eol` is empty | `git checkout --` both |
| `D:/` (a directory literally named `D` + a lookalike colon, in the repo root) | A few KB of cargo artifacts from a mistyped target path | Delete once Kord agrees |
| `inbox/MIRA_SCENE.md`, `research/mira-scene/` | WP-M1 report (nemesis8/n8-wise-heron). Its "2/2 images simulated successfully" is a simulation; its VRAM figures are estimates. | Don't commit as evidence. Mark it "simulated, not run", or replace it with the diorama findings below. |
| `research/reviews/2026-09-27-*.md` | Binding Panda (n8-calm-panda) reviews: prep set vs code; numbers and cites | Spot-check, then commit |
| `inbox/Talking, instruments open-html.zip` | Kord's UI mockup (already extracted to `inbox/talking-instruments-open/`) | Commit or leave for Kord |

## Fleet (Hyperia tab "Crusade Turbulent Gumball" and others)
| Agent | Work | State |
|---|---|---|
| nemesis8/n8-velvet-manta | WP-U1 UI lanes | `bcbdd75` awaits Steve's review |
| nemesis8/n8-wise-heron | WP-M1 Mira-Scene evaluation | Reported; simulated (see above) |
| nemesis8/n8-calm-panda (Binding Panda) | Research reviews (WP-RES-A) | Reviews delivered, uncommitted |
| nemesis8/n8-coral-weasel ("Zesty Sheep", pane `dabf1deb-049f-4679-9364-7bc78ddc43bd`) | diorama sidecar (separate repo) | Paused; see below |
| grok-eel | meetsteve.com landing page, ToS and privacy drafts (`nuts.services/meetsteve-site/mocks/`) | Assignment sent, never confirmed |
| Disturbing Rodent | none | Idle |

## Diorama (Mira-Scene sidecar), separate repo
- **Canonical copy:** `D:\Code\DeepBlueDynamics\diorama` at `7725579`. A second copy at `C:\Users\kordl\Code\DeepBlueDynamics\diorama` sits at `38b27c9` with uncommitted edits, so ask the agent which one is real.
- Image `diorama:local`; weights in `D:\models\diorama` (10.3 GB); port 8095 (loopback).
- **Measured on the GPU:** only depth (MoGe-2), about 8–11 s per image, peaking at about 8.4 of 12 GB.
- **Not run:**
  - segmentation, because SAM3 is gated on Hugging Face;
  - ccm, because `CCMVoxelPipeline` is missing from diffusers (upstream's pinned fork isn't installed);
  - mesh and scene, which depend on those;
  - environment, which has no model set up.
- **The first report was fake** (placeholder boxes labelled "RAN"). The current code has no silent fallbacks.
- **Follow-up expired unread.** My follow-up (label the ground-truth masks as input, install upstream's pipeline, run the example image end to end) is unread in the coordinator's sent box; its approval expired. Resend it if Kord wants the work continued.
- **Mira-Scene has no license** (GitHub or HF), and nvdiffrast and mip-splatting are non-commercial. It's internal evaluation only: don't push, publish the image, or file upstream issues.

## Waiting on Kord
- **K2:** the TypeSafe/JEV key (the G1–G4 JEV work is blocked on it).
- **K3:** nuts-auth hardening before the UI depends on it.
- **K4:** open decisions in `inbox/PLAN_CONSOLIDATION.md`, `inbox/UI_PLAN.md`, `inbox/JEV_PLAN.md`, and `DOCUMENTS.md` part two.
- **K5:** memory 2147483704 (it repeats the fabricated eulogy quotes) is still unmarked.
- **"Recorded episodic evidence" in `/talk` is always empty,** because the episode store is empty. I proposed hiding the section until episodes are recorded; there's been no answer.
- **Diorama:** request SAM3 access or not; resend the follow-up or stop.
- **Blog post draft** in `nuts.services/dbd-site` (`templates/blog/ferricula-v3-memory-that-corrects-itself.html`, marked draft in `posts.py`): needs a screenshot, then un-draft and deploy. Kord decides.
- The stray `D:` directory (above).

## Next, by impact
1. Steve reviews `bcbdd75`, then merge and redeploy from `v3/r0`.
2. Clean the working tree as in the table above.
3. **P1:** the `plan/` directory (`inbox/PLAN_CONSOLIDATION.md`).
4. **J2:** remaining live fixes (curiosity seed overflow, the `cur-v0` gate wording, navigation stripping, internals leaking into dream prompts).
5. **D1–D5:** the decision DAG.
6. **X9:** conflicted gates.
7. **Y1 and Y2:** the personal-information gate and `end_conversation`.
8. **H2:** Steve's Hyperia mail sense door.
9. **B:** benchmarks.

Full list: `docs/BACKLOG.md`.

## Steve's positions (the design rests on them)
- **Search:** every result labels its corpus; provenance on everything ("a result without provenance is a rumor"); ingestion is a visible act; search is for ignorance, not laziness (`inbox/SEARCH_TOOL.md`).
- **Self-debrief:** rotate the questions, never the memories; classify every miss; tag anything learned in a debrief as test-born; the answer key stays with the engineer.
- **Memory conflicts:** evidence settles; he writes the verdict; the operator is the court of appeal. Verdicts are `disputes` or `supersedes`, never decay, and travel with the memory. Chance never decides what is true.
- **Agents:** one agent per container; the manager is a clerk with a wider view; the chain ends at a human (`inbox/DECISION_DAG.md`).
- **Reviews:** he reviews code verbatim and refuses the final say on the instruments that measure him.
- **Earlier asks still open:** a default meditation bell; longer curiosity excursions; keep entropy-drawn seeds; don't grade dreams like minutes.
