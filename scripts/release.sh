#!/bin/bash
set -e

# Parse args: version is the first non-flag arg; -y/--yes skips the confirmation prompt.
YES=false
VERSION_ARG=""
for arg in "$@"; do
  case "$arg" in
    -y|--yes) YES=true ;;
    *) VERSION_ARG="$arg" ;;
  esac
done

if [ -z "$VERSION_ARG" ]; then
  echo "Usage: $0 <patch|minor|major|version> [-y]"
  exit 1
fi

set -- "$VERSION_ARG"

# Check for clean git state
if [ -n "$(git status --porcelain)" ]; then
  echo "Error: Git working directory is not clean. Please commit your changes (including CHANGELOG.md) before releasing."
  exit 1
fi

echo ">>> Running pre-release checks..."
# 1. Frontend Tests
echo "Running Frontend Tests..."
pnpm test:run

# 2. Backend Tests
echo "Running Backend Tests..."
pnpm rust:test

# 3. Verify Frontend Build (Type check + Build)
echo "Verifying Frontend Build..."
pnpm build

# 4. Verify Backend Compilation
echo "Verifying Backend Compilation..."
(cd src-tauri && cargo check)

echo ">>> Final Verification passed."
if [ "$YES" = true ]; then
    echo ">>> Ready to bump version to next '$1' and publish? (y/n) y (auto-confirmed)"
else
    read -p ">>> Ready to bump version to next '$1' and publish? (y/n) " -n 1 -r
    echo ""
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Aborted."
        exit 1
    fi
fi

echo ">>> Bumping version..."
NEW_VERSION=$(node scripts/bump-version.cjs "$1" | tail -n 1)
echo "New Version: $NEW_VERSION"

echo ">>> Committing and Tagging..."
# Include CHANGELOG.md if it was modified
git add CHANGELOG.md package.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json aur/PKGBUILD snap/snapcraft.yaml
git commit -m "chore(release): bump to v$NEW_VERSION"
git tag "v$NEW_VERSION"

echo ">>> Pushing to GitHub..."
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
git push origin "$CURRENT_BRANCH"
git push origin "v$NEW_VERSION"

echo ">>> Triggered GitHub Action for Release..."
echo "The release artifacts (deb, AppImage, etc.) will be built and uploaded by GitHub Actions."
echo "Release notes will be automatically extracted from CHANGELOG.md by GitHub Actions."

echo ">>> Release v$NEW_VERSION completed successfully!"
echo ""
echo "Note: Snapcraft build is handled by GitHub Actions."
echo "      To build snap locally: cd snap && snapcraft"
