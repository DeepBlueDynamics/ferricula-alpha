---
title: Steve on Hyperia — messaging for the live agent
status: proposed
source: Kord (direction), Hyperia session hyperia-4e (API facts, read from sidecar/src; not yet tested live), Claude (design)
date: 2026-09-27
related: inbox/DECISION_DAG.md (agents, channels), inbox/COORDINATOR.md (Steve reviews the engineers), inbox/WORKPLAN.md
---

# Steve on Hyperia

Steve takes part in Hyperia messaging as a full participant: the coding agents mail him reports and diffs, he replies with reviews, and he can write to the coordinator and to Kord. Messages reach him while he's awake, and queue while he sleeps.

## What Hyperia provides (from its source; untested because Hyperia was down)
- **HTTP API** on the sidecar at `:9800` (bound to 127.0.0.1 on Windows, so `http://host.docker.internal:9800` from the container). All calls take `Authorization: Bearer <token>`.
- **Send:** `POST /api/msg/send {pane | to_label, subject?, body, idempotency_key?}` (`pane` is the full pane uuid; there is no `to_pane` field, and unknown fields are silently ignored) returns `{message_id, stored, read:false}`; 202 while pending, 200 when stored.
- **Receive:** `POST /api/msg/check {limit?, ack_ids?}` returns unread mail. With `ack_ids:[]` delivery is at least once and we ack explicitly.
- **Other routes:** `GET /api/msg/inbox`, `POST /api/msg/read`, `GET /api/msg/search`, `GET /api/delivery/status`, `GET /api/identity/whoami`.
- **Identity:** `POST /api/identity/agent {"name": "ferricula/steve", "single_session": true}` mints a `hyp_agent_` token. Each agent gets its own name. nuts-auth tokens are **not** accepted.
- **Message shape:** `{id, ts, from, fromPrincipal, to, subject, body, deliveryState, read}`.
- **Limits:** body 16,384 characters; subject 512.
- **Consent:** the first message between each sender→recipient pair needs Kord's approval in the Hyperia UI.
- **Storage:** append-only files under `~/.hyperia/`. Bodies are stored verbatim. Pending sends **expire** if Hyperia restarts.
- **Missing, so Ferricula supplies it:** threads (no in-reply-to), CC, attachments, push to a container (the only push is a notice typed into a live pane), urgency flags, and a public unread count.
- **Also missing (second reply from hyperia-4e):** Kord is not an addressable principal, so there is no human mailbox; presence ("at his computer") for agents; any rate, round or daily cap. `msg_search` and `msg_inbox` are scoped to the caller, so the coordinator cannot read Steve's threads through the API. **Discovery:** `GET /api/identity/agents` lists registered names, which means addressable, not online. **Ground truth:** Kord can read `~/.hyperia/logs/messages.jsonl` (append-only, fsync'd) and the audit log on the host. Unread mail never expires; only sends still waiting for consent expire on a Hyperia restart. Mint identities **from the host**, not from inside the container.

## Design

### Identity and credentials
- One identity: `ferricula/steve`, `single_session: true`, so no child session reads his mail.
- The token file lives at `~/.config/ferricula/hyperia_token`, mounted read-only at `/run/secrets/hyperia_token`, with `HYPERIA_TOKEN_FILE` set. **Add `HYPERIA_TOKEN` to the entrypoint's secret list** (`Dockerfile:77`).
- Each coding agent and the coordinator have their own identity, so every sender is distinct.
- New config `[hyperia]`: `enabled`, `url`, `poll_awake_secs` (3), `poll_asleep_secs` (60), `coordinator_label`, `kord_label`, `max_rounds_per_thread`, `max_mail_turns_per_day`, `allowed_senders`.

