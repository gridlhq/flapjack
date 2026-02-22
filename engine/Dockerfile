# Multi-stage build for Flapjack server
FROM rust:1.93 as builder

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
COPY engine/dashboard/dist ./dashboard/dist

# Build release binary
RUN cargo build --release -p flapjack-server --features vector-search

# Runtime stage — must match builder's glibc version
FROM debian:trixie-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/flapjack /usr/local/bin/flapjack

# Create data directory
RUN mkdir -p /data

# Expose default port
EXPOSE 7700

# Copy entrypoint script (writes node.json from env vars)
COPY engine/_dev/docker/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

# Set working directory for data
WORKDIR /data

ENTRYPOINT ["entrypoint.sh"]
CMD ["flapjack"]
