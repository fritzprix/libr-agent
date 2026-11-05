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
