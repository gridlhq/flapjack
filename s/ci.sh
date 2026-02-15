#!/usr/bin/env bash
set -euo pipefail

echo "Triggering CI workflow..."
gh workflow run ci.yml
echo ""
echo "CI workflow started. Watch progress:"
echo "  gh run list --workflow=ci.yml --limit=1"
echo "  gh run watch"
