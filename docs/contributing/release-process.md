# Release Process

This document outlines the process for releasing new versions of LibrAgent.

Companion documents:

- `open-source-launch-finale.md` (final launch execution runbook)
- `github-release-notes-template.md` (copy-ready release notes template)

## 1. Versioning

- **Semantic Versioning**: The project follows Semantic Versioning (SemVer).
- **Tags**: Each release is marked with a Git tag (e.g., `v1.2.3`).

## 2. Release Checklist

1. **Update Version**: Update the version number in `package.json` and `Cargo.toml`.
2. **Run Tests**: Ensure that all tests are passing.
3. **Update Changelog**: Update the `CHANGELOG.md` file with the latest changes.
4. **Open PR**: Open `dev/0.8.x` → `main` (or your release integration branch → `main`).
5. **Merge with merge commit**: Use **Create a merge commit** only. **Never squash** release PRs — squash diverges `dev/0.8.x` from `main` and causes repeat merge conflicts on the next PR.
6. **Sync dev after merge**: Immediately merge `main` back into `dev/0.8.x` and push so both branches share history.
7. **Build a Production Version**: Create a production build using `pnpm tauri build`.
8. **Create Git Tag**: Create a new Git tag for the release.
9. **Push to GitHub**: Push tags to GitHub.
10. **Create GitHub Release**: Create a new release on GitHub with the release notes and build artifacts.

### Merge policy (required)

- **Allowed**: merge commit (`Create a merge commit`), rebase merge for short-lived feature branches if needed.
- **Forbidden**: squash merge on `dev/*` → `main` release PRs.
- **Repo setting**: `allow_squash_merge` is disabled on GitHub so squash is not offered in the UI.

After every release merge into `main`:

```bash
git checkout dev/0.8.x
git fetch origin
git merge origin/main
git push origin dev/0.8.x
```

## 3. Automated Releases

- **GitHub Actions**: The release process is automated using GitHub Actions. The `release.yml` workflow handles the building, tagging, and releasing of new versions.
