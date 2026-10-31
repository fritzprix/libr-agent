# Verification & Challenges

## Verification Rules

Run every command from `$WT`.

- **Rust**:
  `pnpm rust:test --lib <filter>` or `pnpm rust:test --test <name>`, `pnpm rust:clippy`, `pnpm rust:fmt`.
  **Never raw `cargo test/build/clippy`** (prevents high memory usage and system freeze).
- **Frontend**:
  Targeted `pnpm vitest run <path>`, `pnpm lint` on changed files, `pnpm tsc --noEmit` when types change.
- **Skills**:
  `python3 src-tauri/bundled_skills/skill-creator/scripts/validate_skill.py --strict <dir>`.
- **Full pipeline**:
  Never run `pnpm refactor:validate` unless explicitly requested by the user.

## Challenges to the Loop's Behavior

When the user says the loop did something wrong (e.g. "it is only waiting for PRs"):
1. Check `state.json` `history`, `skipped`, and `needsUser` before answering.
2. If the records contradict the claim, do not agree: explain what the loop actually did, citing the records.
3. Say "I will change X" only when behavior actually changes (a skill edit or a state change); never present existing behavior as a fix.
