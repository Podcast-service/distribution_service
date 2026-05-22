# syntax=docker/dockerfile:1.7

# ─── Build stage ─────────────────────────────────────────────
FROM rust:1.83-slim-bookworm AS builder

WORKDIR /app

RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
 && rm -rf /var/lib/apt/lists/*

# Cache dependencies independently of source changes.
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src \
 && echo 'fn main() {}' > src/main.rs \
 && cargo build --release \
 && rm -rf src target/release/deps/distribution_service*

# Real source.
COPY src ./src
RUN cargo build --release \
 && strip target/release/distribution_service

# ─── Runtime stage ───────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --create-home --uid 10001 app

WORKDIR /app

COPY --from=builder /app/target/release/distribution_service /usr/local/bin/distribution_service

USER app

ENV BIND_ADDR=0.0.0.0:8788 \
    RUST_LOG=distribution_service=info,tower_http=info,sqlx=warn

EXPOSE 8788

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -fsS http://127.0.0.1:8788/health || exit 1

ENTRYPOINT ["/usr/local/bin/distribution_service"]
