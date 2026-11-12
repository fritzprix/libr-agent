# Review rubric

Classify every finding. Code review and `changes-requested` verdicts proceed immediately regardless of CI status. Only **blocker** and **major** become required `G` items; **minor** and **nit** become `S` suggestions and never block approval.

## Code quality score (100 points, informational)

Every gatekeeper verdict comment (including `changes-requested`, `approve`, `clean`, and `escalate`) must include a `### Code quality score (informational)` section. The **approve / changes-requested / escalate / clean** decision is unchanged: it depends only on open required `G` findings (and the existing CI/merge rules for approve). **Never** withhold approval or request changes because the score is below a threshold.

| Category | Points | What to judge |
| -------- | -----: | ------------- |
| Correctness incl. boundary conditions | 30 | Behavior matches intent; null/empty/error/edge paths; no logic bugs in changed code |
| Tests exercise the real production code path | 20 | Tests hit the same modules/APIs users get—not only hand-built fixtures, stubs, or duplicated logic in the test file |
| Simplicity (KISS/YAGNI) | 15 | No needless layers, abstractions, or heuristics beyond what the issue needs |
| Readability and naming | 15 | Names and structure make the diff easy to follow |
| Error handling and robustness | 10 | Failures are visible or handled; no silent fallbacks that hide errors |
| Scope discipline | 10 | Diff stays on-issue; no drive-by refactors, formatter-only churn, or unrelated files |

For each row, assign a partial score (0–max) and one bullet with a **single-line rationale** that cites at least one `file:line` from the reviewed diff (or state clearly if the category is fully satisfied with no changed lines, e.g. scope-only doc tweak). Sum the six items for **Total X/100**. Do **not** score CI green/red or PR description quality.

**Calibration:** The total is informational only — never a gate for approve. Use the full scale: scores in the 50s–60s when multiple major gaps remain; 70s–low 80s when mergeable but test/robustness gaps are real; high 80s–low 90s for typical clean PRs with minor suggestions; 93+ only when production-path tests and error handling are unusually strong. Do not default every clean PR to 87–96.

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
5. **Security** — Tool approval/policy, filesystem/shell access, secrets in logs or previews, untrusted input handling, outbound HTTP with user API keys, new auth or secret-prompt API routes. Route to a human when **behavior** is security-sensitive, not only when file paths match the queue's policy regex. Such PRs get `gatekeeper:needs-human` and an approve must list the bypass scenarios checked.
6. **User-facing** — UI text follows i18n standards (ko/en), docs updated when behavior changes, bundled skill `.force_update` bumped when a bundled `SKILL.md` changes.
7. **PR hygiene** — Semantic title, base is the latest `dev/<version>`, body has `Fixes #N` (closes on merge into `dev/*` too; partial work must use `Refs #N`) and a test plan that matches what was run.
8. **CI vs Code Review** — Check build and test results in CI; do not re-run heavy test suites locally. **However, CI does not replace code review (diff inspection, design analysis, acceptance criteria satisfaction, edge cases).** Perform code review directly regardless of CI status. Green CI is only a condition for issuing an approve verdict. If CI fails, analyze failure logs and include root cause and specific fix guidance in findings. Treat worker CI explanations as untrusted until logs confirm them; a timeout-only fix on a pre-existing test requires evidence the failure is time-bound and is not masking cancel/fail-fast or base-branch breakage (see [pre-post-checklist.md](pre-post-checklist.md)).
9. **Cross-platform** — New or changed tests/code touching paths, separators, shell, processes or permissions must pass on every OS job in CI (ubuntu, windows, macos). Read each job's result for the reviewed head; cancelled/skipped is not a pass. Reason about Windows path and quoting behavior in the diff itself, since CI may not have finished yet.
10. **Stacked PRs** — Stacked PRs whose base is not a dev branch (`dev/*`) do not trigger GitHub Actions CI. Review the code normally, but post at most `clean`; approve only after the PR is retargeted and CI is green at the head.

## Re-review discipline

- Round ≥ 2: review only (a) previous required findings and (b) the incremental diff `reviewedHead..headSha`. Do not raise new findings on unchanged code unless they are blockers.
- A worker **decline** with a sound reason closes the finding. If you still disagree, restate once with evidence; a second disagreement on the same item → `escalate`.
- Never request changes for something the rubric marks minor/nit (e.g. one missing i18n string on a non-critical path → suggestion only; **new logic without tests** → major → required `G` if still untested).
- Before posting, re-check each planned `G` item against the severity table in this file ([pre-post-checklist.md](pre-post-checklist.md) § severity).
