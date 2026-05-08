# ── Stage 1: builder ────────────────────────────────────────────────────────
FROM rust:1.94-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY Cargo.toml ./
COPY Cargo.lock* ./

# Pre-cache dependency build with a dummy main.
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release || true
RUN rm -rf src target/release/quiz-helper target/release/deps/quiz_helper*

COPY src ./src
COPY migrations ./migrations

ENV SQLX_OFFLINE=true
RUN cargo build --release

# ── Stage 2: runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/quiz-helper /app/quiz-helper
COPY --from=builder /app/migrations /app/migrations
COPY static /app/static

EXPOSE 8080

CMD ["/app/quiz-helper"]
