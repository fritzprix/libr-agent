# Review rubric

Classify every finding. Code review and `changes-requested` verdicts proceed immediately regardless of CI status. Only **blocker** and **major** become required `G` items; **minor** and **nit** become `S` suggestions and never block approval.

| Severity | Examples |
| --- | --- |
| blocker | Wrong behavior, data loss, security/privacy leak, broken build, failing or deleted tests, migration without rollback path, diagnosed CI failure caused by the PR |
| major | Linked issue's acceptance criteria / checklist not met; new logic without tests; unhandled error path that users will hit; cross-platform path bug; violates `agents.md` hard rules (`as any`, raw `cargo`, protected-path edits outside scope); unrelated changes bundled in |
| minor | Naming, small duplication, missing doc/i18n string for a non-critical path, comment noise |
| nit | Style preferences the formatter/linter does not enforce |

## Checklist

1. **Scope** — Read the linked issue (`Fixes #N`). Does the diff deliver what it promises, and nothing unrelated? Large leftovers must be stated in the PR body.
2. **Correctness** — Trace changed code paths, including error and edge cases. Read surrounding code with `git show origin/<head>:<path>`, not just the hunk.
3. **Tests** — New behavior has tests at the right level; tests assert behavior, not implementation detail.
4. **Conventions** — `agents.md`/`CLAUDE.md`: KISS/YAGNI, no unsafe casts, comment density matches surroundings, Rust via `pnpm rust:*`.
5. **Security** — Tool approval/policy, filesystem/shell access, secrets in logs or previews, untrusted input handling. Security-sensitive PRs get `gatekeeper:needs-human` and an approve must list the bypass scenarios checked.
6. **User-facing** — UI text follows i18n standards (ko/en), docs updated when behavior changes, bundled skill `.force_update` bumped when a bundled `SKILL.md` changes.
7. **PR hygiene** — Semantic title, base is the latest `dev/<version>`, body has `Fixes #N` (closes on merge into `dev/*` too; partial work must use `Refs #N`) and a test plan that matches what was run.
8. **CI vs Code Review** — 빌드와 테스트를 확인할 때 로컬에서 무겁게 다시 돌리지 말고 CI 결과를 본다. **단, CI는 코드 리뷰(diff 검토, 설계 분석, 인수 조건 충족, 에지 케이스)를 대신하지 않는다.** CI 통과 여부와 무관하게 코드 리뷰를 직접 수행하며, CI 녹색은 `approve`를 낼 때만 확인하는 조건이다. CI가 실패한 경우에도 로그를 분석하여 실패 원인과 구체적 수정 지침을 finding으로 보고한다.
9. **Cross-platform** — New or changed tests/code touching paths, separators, shell, processes or permissions must pass on every OS job in CI (ubuntu, windows, macos). Read each job's result for the reviewed head; cancelled/skipped is not a pass. Reason about Windows path and quoting behavior in the diff itself, since CI may not have finished yet.
10. **Stacked PRs** — Base가 `dev/*`가 아닌 스택 PR은 GitHub Actions CI가 트리거되지 않는다. `ciPending`을 무시하고 코드 리뷰를 진행하며, approve 시 *"CI는 retarget 후 확인 필요"*라는 조건을 명시한다.

## Re-review discipline

- Round ≥ 2: review only (a) previous required findings and (b) the incremental diff `reviewedHead..headSha`. Do not raise new findings on unchanged code unless they are blockers.
- A worker **decline** with a sound reason closes the finding. If you still disagree, restate once with evidence; a second disagreement on the same item → `escalate`.
- Never request changes for something the rubric marks minor/nit.
