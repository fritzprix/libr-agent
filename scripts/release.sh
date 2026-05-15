#!/bin/bash
set -e

if [ -z "$1" ]; then
  echo "Usage: $0 <patch|minor|major|version>"
  exit 1
fi

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
echo ">>> Proceeding to bump version to next '$1' and publish..."

echo ">>> Bumping version..."
NEW_VERSION=$(node scripts/bump-version.cjs "$1" | tail -n 1)
echo "New Version: $NEW_VERSION"

echo ">>> Committing and Tagging..."
# Include CHANGELOG.md if it was modified
git add CHANGELOG.md README*.md package.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json aur/PKGBUILD snap/snapcraft.yaml
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
