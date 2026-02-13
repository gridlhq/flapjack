#!/bin/bash
# https://claude.ai/chat/4ac0f8a5-d9f6-450f-84d1-f0d4f2a55cba
# Key changes:

# Added flag parsing at the top that sets USE_RELEASE based on presence of --release flag
# Modified start() to conditionally build and use debug or release based on the flag
# Changed stop() to use return 0 instead of exit 0 so it doesn't exit the script when called from restart()
# Now restart will start the server even if it's already stopped (which is a good idea - "restart" should be idempotent)
# Updated usage message to show the optional --release flag

# Usage:

# ./script.sh start - builds and starts debug version
# ./script.sh start --release - builds and starts release version
# ./script.sh restart --release - restarts with release version

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"  # Go up from engine/_dev/s/ to root
DATA_DIR="$REPO_ROOT/engine/_dev/dev-data"
PID_FILE="$DATA_DIR/server.pid"
LOG_FILE="$DATA_DIR/server.log"

# Parse flags
USE_RELEASE=false
for arg in "$@"; do
    if [ "$arg" = "--release" ]; then
        USE_RELEASE=true
    fi
done

start() {
    if [ -f "$PID_FILE" ]; then
        PID=$(cat "$PID_FILE")
        if ps -p $PID > /dev/null 2>&1; then
            echo "Server already running (PID $PID)"
            exit 1
        fi
        rm "$PID_FILE"
    fi
    
    mkdir -p "$DATA_DIR"
    
    cd "$REPO_ROOT/engine"

    if [ "$USE_RELEASE" = true ]; then
        echo "Building release version..."
        cargo build --release -p flapjack-server
        BUILD_TYPE="release"
    else
        echo "Building debug version..."
        cargo build -p flapjack-server
        BUILD_TYPE="debug"
    fi

    echo "Starting server ($BUILD_TYPE build, data=$DATA_DIR)..."
    RUST_LOG=warn FLAPJACK_DATA_DIR="$DATA_DIR" ./target/$BUILD_TYPE/flapjack > "$LOG_FILE" 2>&1 &
    echo $! > "$PID_FILE"
    
    for i in {1..10}; do
        if curl -s http://localhost:7700/health > /dev/null 2>&1; then
            echo "Server started (PID $(cat $PID_FILE))"
            echo "Logs: $LOG_FILE"
            exit 0
        fi
        sleep 0.5
    done
    
    echo "Server failed to start. Check $LOG_FILE"
    exit 1
}

stop() {
    if [ ! -f "$PID_FILE" ]; then
        echo "Server not running"
        return 0
    fi
    
    PID=$(cat "$PID_FILE")
    if ps -p $PID > /dev/null 2>&1; then
        kill $PID
        rm "$PID_FILE"
        echo "Server stopped"
    else
        echo "Server not running (stale PID file)"
        rm "$PID_FILE"
    fi
}

restart() {
    stop
    sleep 1
    start
}

logs() {
    if [ ! -f "$LOG_FILE" ]; then
        echo "No logs found"
        exit 1
    fi
    tail -f "$LOG_FILE"
}

# Remove --release flag from command arguments
COMMAND=""
for arg in "$@"; do
    if [ "$arg" != "--release" ]; then
        COMMAND="$arg"
    fi
done

case "${COMMAND:-}" in
    start) start ;;
    stop) stop ;;
    restart) restart ;;
    logs) logs ;;
    *)
        echo "Usage: $0 {start|stop|restart|logs} [--release]"
        exit 1
        ;;
esac


# #!/bin/bash
# set -e

# SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
# DATA_DIR="$REPO_ROOT/dev-data"
# PID_FILE="$DATA_DIR/server.pid"
# LOG_FILE="$DATA_DIR/server.log"

# start() {
#     if [ -f "$PID_FILE" ]; then
#         PID=$(cat "$PID_FILE")
#         if ps -p $PID > /dev/null 2>&1; then
#             echo "Server already running (PID $PID)"
#             exit 1
#         fi
#         rm "$PID_FILE"
#     fi
    
#     mkdir -p "$DATA_DIR"
    
#     cd "$REPO_ROOT"
#     cargo build --release
#     # cargo build
    
#     echo "Starting server (debug build, data=$DATA_DIR)..."
#     # RUST_LOG=warn FLAPJACK_DATA_DIR="$DATA_DIR" ./target/debug/flapjack > "$LOG_FILE" 2>&1 &
#     RUST_LOG=warn FLAPJACK_DATA_DIR="$DATA_DIR" ./target/release/flapjack > "$LOG_FILE" 2>&1 &
#     echo $! > "$PID_FILE"
    
#     for i in {1..10}; do
#         if curl -s http://localhost:7700/health > /dev/null 2>&1; then
#             echo "Server started (PID $(cat $PID_FILE))"
#             echo "Logs: $LOG_FILE"
#             exit 0
#         fi
#         sleep 0.5
#     done
    
#     echo "Server failed to start. Check $LOG_FILE"
#     exit 1
# }

# stop() {
#     if [ ! -f "$PID_FILE" ]; then
#         echo "Server not running"
#         exit 0
#     fi
    
#     PID=$(cat "$PID_FILE")
#     if ps -p $PID > /dev/null 2>&1; then
#         kill $PID
#         rm "$PID_FILE"
#         echo "Server stopped"
#     else
#         echo "Server not running (stale PID file)"
#         rm "$PID_FILE"
#     fi
# }

# restart() {
#     stop
#     sleep 1
#     start
# }

# logs() {
#     if [ ! -f "$LOG_FILE" ]; then
#         echo "No logs found"
#         exit 1
#     fi
#     tail -f "$LOG_FILE"
# }

# case "${1:-}" in
#     start) start ;;
#     stop) stop ;;
#     restart) restart ;;
#     logs) logs ;;
#     *)
#         echo "Usage: $0 {start|stop|restart|logs}"
#         exit 1
#         ;;
# esac