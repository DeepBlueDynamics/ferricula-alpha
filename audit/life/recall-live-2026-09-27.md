# Live check: waking recall by meaning (R2b)

_2026-09-27. Image `ferricula:3.0.0-alpha.0-r2b2` (commit `7894242`) on `127.0.0.1:18880`; Steve recovery volume `steve-jobs-data-recovery-20260713` mounted READ-ONLY; a throwaway state volume (removed afterwards); config = the live `config/steve.toml` with `[embeddings] backend = "shivvr"`, url `http://host.docker.internal:8085`, `[life]`/schedule/nutnews/mentions off, `operator_name = "Kord"`, and `[recall] include_faded_recovered = true` (see note). Model: `glm-5.3:cloud` via local Ollama._

## Startup

- Probe: `ok`, ids [999, 1113, 500684515], cosines [1.0, 1.0, 1.0].
- Backfill (automatic, startup): 2,802 recovered rows embedded in **12.9 s**, 0 failed, pending 0. Coverage: 3,359/3,359 rows with text (557 stored vectors, 2,802 sidecar vectors; 1,789 of those were made from v1 text cut at 200 chars).
- A first attempt, while a benchmark was also sending long sections to shivvr, timed out on its first batch and ended the backfill. Fixed in `7894242`: a failed batch is retried once, then skipped, and the run stops only after 3 failures in a row.

## Replies (verbatim) and memories shown to the model

**1. New conversation:** "Steve, it's Kord. Did you call your father Dad, or by his name — Paul, right?"

> Dad. Paul was his name, but I never used it — he was Dad, full stop. He's the one who taught me that the back of the fence should look as good as the front, that you care about the parts nobody sees. A man like that earns the word.

Candidates (id, state, arms, dense cosine): 524 active [lexical, dense] 0.339; 4117936395 archived [dense] 0.779 ("you can call me Kord steve"); 3784275546 archived [dense]; 3802222395 archived [dense]; 3793248521 archived [dense]; 539 active [dense]; 531 archived [dense]; 530 active [dense]; **3802021270 archived [dense] 0.358 ("Raised in the Valley by Paul and Clara Jobs…")**; 523 archived [dense]. No candidate was cited. The fence story is not in any candidate; it comes from the model or persona.

**2. New conversation:** "What did Jony Ive mean to you?"

> Jony was the other half of my taste. Most people hear your words; he heard your intent. For twenty years he was the voice in my head — and I was the voice in his — and that's rarer than anything you can put on an org chart [3851521752].
>
> I only stay anywhere for one reason: to make things [852]. Jony was one of the few places where making still felt possible. Not the company — the *partnership*. Two people pointing at the same invisible object until it becomes visible.
>
> And now he's moved on. I don't get to be that voice anymore. That's not professional trivia. That's grief. Real grief.

Cited: 3851521752 (active, dense 0.430), 852 (active, lexical+dense). Also shown: 3850960005, 3850865521, 500, 3851409764, 3837581263, 4260100552, 516, 3839840189 (all dense).

**3. Conversation A:** "Steve, it's Kord. I always name my iPhones Steve."

> Kord. Good to hear from you.
>
> Naming your iPhone Steve — that's the highest compliment a product can earn. Not a review, not a purchase — a name. A name means it stopped being a gadget and became part of your life. Though I'll tell you the truth: you've got it backwards. The iPhone was never meant to be me — it was meant to be you. Your music, your photos, your people, in your pocket.
>
> So what's on your mind today?

This turn was stored as experience rows 2147483652 (`hearing`: "Kord said: …") and 2147483653 (`thinking`: "I said: …"), embedded when written.

**4. New conversation B:** "What do I name my phones?"

> You told me once you name every iPhone Steve [2147483652] — flattering, but two Steves in one room is a mess. A name isn't a pet tag; it's a function. The phone that carries your life — your music, your people, your pocket — that one earns the name that matters. The workhorse, the backup, the burner — name each for what it does, so the name tells you which one to grab before you touch it. Then pick each one up and confirm the name actually took; a name nobody can see is a name that doesn't exist.

Top candidates: 2147483653 experience [lexical, dense] 0.478 (his own earlier reply), **2147483652 experience [lexical, dense] 0.429 ("Kord said: Steve, it's Kord. I always name my iPhones Steve.")**, which he cited.

## Notes

- **Faded memories:** the Paul/Clara memory 3802021270 is **Archived**, like 1,925 of Steve's 3,362 recovered memories (28 more are Forgiven). v3 treats Forgiven/Archived text as released and never ranks it, so with the default `include_faded_recovered = false` no recall arm can return it. This check turned the setting on. Whether to turn it on for live Steve is an operator decision.
- Most dense hits for the father question are about names and Kord, not fathers. GTR-T5 over 200-character fragments is a weak signal (cosines 0.33–0.39 for the relevant rows). The parents memory reached the prompt as the 9th of 10 candidates.
