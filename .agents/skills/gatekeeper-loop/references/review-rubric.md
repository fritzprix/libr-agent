# Review rubric

Classify every finding. Only **blocker** and **major** become required `G` items; **minor** and **nit** become `S` suggestions and never block approval.

| Severity | Examples |
| --- | --- |
| blocker | Wrong behavior, data loss, security/privacy leak, broken build, failing or deleted tests, migration without rollback path |
| major | Linked issue's acceptance criteria / checklist not met; new logic without tests; unhandled error path that users will hit; violates `agents.md` hard rules (`as any`, raw `cargo`, protected-path edits outside scope); unrelated changes bundled in |
| minor | Naming, small duplication, missing doc/i18n string for a non-critical path, comment noise |
| nit | Style preferences the formatter/linter does not enforce |

## Checklist

1. **Scope** — Read the linked issue (`Fixes #N`). Does the diff deliver what it promises, and nothing unrelated? Large leftovers must be stated in the PR body.
2. **Correctness** — Trace changed code paths, including error and edge cases. Read surrounding code with `git show origin/<head>:<path>`, not just the hunk.
3. **Tests** — New behavior has tests at the right level; tests assert behavior, not implementation detail.
4. **Conventions** — `agents.md`/`CLAUDE.md`: KISS/YAGNI, no unsafe casts, comment density matches surroundings, Rust via `pnpm rust:*`.
5. **Security** — Tool approval/policy, filesystem/shell access, secrets in logs or previews, untrusted input handling.
6. **User-facing** — UI text follows i18n standards (ko/en), docs updated when behavior changes, bundled skill `.force_update` bumped when a bundled `SKILL.md` changes.
7. **PR hygiene** — Semantic title, base is the latest `dev/<version>`, body has `Fixes #N` and a test plan that matches what was run.
8. **CI** — All checks green at the reviewed head before approve.

## Re-review discipline

- Round ≥ 2: review only (a) previous required findings and (b) the incremental diff `reviewedHead..headSha`. Do not raise new findings on unchanged code unless they are blockers.
- A worker **decline** with a sound reason closes the finding. If you still disagree, restate once with evidence; a second disagreement on the same item → `escalate`.
- Never request changes for something the rubric marks minor/nit.
