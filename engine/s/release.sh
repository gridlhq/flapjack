#!/usr/bin/env bash
set -euo pipefail

GH_REPO="gridlhq/flapjack"

# Get latest version from git tags, or default to 0.0.0
LATEST_TAG=$(gh release view --repo "$GH_REPO" --json tagName -q .tagName 2>/dev/null | sed 's/^v//' || echo "0.0.0")
LATEST_VERSION="${LATEST_TAG%-*}"  # Strip any existing pre-release suffix (e.g. -beta)

# Parse version components
IFS='.' read -r major minor patch <<< "$LATEST_VERSION"

# Auto-increment patch version with -beta suffix
NEW_PATCH=$((patch + 1))
AUTO_VERSION="${major}.${minor}.${NEW_PATCH}-beta"

# Use provided version or auto-incremented version
VERSION="${1:-$AUTO_VERSION}"

echo "Latest release: v${LATEST_TAG}"
echo "Triggering release v${VERSION} on $GH_REPO..."
gh workflow run release.yml --repo "$GH_REPO" -f version="$VERSION"
echo ""
echo "Release workflow started. Watch progress:"
echo "  gh run list --repo $GH_REPO --workflow=release.yml --limit=1"
echo "  gh run watch --repo $GH_REPO"
