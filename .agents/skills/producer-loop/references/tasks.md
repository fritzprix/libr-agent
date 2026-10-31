# Tasks

Each task runs read-only checks and returns findings for `file_issue.py`. The default period is one day (`DEFAULT_PERIOD_SECONDS`). Override it per task in `state.json` under `periods`.

A task reuses an existing skill for the method. It must not copy that skill's logic here. The findings are only the entry point to `file_issue.py`.

| Task | Reused skill |
| --- | --- |
| `security` | `code-audit-expert` |
| `e2e` | `libr-tool-e2e-harness-loop` |
| `uiux` | `react-best-practices` |
| `regression` | `libragent-quality` |
| `usage-logs` | `trace-analyzer` |
| `doc-drift` | `audit-docs` |
| `cleanup` | `code-audit-expert` |

## security

- **Inputs**: source changed since the last run (`git log --since` on `lastRun.security`), plus `src-tauri/` command handlers, shell and file tools, approval and permission code.
- **Checks**: unbounded input, missing validation at trust boundaries, approval or permission bypass, secrets in logs or files, unsafe shell construction. Confirm each claim against the code, as `code-audit-expert` requires.
- **Fingerprint key**: `<path>:<rule>` (for example `src-tauri/src/foo.rs:unbounded-read`). No line numbers.
- **Rules**: report only. Do not change `src-tauri/`. Do not describe a working exploit. Describe the class of problem.

## e2e

- **Inputs**: the builtin tools and the E2E harness from `libr-tool-e2e-harness-loop`.
- **Checks**: run the harness and list tools whose real-session behavior fails or differs from its documented behavior.
- **Fingerprint key**: `<tool>:<scenario>`.
- **Rules**: Linux only. Run `unsafe` sessions only in a scratch directory outside the repository. Do not touch the user's workspace.

## uiux

- **Inputs**: frontend code under `src/` changed since the last run.
- **Checks**: the review rules of `react-best-practices`, plus accessibility gaps, missing empty, loading, and error states, and text that is not localized.
- **Fingerprint key**: `<component path>:<rule>`.
- **Rules**: report only.

## regression

- **Inputs**: recent merged commits and the quality commands of `libragent-quality`.
- **Checks**: failing or newly flaky tests, new lint or type errors, and behavior that a recent commit changed without a test. Run only the targeted commands from `libragent-quality`. Never run `pnpm refactor:validate`.
- **Fingerprint key**: `<test or file>:<symptom>`.
- **Rules**: a flaky test needs two failing runs as evidence.

## usage-logs

- **Inputs**: session trace files that the user has stored locally (`*.trace.json`).
- **Checks**: use `trace-analyzer` to find repeated tool errors, retries, loops, and wasted turns.
- **Fingerprint key**: `<tool or prompt area>:<error class>`.
- **Rules**: traces hold private data. Never quote prompts, file contents, or paths in an issue. Quote the error class and counts only.

## doc-drift

- **Inputs**: `docs/`, READMEs, and the code they describe.
- **Checks**: run `audit-docs` and keep only statements that the code contradicts. Do not file missing-coverage wishes.
- **Fingerprint key**: `<doc path>:<heading or claim id>`.
- **Rules**: report only. Fixing docs is a normal worker task after approval.

## cleanup

- **Inputs**: branches, stale issues and PRs, dead files, unused dependencies.
- **Checks**: list candidates with evidence (last commit date, no references, merged state).
- **Fingerprint key**: `cleanup:<ISO week>` (one issue per week; put all candidates in it).
- **Rules**: propose only. Never delete branches or close issues. `main` and `dev/*` are long-lived and are never proposed for deletion.
