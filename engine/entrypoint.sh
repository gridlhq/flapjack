#!/bin/sh
# entrypoint.sh — Docker entrypoint for Flapjack server.
# Writes node.json from env vars when FLAPJACK_NODE_ID and FLAPJACK_PEERS are set
# (used for replication setup). Otherwise just runs the binary directly.
#
# FLAPJACK_PEERS format: "node-id=http://host:port,node-id2=http://host2:port"

set -e

if [ -n "$FLAPJACK_NODE_ID" ] && [ -n "$FLAPJACK_PEERS" ]; then
  DATA_DIR="${FLAPJACK_DATA_DIR:-/data}"
  mkdir -p "$DATA_DIR"

  # Build peers JSON array
  PEERS="["
  FIRST=true
  IFS=','
  for peer in $FLAPJACK_PEERS; do
    PEER_ID="${peer%%=*}"
    PEER_ADDR="${peer#*=}"
    if [ "$FIRST" = true ]; then
      FIRST=false
    else
      PEERS="$PEERS,"
    fi
    PEERS="$PEERS{\"node_id\":\"$PEER_ID\",\"addr\":\"$PEER_ADDR\"}"
  done
  PEERS="$PEERS]"

  BIND="${FLAPJACK_BIND_ADDR:-0.0.0.0:7700}"

  cat > "$DATA_DIR/node.json" <<EOF
{"node_id":"$FLAPJACK_NODE_ID","bind_addr":"$BIND","peers":$PEERS}
EOF

  echo "[entrypoint] Wrote $DATA_DIR/node.json: node=$FLAPJACK_NODE_ID peers=$FLAPJACK_PEERS"
fi

exec "$@"
