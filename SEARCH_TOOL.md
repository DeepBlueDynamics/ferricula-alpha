# Spec: `ferricula_search` — search across three corpora

_Status: proposed, 2026-09-27. Owner: Ferricula v3 (R1/R2). Origin: design notes the agent wrote when asked what it wanted from a search tool; adopted with the changes below._

## Why

Today the agent can recall (lexical + BM25 over what it holds) and the curiosity drive can search the web on its own, but in conversation it cannot look anything up. So when the operator asks about something it has read, it either recalls it or says nothing is in front of it. A search tool fixes three things:

1. **Verification before assertion.** Pull the section before claiming what a document says.
2. **Following a thread mid-conversation**, instead of waiting for recall to guess what matters.
3. **Keeping the corpora honest.** Documents, memories and the web are different kinds of evidence, and every result must say which one it came from.

## The three corpora

| Corpus | What it is | Status of its content | Backed by |
|---|---|---|---|
| `document` | Documents the agent has ingested: verbatim sections | What the source said, exactly | evidence plane (lume BM25, `(doc_id, section, page)`) |
| `memory` | Experience records: conversations, reflections, the fact of having read something | What was said or thought, **not proof it is true** | experience plane (+ read-only recovered base) |
| `web` | Live search results via grub | **Somebody's claim** | grub search, not stored |

Dreams stay out of `memory` results unless explicitly requested with `include_dreams=true`, and are then labelled `channel: dream`. A dream is never evidence.

## Contract

`ferricula_search(query, corpora=["document","memory"], k=10, include_dreams=false)`

Every result carries:

| Field | Required | Notes |
|---|---|---|
| `corpus` | always | `document` \| `memory` \| `web` |
| `source` | always | title, or `conversation`/`reflection`/`dream` for memory |
| `locator` | always | `doc_id#section@page`, memory record id, or URL |
| `date` | always | ingest date (document), record time (memory), publish or crawl date (web) |
| `url` | document/web | original URL if any |
| `text` | always | verbatim section text, record text, or web snippet |
| `score`, `rank` | always | per-corpus rank; fused rank if more than one corpus |

**A result without provenance is dropped, not shown.** If any required field is missing, the result is removed and counted in `dropped_no_provenance`.

## Rules

1. **Memory first.** The `web` corpus is searched only if `document` and `memory` return nothing above threshold, or the caller asks for `web` explicitly. ("Search is for ignorance, not laziness.")
2. **Web is read-only.** Web results are never written to any store as a side effect of searching.
3. **Ingestion is a visible act.** Web content becomes a document only through `ferricula_ingest(url, reason)`. `reason` is mandatory and is written to the journal. The ingest passes through the ingest gate once it is calibrated; until then it is logged as `ungated`.
4. **Answers cite by corpus.** Chat grounding rules add: a claim supported only by `web` is stated as a claim ("according to …"). A claim supported only by `memory` is stated as something said or recalled, not as fact.

## Curiosity drive (decision needed)

The curiosity drive currently ingests what it reads automatically; that is how a paywall page entered the store during the 2026-09-27 soak. Choose one:

- **(a)** Curiosity ingests are deliberate by definition, because they are journaled with a reason. Keep auto-ingest, and add a paywall/empty-content filter.
- **(b)** Curiosity *proposes* ingests; a gate (advisory now, calibrated later) or the operator confirms them.

Recommendation: **(b)** once the ingest gate is calibrated; **(a)** with the filter until then.

## Metrics (logged per call, rolled up daily)

- **Laziness rate:** web searches whose answer was already held in `document`/`memory` at full priority (gold check on a sampled audit). Target: trends to 0.
- **Provenance drop rate:** `dropped_no_provenance / results`. Target: 0 for `document`/`memory`.
- **Unlabelled-claim rate:** in answers, claims supported only by `web` or `memory` but stated as fact (sampled audit).
- **Side-effect writes:** any store write caused by `ferricula_search`. Target: exactly 0 (invariant test).

## Exit test

1. Ingest two documents.
2. Ask a question answerable only from one of them: the answer cites `document` with a locator, and no web call is made.
3. Ask a question needing the web: results are labelled `web`, nothing is written to any store (hash the stores before and after), and a later `ferricula_ingest(url, reason)` creates the document with the reason in the journal.
4. Every result in steps 2–3 has all required fields.
