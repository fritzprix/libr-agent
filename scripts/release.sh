#!/bin/bash
set -e

if [ -z "$1" ]; then
  echo "Usage: $0 <patch|minor|major|version>"
  exit 1
fi

# Check for clean git state
if [ -n "$(git status --porcelain)" ]; then
  echo "Error: Git working directory is not clean."
  exit 1
fi

# Check for gh CLI
if ! command -v gh &> /dev/null; then
    echo "Error: GitHub CLI (gh) is not installed."
    exit 1
fi

echo ">>> Bumping version..."
NEW_VERSION=$(node scripts/bump-version.cjs "$1" | tail -n 1)
echo "New Version: $NEW_VERSION"

echo ">>> Building project (this may take a while)..."
# Allow tauri build to fail (e.g. AppImage failure) as long as deb is created
pnpm tauri build || true

# Check for both lowercase and CamelCase filenames
DEB_PATH_LOWER="src-tauri/target/release/bundle/deb/libragent_${NEW_VERSION}_amd64.deb"
DEB_PATH_CAMEL="src-tauri/target/release/bundle/deb/LibrAgent_${NEW_VERSION}_amd64.deb"

if [ -f "$DEB_PATH_LOWER" ]; then
    DEB_PATH="$DEB_PATH_LOWER"
elif [ -f "$DEB_PATH_CAMEL" ]; then
    DEB_PATH="$DEB_PATH_CAMEL"
else
    echo "Error: .deb file not found at $DEB_PATH_LOWER or $DEB_PATH_CAMEL"
    exit 1
fi

echo ">>> Committing and Tagging..."
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json aur/PKGBUILD snap/snapcraft.yaml
git commit -m "chore(release): bump to v$NEW_VERSION"
git tag "v$NEW_VERSION"

echo ">>> Pushing to GitHub..."
git push origin dev/0.3.x
git push origin "v$NEW_VERSION"

echo ">>> Creating GitHub Release..."
gh release create "v$NEW_VERSION" "$DEB_PATH" --generate-notes --title "v$NEW_VERSION"

echo ">>> Updating AUR..."
# Create a temporary dir for AUR
mkdir -p aur_deploy
cd aur_deploy
# Clone AUR repo (assuming ssh access is set up)
if [ ! -d ".git" ]; then
    git clone ssh://aur@aur.archlinux.org/libragent.git .
fi
git pull origin master
cp ../aur/PKGBUILD ../aur/libragent.desktop .
makepkg --printsrcinfo > .SRCINFO
git add PKGBUILD libragent.desktop .SRCINFO
git commit -m "Release v$NEW_VERSION"
git push origin master
cd ..
rm -rf aur_deploy

echo ">>> AUR Update Complete."

# Snapcraft step (Optional)
if command -v snapcraft &> /dev/null; then
    echo ">>> Building Snap..."
    cd snap
    snapcraft
    # If successful, you can upload with: snapcraft upload --release=stable libragent_*.snap
    echo "Snap built. You can upload it manually using 'snapcraft upload'."
    cd ..
else
    echo ">>> Snapcraft not found. Skipping Snap build."
    echo "To build snap: Install snapcraft and run 'cd snap && snapcraft'"
fi

echo ">>> Release v$NEW_VERSION completed successfully!"
