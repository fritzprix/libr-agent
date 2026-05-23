---
name: release-manager
description: Manage the release process for LibrAgent. Use this skill to analyze changes, update the changelog intelligently, and publish new versions.
---

# Release Manager Skill

This skill defines the standard procedure for releasing a new version of LibrAgent. You (the Agent) act as the Release Manager, responsible for understanding the changes and communicating them clearly to users.

## Workflow Overview

1.  **Analyze**: Read git history to understand what changed.
2.  **Document**: Intelligently update `CHANGELOG.md` with a user-facing summary.
3.  **Commit Docs**: Commit changelog updates so release scripts can run on a clean git state.
4.  **Verify & Publish**: Use release scripts to run checks, bump versions, tag, and push.

## Step-by-Step Instructions

### 1. Analysis (The "Brain" Work)

First, determine what has changed since the last release.

```bash
# Find the last tag
git describe --tags --abbrev=0

# Define baseline tag
LAST_TAG=$(git describe --tags --abbrev=0)

# List commits since that tag (exclude merge noise)
git log ${LAST_TAG}..HEAD --no-merges --pretty=format:"%h %s"

# Optional but recommended: inspect impact scope
git diff --name-only ${LAST_TAG}..HEAD
git diff --shortstat ${LAST_TAG}..HEAD
```

**Task**: Read the commit messages. Group them mentally into:

- **Features**: New capabilities, UI improvements (User facing).
- **Fixes**: Bug fixes (User facing).
- **Refactoring/Internal**: Code cleanup, testing, dev scripts (Developer facing).

**Note**: If many merge commits exist, prioritize underlying non-merge commits and actual file diffs over merge titles.

### 2. Update Changelog

Read the current `CHANGELOG.md`.

```bash
cat CHANGELOG.md
```

**Task**: Edit `CHANGELOG.md` to insert a new section for the upcoming version.

- **Format**: Follow the existing style (`## [Version] - YYYY-MM-DD`).
- **Content**: Summarize the changes identified in Step 1.
  - _Do not_ just copy-paste commit messages.
  - _Do_ consolidate related small commits into one meaningful bullet point.
  - _Do_ filter out trivial internal changes (like "fix typo", "update script") unless significant.
  - _Do_ use emojis (🚀, 🐛, 🔧) consistent with the file style.

**Versioning Rule**:

- If releasing from current `x.y.z` with patch bump, prepare `x.y.(z+1)` section.
- Example: if latest tag is `v0.5.8`, draft `## [0.5.9] - YYYY-MM-DD`.

### 3. Commit Documentation

Once the changelog is updated, commit it. This ensures the release script runs on a clean state.

```bash
git add CHANGELOG.md
git commit -m "docs: update changelog for v<NEW_VERSION>"
```

**Critical**: Release scripts require a clean working tree before execution.
Make sure changelog edits are committed first.

### 4. Verification & Publishing (The "Grunt" Work)

Finally, use the provided scripts to handle mechanical steps: tests, build checks, version bump, commit, tag, and push.

```bash
# Linux/macOS
./scripts/release.sh <patch|minor|major|x.y.z>

# Windows PowerShell
./scripts/release.ps1 <patch|minor|major|x.y.z>
```

- **Checks**: Scripts abort on failed checks (`pnpm test:run`, `pnpm rust:test`, `pnpm build`, `cargo check`).
- **Automation**: Scripts run `scripts/bump-version.cjs`, which automatically synchronizes direct download links inside both the root READMEs (including localized `README.ko.md`, `README.zh.md`, etc.) and the frontend integration README `src/README.md`. It also updates release files (`package.json`, `Cargo.toml`, `Cargo.lock`, `tauri.conf.json`, packaging manifests), commits, tags, and pushes.

- **Tagging**: The resulting tag format is `v<NEW_VERSION>`.

## Quick Release Checklist

1. Confirm baseline tag and diff scope.
2. Draft user-facing changelog section (features/fixes/internal).
3. Commit changelog changes.
4. Run release script with `patch`/`minor`/`major` or explicit `x.y.z`.
5. Verify branch push + tag push completed successfully.
6. Confirm GitHub Actions release workflow started.
