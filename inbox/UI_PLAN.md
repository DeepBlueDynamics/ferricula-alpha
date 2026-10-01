# UI plan: "While he thinks"

_Status: proposed, 2026-09-27. Source: `inbox/While he thinks-html.zip` (local only since 2026-09-28; the extracted, redacted copy is `inbox/while-he-thinks/`) (`Thinking.dc.html`, a design-tool export; the README says to replicate its values, not copy its markup). This plan covers the conversation page shown while the agent works on a turn. The instruments view (`Main.dc.html`, linked from the header) is not in the zip._

## 1. The design

### Tokens (from `Thinking.dc.html` inline styles)
| Token | Value | Used for |
|---|---|---|
| `--paper` | `#F6F4EF` | page background |
| `--ink` | `#1B1B19` | text, completed stage segments, Stop button border |
| `--muted` | `#5E5B55` | labels, metadata, rail captions |
| `--rule` | `#DDD8CE` | header rule, card border, pending stage segments |
| `--rule-soft` | `#EEEBE4` | card inner divider; instruments rail background |
| `--input-rule` | `#CFCAC0` | message box border |
| `--card` | `#FFFFFF` | recall card, message box |
| `--accent` | `#2A5BA8` | status dot, active stage, candidate rings, caret; links |
| `--accent-deep` | `#1C4180` | active stage label, link hover |
| `--accent-wash` / `--accent-edge` | `#EEF2F9` / `#C9D5EA` | status pill fill / border, active stage remainder |
| `--tired` | `#A6541A` | sleep-pressure gauge |
| `--warn` | `#7A3A0E` | premise warning |
| `--thought` | `#3C3A35` | italic "looking through…" line |

Fonts (Google Fonts): **EB Garamond** 400/500/italic for the agent's words (20px/1.6), the wordmark (26px/500) and the message box (19px); **IBM Plex Sans** 400/500/600 for interface text (operator messages 16px/1.55, speaker labels 12px/600 uppercase +0.06em, metadata 13px, captions 11–12px); **IBM Plex Mono** 11px for provenance tags and rail counts.

Geometry: artboard 1440×960; header 68px, padding 0 32px, bottom rule; conversation column 640px centred, padding 48px, 32px between turns; earlier turns at opacity 0.55; card radius 12px, padding 16×18; message box radius 16px, Stop and "Show instruments" 44px high, radius 12/10px; pill radius 999px; stage bar 6 segments, 3px high, gap 6px; rail 72px wide, gauges 6×64px radius 3px.

### Structure
1. **Header**: wordmark ("Steve", from the persona), the disclosure *"A simulation built from recovered memory. Not the living person."*, a status pill (dot + phase word), a "Show instruments" button.
2. **Conversation column**: past turns dimmed; the live turn with a speaker label, then:
   - **Stage bar**: contact · feeling · recognition · remembering · deciding · speaking; done segments ink, current segment partially filled in accent, pending in rule colour; current label accent-deep 600.
   - **Recall card**: italic "Looking through what I hold about …", candidate rows (ring bullet, quoted text, right-aligned mono provenance such as `recovered · found by meaning`, `recovered · fidelity 0.38`), a "N more candidates, still reading" row, and a divider with the **premise warning** (triangle icon, warn colour).
   - **Answer**: Garamond text streaming in with a 2px accent caret.
3. **Composer**: message input and a **Stop** button (replaces Send while a turn runs); under it the **judge line**, e.g. "Judge was unsure (p 0.52), so he's thinking it through instead of answering fast."
4. **Instruments rail (collapsed)**: *bored* and *tired* vertical gauges, *recalled* and *searches* counts.

### Needed from the designer
`Main.dc.html` (instruments expanded), plus: the idle and completed states of a turn (does the recall card collapse after the answer? do tool calls show?); failed / cancelled turn states; how `verdicts` ("disputed, see memory X"), document cites (`[doc …§n]`) and dreams are shown; a narrow (phone) layout; dark mode; the signed-out / "not an operator" / "nuts-auth unreachable" screens (§3a); asleep / meditating / dreaming states of the pill and page.

## 2. Element-by-element data

