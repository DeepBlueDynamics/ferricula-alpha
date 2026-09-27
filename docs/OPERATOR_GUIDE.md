# Operating the agent: how to do what we do to Steve

How the operator (Kord, or an agent working for him) talks to, feeds, wakes, rests and tests a running Ferricula agent. Examples use the live Steve at `http://127.0.0.1:18875`. Every route except `/` and `/health` needs `Authorization: Bearer <token>`; the token is in `secrets/ferricula_operator_token` (gitignored). Never print it.

```powershell
$tok = (Get-Content secrets\ferricula_operator_token -Raw).Trim()
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

## Give him something to read
`POST /documents` with one of:
```json
{ "kind": "url",  "url": "https://...", "note": "why I'm giving you this" }
{ "kind": "text", "title": "...", "text": "...", "note": "..." }
{ "kind": "pdf",  "name": "file.pdf", "base64": "<base64 bytes>", "note": "..." }
```
Web pages go through the grub crawler; PDFs are extracted page by page. Everything is kept verbatim and never decays. Paywalls, login walls and pages under ~150 words of prose are rejected (`rejected: paywall|login_wall|cookie_wall|too_thin`). `GET /documents` lists what he has read; `GET /documents/{id}/sections/{i}` returns one section verbatim; `POST /documents/search {query,k,doc_id?}` searches them.
**Limit today:** in chat he only sees the ~3 sections that match the message; he cannot search or read a whole document himself yet (backlog N0, docs/HANDOFF.md Job 1). Ask narrowly, or quote the part you want him to look at.

## See what is in his memory
- `POST /memory/recall {query, k}` — hybrid recall: word match, BM25 over documents, meaning (shivvr embeddings), one graph hop; each candidate lists the arms that found it. Faded (v1-archived) memories are included, marked `archived`.
- `GET /status` — mode, memory counts, `embeddings` (probe state), `meaning` (backfill progress), spend.
- `GET /identity`, `GET /models/status`.

## His life: drives, curiosity, sleep, dreams, meditation
- `GET /life?journal=N` — phase (`resting | engaged | asleep | meditating`), boredom, sleep pressure, curiosity count today, the journal (curiosity excursions with query, pages read and reflection; sleeps; consolidations; dreams with text, question, entropy source and grounding).
- **On his own:** boredom rises ~0.033/min while nothing arrives; at 1.0 he follows curiosity (the most recent conversation thread first, otherwise a memory drawn with radio entropy), searches through grub, reads up to 2 pages, writes a reflection. Sleep pressure rises with waking time and tokens; at 1.0 he sleeps, consolidates, dreams, and wakes rested. A message from the operator always wakes him.
- **Force it:** `POST /life/urge {"urge": "follow_curiosity" | "sleep" | "dream"}` — runs now, still within budgets; returns the journal entries.
- **Meditation:** `POST /life/meditate` holds his drives (no boredom, no sleep pressure, no curiosity; only the operator gets in); `POST /life/end-meditation` rings the bell. There is no built-in timer yet — ring it yourself after N minutes. Object, breath and thought injection are built in `cognition/meditation.rs`, not yet wired.
- **Pause everything:** `POST /control/pause`; resume with `POST /control/wake`.

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
