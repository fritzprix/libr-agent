# Release Process

This document outlines the process for releasing new versions of SynapticFlow.

## 1. Versioning

- **Semantic Versioning**: The project follows Semantic Versioning (SemVer).
- **Tags**: Each release is marked with a Git tag (e.g., `v1.2.3`).

## 2. Release Checklist

1.  **Update Version**: Update the version number in `package.json` and `Cargo.toml`.
2.  **Run Tests**: Ensure that all tests are passing.
3.  **Update Changelog**: Update the `CHANGELOG.md` file with the latest changes.
4.  **Create Release Branch**: Create a new release branch from `main`.
5.  **Build a Production Version**: Create a production build using `pnpm tauri build`.
6.  **Create Git Tag**: Create a new Git tag for the release.
7.  **Push to GitHub**: Push the release branch and tag to GitHub.
8.  **Create GitHub Release**: Create a new release on GitHub with the release notes and build artifacts.

## 3. Automated Releases

- **GitHub Actions**: The release process is automated using GitHub Actions. The `release.yml` workflow handles the building, tagging, and releasing of new versions.
