# Installing and running Ferricula

How to build Ferricula, create an agent, and run it, as the code works today (v3 alpha, branch `v3/r0`). For day-to-day use see [USING.md](USING.md). For how documents in this repository are organized see [DOCUMENTS.md](../DOCUMENTS.md).

## 1. Prerequisites

| Need | For | Notes |
|---|---|---|
| **Docker** | Running the agent | The image builds from `Dockerfile` (Rust `1.96` builder, Debian runtime). |
| **Rust 1.85+** (2024 edition) | Building and testing outside Docker, benchmarks, the `lume` CLI | Build output can be large. Point it at a disk with room using `CARGO_TARGET_DIR`, or a local `.cargo/config.toml` with `[build] target-dir = "…"`. (Kord's machine keeps it on `D:/cargo-target/ferricula-alpha` because `C:` is nearly full.) |

External services the agent talks to. The agent runs without the optional ones and reports what's missing.

| Service | Default address | Role | Required? |
|---|---|---|---|
| **Ollama** | `127.0.0.1:11434` | The language model (chat, reflections, dreams). Any chat model; thinking models need `ollama_reasoning_tokens`. | Yes, for anything that talks |
| **Ollaya** | `127.0.0.1:11435` | Fast judges (gates), e.g. "is this worth researching?". Advisory only until calibrated. `docker run -d --gpus=all -p 127.0.0.1:11435:11435 ghcr.io/ollaya-dev/ollaya:cuda`, then pull `laya`. | Optional |
| **shivvr** | `127.0.0.1:8085` | Text embeddings (`POST /embed`, GTR-T5 768-d) for recall by meaning. Configured under `[embeddings]`. | Optional; without it recall is lexical + BM25 only |
| **grub** crawler | `127.0.0.1:6792` | Renders web pages to markdown for URL ingest and curiosity. It can't read PDFs; PDF URLs are fetched directly. | Optional; needed for URLs and curiosity |
| **Radio entropy** | `radio_url` in `[life]`, e.g. `https://sdrrand.nuts.services` | Entropy for curiosity seeds and dreams. The OS RNG is used when it's unreachable, and every draw records its source. | Optional |
| **ComfyUI** | `127.0.0.1:8188` | Dream images. The client is built but not yet wired into the running agent. | Optional, not used yet |

From inside a container, use `host.docker.internal` instead of `127.0.0.1` (the example Steve config does this for Ollama).

## 2. Build

```bash
cargo build --release -p ferricula-server
cargo test --workspace --exclude ferricula-semantic   # semantic's ONNX runtime needs glibc >= 2.38 on Linux
docker build -t ferricula:3.0.0-alpha.0 .
```

The Docker build copies `Cargo.toml`, `Cargo.lock`, `LICENSE.md` and **`THIRD_PARTY_NOTICES.md`** (`Dockerfile:23`); the build fails if any of them is missing. `.dockerignore` is an allowlist, so configs, memories and credentials never enter the image.

## 3. Create an agent

The engine has no built-in persona. An agent is a memory directory plus a config file.

```bash
ferricula-server init ./agents/ada/memory --agent-id ada --name "Ada" --role "a patient reader of papers"
```

This writes `identity.json` (with a birth hexagram cast from the entropy source) and a starter persona `agent.toml` in that directory. Edit the persona to give the agent its voice.

Then make a runtime config:

```bash
cp config/agent.example.toml config/agent.toml   # gitignored; every key is documented in the example
```

Set at least:
- `expected_agent_id` (must match `identity.json`; startup refuses a different agent);
- `memory_dir` and `state_dir`;
- `[models.identity]`;
- the Ollama settings.

`memory_dir` is always opened **read-only**. Everything the agent writes goes under `state_dir`, which must not be inside `memory_dir`. `config/examples/steve/` is a complete example: an agent built from a recovered v1 memory.

`ferricula-server inspect <memory_dir> [--json]` checks a memory directory without starting anything.

## 4. The operator token

Until login moves to nuts.services (planned in `inbox/UI_PLAN.md`), every route except `/` and `/health` needs `Authorization: Bearer <operator token>`. Keep the token **outside the repository**:

```bash
mkdir -p ~/.config/ferricula
openssl rand -hex 32 > ~/.config/ferricula/operator_token
chmod 600 ~/.config/ferricula/operator_token
```

In the container, the entrypoint turns `FERRICULA_OPERATOR_TOKEN_FILE=/run/secrets/…` into `FERRICULA_OPERATOR_TOKEN`, so the value never appears in the image or in `docker inspect`. It does the same for `NUTNEWS_TOKEN`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, and any names listed in `FERRICULA_SECRET_ENV`. Never print the token or put it in a config file.

## 5. Run

### Bare (no container)

Set `bind = "127.0.0.1:8875"` in the config, then:

```bash
export FERRICULA_OPERATOR_TOKEN="$(cat ~/.config/ferricula/operator_token)"
ferricula-server serve --config config/agent.toml
```

### Docker Compose

```bash
FERRICULA_MEMORY_VOLUME=my-agent-memory FERRICULA_CONFIG=./config/agent.toml docker compose up -d --build
```

| Variable | Default | Meaning |
|---|---|---|
| `FERRICULA_CONFIG` | `./config/agent.example.toml` | Host path of the runtime config (mounted read-only) |
| `FERRICULA_MEMORY_VOLUME` | `ferricula-memory` | **Existing** volume holding the agent's memory; mounted read-only; compose never creates or removes it |
| `FERRICULA_RUNTIME_VOLUME` | `ferricula-runtime` | Writable state volume |
| `FERRICULA_IMAGE` | `ferricula:3.0.0-alpha.0` | Image tag |
| `FERRICULA_SECRETS_DIR` | `$HOME/.config/ferricula` | Where the compose secrets (`operator_token`, …) are read from |

`compose.isolated.yaml` is a variant for isolated runs.

### Plain `docker run`

This is how the live Steve runs:

```bash
docker run -d --name my-agent --restart unless-stopped \
  -p 127.0.0.1:18875:8875 \
  --read-only --tmpfs /tmp:size=16m --cap-drop ALL --security-opt no-new-privileges:true \
  --add-host host.docker.internal:host-gateway \
  -e FERRICULA_OPERATOR_TOKEN_FILE=/run/secrets/ferricula_operator_token \
  -v "$HOME/.config/ferricula/operator_token:/run/secrets/ferricula_operator_token:ro" \
  -v "$PWD/config/agent.toml:/app/config/agent.toml:ro" \
  -v my-agent-memory:/data/agent-memory:ro \
  -v my-agent-runtime:/data/agent-runtime \
  ferricula:3.0.0-alpha.0 serve --config /app/config/agent.toml
```

The image runs as a non-root user with a read-only root filesystem. Publish the port on **host loopback only**; the control plane must never be exposed directly.

### Check it

```bash
curl http://127.0.0.1:8875/health          # unauthenticated; the container HEALTHCHECK uses it
TOKEN="$(cat ~/.config/ferricula/operator_token)"
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8875/status
```

On startup the log prints the listen address, an embeddings probe (`embeddings: ok (backend shivvr, …)` or a degraded status), and the meaning-index backfill count.

## 6. Upgrade or redeploy without losing state

All state lives on the runtime volume. That covers conversations, documents, the experience store, drives, the life journal and verdicts. The image holds none of it. To upgrade:

```bash
docker build -t ferricula:3.0.0-alpha.0 .
docker stop my-agent && docker rm my-agent
# re-run the same docker run command (same volumes, same flags)
```

A turn that was mid-inference during a restart is marked `interrupted` and is not replayed. `ferricula-server embed-backfill --config <file>` backfills embeddings offline; don't run it while a server is using the same `state_dir`.

## 7. Troubleshooting

| Symptom | Cause and fix |
|---|---|
| Chat fails with "model returned an empty response" | A thinking model spent its whole output allowance on reasoning. Raise `ollama_reasoning_tokens` (Steve uses 12000). The server retries once with double headroom before failing. |
| Evidence cards cut short, or the model sees little context | Raise `ollama_context_tokens` to the model's real context (the chat input budget is derived from it, at 3 bytes per token, capped at 400 KB). |
| `docker build` fails at `COPY … THIRD_PARTY_NOTICES.md` | The notices file must exist in the repository root. It also carries the lume license notice. |
| Log says `embeddings: degraded` | shivvr is unreachable or in the wrong space. Recall falls back to lexical + BM25, and chat still works. Check `[embeddings] url` and `space`. |
| Gate results in the life journal say `abstain: state_truncated` | Laya's input window is 512 tokens and the gate input was longer. Known issue with curiosity seeds; the gate is advisory, so nothing is blocked. |
| `401` on every route | Missing or wrong bearer token. Check `FERRICULA_OPERATOR_TOKEN_FILE` points at a readable file; the entrypoint refuses to start if it doesn't. |
