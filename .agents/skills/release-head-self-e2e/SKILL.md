---
name: release-head-self-e2e
description: >
  Cursor-side pointer to the in-app LibrAgent skill for release→HEAD self-E2E
  (tracking ticket + delegated executor/verifier children). Use when editing or
  documenting that workflow from Cursor; runtime instructions live in the bundled
  skill under src-tauri/bundled_skills/release-head-self-e2e/. Not a substitute
  for libr-delegate HTTP harness. Triggers: release-head-self-e2e skill, HEAD 릴리스 E2E.
---

# Release → HEAD self-E2E (Cursor)

**Canonical skill (LibrAgent agents):** `src-tauri/bundled_skills/release-head-self-e2e/SKILL.md`

Load that path when verifying bundled content. In a **running LibrAgent** session, invoke `@skill:release-head-self-e2e`.

## Cursor vs in-app

| Runner | Mechanism |
| --- | --- |
| LibrAgent parent orchestrator | `@skill:release-head-self-e2e` + `@skill:delegate` + optional `@skill:delegation-eval-loop` |
| Cursor external bench | `.agents/skills/libr-delegate` and/or `pnpm bench:session-e2e` (Harbor — secondary) |

## Scopes

See `src-tauri/bundled_skills/release-head-self-e2e/references/scopes.md`. Creating the skill does **not** authorize GitHub issue creation; each run needs an explicit `github-write` phrase.

## Scripts (repo root)

```bash
python3 src-tauri/bundled_skills/release-head-self-e2e/scripts/pin_release_range.py --repo . --json
```

Examples: bundled `references/usage.md`.
