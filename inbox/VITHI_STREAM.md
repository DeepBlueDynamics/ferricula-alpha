---
title: Vīthi record and live turn view (X2 + WP-U2), shared contract
status: active
source: Claude (coordinator), for Kord ("I still have zero visibility into it")
date: 2026-10-01
related: inbox/UI_PLAN.md §3, research/citta-vithi.md, research/2026-10-01-abhidhamma-fidelity.md
---

# Vīthi record and live turn view

## Contract
- **Server:** during a chat turn, emit one `stage` event per stage on `POST /chat/stream`, in order.
- **Durable record:** store the same objects on the turn as `vithi: [...]`, so a past turn shows the same thing.
- **Unmeasured stages:** a stage that isn't measured is emitted with `measured: false` and a one-line `summary` saying why. It is never omitted and never faked.

```json
{ "event": "stage", "stage": "<name>", "measured": true, "summary": "<one line a human reads>",
  "detail": { ... }, "advisory": false, "t": <ms since turn start> }
```

| stage | Pāli | measured from (code as it is today) | `detail` |
|---|---|---|---|
| contact | phassa | message arrival in `converse_with_events` | `bytes`, `origin`, `conversation_id` |
| feeling | vedanā | the vedanā gate (`ferricula_gates::ollaya::vedana_questions`, via `gate_decide` so JEV/Ollaya tiers apply) on the operator's message; **advisory** (uncalibrated); also the novelty value that relieves boredom | `valence` probabilities, `intensity`, `tier`, `novelty`, `abstain?` |
| recognition | saññā | hybrid recall result | per-arm counts (lexical, bm25, dense, graph), top candidate ids, best dense cosine, `duplicates_collapsed`, `fragments` |
| investigation | santīraṇa | tool calls in the loop | list of `{name, secs, bytes, error?}` |
| determining | voṭṭhapana | gates that fired (citation_check, speak_modality, email house gate) and the curator briefing | list of gate records, `curator: ok/skipped` |
| impulsion | javana | model rounds | `rounds`, per round `{secs, finish, output_tokens}`, `retries` |
| registration | tadārammaṇa | what was stored | `remembered_ids`, `cited` (ids counted by the recall overlay) |

Order on the stream:
1. contact
2. feeling
3. recognition
4. (rounds, with investigation and determining accumulating)
5. investigation and determining summaries
6. impulsion
7. registration
8. done

Live `round`, `tool_call` and `gate` events keep streaming as now.

## Rules
- **Feeling-tone is advisory.** It's displayed, never used for ranking or lifecycle, until calibrated (ECE < 0.05).
- **No stage is drawn as a progress bar** unless it has a real number.
- **The page states what isn't measured, in words.**
