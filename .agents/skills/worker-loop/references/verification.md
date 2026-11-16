# Verification & Challenges

## Verification Rules

Run every command from `$WT`.

- **Rust**:
  `pnpm rust:test --lib <filter>` or `pnpm rust:test --test <name>`, `pnpm rust:check`, `pnpm rust:clippy`, `pnpm rust:fmt:check`.
  **Never raw `cargo test/build/clippy`** (prevents high memory usage and system freeze).
- **Frontend**:
  Targeted `pnpm vitest run <path>`, `pnpm lint` on changed files, `pnpm tsc --noEmit` when types change.
- **Skills**:
  `python3 src-tauri/bundled_skills/skill-creator/scripts/validate_skill.py --strict <dir>`.
- **Full pipeline**:
  Never run `pnpm refactor:validate` unless explicitly requested by the user.

## CI after skipped local checks

If a local check was skipped (Rust build-slot wait, timeout, OS-only failure) and the work was pushed with "left to CI":

1. On the next tick where that PR has `failingChecks`, open `gh run view <id> --log-failed` before any other action on that PR.
2. Fix the cause shown in the log (or file/link a base-branch issue if the failure is unrelated to this PR's diff).
3. Do not post a gatekeeper/`worker-loop: response` that ignores an unread red CI on the same head.
