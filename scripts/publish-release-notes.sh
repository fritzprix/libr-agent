#!/bin/bash
set -e

if [ -z "$1" ]; then
  echo "Usage: $0 <version>"
  echo "Example: $0 0.4.0"
  exit 1
fi

VERSION=$1
TAG="v$VERSION"

# Check if tag exists
if ! git rev-parse "$TAG" >/dev/null 2>&1; then
    echo "Error: Tag $TAG does not exist"
    exit 1
fi

# Check for gh CLI
if ! command -v gh &> /dev/null; then
    echo "Error: GitHub CLI (gh) is not installed."
    echo "Install it from: https://cli.github.com/"
    exit 1
fi

echo ">>> Publishing release notes for $TAG..."

# Extract changelog content
# Look for [Unreleased] or [VERSION] section
CHANGELOG_CONTENT=$(sed -n "/^## \[\(Unreleased\|$VERSION\)\]/,/^## \[/p" CHANGELOG.md | head -n -1 | tail -n +3)

if [ -z "$CHANGELOG_CONTENT" ]; then
    echo "Error: No changelog content found for version $VERSION"
    echo "Please update CHANGELOG.md with a section for [$VERSION]"
    exit 1
fi

# Check if release already exists
if gh release view "$TAG" >/dev/null 2>&1; then
    echo "Release $TAG already exists."
    read -p "Do you want to update the release notes? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        # Update existing release
        gh release edit "$TAG" --notes "$CHANGELOG_CONTENT"
        echo "✓ Release notes updated for $TAG"
    fi
else
    # Create new draft release
    gh release create "$TAG" \
        --title "Release $TAG" \
        --notes "$CHANGELOG_CONTENT" \
        --draft \
        --verify-tag
    echo "✓ Draft release created: https://github.com/fritzprix/libr-agent/releases/tag/$TAG"
    echo "  Please review and publish the release manually on GitHub."
fi

echo ">>> Done!"