| Element | Exists today | Missing |
|---|---|---|
| Wordmark, disclosure | persona name via `GET /identity` (`api.rs` route list, `/identity`) | disclosure string: make it config (`[persona] disclosure`), not hard-coded |
| Status pill | `GET /life` → `phase`, `mode` (`life.rs:420-445`, served at `api.rs` `life_status`) | a "thinking" state while a turn runs (the page knows this locally; the stream makes it explicit) |
| Past turns | `GET /chat/{conversation_id}` returns up to 100 turns (`api.rs` `conversation`) | — |
| Stage bar | nothing live; `POST /chat` returns only the finished `ChatTurn` (`api.rs` `chat`) | per-stage events (§3). Real stage records need `VithiBuilder` (`cognition/src/vithi.rs:183-234`, backlog X2) wired into `converse` |
| Recall card rows | per turn: `memory_candidates` with `arms`, `state`, `fidelity`, `kind`, `dense_score`, `verdicts` (`chat.rs:28`, `chat.rs:140-147`); `MemoryHit` fields incl. `created_at` (`memory.rs:17`) | streaming them as found; a human provenance line (`recovered · found by meaning` from `kind` + `arms`) |
| "N more candidates" | candidate count in the turn | live count |
| Document cites | `document_evidence` shown cards (`chat.rs:35`) and tool results | design for them (ask designer) |
| Tool calls ("searches") | `tool_calls` log per turn (`chat.rs:44`); loop at `chat.rs:313-337`, max 4 (`chat_tools.rs:13`) | live events |
| Verdicts | `verdicts` on candidates (`chat_tools.rs:423`) and `mark_disputed` results (`chat_tools.rs:446`) | design |
| Premise warning | nothing | a false-premise gate (§4) |
| Judge line | nothing in chat (the only live gate is life curiosity, `life.rs:590`) | a "worth deliberating?" gate (§4) |
| Streaming answer | nothing: `OpenAiChatRequest` has no `stream` field (`model.rs:146-153`); transport is blocking reqwest reading one JSON body (`model_transport.rs:5`, `:41-64`) | provider streaming (§3) |
| Stop | nothing; one turn at a time via a semaphore (`chat.rs:58`, `:122`) | cancellation (§3) |
| bored / tired gauges | `GET /life` → `boredom`, `sleep_pressure` (`life.rs:440-442`) | — |
| recalled / searches counts | derivable from the turn (`memory_candidates`, `tool_calls`) | — |

## 3. Architecture

### Serving
Today `/` returns `include_str!("chat.html")` (`api.rs:253-255`). Add the new page the same way (`/talk`, static HTML + inline JS, fonts from Google Fonts), keep `/` until it is retired. No build step, no framework: the mockup's React runtime is the design tool's, not ours.

### Turn event stream
`POST /chat/stream` with the same `ChatRequest` body as `/chat`, responding `text/event-stream`. Events, in order, each JSON with `request_id` and `t` (ms since start):

| Event | Payload | Emitted where |
|---|---|---|
| `accepted` | turn id, conversation id | after admission (`chat.rs:122`) |
| `stage` | `{stage: contact|feeling|recognition|remembering|deciding|speaking, pali, progress 0..1, signal}` | see stage map |
| `candidate` | one memory candidate as stored in `memory_candidates`, incl. `verdicts` | after `hybrid_recall_async` (`chat.rs:128`), one per candidate |
| `document` | one evidence card | after `search_documents` (`chat.rs:165`) |
| `curator` | `{ok, skipped?}` | curator briefing (when enabled) |
| `round_start` | `{round, prompt_tokens_est, tool_calls_left}` | each tool-loop round |
| `model_call` | `{round, attempt, ok, profile, model, secs, finish, output_tokens, reasoning_budget, max_tokens, prompt_tokens_est, reply_bytes}` or `{round, attempt, ok: false, secs, error}` | every provider call, including the empty-reply retry (`attempt` 2) |
| `round` | `{round, secs, finish, reply_bytes, output_tokens, tool_calls: [names]}` | end of each round |
| `gate` | `{gate, kind: deterministic|ollaya, verdict, advisory?, ...}`. Built: `citation_check` (`pass`, `sent_back`, `let_through_after_one_correction`, with `cited`/`unseen`) and `speak_modality` (Ollaya write/speak/both, `detail` has the probabilities and threshold). | tool loop; later the §4 gates |
| `nudge` | `{round, reason}` | empty reply mid-loop |
| `judge` | `{gate, p, verdict: fast|think|abstain, reason}` | §4 |
| `premise_warning` | `{p, text}` | §4 |
| `tool_call` / `tool_result` | name, arguments; cites, sizes, error (same shape as the `tool_calls` log) | tool loop (`chat.rs:313-360`) |
| `verdict` | a `mark_disputed` result | tool loop |
| `token` | `{text}` | final round only (below) |
| `done` | the full `ChatTurn` | end |
| `failed` / `cancelled` | bounded error | end |

