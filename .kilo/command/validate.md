---
description: Run the complete quality gate pipeline
---

Run the full validation pipeline for LibrAgent.

Execute in order:

1. `pnpm lint`
2. `pnpm format`
3. `pnpm rust:fmt`
4. `pnpm rust:clippy`
5. `pnpm rust:check:all`
6. `pnpm rust:test`
7. `pnpm build`
8. `pnpm perf:bundle`
9. `pnpm dead-code`

All steps must pass. This is the mandatory post-change validation workflow documented in AGENTS.md.
