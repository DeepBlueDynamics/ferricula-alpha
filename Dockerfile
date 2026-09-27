# syntax=docker/dockerfile:1.7
# Production image for the Ferricula agent runtime (`ferricula-server serve`).
# Persona-neutral: the persona arrives with the memory volume and config.
# Build:  docker compose build          (preferred; see compose.yaml)
# The image contains no config, no secrets, and no agent memory data —
# all three arrive at run time via bind mount, Docker secrets, and volumes.

########################################################################
# Stage 1 — build
########################################################################
# Rust >= 1.85 required: ferricula-core/-cognition/-server are edition 2024.
FROM rust:1.96-slim-bookworm AS builder

WORKDIR /build

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/*

# The .dockerignore allowlist admits only the workspace manifests, license
# files, and crates/ — .runtime/ (agent identity + memory) can never enter
# the build context.
COPY Cargo.toml Cargo.lock LICENSE.md THIRD_PARTY_NOTICES.md ./
COPY crates ./crates
# Example configs are test fixtures for the builder stage only; they never
# reach the runtime image.
COPY config ./config

# Only the server binary and its dependency subtree (core, cognition,
# episode, ingest + search, and ferricula-semantic WITHOUT its `ml` feature)
# are built; ingest is pure Rust (pdf-extract, ureq with rustls/webpki
# roots), so no system TLS or PDF libraries. Text embeddings come from shivvr
# over HTTP; the `ml` ONNX stack in ferricula-semantic is not compiled, so no
# model downloads and no ort linkage happen here.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    # COPY keeps source mtimes; bump them so cargo never reuses a cached
    # crate compiled from older sources in the shared target cache.
    find crates -name '*.rs' -exec touch {} + \
    && cargo test --locked -p ferricula-core --lib \
    && cargo test --locked -p ferricula-episode \
    && cargo test --locked -p ferricula-cognition --lib \
    && cargo test --locked -p ferricula-ingest --lib \
    && cargo test --locked -p ferricula-server --lib \
    && cargo build --release --locked -p ferricula-server \
    && cp target/release/ferricula-server /usr/local/bin/ferricula-server

########################################################################
# Stage 2 — runtime
########################################################################
FROM debian:bookworm-slim AS runtime

# ca-certificates: outbound TLS to Nuts News / model providers (rustls).
# curl: container-local health probe against the unauthenticated /health.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 ferricula \
    && useradd --system --uid 10001 --gid ferricula --no-create-home \
        --shell /usr/sbin/nologin ferricula \
    && mkdir -p /app/config /data/agent-memory /data/agent-runtime \
    && chown ferricula:ferricula /data/agent-runtime

COPY --from=builder /usr/local/bin/ferricula-server /usr/local/bin/ferricula-server

# Docker-secrets shim: for each listed variable, VAR_FILE=/run/secrets/x is
# resolved to VAR=<file contents> in the process environment. Secret VALUES
# therefore never appear in the image, compose file, or `docker inspect`
# environment of the compose service — only the _FILE paths do. Command
# substitution strips a single trailing newline, so `printf '%s'`-written
# and editor-written secret files both work. Additional variable names (e.g.
# a deployment-specific token_env) can be listed, space-separated, in
# FERRICULA_SECRET_ENV.
COPY <<'EOF' /usr/local/bin/ferricula-entrypoint
#!/bin/sh
set -eu
for name in FERRICULA_OPERATOR_TOKEN NUTNEWS_TOKEN ANTHROPIC_API_KEY OPENAI_API_KEY ${FERRICULA_SECRET_ENV:-}; do
    case "$name" in
        *[!A-Za-z0-9_]*|[0-9]*) echo "refusing to start: invalid secret variable name $name" >&2; exit 1 ;;
    esac
    file_var="${name}_FILE"
    eval "file_path=\${$file_var:-}"
    [ -n "$file_path" ] || continue
    eval "existing=\${$name:-}"
    if [ -n "$existing" ]; then
        echo "refusing to start: both $name and $file_var are set" >&2
        exit 1
    fi
    if [ ! -r "$file_path" ]; then
        echo "refusing to start: $file_var points at unreadable $file_path" >&2
        exit 1
    fi
    export "$name"="$(cat "$file_path")"
done
exec /usr/local/bin/ferricula-server "$@"
EOF
RUN chmod 0755 /usr/local/bin/ferricula-entrypoint

USER ferricula
WORKDIR /app

# Control-plane HTTP (health, status, mode control, tasks). Published only on
# the host loopback by compose.yaml — never expose this port directly.
EXPOSE 8875

# /health is deliberately unauthenticated in api.rs, so the probe needs no token.
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8875/health || exit 1

ENTRYPOINT ["/usr/local/bin/ferricula-entrypoint"]
CMD ["serve", "--config", "/app/config/agent.toml"]
