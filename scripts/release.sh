#!/bin/bash
set -e

if [ -z "$1" ]; then
  echo "Usage: $0 <patch|minor|major|version>"
  exit 1
fi

# Check for clean git state
if [ -n "$(git status --porcelain)" ]; then
  echo "Error: Git working directory is not clean. Please commit or stash changes."
  exit 1
fi

echo ">>> Generating Changelog Draft..."
node scripts/generate-changelog.cjs

echo ""
echo ">>> Review the draft above."
echo ">>> Please open CHANGELOG.md and update it with the new version notes."
echo ">>> Do NOT commit the changes yet (save the file)."
read -p ">>> Press Enter when you have updated CHANGELOG.md (or Ctrl+C to abort)..."

echo ">>> Cleaning up artifacts (logs, temp files)..."
rm -f *.log *.txt
rm -rf temp/ test-results/
echo "Cleanup complete."

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
read -p ">>> Ready to bump version to next '$1' and publish? (y/n) " -n 1 -r
echo ""
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 1
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
