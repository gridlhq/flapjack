#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"  # Go up from engine/_dev/s/ to root
DATA_DIR="$REPO_ROOT/engine/_dev/dev-data"

# Parse flags
USE_RELEASE=false
for arg in "$@"; do
    if [ "$arg" = "--release" ]; then
        USE_RELEASE=true
    fi
done

build_dashboard() {
    local DASH_DIR="$REPO_ROOT/engine/dashboard"
    local DIST="$DASH_DIR/dist/index.html"

    # Skip if dist is fresh (index.html exists and is newer than src/)
    if [ -f "$DIST" ] && [ -z "$(find "$DASH_DIR/src" -newer "$DIST" -print -quit 2>/dev/null)" ]; then
        return 0
    fi

    echo "Building dashboard..."
    cd "$DASH_DIR"
    if [ ! -d node_modules ]; then
        npm ci --silent
    fi
    npx vite build --logLevel error
    cd "$REPO_ROOT/engine"
}

start() {
    # Kill anything already on the port
    EXISTING=$(lsof -ti :7700 -sTCP:LISTEN 2>/dev/null || true)
    if [ -n "$EXISTING" ]; then
        echo "Stopping existing server (PID $EXISTING)..."
        kill $EXISTING 2>/dev/null || true
        sleep 1
    fi

    mkdir -p "$DATA_DIR"
    cd "$REPO_ROOT/engine"

    build_dashboard

    if [ "$USE_RELEASE" = true ]; then
        cargo build --release -p flapjack-server
        BUILD_TYPE="release"
    else
        cargo build -p flapjack-server
        BUILD_TYPE="debug"
    fi

    # Source .env.secret if it exists (provides FLAPJACK_ADMIN_KEY, etc.)
    local ENV_FILE="$REPO_ROOT/engine/.secret/.env.secret"
    if [ -f "$ENV_FILE" ] && [ -z "$FLAPJACK_ADMIN_KEY" ]; then
        eval "$(grep '^FLAPJACK_ADMIN_KEY=' "$ENV_FILE")"
        export FLAPJACK_ADMIN_KEY
    fi

    # Run the binary directly — its output IS the output
    env RUST_LOG=warn FLAPJACK_DATA_DIR="$DATA_DIR" ${FLAPJACK_ADMIN_KEY:+FLAPJACK_ADMIN_KEY="$FLAPJACK_ADMIN_KEY"} ./target/$BUILD_TYPE/flapjack
}

stop() {
    EXISTING=$(lsof -ti :7700 -sTCP:LISTEN 2>/dev/null || true)
    if [ -n "$EXISTING" ]; then
        kill $EXISTING 2>/dev/null || true
        echo "Server stopped (PID $EXISTING)"
    else
        echo "Server not running"
    fi
}

restart() {
    stop
    sleep 1
    start
}

# Remove --release flag from command arguments
COMMAND=""
for arg in "$@"; do
    if [ "$arg" != "--release" ]; then
        COMMAND="$arg"
    fi
done

case "${COMMAND:-start}" in
    start) start ;;
    stop) stop ;;
    restart) restart ;;
    *)
        echo "Usage: $0 {start|stop|restart} [--release]"
        exit 1
        ;;
esac
