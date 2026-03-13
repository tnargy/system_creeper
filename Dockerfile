# ── Stage 1: build & test ──────────────────────────────────────────────────────
FROM rust:1 AS builder

# Install system dependencies required by the workspace crates
RUN apt-get update && apt-get install -y \
    libssl-dev \
    pkg-config \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the full workspace
COPY . .

# Run all non-WASM tests (debug build)
RUN cargo test --verbose \
        --workspace \
        --exclude dashboard

# Build release binaries for the runtime stage
RUN cargo build --release \
        --bin agent \
        --bin collector

# ── Stage 2: minimal runtime image for the collector ──────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/collector ./collector
COPY --from=builder /app/target/release/agent     ./agent

EXPOSE 8080

CMD ["./collector"]
