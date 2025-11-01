---
argument-hint: '[plan-file]'
description: Execute a refactoring plan from a referenced file — review it, ask follow-ups if needed, otherwise implement and validate
allowed-tools: Bash(pnpm refactor:validate:*), Bash(pnpm build:*), Bash(pnpm lint:*), Bash(git status:*), Bash(git diff:*), Bash(git add:*), Bash(git commit:*), Bash(node:*), Bash(pnpm -v:*)
---

# Refactor from plan file

Use this command to drive a refactor based on a plan document. The plan file should be attached as a file reference (preferred: `@path/to/plan.md`). If the plan is provided as `#file:...`, resolve it to a repository path and include its contents.

## Inputs

- Plan file: $ARGUMENTS
  - Accepted formats: `@docs/history/...md`, `@.../refactoring.md`, or `#file:...` (resolve to an actual repo path)

## Guardrails (project-specific)

- TypeScript: no `any`; prefer precise types or `unknown` with narrowing
- Use centralized logger (`getLogger`) instead of `console.*`
- Rust: correct method vs associated function usage (self vs TypeName::function)
- Tailwind: avoid arbitrary classes unless safelisted; prefer utilities
- Validate with `pnpm refactor:validate` before considering done

## Process

1. Understand the plan
   - Parse the plan file into a checklist of concrete tasks and explicit acceptance criteria.
   - If any ambiguity remains, ask 1–3 concise follow-up questions that unblock execution.
   - If the plan is clear, skip questions and proceed.

2. Implement the refactor
   - Make targeted edits, keeping changes minimal and aligned with the repo’s architecture (features, service layer, web-MCP modules, Rust/Tauri patterns).
   - Add or adjust small tests/docs when they de-risk the change.

3. Validate
   - Run the full validation pipeline and summarize outcomes:
     - !`pnpm refactor:validate`
   - If failures occur, iterate with focused fixes (up to two cycles) and re-run validation.

4. (Optional) Commit
   - If changes are correct and validated, prepare a conventional commit message and commit in one shot:
     - Stage: `git add -A`
     - Commit: `git commit -m "refactor(scope): implement <short summary> (from <plan-file>)"`
   - Only commit if the user confirms or if the argument includes an explicit `--commit` flag.

## Output format

- Plan summary (bullet list of tasks)
- Follow-up questions (only if needed)
- Actions taken (files changed, rationale)
- Validation result (PASS/FAIL with brief notes)
- Next steps (if any)
