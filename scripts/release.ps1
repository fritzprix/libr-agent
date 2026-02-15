param(
    [string]$VersionBump
)

$ErrorActionPreference = "Stop"

if (-not $VersionBump) {
    Write-Host "Usage: .\release.ps1 <patch|minor|major|version>"
    exit 1
}

Write-Host ">>> Checking git state..."
$gitStatus = git status --porcelain 2>$null
if ($gitStatus) {
    Write-Host "Error: Git working directory is not clean."
    exit 1
}

Write-Host ">>> Checking for GitHub CLI..."
$ghCommand = Get-Command gh -ErrorAction SilentlyContinue
if (-not $ghCommand) {
    Write-Host "Error: GitHub CLI (gh) is not installed."
    exit 1
}

Write-Host ">>> Running pre-release checks..."
# 1. Frontend Tests
Write-Host "Running frontend tests..."
pnpm test:run
if ($LASTEXITCODE -ne 0) {
    Write-Host "Error: Frontend tests failed"
    exit 1
}

# 2. Backend Tests
Write-Host "Running backend tests..."
pnpm rust:test
if ($LASTEXITCODE -ne 0) {
    Write-Host "Error: Backend tests failed"
    exit 1
}

# 3. Verify Frontend Build (Type check + Build)
Write-Host "Verifying frontend build..."
pnpm build
if ($LASTEXITCODE -ne 0) {
    Write-Host "Error: Frontend build failed"
    exit 1
}

# 4. Verify Backend Compilation
Write-Host "Verifying backend compilation..."
Push-Location src-tauri
cargo check
if ($LASTEXITCODE -ne 0) {
    Pop-Location
    Write-Host "Error: Backend check failed"
    exit 1
}
Pop-Location

Write-Host ">>> Bumping version..."
$newVersionOutput = node scripts/bump-version.cjs $VersionBump 2>&1
$newVersion = $newVersionOutput | Select-Object -Last 1
Write-Host "New Version: $newVersion"

Write-Host ">>> Committing and Tagging..."
git add CHANGELOG.md package.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json aur/PKGBUILD snap/snapcraft.yaml
if ($LASTEXITCODE -ne 0) {
    Write-Host "Error: git add failed"
    exit 1
}

$hasStagedChanges = $true
git diff --cached --quiet
if ($LASTEXITCODE -eq 0) {
    $hasStagedChanges = $false
}

if ($hasStagedChanges) {
    git commit -m "chore(release): bump to v$newVersion"
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Error: git commit failed"
        exit 1
    }
} else {
    Write-Host "No release file changes detected. Skipping commit."
}

$tagName = "v$newVersion"
git rev-parse -q --verify "refs/tags/$tagName" *> $null
if ($LASTEXITCODE -eq 0) {
    Write-Host "Tag $tagName already exists locally. Skipping tag creation."
} else {
    git tag "$tagName"
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Error: git tag failed"
        exit 1
    }
}

Write-Host ">>> Pushing to GitHub..."
$currentBranch = git rev-parse --abbrev-ref HEAD
if ($LASTEXITCODE -ne 0) {
    Write-Host "Error: Could not determine current branch"
    exit 1
}

git push origin "$currentBranch"
if ($LASTEXITCODE -ne 0) {
    Write-Host "Error: git push branch failed"
    exit 1
}

git push origin "$tagName"
if ($LASTEXITCODE -ne 0) {
    Write-Host "Error: git push tag failed"
    exit 1
}

Write-Host ">>> Triggered GitHub Action for Release..."
Write-Host "The release artifacts (deb, AppImage, etc.) will be built and uploaded by GitHub Actions."
Write-Host "Release notes will be automatically extracted from CHANGELOG.md by GitHub Actions."

Write-Host ""
Write-Host ">>> Release v$newVersion completed successfully!"
Write-Host ""
Write-Host "Note: Snapcraft build is handled by GitHub Actions."
Write-Host "      To build snap locally: cd snap; snapcraft"
