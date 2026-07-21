---
description: Execute the release pipeline (version bump, changelog, tag, build)
---

Execute the LibrAgent release pipeline.

Steps:

1. Verify `dev/*` → `main` merge policy (no squash merges)
2. Bump version in `package.json` and `src-tauri/Cargo.toml`/`tauri.conf.json`
3. Update `CHANGELOG.md` with release notes
4. Create git tag: `v<version>`
5. Run multi-platform builds: `pnpm tauri build`
6. Create GitHub Release with artifacts

Platform scripts:

- Unix/macOS: `bash scripts/release.sh`
- Windows: `powershell scripts/release.ps1`

Use the `release-manager` skill for detailed guidance.
