#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/lib/ui.sh"

GH_REPO="gridlhq/flapjack"

# Get latest version from git tags, or default to 0.0.0
LATEST_TAG=$(gh release view --repo "$GH_REPO" --json tagName -q .tagName 2>/dev/null | sed 's/^v//' || echo "0.0.0")
LATEST_VERSION="${LATEST_TAG%-*}"  # Strip any existing pre-release suffix

# Parse version components
IFS='.' read -r major minor patch <<< "$LATEST_VERSION"

# Auto-increment patch version with -beta suffix
NEW_PATCH=$((patch + 1))
AUTO_VERSION="${major}.${minor}.${NEW_PATCH}-beta"

VERSION="${1:-$AUTO_VERSION}"

banner "Trigger Release" "v${VERSION}"

kv "Latest" "v${LATEST_TAG}"
kv "New" "v${VERSION}"
echo ""

spin_start "Triggering release workflow"
gh workflow run release.yml --repo "$GH_REPO" -f version="$VERSION"
spin_stop success "Release workflow triggered for v${VERSION}"
echo ""

next_steps \
  "gh run watch --repo $GH_REPO" \
  "gh run list --repo $GH_REPO --workflow=release.yml --limit=1"
echo ""
