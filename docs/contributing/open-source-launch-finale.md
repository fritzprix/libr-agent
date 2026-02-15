# Open Source Launch Finale Playbook

This is the final runbook before public launch.
Use this when you want high confidence, clean messaging, and zero panic.

## 1) T-7 to T-2: Stabilize

- Freeze risky refactors unless they fix release blockers.
- Run full validation loop daily:
  - `pnpm refactor:validate`
- Confirm `CHANGELOG.md` includes only user-visible changes.
- Confirm migration path is reversible/safe for current release scope.

## 2) T-1: Release Candidate Gate

Run this exact sequence on the release branch:

1. `pnpm install`
2. `pnpm refactor:validate`
3. `pnpm tauri build`

Release candidate gate is **pass only** when:

- lint/format/build pass,
- critical user flows work (session create, resume, delete),
- no blocker regressions in lineage/tree UX,
- release artifacts are generated successfully.

## 3) Launch Day: Execution Order

1. Bump version and verify consistency (`package.json`, Cargo crates if applicable).
2. Update `CHANGELOG.md` with final wording.
3. Build production artifacts: `pnpm tauri build`.
4. Create and push release tag.
5. Publish GitHub release notes using template:
   - `docs/contributing/github-release-notes-template.md`
6. Attach build artifacts to GitHub release.

## 4) Post-Launch (First 24h)

- Watch new issues and triage quickly:
  - `bug`
  - `regression`
  - `release-blocker`
- If severe regression appears:
  - pause promotion,
  - publish known issue note,
  - hotfix on top of tagged baseline.

## 5) Non-Negotiables

- No release without validation evidence.
- No ambiguous release notes.
- No hidden breaking behavior.
- No ego shipping: reliability first.

## One-line launch rule

Ship only what you can explain, reproduce, and recover.
