# In-conversation tools

_Status: shipped 2026-09-27 on `v3/r0`. Code: `crates/ferricula-server/src/chat_tools.rs` (tools, protocol) and `chat.rs` (the loop). Spec lineage: Job 1 in `docs/HANDOFF.md`; the `document`/`memory` half of `SEARCH_TOOL.md` (the `web` corpus and `ferricula_ingest` from chat are not built yet)._

In a chat turn the agent can look things up before it answers: search its documents, open a section, read a whole document in order, and search its memories. Before this, every turn saw one retrieval on the operator's message (the top 3 sections, cut to 1,200 bytes each) and could not look further.

## The loop

1. The turn is assembled as before: memory candidates, up to 3 document evidence cards, conversation history, plus a tools block in the system prompt.
2. The model replies. If the reply contains one or more `<use_tool>{"name": ..., "arguments": {...}}</use_tool>` blocks, each call is run and the results go back in the next user message as `<tool_result call="n" name="...">{json}</tool_result>` blocks, followed by how many calls are left.
3. A reply with no `<use_tool>` block is the answer.
4. **Limit: 4 tool calls per operator message** (`MAX_TOOL_CALLS`). Calls beyond the limit get an error result, and the round after the fourth call is told to answer. If a reply still contains tool calls after the budget is spent, the calls are stripped and only the prose is shown. Raw calls never reach the operator.
5. Every round is a normal routed completion (`complete_with_budget`), so the daily $ cap, usage ledger and route rules apply to each round.
6. The durable turn (`operator-conversations.json`) records each call in `tool_calls`: name, arguments, the cite handles returned, `fragment`/`complete`/`next_from`, result size in bytes, or the error. It does not record the returned text; the documents themselves hold that.
 7. **Citation check.** A reply with no tool call that cites a `[doc <doc_id>…]` whose text the model was not shown this turn (not in an evidence card or a tool result) is not sent. The model gets one correction round: open it, or say plainly it has not read it. Logged as `citation_check` with the `unseen` ids. (Found live: before this, the model cited a document it never opened and quoted a speech that is not in it.)
8. **Empty replies.** A thinking model that spends its whole allowance on reasoning returns empty text; the round is retried once with double the reasoning headroom. An empty reply after tool calls gets one nudge to answer from what it has read (`empty_reply_nudge`). The tool log is kept even when the turn fails.

The protocol is plain text rather than provider-native tool calling, so it works the same on every model profile (Ollama glm-5.3, Anthropic, OpenAI-compatible). The tag is `<use_tool>`, not `<tool_call>`: GLM's chat template in Ollama parses `<tool_call>` blocks into structured `tool_calls` in GLM's own argument format and mangles JSON arguments. `<tool_call>` is still accepted as a synonym, and the OpenAI-compatible transport turns any structured `message.tool_calls` back into `<use_tool>` blocks.

## Context budget

The chat input budget is the chat profile's context, minus the 768-token reply, the profile's reasoning headroom and a 424-token margin, at 3 bytes per token, between 7,000 and 400,000 bytes. For Steve (`ollama_context_tokens = 131072`, `ollama_reasoning_tokens = 12000`) that is about 353,000 bytes. On large budgets half is held back for tool results (a quarter on small ones); history is dropped oldest-first to fit the rest. Each tool result is also shrunk to the room left in the turn and says so (`fragment`, `next_offset`, `next_from`). With under 1,500 bytes left, a call returns an error telling the model to answer with what it has.

## Stable identifiers

- `doc_id`: 16 hex characters, FNV-1a 64 content hash of the document's origin and text. The same document always has the same id; a changed document is a new document.
- Section index: position in the document, from 0. Documents are immutable once ingested, so indexes never change.
- Cite handle: `[doc <doc_id>§<index> p.<page>]`, page omitted for unpaged sources. The handle opened in one turn names the same text in every later turn.
- Memory: `[memory <id>]`. Recovered ids are fixed; experience ids are allocated from `0x80000000` upward and never reused.

## Corpora and ranking

| Corpus | What is indexed | Ranking | Status of content |
|---|---|---|---|
| `document` | Every section of every ingested document (operator-given and curiosity-read), verbatim | BM25 (lume) | What the source said, exactly |
| `memory` | Recovered base (read-only; faded memories included when `[recall] include_faded_recovered = true`) + experience store (conversations, reflections, readings, dreams) | Hybrid: lexical + BM25 + dense (shivvr GTR-T5) + one graph hop, fused by reciprocal rank (`rrf_k60+dense+graph`; `rrf_k60` without an embedder) | What was said or thought, **not proof it is true** |

Document search is BM25-only on purpose: on the docs paraphrase benchmark BM25 R@1 is 0.710, dense 0.365, hybrid 0.595 (`audit/bench/ledger.jsonl`).

