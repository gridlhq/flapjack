#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if ! command -v cargo &> /dev/null; then
    echo "cargo not found"
    exit 1
fi

cd "$REPO_ROOT"

echo "Building flapjack..."
cargo build --release

if [ $? -ne 0 ]; then
    echo "Build failed"
    exit 1
fi

echo "Cleaning data directory..."
rm -rf "$SCRIPT_DIR/data"
mkdir -p "$SCRIPT_DIR/data"

echo "Killing existing flapjack servers..."
pkill -x flapjack || true
sleep 0.5

echo "Starting flapjack server..."
FLAPJACK_DATA_DIR="$SCRIPT_DIR/data" ./target/release/flapjack > "$SCRIPT_DIR/server.log" 2>&1 &
SERVER_PID=$!

cleanup() {
    echo "Stopping server..."
    kill $SERVER_PID 2>/dev/null || true
    wait $SERVER_PID 2>/dev/null || true
}
trap cleanup EXIT

echo "Waiting for server startup..."
for i in {1..30}; do
    if curl -s http://localhost:7700/health > /dev/null 2>&1; then
        echo "Server ready"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "Server failed to start after 30 seconds"
        exit 1
    fi
    sleep 1
done

cd "$SCRIPT_DIR"
npm test