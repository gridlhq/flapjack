#!/usr/bin/env bash
set -euo pipefail

# Get latest version from git tags, or default to 0.0.0
LATEST_TAG=$(git tag --sort=-v:refname | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' | head -n1 || echo "v0.0.0")
LATEST_VERSION="${LATEST_TAG#v}"  # Strip 'v' prefix

# Parse version components
IFS='.' read -r major minor patch <<< "$LATEST_VERSION"

# Auto-increment patch version
NEW_PATCH=$((patch + 1))
AUTO_VERSION="${major}.${minor}.${NEW_PATCH}"

# Use provided version or auto-incremented version
VERSION="${1:-$AUTO_VERSION}"

echo "Latest release: v${LATEST_VERSION}"
echo "Triggering release v${VERSION}..."
gh workflow run release.yml -f version="$VERSION"
echo ""
echo "Release workflow started. Watch progress:"
echo "  gh run list --workflow=release.yml --limit=1"
echo "  gh run watch"
