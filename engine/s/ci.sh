#!/usr/bin/env bash
set -euo pipefail

GH_REPO="gridlhq/flapjack"

echo "Triggering CI workflow on $GH_REPO..."
gh workflow run ci.yml --repo "$GH_REPO"
echo ""
echo "CI workflow started. Watch progress:"
echo "  gh run list --repo $GH_REPO --workflow=ci.yml --limit=1"
echo "  gh run watch --repo $GH_REPO"
