# Review rubric

Classify every finding. Code review and `changes-requested` verdicts proceed immediately regardless of CI status. Only **blocker** and **major** become required `G` items; **minor** and **nit** become `S` suggestions and never block approval.

| Severity | Examples                                                                                                                                                                                                                                                                      |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| blocker  | Wrong behavior, data loss, security/privacy leak, broken build, failing or deleted tests, migration without rollback path, diagnosed CI failure caused by the PR                                                                                                              |
| major    | Linked issue's acceptance criteria / checklist not met; new logic without tests; unhandled error path that users will hit; cross-platform path bug; violates `agents.md` hard rules (`as any`, raw `cargo`, protected-path edits outside scope); unrelated changes bundled in |
| minor    | Naming, small duplication, missing doc/i18n string for a non-critical path, comment noise                                                                                                                                                                                     |
| nit      | Style preferences the formatter/linter does not enforce                                                                                                                                                                                                                       |

## Checklist

1. **Scope** — Read the linked issue (`Fixes #N`). Does the diff deliver what it promises, and nothing unrelated? Large leftovers must be stated in the PR body.
2. **Correctness** — Trace changed code paths, including error and edge cases. Read surrounding code with `git show origin/<head>:<path>`, not just the hunk.
3. **Tests** — New behavior has tests at the right level; tests assert behavior, not implementation detail.
4. **Conventions** — `agents.md`/`CLAUDE.md`: KISS/YAGNI, no unsafe casts, comment density matches surroundings, Rust via `pnpm rust:*`.
5. **Security** — Tool approval/policy, filesystem/shell access, secrets in logs or previews, untrusted input handling. Security-sensitive PRs get `gatekeeper:needs-human` and an approve must list the bypass scenarios checked.
6. **User-facing** — UI text follows i18n standards (ko/en), docs updated when behavior changes, bundled skill `.force_update` bumped when a bundled `SKILL.md` changes.
7. **PR hygiene** — Semantic title, base is the latest `dev/<version>`, body has `Fixes #N` (closes on merge into `dev/*` too; partial work must use `Refs #N`) and a test plan that matches what was run.
8. **CI vs Code Review** — Check build and test results in CI; do not re-run heavy test suites locally. **However, CI does not replace code review (diff inspection, design analysis, acceptance criteria satisfaction, edge cases).** Perform code review directly regardless of CI status. Green CI is only a condition for issuing an approve verdict. If CI fails, analyze failure logs and include root cause and specific fix guidance in findings.
9. **Cross-platform** — New or changed tests/code touching paths, separators, shell, processes or permissions must pass on every OS job in CI (ubuntu, windows, macos). Read each job's result for the reviewed head; cancelled/skipped is not a pass. Reason about Windows path and quoting behavior in the diff itself, since CI may not have finished yet.
10. **Stacked PRs** — Stacked PRs whose base is not a dev branch (`dev/*`) do not trigger GitHub Actions CI. Ignore `ciPending` and perform code review normally. When approving, add the explicit condition: _"CI requires verification after retargeting"_.

## Re-review discipline

- Round ≥ 2: review only (a) previous required findings and (b) the incremental diff `reviewedHead..headSha`. Do not raise new findings on unchanged code unless they are blockers.
- A worker **decline** with a sound reason closes the finding. If you still disagree, restate once with evidence; a second disagreement on the same item → `escalate`.
- Never request changes for something the rubric marks minor/nit.
