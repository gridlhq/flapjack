#!/bin/bash
# Start the stable flapjack binary for dashboard development.
# This binary is decoupled from ongoing server code changes.
# Uses port 7700 by default (override with FLAPJACK_BIND_ADDR).
#
# Usage: ./scripts/start-stable-server.sh
# Rebuild: cargo build -p flapjack-server --release && cp target/release/flapjack dashboard/bin/flapjack-stable

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BIN="$SCRIPT_DIR/../bin/flapjack-stable"

if [ ! -f "$BIN" ]; then
  echo "Stable binary not found at $BIN"
  echo "Build it with: cargo build -p flapjack-server --release && cp target/release/flapjack dashboard/bin/flapjack-stable"
  exit 1
fi

export FLAPJACK_ADMIN_KEY="${FLAPJACK_ADMIN_KEY:-abcdef0123456789}"
export FLAPJACK_DATA_DIR="${FLAPJACK_DATA_DIR:-/tmp/flapjack-dashboard-clean}"
export FLAPJACK_BIND_ADDR="${FLAPJACK_BIND_ADDR:-127.0.0.1:7700}"

echo "Starting stable flapjack on $FLAPJACK_BIND_ADDR"
echo "  Admin key: $FLAPJACK_ADMIN_KEY"
echo "  Data dir:  $FLAPJACK_DATA_DIR"
echo ""

exec "$BIN"
