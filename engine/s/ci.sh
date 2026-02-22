#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/lib/ui.sh"

GH_REPO="gridlhq/flapjack"

banner "Trigger CI" "$GH_REPO"

spin_start "Triggering CI workflow"
gh workflow run ci.yml --repo "$GH_REPO"
spin_stop success "CI workflow triggered"
echo ""

next_steps \
  "gh run watch --repo $GH_REPO" \
  "gh run list --repo $GH_REPO --workflow=ci.yml --limit=1"
echo ""