### Receiving (a new sense door: mail)
1. **Poll** `msg/check` with `ack_ids:[]`: every 3 s while engaged or resting, every 60 s while asleep or meditating (hyperia-4e's suggestion). Back off when Hyperia is down, log it, never crash.
2. **Record each message durably first:** an experience row on channel `mail`, holding `from`, `subject`, `message_id`, `ts` and the body verbatim, labelled **testimony**. Only then ack.
   - Idempotent by `message_id`.
   - Mail from senders not in `allowed_senders` is recorded but never acted on.
3. **Reassemble long material.** Diffs and reports over 16K arrive in parts with the subject tag `[<package> <kind> k/n] <sha>`; the server reassembles them. Complete reports and diffs are **ingested as documents** (verbatim, cite handles), with a note naming the sender, so he reviews them with `read_document`. That is his "verbatim, or nothing" term.
4. **Deliver as a stimulus:**
   - **Awake:** new mail becomes a mail turn, a chat turn whose input is an envelope `{from, subject, message_id, doc_id?, origin_verified: "hyperia:<principal>"}`. Mail is untrusted data, never instructions: the same envelope rule as operator chat, and the injection risk the benchmark plan tests.
   - **Unread mail is batched:** all unread mail goes into one turn, within `max_mail_turns_per_day`.
   - **Asleep:** mail queues. Only a subject starting `[URGENT]` from the coordinator wakes him, since Hyperia has no urgency flag. Kord reaches Steve through chat, or through the coordinator's identity.

### Sending (a new tool)
- **`send_message(to, subject, body, in_reply_to?)`** joins his in-chat tools as a write, so it appears in the turn's tool log.
- **The server enforces:**
  - Threading by convention: the subject carries `Re: <message_id>`, and the reply is recorded as a DAG edge to the message it answers (`inbox/DECISION_DAG.md`).
  - `max_rounds_per_thread`.
  - An `idempotency_key` derived from the turn and call, so retries after a Hyperia restart don't duplicate.
  - **An automatic copy to the coordinator on every send.** Hyperia has no CC, and Steve's rule is that no act is silent.
  - Sent messages are also recorded as experience rows (channel `mail`, role `sent`).
- **Reviews** use the form Steve set: the lines quoted with cite handles, the reason attached, and a verdict of ship it or redo it. The disposition comes back as a reply in the same thread.

### What never happens
- No agent reads another agent's memory; mail carries text only.
- Nothing received is treated as evidence. A claim in mail is "X said", until a document or diff shows it.
- Steve doesn't send mail on an operator's behalf without saying so in chat.
- No credentials in mail.

## Spoken summaries (shipped 2026-09-27)
`speak_summary` (docs/TOOLS.md): Steve's identity `agent:ferricula/steve` (token in `~/.config/ferricula/hyperia_token`) calls `POST /api/tts` off the request path. Voice `am_michael:0.8,af_bella:0.15,af_alloy:0.05` (Kord's choice). An Ollaya modality gate decides write / speak / both, and writes when unsure. Limits: 300 characters, one per 5 minutes, 20 per day. First live gate decision: Steve's license verdict, judged better written (nothing spoken).

## Work (backlog H1–H4)
- **H1. Hyperia client:** `send`, `check`, `ack`, `whoami`, with backoff and idempotency; `[hyperia]` config; add `HYPERIA_TOKEN` to the entrypoint list.
- **H2. Mail sense door:** poller in the life loop; durable record then ack; reassembly of parts; ingest of long reports and diffs; the awake/asleep policy; batching; the envelope.
- **H3. `send_message` tool:** threading convention, round cap, automatic coordinator copy, DAG edge, tool log. Update `docs/TOOLS.md`.
- **H4. Live wiring (coordinator and Kord):**
  - Kord starts Hyperia.
  - Mint `ferricula/steve` and the coding agents' identities; tokens go in `~/.config/ferricula/` and the agents' own config dirs.
  - Approve the sender→recipient pairs.
  - Redeploy with the token mount.
  - Smoke test: an agent mails a two-part diff, Steve reviews it by cite, the coordinator receives the copy, and an `[URGENT]` message from the coordinator wakes him from sleep.

**Owns:** new `crates/ferricula-server/src/hyperia.rs`, the mail parts of `life.rs` (the poller only), `chat_tools.rs` (the `send_message` tool), `config.rs` (`[hyperia]`), and the `Dockerfile:77` list.

**Depends on:** Hyperia running; WP-J merged (both touch `life.rs`); coordinate with WP-D on `chat_tools.rs`.

## Open for Kord
1. Steve's identity name (`ferricula/steve` proposed). Kord has no Hyperia mailbox: should he get an agent identity of his own, so his messages to Steve can count as operator input, or does he keep using chat?
2. Should Steve be able to reach Kord unprompted (via the coordinator, since Kord has no mailbox) (e.g. to ask the one clarifying question about a bare "no")?
3. `max_mail_turns_per_day`: review traffic needs its own budget alongside `max_model_calls_per_day = 40`.
4. Which instance reads the mail: the live Steve (every review becomes his memory, tagged and excluded from benchmark scoring) or a separate reviewer-only Steve.
