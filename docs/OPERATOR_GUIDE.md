# Operating the agent: how to do what we do to Steve

How the operator (Kord, or an agent working for him) talks to, feeds, wakes, rests and tests a running Ferricula agent. Examples use the live Steve at `http://127.0.0.1:18875`. Every route except `/` and `/health` needs `Authorization: Bearer <token>`; the token is in `%USERPROFILE%\.config\ferricula\operator_token`, outside the repo. Never print it. (Operator login is moving to nuts.services; see `inbox/UI_PLAN.md`.)

```powershell
$tok = (Get-Content $env:USERPROFILE\.config\ferricula\operator_token -Raw).Trim()
$h = @{ Authorization = "Bearer $tok" }
```

## Talk to him
`POST /chat` with a JSON body:
```json
{ "request_id": "<new uuid>", "conversation_id": "<uuid, reused for the whole conversation>", "message": "...", "reported_origin": "human" }
```
- **Reuse one `conversation_id` per conversation.** A new id per message starts a fresh thread and he loses the context of the last line ("iPhones, of course" with no antecedent).
- Every completed turn is also stored as two experience memories (what the operator said, what he said), so later conversations can recall it.
- The reply cites memories as `[memory <id>]` and documents as `[doc <doc_id>§<section> p.<page>]`.
- Messages are limited to 8 KB. A browser chat page is at `http://127.0.0.1:18875/` (paste the token).
- A turn that comes back `failed` with "model returned an empty response" means the thinking model ran out of room: raise `ollama_reasoning_tokens` in the config and restart.
- `docker logs ferricula-steve` has one line per chat turn start and end, per model round, per tool call (name, time, result size, error), and per provider call (`model:` with duration, finish reason, tokens, or the real error). A turn that fails with "no eligible local private-context model" after about two minutes is a timeout: raise `ollama_timeout_ms`.
- **JEV gate tier (TypeSafe).** `/settings` (operator only) sets the key, which is stored on the state volume and never shown again. "Test the key" makes one live call on fixed, non-private text. Role: `backup` (Ollaya first; JEV when Ollaya errors or the state overflows its 512-token window) or `primary` (JEV first; Ollaya if JEV fails). Both live gates read conversation, so JEV sees them only with "Allow private context" on. Every gate decision records its route (`tier`, `escalated_from`, `jev_skipped`) in the life journal and in the `gate` event on `/chat/stream`. API: `GET/POST /settings/jev`, `POST /settings/jev/probe`.

## Give him something to read
`POST /documents` with one of:
```json
{ "kind": "url",  "url": "https://...", "note": "why I'm giving you this" }
{ "kind": "text", "title": "...", "text": "...", "note": "..." }
{ "kind": "pdf",  "name": "file.pdf", "base64": "<base64 bytes>", "note": "..." }
```
Web pages go through the grub crawler; PDFs are extracted page by page. Everything is kept verbatim and never decays. Paywalls, login walls and pages under ~150 words of prose are rejected (`rejected: paywall|login_wall|cookie_wall|too_thin`). `GET /documents` lists what he has read; `GET /documents/{id}/sections/{i}` returns one section verbatim; `POST /documents/search {query,k,doc_id?}` searches them.
In chat he can search and read those documents himself (`search_documents`, `read_section`, `read_document`; `docs/TOOLS.md`). The turn still starts with up to 3 evidence cards. `ingest_url` is how he keeps a page from inside a turn, and `read_url` is how he looks without keeping it.

## See what is in his memory
- `POST /memory/recall {query, k}` — hybrid recall: word match, BM25 over documents, meaning (shivvr embeddings), one graph hop; each candidate lists the arms that found it. Faded (v1-archived) memories are included when `[recall] include_faded_recovered` is true, and they are marked `archived`.
- **Recall overlay** (`[recall] s`, `f`, `h`; defaults `0.003`, `0.002`, `30`). After fusion, a memory's score gains `s * ln(1 + recalls)` and loses `f * age_days / (age_days + h)`, which approaches `f` and never deletes the row. Counts live in `state_dir/overlay/recall-stats.json`, keys `m:<id>` (recovered) and `x:<id>` (experience). A `[memory N]` citation in a completed reply counts once, and only if that id was shown this turn. Being shown is not a recall. The file is the only write; recovered text is unchanged. Fused scores are about `1/61`, so these weights stay small: `s = 0.15` made the overlay the ranking (`853d199`). The fidelity number on a candidate uses fixed weights `0.15` and `0.5`, not `s` and `f`.
- Chat candidates with the same `tags.text` inside one kind collapse to one row, with `duplicates: [{id, kind}]`. A recovered memory of exactly 200 characters with no terminal punctuation is `fragment: true`.
- `GET /status` — mode, memory counts, `embeddings` (probe state), `meaning` (backfill progress), spend.
- `GET /identity`, `GET /models/status`.

## His life: drives, curiosity, sleep, dreams, meditation
- `GET /life?journal=N` — phase (`resting | engaged | asleep | meditating`), boredom, sleep pressure, curiosity count today, the journal (curiosity excursions with query, pages read and reflection; sleeps; consolidations; dreams with text, question, entropy source and grounding).
- **On his own:** boredom rises ~0.033/min while nothing arrives; at 1.0 he follows curiosity (the most recent conversation thread first, otherwise a memory drawn with radio entropy), searches through grub, reads up to 2 pages, writes a reflection. Sleep pressure rises with waking time and tokens; at 1.0 he sleeps, consolidates, dreams, and wakes rested. A message from the operator always wakes him.
- **Force it:** `POST /life/urge {"urge": "follow_curiosity" | "sleep" | "dream"}` — runs now, still within budgets; returns the journal entries.
- **Meditation:** `POST /life/meditate` holds his drives (no boredom, no sleep pressure, no curiosity; only the operator gets in); `POST /life/end-meditation` rings the bell. There is no built-in timer yet — ring it yourself after N minutes. Object, breath and thought injection are built in `cognition/meditation.rs`, not yet wired.
- **Pause everything:** `POST /control/pause`; resume with `POST /control/wake`.