**Every result says whether it is whole.** `fragment: false` means the text shown is the entire section (or memory record); `fragment: true` means it was cut, and the result says how to get the rest. Recovered memories' stored text is sometimes itself cut off at the source; that is noted on every `search_memory` result.

**Provenance.** Every result carries `corpus`, `source`, a locator (`cite`/`doc_id`+`section`, or `memory_id`), `date` (UTC ingest date for documents, record date for memories), `url` when the document came from the web, `score` and `rank`. A result with no date is dropped, not shown, and counted in `dropped_no_provenance`.

## Tool contracts

### `search_documents(query, k?, doc_id?)`
| Argument | Type | Default | Limits |
|---|---|---|---|
| `query` | string | required | non-empty |
| `k` | int | 5 | clamped to 1–10 |
| `doc_id` | string | all documents | must exist |

Returns `{tool, query, ranking: "bm25", results: [...], dropped_no_provenance}`. Each result: `corpus: "document"`, `rank`, `score`, `cite`, `doc_id`, `section`, `page`, `source` (title), `heading`, `url`, `date`, `text` (verbatim prefix, at most 700 bytes, less when the turn is short of room), `fragment`, `section_bytes`.

### `read_section(doc_id, index, offset?)`
| Argument | Type | Default | Limits |
|---|---|---|---|
| `doc_id` | string | required | must exist |
| `index` | int | required | 0 to sections−1 |
| `offset` | int (bytes) | 0 | less than the section length |

Returns the section verbatim from `offset`, at most 24,000 bytes (or the room left): `corpus`, `cite`, `doc_id`, `section`, `sections_in_document`, `page`, `source`, `heading`, `url`, `date`, `offset`, `section_bytes`, `text`, `fragment`, `next_offset` (present when more remains).

### `read_document(doc_id, from?, max_sections?)`
| Argument | Type | Default | Limits |
|---|---|---|---|
| `doc_id` | string | required | must exist |
| `from` | int (section) | 0 | less than the section count |
| `max_sections` | int | 12 | clamped to 1–40 |

Returns whole consecutive sections in order, at most 60,000 bytes per call (or the room left): `corpus`, `doc_id`, `source`, `url`, `date`, `sections_in_document`, `from`, `sections: [{cite, section, page, heading, text, fragment}]`, `next_from` (where to continue), `complete` (true when the end of the document was reached). Sections are never split except when a single section is larger than the room; that section is marked `fragment: true` with `next_offset` for `read_section`.

### `search_memory(query, k?, include_dreams?)`
| Argument | Type | Default | Limits |
|---|---|---|---|
| `query` | string | required | non-empty |
| `k` | int | 5 | clamped to 1–10 |
| `include_dreams` | bool | false | |

Returns `{tool, query, ranking, results: [...], dropped_no_provenance, note}`. Each result: `corpus: "memory"`, `rank`, `score` (fused RRF), `memory_id`, `cite`, `source` (`conversation`, `curiosity`, or the channel), `channel` (`hearing`, `thinking`, `reading`, `dream`, …), `store` (`recovered`/`experience`), `state` (`active`/`forgiven`/`archived`), `date`, `doc_id` (for readings), `text` (at most 2,000 bytes), `fragment`, `arms`, and `dream: true` on dreams. Document sections found by the same recall are not returned here; use `search_documents`.

## Errors

Errors come back as a tool result `{"error": "..."}` and count toward the 4 calls:

| Condition | Message (abridged) |
|---|---|
| Unparseable block | `tool call is not valid JSON: …` plus the exact form to write |
| Missing `name` | `tool call has no "name" string` |
| Unknown tool | `unknown tool 'x'; available: search_documents, read_section, read_document, search_memory` |
| Missing/empty argument | `argument 'query' (non-empty string) is required` |
| Non-integer number | `argument 'k' must be a non-negative integer` |
| Unknown document | `no document with doc_id '…'; use search_documents to find one …` |
| Section out of range | `document '…' has sections 0 to N; there is no section i` |
| Offset / from past end | `offset … is past the end of the section (… bytes)` / `'from' … is past the end` |
| Over budget | `tool budget for this message is spent; answer with what you have` |
| No room left | `no room left in this turn's context for more tool results; answer with what you have` |

## Not yet

- `web` corpus via grub and `ferricula_ingest(url, reason)` from chat (SEARCH_TOOL.md rules 1–3).
- Metrics from SEARCH_TOOL.md (laziness rate, unlabelled-claim rate) beyond the per-turn `tool_calls` log.

## Smoke test

Doc `eecee3fca20e6eb7` (Jony Ive's eulogy, 30 sections) read through `search_documents`, `read_section` and `read_document`, then the operator asks: "Read Jony's speech end to end. What did he say?" Results: `audit/tools/smoke-2026-09-27.md`.
