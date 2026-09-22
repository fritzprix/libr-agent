---
name: review-local-changes
description: Comprehensive review and audit of uncommitted local code changes in git working copy (staged, unstaged, untracked). Use when asked to review local changes, audit git diff, check uncommitted work before commit/PR/push, or inspect working copy edits for potential bugs, security issues, formatting, or regression risks.
---

# Review Local Code Changes

Use this skill to perform a thorough, evidence-based review of uncommitted local code modifications in the working repository.

---

## 🚫 HARD RULE — Rust validation (OOM / system freeze)

LibrAgent `src-tauri/tests/*.rs` files are **separate Cargo test binaries** (~600MB+ each with Tauri). Linking many at once OOMs even a **32GB** machine.

### NEVER

| Forbidden | Why |
| --- | --- |
| `cargo test` | Multi-target link graph → OOM |
| `cargo test --tests` | Same, worse |
| `cargo test --all` / bare `cargo test` in `src-tauri/` | Same |
| `cargo clippy` / `cargo build` / `cargo check` raw | Use pnpm wrappers (jobs/nice caps) |
| Assuming “full suite needs `--test` only” and skipping `pnpm rust:test` when user asked for full validation | Full suite is safe **only** via sequential runner |

### ALWAYS

| Command | When |
| --- | --- |
| `pnpm rust:fmt:check` | Format gate |
| `pnpm rust:check` | Type/compile gate |
| `pnpm rust:clippy` | Lint gate |
| `pnpm rust:test --test <target>` | Touched targets while iterating |
| `pnpm rust:test` | **Required for local-change audit / commit readiness** when Rust behavior changed — full suite, sequential (safe; slow OK) |

`pnpm rust:test` (full) is **required verification**, not optional. Skipping it and only running fmt/check is **not** a functional audit.

`pnpm rust:test` without `--test` is **safe** (sequential). Raw `cargo test --tests` is **not**.

If you (or another agent) already started a raw `cargo test --tests` and the machine is thrashing: **stop it**; switch to `pnpm rust:test --test <name>`.

Target name = basename of `src-tauri/tests/<name>.rs` (e.g. `workspace_skill_access_regression_tests`). Modules under `tests/integration/` run via `--test integration_tests` (Linux/macOS only; skipped on Windows).

---

## Workflow

### 1. Identify Working Copy State

Run git commands to inspect modified, staged, and untracked files:

```bash
git status -s
git diff --stat
```

- Distinguish between **staged** changes (`git diff --cached`), **unstaged** changes (`git diff`), and **untracked files**.
- If a target branch or base branch is specified (e.g., `main`), check branch context using `git branch --show-current`.

### 2. Extract and Inspect Detailed Diffs

Retrieve full diffs for analysis:

- For unstaged edits: `git diff`
- For staged edits: `git diff --cached`
- For all uncommitted working tree changes: `git diff HEAD`
- For untracked files: read file contents if relevant to the change context.

If the diff is large (>300 lines), analyze file by file or by functional area to avoid missing subtle bugs.

### 3. Analyze Code Changes

Evaluate changes against [references/review-checklist.md](references/review-checklist.md) covering:

1. **Correctness & Logic**: Functional intent, edge cases, error handling, async/concurrency.
2. **Type Safety & Contracts**: Explicit types, Zod/schema validations, interface alignment.
3. **Leftovers & Cleanliness**: Unintended `console.log`, `dbg!`, commented code, hardcoded credentials.
4. **Security & Safety**: Command injection, path traversal, secrets leakage.
5. **Performance & Architecture**: Unnecessary re-renders, resource leaks, breaking changes.
6. **Rust test / harness regressions**: new root-level `tests/*.rs` binaries without need; skill-alias write/read split; resource-unsafe scripts.

### 4. Code Health Verification

When verifying code health during a review:

- **TypeScript/React**: `pnpm lint`; run `pnpm test:run` if frontend behavior changed.
- **Rust** — follow the HARD RULE above. **Functional / harness / path / tool changes MUST run tests**, not fmt/check alone:
  1. `pnpm rust:fmt:check`
  2. `pnpm rust:check`
  3. `pnpm rust:clippy` (when touching Rust)
  4. Touched targets: `pnpm rust:test --test <target>` (e.g. `integration_tests` for `tests/integration/*`)
  5. **Default for local-change audit of Rust behavior:** `pnpm rust:test` (full suite, sequential — OOM-safe). Do not skip this and call the audit “done”.
- **🚫 NEVER** raw `cargo test` / `cargo test --tests` / raw `cargo clippy|build|check`.
- Do **not** run `pnpm refactor:validate` unless the user explicitly requested the full pipeline.

Report exact compiler errors, linter output, or failing test names if validation fails.

### 5. Structure the Review Report

Present review findings clearly in the following structure:

1. **Executive Summary**: High-level overview of modified files and purpose of changes.
2. **Critical Issues / Blockers** (if any): Logic bugs, security vulnerabilities, breaking changes, or build failures.
3. **Warnings & Code Quality** (if any): Type safety gaps, missing error handling, debug artifacts, performance concerns.
4. **Suggestions & Best Practices**: Cleanups, readability improvements, or minor refactorings.
5. **Validation Status**: Exact commands run + results (especially which `--test` targets).
6. **Verdict**: `Approved`, `Approved with minor suggestions`, or `Needs changes before commit`.