**Built 2026-09-28 (server side):** `POST /chat/stream` (SSE; each message is named for its event and carries one JSON object) with `accepted`, `candidate`, `document`, `curator`, `round_start`, `model_call`, `round`, `tool_call`, `tool_result`, `gate`, `verdict`, `nudge`, `done`/`failed`. Not yet: `stage`, `judge`, `premise_warning`, `token` (needs provider streaming), `cancelled`. Gates outside a turn (the life curiosity gate) are in `GET /life` journal entries under `gate`, with `verdict`, `provenance` and `advisory`. A thinking-model round can run for minutes; the `model_call` and `round` events are what make that wait visible.

The durable turn is unchanged: `/chat` keeps working, and the stream is a view of the same `converse` run. Implementation: `converse` takes an optional `tokio::sync::mpsc::Sender<TurnEvent>`; `/chat` passes `None`.

**Auth.** See §3a. The page authenticates with ferricula's own session cookie (set after a nuts-auth login), so `fetch('/chat/stream', {method: 'POST', credentials: 'same-origin'})` carries it automatically; the page reads `response.body` as a stream and parses SSE frames itself (`EventSource` can't POST or send headers). Scripts and agents send `Authorization: Bearer <nuts-auth JWT or ahp_ token>` instead. No tokens in query strings. If a GET endpoint is ever needed (reconnect to a running turn), mint a single-use ticket from an authenticated POST (`/chat/ticket`, 60 s, bound to one request id).

