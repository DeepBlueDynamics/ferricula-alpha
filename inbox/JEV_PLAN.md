# Plan: hosted JEV (TypeSafe) as the second gate tier

_Status: J1 and J2 built 2026-09-28, awaiting the key (Kord enters it at `/settings`). What changed from this plan: the key is set from the UI and stored on the state volume (`state_dir/secrets/typesafe_api_key`, owner-only), with `TYPESAFE_API_KEY` as the fallback, instead of a host file mount. The settings live in `state_dir/settings/jev.json`, not TOML. Both live gates (curiosity, speak modality) route through `gate_decide`/`gate_yes_no` in `crates/ferricula-server/src/jev.rs`, in `backup` mode (JEV when Ollaya errors or truncates) or `primary` mode (JEV first, Ollaya as fallback). `private_context` defaults off. The route is recorded in the gate's journal entry and in the `gate` stream event. Still open: J1's recorded live fixtures (run the probe once the key is in), the low-confidence escalation, J3 (merge gate) and J4 (curve). Original status: proposed 2026-09-27. Owner: v3 R2. Related: PLAN_V3 R2 ("`Jev` hosted TypeSafe"), backlog X7 ("hosted JEV backend absent"), X9 (entropy on conflicted gates, as revised by the agent), `docs/BENCH_PLAN.md` / benchmark-plan PDF §4 (cascade curve, gate calibration)._

## Where we are

- One gate backend: Ollaya/Laya over `POST {ollaya_url}/api/decide` with `{model, state, questions, keep_alive}` (`crates/ferricula-gates/src/ollaya.rs:46-64`). Transport or HTTP failure → `AbstainReason::ProviderError` (`ollaya.rs:60,64`). A truncated state → `Abstain(StateTruncated)` regardless of answer (`ollaya.rs:248-249`). Low confidence → `LowConfidence { p_max }` (`ollaya.rs:272-290`).
- Types are backend-neutral: `Judged<T> { verdict, provenance }`, `Verdict::{Answer, Abstain}`, `AbstainReason`, `GateProvenance` (gate, gate_version, calibration_sha256, calibrated, features_sha256, latency_us, state_truncated, cost), `UsageCost { input_tokens, output_tokens, usd, profile }` (`crates/ferricula-cognition/src/gates.rs:23-66`). `Calibration { sha256, temperature, lifecycle_authorized }` (`crates/ferricula-gates/src/lib.rs:34-39`).
- **Live gate calls: one.** `life_worth_researching` (`crates/ferricula-server/src/life.rs:590-612`): advisory, `yes_no(..., 0.6)`, skip only on p < 0.2, state cut to 4,000 bytes (Laya `en` reads 512 tokens, so thread seeds always truncate and abstain).
- The bhāvanā merge gate exists (`crates/ferricula-cognition/src/bhavana.rs:440-458`, `impl MergeGate for OllayaGate` at `ollaya.rs:370`) but the server passes `None` (`life.rs:820-822`), so no merge judgments run live.
- Config: `[life] ollaya_url`, `ollaya_model` (`crates/ferricula-server/src/config.rs:96-97,114-115`; `config/steve.toml:82-83`), `max_model_calls_per_day = 40` (`config.rs:104,118`), `[budgets] max_model_usd_per_day` (`config.rs:729`).
- Secrets: the entrypoint turns `NAME_FILE` into `NAME` for `FERRICULA_OPERATOR_TOKEN NUTNEWS_TOKEN ANTHROPIC_API_KEY OPENAI_API_KEY ${FERRICULA_SECRET_ENV}` and refuses to start if both are set or the file is unreadable (`Dockerfile:74-95`). The server reads the resulting env var (`runtime.rs:872`). `.dockerignore` is an allowlist (`*` then `!Cargo.toml`, `!crates`, a few example configs), so nothing else in the tree can enter the build context. The repo `secrets/` directory (gitignored, `.gitignore:6`) is being removed; credentials move outside the repo (§1).

## What JEV is — verified vs assumed

