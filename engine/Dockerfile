# syntax=docker/dockerfile:1
# Multi-stage build for Flapjack server.
# Build context must be the repository root (not engine/):
#   docker build -f engine/Dockerfile .

# ── Stage 1: Build dashboard ─────────────────────────────────────────────────
FROM node:20-alpine AS dashboard-builder
WORKDIR /app
COPY engine/dashboard/package.json engine/dashboard/package-lock.json ./
RUN npm ci
COPY engine/dashboard/ .
RUN npm run build

# ── Stage 2: Build Rust binary ───────────────────────────────────────────────
FROM rust:1 AS builder
WORKDIR /app

# Copy manifests
COPY engine/Cargo.toml engine/Cargo.lock ./

# Copy workspace crates
COPY engine/src ./src
COPY engine/flapjack-http ./flapjack-http
COPY engine/flapjack-server ./flapjack-server
COPY engine/flapjack-replication ./flapjack-replication
COPY engine/flapjack-ssl ./flapjack-ssl
COPY engine/benches ./benches
COPY engine/tests ./tests
COPY engine/package ./package

# Copy built dashboard (required by RustEmbed)
COPY --from=dashboard-builder /app/dist ./dashboard/dist

# Build release binary (glibc — vector-search compatible, unlike musl targets)
RUN cargo build --release -p flapjack-server --features vector-search

# ── Stage 3: Runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/flapjack /usr/local/bin/flapjack
COPY engine/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

RUN mkdir -p /data

EXPOSE 7700

WORKDIR /data

ENTRYPOINT ["entrypoint.sh"]
CMD ["flapjack", "--data-dir", "/data"]
