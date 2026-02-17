# Multi-stage build for Flapjack server
FROM rust:1.91 as builder

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

# Build release binary
RUN cargo build --release --bin flapjack

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/flapjack /usr/local/bin/flapjack

# Create data directory
RUN mkdir -p /data

# Expose default port
EXPOSE 7700

# Set working directory for data
WORKDIR /data

# Run the server
CMD ["flapjack"]
