---
argument-hint: '[optional: area]'
description: Run the repo's full validation pipeline and summarize failures with actionable fixes
allowed-tools: Bash(pnpm refactor:validate:*), Bash(node:*), Bash(pnpm -v:*), Bash(rustc:*), Bash(cargo:* )
---

# LibrAgent Validation

## Context

- Node version: !`node -v`
- pnpm version: !`pnpm -v`
- Rust toolchain: !`rustc --version`

## Your task

Run the project's full validation pipeline and report a concise result:

1. Execute: `pnpm refactor:validate`
2. If any step fails, identify which step failed (lint, format, build, Rust fmt/clippy, dead-code)
3. Provide minimal, concrete next steps to fix the failure(s)
4. If everything passes, reply with: "Validation PASS — all checks green"

If an optional [area] argument is provided (e.g., "frontend", "rust"), tailor your review and suggestions accordingly.

## Notes

- Do not make code changes in this command. Only run checks and summarize.
- Prefer short bullet points; keep it skimmable.
