---
name: release-manager
description: Manage the release process for LibrAgent. Use this skill to analyze changes, update the changelog intelligentlly, and publish new versions.
---

# Release Manager Skill

This skill defines the standard procedure for releasing a new version of LibrAgent. You (the Agent) act as the Release Manager, responsible for understanding the changes and communicating them clearly to users.

## Workflow Overview

1.  **Analyze**: Read git history to understand what changed.
2.  **Document**: Intelligently update `CHANGELOG.md` with a user-facing summary.
3.  **Verify & Publish**: Use the `release.sh` script to run tests, build, and publish.

## Step-by-Step Instructions

### 1. Analysis (The "Brain" Work)

First, determine what has changed since the last release.

```bash
# Find the last tag
git describe --tags --abbrev=0

# List commits since that tag
git log $(git describe --tags --abbrev=0)..HEAD --pretty=format:"%h %s"
```

**Task**: Read the commit messages. Group them mentally into:

- **Features**: New capabilities, UI improvements (User facing).
- **Fixes**: Bug fixes (User facing).
- **Refactoring/Internal**: Code cleanup, testing, dev scripts (Developer facing).

### 2. Update Changelog

Read the current `CHANGELOG.md`.

```bash
read_file CHANGELOG.md
```

**Task**: Edit `CHANGELOG.md` to insert a new section for the upcoming version.

- **Format**: Follow the existing style (`## [Version] - YYYY-MM-DD`).
- **Content**: Summarize the changes identified in Step 1.
  - _Do not_ just copy-paste commit messages.
  - _Do_ consolidate related small commits into one meaningful bullet point.
  - _Do_ filter out trivial internal changes (like "fix typo", "update script") unless significant.
  - _Do_ use emojis (🚀, 🐛, 🔧) consistent with the file style.

### 3. Commit Documentation

Once the changelog is updated, commit it. This ensures the release script runs on a clean state.

```bash
git add CHANGELOG.md
git commit -m "docs: update changelog for v<NEW_VERSION>"
```

### 4. Verification & Publishing (The "Grunt" Work)

Finally, use the provided script to handle the mechanical parts: running tests, building, bumping versions, and tagging.

```bash
./scripts/release.sh <patch|minor|major|version>
```

- **Checks**: The script will abort if tests fail.
- **Automation**: The script updates `package.json`, `Cargo.toml`, etc., creates the git tag, and pushes to GitHub.