**Verified (public docs, fetched 2026-09-27):**
- Endpoint `POST https://api.typesafe.ai/v1/systemone`; bearer auth; request `{state, model, questions: {name: {type, instructions, criteria}}}`; response `{model, answers: {name: {...}}, usage: {input_tokens, output_tokens}}`; models `jev-1.13.0`, `jev-latest`, `jev-preview`; no streaming. — [LiteLLM: TypeSafe AI (Jev)](https://docs.litellm.ai/docs/pass_through/typesafe)
- Three primitives: **choice** (selected option, per-option probabilities, confidence), **noul** (probability of yes), **score** (probability-weighted position, per-level probabilities, confidence); every response has `usage.cost` in USD; output tokens are free; priced per input token. — [OpenRouter: Jev guide](https://openrouter.ai/docs/guides/community/jev)
- Also reachable through OpenRouter (`POST https://openrouter.ai/api/v1/systemone`, model `typesafe/jev-1.13`). — same page
- Noul answers carry a single probability and no separate confidence ("the number already is the belief"). — [DEV: How to use Jev](https://dev.to/valyuai/how-to-use-jev-a-practical-guide-to-typesafes-system-one-model-g5e)

**Conflicting / secondary (confirm with TypeSafe):**
- Context: OpenRouter says 32,000 tokens (state + questions); the DEV article says 64k for state + all questions, 32k for state + the longest question. Either way ≫ Laya's 512.
- Price $0.042 per million input tokens, 250k tokens/s and 1,200 requests/min, 70–500 ms latency — DEV article only.

**Assumed (unverified; PLAN_V3 said "same wire as Ollaya"):**
- The question objects we already build (`noul(...)`, `score(...)`, `choice(...)` at `ollaya.rs:189-203`, `{type, instructions, criteria}`) are accepted unchanged. The shape matches the docs; field-level details (criteria as `{"true","false"}` map, the answer key `noul`) are not confirmed.
- No `state_truncated` field; behaviour on over-length state (reject vs truncate) unknown.
- Error body shape, retention and training use of submitted data unknown.

So: **not** the same wire (different path `/v1/systemone`, auth required, `usage.cost`), but close enough that one client can serve both with a thin adapter.

## 1. The token — where to put it

**Outside the repo.** Kord's decision (2026-09-27): the repo's `secrets/` directory is being removed entirely, so no credential lives anywhere under the working tree.

1. **Location:** `%USERPROFILE%\.config\ferricula\typesafe_api_key` (i.e. `C:\Users\kordl\.config\ferricula\typesafe_api_key`), the file holding only the key. Kord creates it without pasting it into chat: from PowerShell, `! mkdir $env:USERPROFILE\.config\ferricula` then `! notepad $env:USERPROFILE\.config\ferricula\typesafe_api_key`. Restrict it to his account (`icacls ... /inheritance:r /grant:r %USERNAME%:R`).
2. **Env var:** `TYPESAFE_API_KEY`, delivered as `TYPESAFE_API_KEY_FILE` through the existing entrypoint (`Dockerfile:77`): add `TYPESAFE_API_KEY` to that list (or pass `-e FERRICULA_SECRET_ENV=TYPESAFE_API_KEY` with no image change). The entrypoint already refuses to start if both are set or the file is unreadable.
3. **Mount read-only** in `/d/steve-redeploy.sh`:
   ```
   -e TYPESAFE_API_KEY_FILE=/run/secrets/typesafe_api_key \
   -v "C:\\Users\\kordl\\.config\\ferricula\\typesafe_api_key:/run/secrets/typesafe_api_key:ro" \
   ```
   Alternative with no host bind mount: a Docker secret (`docker secret create`, needs swarm/compose `secrets:`) or a small named volume holding only the key, mounted `:ro`. The bind mount is simplest on Docker Desktop.
4. **Config names the variable, never the value** (`api_key_env = "TYPESAFE_API_KEY"`), exactly like model profiles. The key is read only at call time; errors never echo headers or bodies (provider errors are already bounded, `ollaya.rs:60`).
5. Not in `steve.toml`, not in any log, not in the image: `.dockerignore` is an allowlist, and the file is outside the build context anyway.
6. **Not related to operator login.** Operator login for the UI is moving to nuts.services auth (nuts-auth). That authenticates people *to* Steve. The JEV token is a server-side outbound credential Steve's runtime uses to call TypeSafe; it is never shown to the browser, never tied to an operator session, and rotating one does not affect the other. The operator token moves to the same folder (`%USERPROFILE%\.config\ferricula\operator_token`, bind-mounted the same way) until nuts-auth replaces it.

## 2. Config

```toml
[gates.jev]
enabled = false                  # operator opt-in
base_url = "https://api.typesafe.ai"
model = "jev-1.13.0"             # pin; never "latest" for calibrated gates
api_key_env = "TYPESAFE_API_KEY"
timeout_ms = 3000
private_context = false          # must be true before any private state is sent (§4)
max_calls_per_day = 200
usd_per_million_input = 0.042    # confirm; used only when usage.cost is absent
max_state_bytes = 60000          # well under the context limit
```
Provenance: `gate_version = "{VERSION}|{QUESTIONS_VERSION}|jev:{model}"`, `cost.profile = "hosted:jev:{model}"`, `cost.usd` from `usage.cost`. Calls count toward `[budgets] max_model_usd_per_day` and `max_calls_per_day`; over budget → `Abstain(ProviderError{"jev budget spent"})`, never a silent skip.

## 3. Cascade

```
Ollaya (local)
 ├─ Answer, confident ──────────────────────────► use it (advisory until calibrated)
 ├─ Abstain: StateTruncated | ProviderError ────► "unable to judge": escalate to JEV
 ├─ Abstain: LowConfidence (indeterminate band) ► "conflicted": escalate to JEV
 └─ Abstain: NoModel | NotApplicable | Invalid ─► no escalation
JEV
 ├─ Answer ─► use it; provenance says tier = jev
 └─ Abstain / error / budget ─► proceed advisory, exactly as today
```

- **Unable to judge** (input cut, sidecar down) is a capacity problem: JEV's larger window and availability fix it. No entropy.
- **Conflicted**: Ollaya low-confidence, or Ollaya and JEV both answer and disagree (one says yes at p ≥ 0.6, the other no at p ≤ 0.4). Record both; the combined verdict is `Abstain(LowConfidence)` with both p's in provenance. Per the agent's rule (X9 as revised): a second look (JEV, or an entropy-drawn recall and re-ask) is allowed; **chance never decides what is true**. For action gates (curiosity: "worth researching?") a still-split result may let entropy pick the action, recorded as `decided_by: chance`; for truth gates (merge/contradiction, sati-recall) it stays unresolved. A two-judge contradiction on memories may write a `disputes` verdict (flag only), never `supersedes` (evidence only; `docs/TOOLS.md`).
- Implementation shape: a `JevClient` alongside `OllayaClient` with the same `decide(state, questions) -> Result<Decision, AbstainReason>` signature (map `answers.*.noul`/`choice`/`probabilities` into `Decision`, `usage` into cost), and a `CascadeGate { primary: OllayaGate, backup: Option<JevGate> }` implementing the same gate traits, so `life.rs:594` and later the merge gate swap one constructor.

## 4. Privacy

- Gate state today is curiosity seeds built from the operator's conversation (the 07:02 seed was a multi-turn excerpt) and, once wired, memory texts for merge judgments. Both are private.
- JEV is a new third party (Ollama cloud is already approved for the chat model; this is a separate approval). Default `private_context = false`: with it false, JEV receives only states built from non-private sources (web pages the agent read, document sections), or nothing.
- What is sent: `state` (bounded to `max_state_bytes`), the question wording, the model name. Never ids, tokens, file paths.
- Redaction options, in order: (a) send only the curiosity *topic* line, not the thread; (b) strip names/emails/URLs by pattern; (c) send a GTR-T5 nearest-neighbour summary instead of text (loses fidelity).
- Retention/training terms are unknown: ask TypeSafe before `private_context = true`.

## 5. Logging and metrics

- Every gate call appends to `state_dir/gates/calls.jsonl`: gate, tier (`ollaya`/`jev`), escalation reason, p per tier, verdict, latency, cost, features_sha256 (not the state text).
- Daily rollup for the benchmark plan's cascade curve: accuracy vs escalation rate vs $ vs latency, swept over the escalation threshold τ, against gate-only and always-JEV anchors; fraction of final errors that came from confidently accepted Ollaya answers (the cascade's real failure mode).
- **Second labeler for WP-4:** run JEV over the WP-4 label sets (≥ 500 items per gate per language) as an independent annotator next to the LLM-agreement labels; report Cohen's κ; send disagreements to human adjudication. JEV labels are never the gold label alone. The private WP-4 datasets stay local (branch `v3/wp4-calibration` is unpushed on purpose), so this step needs `private_context = true` and Kord's explicit say-so.

## 6. Phases and exit tests

**J1. Client + wire probe.** `JevClient::decide` with bearer auth from `api_key_env`; maps answers, usage, cost; errors → `ProviderError` with bounded messages. Exit: one live probe per primitive (noul, score, choice) on non-private text recorded to `research/gates/fixtures/jev-*.json` (response only, no headers); unit tests replay the fixtures; a wrong key yields `ProviderError` without the key in the message; `grep` of logs finds no key.

**J2. Cascade on the curiosity gate.** `CascadeGate` at `life.rs:594` behind `[gates.jev] enabled`. Exit: with Ollaya stopped, curiosity still gets a JEV verdict (tier `jev` in the journal); with a 4,000-byte seed, Ollaya abstains `state_truncated` and JEV answers; with `private_context = false` and a conversation-derived seed, no JEV call is made and the journal says why; budget exhaustion abstains visibly.

**J3. Merge gate.** Pass a `CascadeGate` as `merge_gate` in `bhavana_cycle` (`life.rs:820-822`, today `None`). Exit: a scratch consolidation over a fixture with a planted contradiction routes the cluster to review with both judges' p's; disagreement produces a `disputes` flag, never a lifecycle change (still gated on `lifecycle_authorized`).

**J4. Ledger + curve.** Exit: `calls.jsonl` rollup produces the cascade curve for the curiosity and merge gates with a ledger row in `audit/bench/ledger.jsonl`; J4 also produces the WP-4 κ table once Kord approves the private run.

## 7. What we need from Kord

1. The key, saved as `%USERPROFILE%\.config\ferricula\typesafe_api_key` (not in chat, not in the repo).
2. Which endpoint: TypeSafe direct (`api.typesafe.ai`) or via OpenRouter (then the key is an OpenRouter key).
3. TypeSafe's API reference (request/response schema, error format), the model to pin, context limit, pricing and rate limits as they apply to this account.
4. Data retention/training terms, and a decision on `private_context` (what private state, if any, may be sent).
