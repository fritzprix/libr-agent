---
name: review-local-changes
description: Comprehensive review and audit of uncommitted local code changes in git working copy (staged, unstaged, untracked). Use when asked to review local changes, audit git diff, check uncommitted work before commit/PR/push, or inspect working copy edits for potential bugs, security issues, formatting, or regression risks.
---

# Review Local Code Changes

Use this skill to perform a thorough, evidence-based review of uncommitted local code modifications in the working repository.

---

## Workflow

### 1. Identify Working Copy State

Run git commands to inspect modified, staged, and untracked files:

```powershell
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

### 4. Run Project Validation (Optional / Recommended)

When project scripts or build tools are available, verify code health:

- For TypeScript/React: `pnpm lint`, `pnpm build`, `pnpm test` (or `pnpm refactor:validate` in LibrAgent)
- For Rust: `cargo clippy`, `cargo test --tests`

Report exact compiler errors, linter output, or failing test names if validation fails.

### 5. Structure the Review Report

Present review findings clearly in the following structure:

1. **Executive Summary**: High-level overview of modified files and purpose of changes.
2. **Critical Issues / Blockers** (if any): Logic bugs, security vulnerabilities, breaking changes, or build failures.
3. **Warnings & Code Quality** (if any): Type safety gaps, missing error handling, debug artifacts, performance concerns.
4. **Suggestions & Best Practices**: Cleanups, readability improvements, or minor refactorings.
5. **Validation Status**: Results of lint/build/test checks.
6. **Verdict**: `Approved`, `Approved with minor suggestions`, or `Needs changes before commit`.