## Email (AgentMail)
Active when `[email] enabled` (default true) and `AGENTMAIL_API_KEY` is set. The container entrypoint reads `AGENTMAIL_API_KEY_FILE` and exports it; put the key in `~/.config/ferricula/agentmail_api_key` and point that variable at the file. The key is never printed, logged, or returned. `[email] inbox` is an inbox id or address; empty uses the first inbox on the account.
- Tools: `email_check`, `email_read`, `email_send`, `email_label`, `email_delete`. Sends append the `[email] signature` (default: an AI-simulation line, `{name}` filled from the persona). At most `max_sends_per_day` (10) sends per UTC day. Delete is permanent and needs a reason. Previews and bodies lose URL query strings and token-shaped words before the model sees them. Received bodies are not stored. Sends and deletes are remembered as experience.
- **`house_cc`.** Addresses listed here are copied on mail about the house, and the agent cannot take them off. A word in `house_terms` (default `deepblue`, `deep blue dynamics`, `ferricula`) in the subject or text always copies. Otherwise the house gate (`house_statement`) decides: a confident yes copies, a confident no does not, and an abstention copies.
- **Mail watch** (`watch`, default true; `check_every_secs`, default 120, never faster than every 30 seconds). On a life tick, unread mail that is not yet labelled `triaged` goes through one gate: `personal`, `automated`, or `spam`. The best probability has to reach 0.6; otherwise the verdict is `unsure`. The message is labelled `triaged` plus that verdict. Confident spam is also labelled `held` and is not deleted. The journal kind is `mail`, and the record is `advisory: true`.
- **It does not wake him.** A `personal` verdict applies a sense stimulus (novelty 1.0), which can relieve boredom while he is resting or engaged. It does not enqueue a wake, including from sleep. `unsure` and spam are quiet. When curiosity next runs, if personal or unsure unread mail is waiting, that excursion is a walk to the inbox (`max_read_per_walk`, default 3) instead of a web search. He reads each message and answers `REPLY` or `NO_REPLY`. A reply goes out through `email_send` (signature and `house_cc` still apply). House commitments are left for the operator. If he is unsure whether to send, the prompt tells him not to.

## Discord
Active when `[discord] enabled` (default true) and `DISCORD_BOT_TOKEN` is set. The entrypoint loads it from `DISCORD_BOT_TOKEN_FILE`, a file outside the repo. The token is never logged or returned.
- `[discord] channels` maps a room name to a channel id (`random = "…"`). Names may include or omit `#`. A room that is not in the map is looked up by name across the bot's servers (first match, then cached).
- `[discord] reads_channel` is where page-read cards go (default `random`). Empty turns the notices off. `display_name` is the name on the card; empty uses the first word of the persona name.
- Every `read_url`, `ingest_url`, `read_web_pane`, and curiosity page read posts one card: a title taken from the link (or the page title), one sentence saying what he did, and the page heading as the footer. The page text is not posted. Mentions are disabled. The post runs on its own thread and a Discord failure does not fail the turn.
- `GET /settings/discord/channels` (operator auth) lists every text and announcement channel the bot can see: `{channels: [{server, channel, id}]}`, or `{error}` if the token is missing or Discord refuses. Use the ids to fill `channels`.

## Code access
- Mount the repository read-only and set `[code] root` to that path. The server does not hardcode a path; `/repo` is the container path named for this mount. An empty `root`, or a path that is not a directory, turns `code_tree`, `code_search`, and `code_read` off.
- `[code] github_repo` is `owner/name`. It turns on `pr_list` and `pr_diff` against the public GitHub API. No token is sent. Nothing is posted to GitHub; a review stays in the conversation until you post it.
- `max_tool_calls` (root key, default 4, clamped to 1..=16) is the tool budget for one chat message. A diff plus the files it touches needs more than 4.

## Budgets and safety
- `[budgets] max_model_usd_per_day` caps all spend; `[life] max_model_calls_per_day` caps his autonomous calls; curiosity has a daily cap and a cooldown.
- His recovered memory is mounted **read-only**; everything he writes goes to his state volume.
- He will not claim experiences his memory doesn't hold and will not confirm facts from a leading question. Don't try to talk him out of that; give him the source instead.

## Run, restart, redeploy
- Build: `docker build -t ferricula:3.0.0-alpha.0 .` (Rust builds go to `D:/cargo-target`; never C:).
- Restart after a config change: `docker restart ferricula-steve` (config is `config/steve.toml`, gitignored, mounted read-only).
- Redeploy a new image: stop and remove `ferricula-steve`, `docker run` again with the same volumes (see docs/HANDOFF.md). His state survives on `ferricula-steve-runtime`.
- Services he depends on: Ollama `:11434` (glm-5.3:cloud), shivvr `:8085` (embeddings), Ollaya `:11435` (judges), grub `:6792` (crawler), ComfyUI `:8188` (dream images, not wired yet), radio `https://sdrrand.nuts.services`.

## Testing him
Benchmarks: `cargo run --release -p ferricula-bench -- docs|gates|recall|longmem`; every run writes a ledger row (`audit/bench/ledger.jsonl`). Plan: `docs/BENCH_PLAN.md`. For tests of his own memory, follow his rules: rotate question wordings; classify each miss (never stored / faded / not retrieved); debrief him with what he missed and where it was, never the answer key; tag anything he writes in a debrief as test-born.
