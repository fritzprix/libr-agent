---
name: release-manager
description: Manage the release process for LibrAgent. Use this skill to prepare, verify, and publish new versions of the application, including changelog generation and artifact cleanup.
---

# Release Manager Skill

This skill guides the release process for LibrAgent.

## Workflow

1.  **Analyze & Changelog**: Run the release script which will first generate a changelog draft based on conventional commits.
2.  **Edit Changelog**: The script will pause. You should update `CHANGELOG.md` with the new version section using the generated draft.
3.  **Clean & Verify**: The script will clean logs/temp files and run the full test/build suite.
4.  **Publish**: Upon confirmation, it acts as the release authority—bumping versions across all files, tagging, and pushing to GitHub.

## Usage

Run the smart release script from the project root:

```bash
./scripts/release.sh <patch|minor|major|version>
```

### Example

To release a new patch version:

```bash
./scripts/release.sh patch
```

## What the Script Does

1.  **Changelog Draft**: calls `scripts/generate-changelog.cjs` to list `feat`, `fix`, `refactor` commits since the last tag.
2.  **Interactive Pause**: Allows you to edit `CHANGELOG.md` without committing (the script includes it in the release commit).
3.  **Cleanup**: Removes `*.log`, `*.txt`, `temp/`, and `test-results/` to ensure a clean build environment.
4.  **Verification**:
    *   `pnpm test:run` (Frontend tests)
    *   `pnpm rust:test` (Backend tests)
    *   `pnpm build` (Frontend build)
    *   `cargo check` (Backend check)
5.  **Versioning**: Updates `package.json`, `tauri.conf.json`, `Cargo.toml`, `PKGBUILD`, and `snapcraft.yaml`.
6.  **Git Operations**: Commits changes with `chore(release): bump to v...`, tags the commit, and pushes to origin.

## Troubleshooting

- **Dirty Git State**: The script refuses to run if there are uncommitted changes (except during the changelog edit phase). Ensure you commit or stash work before starting.
- **Check Failures**: If any verification step fails (tests, build), the script will exit. Fix the issues and run the script again.
