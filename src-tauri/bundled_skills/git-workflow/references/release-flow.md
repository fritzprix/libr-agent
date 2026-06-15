# Release Flow

For **the user's project** on GitHub — not LibrAgent product releases.

## Steps

1. Ensure `default_branch` is green and merged PRs match the release scope.
2. `log_since_tag` — draft user-facing notes (features, fixes, breaking).
3. User confirms version tag (`vX.Y.Z` semver).
4. `release_create` with `--notes-file` or `--generate-notes`.
5. Report release URL.

## Notes quality

- Group by Features / Fixes / Breaking
- Do not paste raw commit hashes only — summarize impact
- Link notable PR numbers when helpful

## Tags

- Prefer annotated tags via `gh release create` (creates tag)
- Use `--draft` for dry-run releases
- Use `--prerelease` for beta/rc
