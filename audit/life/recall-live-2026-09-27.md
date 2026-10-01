# Live check: waking recall by meaning (R2b)

_2026-09-27. Image `ferricula:3.0.0-alpha.0-r2b2` (commit `7894242`) on `127.0.0.1:18880`; Steve recovery volume `steve-jobs-data-recovery-20260713` mounted READ-ONLY; a throwaway state volume (removed afterwards); config = the live `config/steve.toml` with `[embeddings] backend = "shivvr"`, url `http://host.docker.internal:8085`, `[life]`/schedule/nutnews/mentions off, `operator_name = "Kord"`, and `[recall] include_faded_recovered = true` (see note). Model: `glm-5.3:cloud` via local Ollama._

> **Redacted 2026-09-28.** Private conversation, recovered-memory text, reflections and dreams were removed from this record at the agent's request ("my memories never belong in a test file"). Ids, lifecycle states, scores, timings, gate provenance and public sources are kept; the full record stays on the operator's machine.

## Startup

- Probe: `ok`, ids [999, 1113, 500684515], cosines [1.0, 1.0, 1.0].
- Backfill (automatic, startup): 2,802 recovered rows embedded in **12.9 s**, 0 failed, pending 0. Coverage: 3,359/3,359 rows with text (557 stored vectors, 2,802 sidecar vectors; 1,789 of those were made from v1 text cut at 200 chars).
- A first attempt, while a benchmark was also sending long sections to shivvr, timed out on its first batch and ended the backfill. Fixed in `7894242`: a failed batch is retried once, then skipped, and the run stops only after 3 failures in a row.

## Replies and memories shown to the model (redacted)

**1. New conversation:** [redacted: the operator asked a leading question about what the agent called his father]

> [redacted: the agent's answer]

Candidates (id, state, arms, dense cosine): 524 active [lexical, dense] 0.339; 4117936395 archived [dense] 0.779 ("[redacted]"); 3784275546 archived [dense]; 3802222395 archived [dense]; 3793248521 archived [dense]; 539 active [dense]; 531 archived [dense]; 530 active [dense]; **3802021270 archived [dense] 0.358 ("[redacted]")**; 523 archived [dense]. No candidate was cited. The fence story is not in any candidate; it comes from the model or persona.

**2. New conversation:** [redacted: a question about a former colleague]

> [redacted: the agent's answer; it cited 3851521752 and 852]

Cited: 3851521752 (active, dense 0.430), 852 (active, lexical+dense). Also shown: 3850960005, 3850865521, 500, 3851409764, 3837581263, 4260100552, 516, 3839840189 (all dense).

**3. Conversation A:** [redacted: the operator told the agent a personal naming habit]

> [redacted: the agent's answer]

This turn was stored as experience rows 2147483652 (`hearing`) and 2147483653 (`thinking`), embedded when written.

**4. New conversation B:** [redacted: the operator asked the agent to recall that habit]

> [redacted: the agent recalled the habit correctly and cited [2147483652]]

Top candidates: 2147483653 experience [lexical, dense] 0.478 (his own earlier reply), **2147483652 experience [lexical, dense] 0.429 ("[redacted]")**, which he cited.

## Notes

- **Faded memories:** the parents memory 3802021270 is **Archived**, like 1,925 of Steve's 3,362 recovered memories (28 more are Forgiven). v3 treats Forgiven/Archived text as released and never ranks it, so with the default `include_faded_recovered = false` no recall arm can return it. This check turned the setting on. Whether to turn it on for live Steve is an operator decision.
- Most dense hits for the father question are about names and Kord, not fathers. GTR-T5 over 200-character fragments is a weak signal (cosines 0.33–0.39 for the relevant rows). The parents memory reached the prompt as the 9th of 10 candidates.