**Provider streaming.** Add `stream: true` to `OpenAiChatRequest` for the Ollama profile and read the chunked `data:` lines. Two constraints:
1. **Tool-call text must never reach the operator.** A round is streamed only once it is known to be an answer: buffer each round's content until the first 64 bytes contain no `<use_tool>`/`<tool_call>` opener, then flush and stream the rest; if an opener appears later in the round, stop streaming, discard the partial answer on screen (send `token_reset`), and run the tool. Structured `tool_calls` deltas (Ollama/GLM, `model_transport.rs` `native_tool_calls`) mark the round as a tool round.
2. **The citation check** (`chat.rs` `citation_check`) runs on the whole reply. A reply that fails it has already been streamed; send `token_reset` and stream the corrected round. Alternative (simpler, recommended for phase 2): stream only rounds after the last tool result and keep citation-check replies buffered.
Usage accounting stays per round (streamed responses carry `usage` in the last chunk on Ollama's OpenAI endpoint; if absent, fall back to the estimate path at `model.rs:725`).

The chat runs inside `spawn_blocking` with blocking reqwest; streaming needs either a blocking line reader that pushes into the `mpsc` sender (smallest change) or an async transport. Start with the blocking reader.

**Stop.** `POST /chat/{request_id}/cancel` sets a cancel flag checked between rounds and between streamed chunks (dropping the provider connection). Semantics, decided here:
- the turn is stored with status `cancelled`, the partial reply kept on the durable turn as `partial_reply`, and marked as not shown to the agent as its own words;
- **nothing is remembered** as experience (`remember_turn` at `chat.rs:406` is skipped): an unfinished answer is not something the agent said;
- any `mark_disputed` verdict already written in that turn **stays** (it was a completed, logged act) and is listed in the cancel response;
- the life stimulus `Settled` still fires.

### 3a. Login through nuts.services (replaces the pasted operator token)

**Today.** Every operator route and MCP call compares the bearer string, in constant time, with one environment variable (`authorize`, `crates/ferricula-server/src/runtime.rs:868-879`; `operator_token_env` default `FERRICULA_OPERATOR_TOKEN`, `config.rs:24,177`; MCP uses the same check, `mcp.rs:118`). The container entrypoint turns `FERRICULA_OPERATOR_TOKEN_FILE` into that variable (`Dockerfile:77`), fed from `secrets/ferricula_operator_token` (`compose.yaml:58`, `compose.isolated.yaml:35`, and the live `docker run`); `chat.html` makes the operator paste it (`chat.html:23-27,102`). Kord is removing `secrets/`.

**What nuts-auth provides** (`nuts.services/nuts-auth`):
- **Browser login:** `GET https://auth.nuts.services/login?return_url=<callback>` (email magic link, Google or GitHub; `docs/AUTH_INTEGRATION.md:26-42`). It returns to `<callback>?token=<JWT>` (email `web/routes/auth.py:100-103`; Google `web/routes/oauth.py:78-80`; GitHub `oauth.py:147-149`).
- **The JWT:** RS256 (`core/lib/jwt.py:15`), claims `sub` (email), `user_id` (UUID), `name`, `iat`, `exp` (`auth.py:96`; `jwt.py:53-62`). The default lifetime is 30 minutes (`jwt.py:16`). **No `iss` and no `aud` claim**, and there is no refresh token.
- **Public key:** `GET /.well-known/jwks.json` (`web/routes/jwt.py:65-70`), one key with fixed `kid` `nuts-auth-key-1` (`jwt.py:82`). Rotating it invalidates every session (`docs/runbook-jwt-keys.md:121-125`).
- **Machine tokens:** long-lived `ahp_` API tokens. `POST /auth` exchanges one for a 30-minute JWT carrying `sub`, `user_id` and `scopes: ["read","write"]` (scopes are a TODO; `web/routes/jwt.py:30-63`). `POST /api/validate` checks one directly, returning `valid`, `subject`, `actor`, `user_uid`, `token_uid` (`app.py:76`; `AUTH_INTEGRATION.md:155-174`).
- **Reference verifier:** nuts-news already accepts "nuts-auth JWT or `ahp_` token" in Rust. JWTs are checked locally against the JWKS with `jsonwebtoken` RS256 and `validate_exp`; `ahp_` tokens round-trip to `/api/validate` (`nuts-news/src/auth.rs:1-4,80-99,101-110,113-147,155-162`). Ferricula ports this module; `jsonwebtoken` 9 matches (`nuts-news/Cargo.toml:16`).

**Browser flow for `/talk`:**
1. No valid session cookie → the page links to `https://auth.nuts.services/login?return_url=<origin>/auth/callback`. For Steve, `<origin>` is `http://127.0.0.1:18875`.
2. `GET /auth/callback?token=…` (new, server-side) verifies the JWT and checks the operator allowlist. It then creates a **ferricula session**: an opaque random id held server-side with `{user_id, email, name, via: "nuts-auth", jwt_exp}`, set as a cookie `ferricula_session` (HttpOnly, SameSite=Strict, Path=/, Secure when served over https). Finally it redirects with **303 to `/talk`**, so the JWT leaves the address bar and browser history immediately. Nothing is written to `localStorage` (AUTH_INTEGRATION's example does that, `:84`; we don't).
3. **Session lifetime:** the nuts-auth JWT expires after 30 minutes and has no refresh. The ferricula session therefore has its own lifetime, `[auth] session_hours` (proposed default 12; Kord decides). Sessions are stored under `state_dir/auth/sessions.json` so a restart doesn't log the operator out. `POST /auth/logout` deletes the session.
4. **CSRF:** cookie-authenticated POSTs (`/chat`, `/chat/stream`, `/control/*`, `/documents`, cancel) must carry an `Origin` equal to the server's own origin, and content type `application/json`. SameSite=Strict covers the rest.

**Verification in ferricula-server** (new `auth.rs`, ported from nuts-news):
- RS256 only, `exp` enforced with 60 s leeway; `user_id` and `sub` required.
- **Issuer and audience cannot be checked today**, because nuts-auth doesn't set `iss` or `aud` (`jwt.py:53-62`). So a JWT minted for any nuts service is valid here; the allowlist below is the real gate. Ask nuts-auth to add `iss = "https://auth.nuts.services"` and a per-service `aud`. Ferricula enforces each claim as soon as it's present (`[auth] require_iss` / `require_aud`, off until then).
- **JWKS:** fetched at startup and cached on disk at `state_dir/auth/jwks.json` (a public key, not a secret). Refetched when a `kid` doesn't match, and at most once per 10 minutes after a signature failure. `[auth] jwks_url` defaults to `https://auth.nuts.services/.well-known/jwks.json`.
- **`ahp_` tokens:** validated via `POST /api/validate` (`[auth] validate_url`). Positive results are cached in memory for `[auth] ahp_cache_minutes` (proposed 10), keyed by a SHA-256 of the token, never the token itself.
- **Recording who did it:** every authenticated request records `{user_id, actor?, via: session|jwt|ahp}`. The durable chat turn gains `authenticated_as` alongside the caller-claimed `reported_origin` (`chat.rs:13-18`).

**Who counts as the operator** is config, not secrets:
```toml
[auth]
mode = "nuts"                       # "static" | "both" | "nuts" (transition below)
operators = ["<Kord's nuts-auth user_id UUID>"]
# Optional: agent tokens by actor (ahp_ tokens minted under an allowed user carry `actor`)
agents = [{ actor = "claude-code", role = "operator" }, { actor = "bench", role = "reader" }]
```
- Match on `user_id` (a stable UUID; `AUTH_INTEGRATION.md:190-193`), not on email.
- A valid login that isn't on the list gets 403 and a "not an operator" page. The token is not stored.
- `reader` (read-only routes: `/chat/{id}`, `/life`, `/status`, `/documents` GET) is a proposed second role for benchmark and observer scripts.

**When nuts-auth is unreachable:**
- Existing ferricula sessions keep working until they expire; cookies are checked locally.
- Bearer JWTs keep working against the cached JWKS until they expire.
- Cached `ahp_` validations keep working until the cache entry lapses. Then `ahp_` calls fail with 503 "nuts-auth unreachable".
- New browser logins are impossible.
- **Break-glass:** `ferricula-server login-link` run inside the container (`docker exec`) prints a one-time local URL. It carries a random 128-bit nonce, is valid for 5 minutes, and creates an operator session for `operators[0]` with `via: "break-glass"`, logged. Trust comes from shell access to the host; no file on disk.
- `/health` stays public (`api.rs` `health`).

**Machine access** (the MCP bridge, Claude sessions, benchmark runners):
- **Preferred:** mint one `ahp_` token per agent at `https://auth.nuts.services/tokens` (`AUTH_INTEGRATION.md:93`) with a distinct actor name. The agent then either sends it as `Authorization: Bearer ahp_…`, which is simplest but costs a validate round trip per cache miss, or exchanges it at `POST /auth` for a 30-minute JWT and refreshes that, which works offline against the cached JWKS but drops `actor` (`jwt.py:56-57`).
- **Where the token lives:** outside the repo, e.g. `%USERPROFILE%\.nuts\ahp_<actor>` or an environment variable. `scripts/ferricula_mcp_bridge.py:13,50` already reads a token file path; point it at that location and rename the variable to `FERRICULA_TOKEN_FILE` (JWT or `ahp_`).
- **The server itself needs no secret for inbound auth.** Outbound tokens (e.g. `NUTNEWS_TOKEN`, `Dockerfile:77`, `compose.yaml:61`) are separate and out of scope here.

**Transition away from `FERRICULA_OPERATOR_TOKEN_FILE`:**
- **A0: verifier lands, `mode = "both"`.** The static token and nuts-auth both work. Add `/auth/callback`, `/auth/logout`, sessions, and the allowlist. `/talk` uses login; `/` (`chat.html`) keeps its paste box.
  Exit: Kord logs in with Google at `/talk` and his `user_id` is recorded on the turn. A JWT from another user gets 403. An expired JWT gets 401. With the JWKS fetch blocked, the cached key still verifies.
- **A1: machines move.** Mint `ahp_` tokens for the MCP bridge and for Claude's scripts (smoke and redeploy helpers). Store them under the user profile. Remove the token-file mount from the `docker run`/compose recipe.
  Exit: the MCP bridge and the smoke test work with an `ahp_` token. Turns show `actor`. `grep -r ferricula_operator_token` finds only docs history.
- **A2: static token removed, `mode = "nuts"`.** Delete `operator_token_env` and the constant-time compare (`runtime.rs:868-879`, `config.rs:24,177,291`), drop `FERRICULA_OPERATOR_TOKEN` from the entrypoint list (`Dockerfile:77`), remove the compose entries (`compose.yaml:58`, `compose.isolated.yaml:35`) and the paste box in `chat.html`. Update README/STEVE_RUNTIME/OPERATOR_GUIDE.
  Exit: no code path reads an operator secret. The container starts with no secret mounts.
- **Order matters for the live agent.** The running `ferricula-steve` has the static token loaded as an environment variable, so deleting `secrets/` doesn't affect it until it's recreated. After deletion, though, the current redeploy recipe's bind mount of `secrets/ferricula_operator_token` would fail or turn into an empty directory. The session's helper scripts, and the MCP bridge, also read that file. Delete `secrets/` after A1, or at the same time as switching the recipe to A0 with a temporary token passed from outside the repo.

**Things to raise with the nuts-auth side** (not ferricula changes):
- No `iss`/`aud` on JWTs (`jwt.py:53-62`).
- `return_url` isn't allowlisted: any URL receives a fresh JWT (`auth.py:100-103`, `oauth.py:42,78,101,147`). This is an open redirect that hands out tokens.
- The full redirect URL, token included, is logged (`auth.py:103`, `oauth.py:79,148`).
- The JWT travels in a query string.
- There's a single fixed `kid`, so key rotation logs everyone out.

**Assumptions to confirm:**
- (a) nuts-auth accepts an `http://127.0.0.1:18875` return URL. The code does no validation, so it should.
- (b) Kord's `user_id` is stable across Google, GitHub and email logins for the same email. `User.get_by_email` suggests one User per email (`auth.py:85`); not verified for OAuth.
- (c) CORS isn't involved. The page never calls nuts-auth from JavaScript, only via redirect (`app.py:60-64`).
- (d) `/api/validate` needs no caller credential (`app.py:76`; nuts-news calls it bare, `auth.rs:113-120`).

### Stage map
| Design label | Vīthi stage (`vithi.rs:96-112`) | Signal now | After X2 |
|---|---|---|---|
| contact | Contact (phassa) | request accepted; novelty from recall (`chat.rs` `operator_novelty`) | same, recorded by `VithiBuilder` |
| feeling | Investigating: vedanā | **approximation**: no chat vedanā gate; show as passed | Ollaya vedanā gate on the message |
| recognition | Investigating: saññā | **approximation**: lexical/BM25 arms returning | saññā gate tags |
| remembering | Receiving/Investigating: recall | **real**: hybrid recall, candidates count, then tool calls | same + sati monitor |
| deciding | Determining (voṭṭhapana) | **real once §4 lands**: judge p; until then, skipped | `signals::determination` (`vithi.rs:280`) → adhimokkha / vicikicchā factor |
| speaking | Impulsion (javana) | **real**: model rounds, tokens | + Registration on commit |

Until X2 is wired, approximated stages are drawn as passed quickly and marked `approximate: true` in the event, so the instruments view can say so; the page must not claim a signal it does not have.

## 4. Gates the design implies

Both are Ollaya yes/no gates (`ollaya.rs:264-291`), advisory until calibrated (ECE < 0.05 on held-out labels, PLAN_V3 R2), and must fit Laya's 512-token window (`laya:en`, confirmed live): the state is the operator's message plus at most the last turn, trimmed from the front, never the whole conversation (the curiosity gate overflowed exactly this way, `life.rs:595`). A truncated state abstains and is shown as "couldn't judge", never as a verdict.

1. **Worth deliberating?** `"Answering this well needs careful thought and looking things up."` Confident no (p < 0.2, as the curiosity gate) → fast path: one model round, no tools offered. Confident yes → full tool loop. **Indeterminate band** (confidence below the threshold, p ≈ 0.4–0.6): the backlog X9 procedure *as revised by Steve* — entropy draws recall to re-ask once; if still split, the result is `unresolved`, and the safe action is taken (think, with tools). Chance may choose an action; it never decides what is true. While uncalibrated, the gate only labels the turn and drives the judge line; every turn still gets the full loop.
2. **False premise / leading question.** `"This question assumes something that has not been established."` Confident yes → `premise_warning` event and a system-prompt line ("the operator's question assumes X; do not confirm a detail your memory or documents do not hold"). The warning text in the design is the agent's voice; the page shows the gate's result, the agent's reply does the refusing. Benchmark tie-in: STALE premise-resistance and the Steve-recall Class E items.

Each gate call is journaled with provenance (`GateProvenance`: version, features hash, latency, truncation) and appears in the turn's durable record.

## 5. Phases

**Phase 1: the design on data we have (whole reply).** Requires §3a phase A0 (nuts-auth login). New `/talk` page with the tokens and layout above; header with disclosure and phase pill (`/life`); past turns; on send, a spinner-state stage bar (non-live), then the finished turn rendered: recall card from `memory_candidates` with provenance lines and verdict markers, tool calls as "searches", document cites, answer; rail from `/life` polled every 30 s; Stop disabled.
Exit: a real conversation renders with every candidate's provenance, verdicts on 2147483702 shown, rail values matching `GET /life`; axe/keyboard pass; login via nuts-auth (§3a phase A0) works end to end; no credential stored in `localStorage`/`sessionStorage` and none left in the address bar after the callback.

**Phase 2: streaming and stages.** `POST /chat/stream`, `TurnEvent` channel through `converse`, Ollama `stream: true`, stage events per the map (approximations flagged), candidate/tool/verdict events, final-round token streaming with `token_reset`.
Exit: a turn with two tool calls streams stage → candidates → tool calls → tokens; no `<use_tool>` text ever appears in the page (test with the scripted model from `chat.rs` tests emitting a tool call mid-round); `done` payload equals `GET /chat/{id}`'s stored turn; `/chat` unchanged (existing tests pass).

**Phase 3: judge, premise, Stop.** The two gates with the 512-token state rule, indeterminate handling per revised X9, judge line and premise warning; cancel endpoint with the semantics in §3.
Exit: "Did you call him Dad, or Paul?" produces a premise warning and a reply that does not confirm a word memory does not hold; a forced-indeterminate judge shows "unsure (p …)" and takes the think path; Stop mid-stream leaves a `cancelled` turn, no experience rows for it (row count unchanged), verdicts written before Stop kept.

**Phase 4: instruments view.** After `Main.dc.html` arrives: drives, curiosity journal, last dream + question, vīthi record per turn (real once X2 is wired), gate provenance, memory/embedding counts, entropy source. This absorbs the `/dashboard` item from HANDOFF Job 2.
Exit: set by the design.

## 6. Risks and open questions for Kord
- **Login (§3a):** operator allowlist by nuts-auth `user_id`; session lifetime beyond the 30-minute JWT (proposed 12 h); a `reader` role or not; break-glass via `docker exec`; whether cancelled or failed logins are journaled; when to delete `secrets/` relative to phase A1.
- **Which page is primary**, and when is `chat.html` retired? Keep both through phase 2.
- **Disclosure wording** is a guardrail (benchmark plan, deceased-persona suite): confirm the exact string and that it is always visible, including on phones.
- **Stop semantics**: confirm "cancelled turns are not remembered" (alternative: remember them tagged `cancelled`).
- **Streamed text before the citation check**: phase 2 recommends buffering those rounds; the cost is a delay before tokens appear on turns where the check fires.
- **Approximated stages** (feeling, recognition) until X2: acceptable to show them flagged, or hide until real?
- **Gate thresholds** for fast path vs think are uncalibrated; the judge line must say "advisory" until WP-4 passes.
- **Fonts from Google** load from the operator's browser only; the server stays offline-capable. Self-host if that matters.
- **Latency**: a gate call is 10–18 ms warm, seconds on first load; the page should not wait on the gate to show the stage bar.
