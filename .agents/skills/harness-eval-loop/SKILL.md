---
name: harness-eval-loop
description: 'Build the current LibrAgent revision, run the Harbor benchmark that matches the recent work, decide whether the experiment is accepted (adopt / revert / iterate / inconclusive) against a comparable baseline, and file newly found harness problems and performance-regression causes as GitHub issues. With an interval and dataset it repeats this on a schedule. Use when the user types "/harness-eval-loop", "/harness-eval-loop 2h layman", "harness_eval_loop", "벤치 돌려서 experiment 판정", "하네스 평가 루프", or asks to run bench, judge an experiment, and open issues for what the run found.'
---

# Harness Eval Loop

Automates the tail of `harbor-harness-improvement-loop`: **build → bench → judge → file issues**.
Analysis rules (harness-only scope, evidence model, comparability, never edit verifiers or
official limits) are inherited from that skill; read its `SKILL.md` and
`harbor-harness-improvement-loop/references/evidence-model.md` first. This skill adds execution, a decision, and issue filing.

## Invocation

| Command                                   | Behavior                                                                        |
| ----------------------------------------- | ------------------------------------------------------------------------------- |
| `/harness-eval-loop`                      | One run now. Dataset and experiment are inferred from recent context (below).   |
| `/harness-eval-loop <interval> <dataset>` | Run one tick now, then repeat every `<interval>` (`30m`, `2h`) on `<dataset>`.  |
| `/harness-eval-loop stop`                 | Stop the recurring loop.                                                        |
| `--dry-run`                               | Do everything except `gh issue create/comment`; print the issue drafts instead. |

`<dataset>` is a short name or a path: `layman`, `diverse`, `swe`, `media`, `browser`, `fileop`,
`compaction`, or any `benchmarks/harbor/<dir>`. Map names to the `pnpm bench:*` scripts in
`package.json` (e.g. `layman` → `pnpm bench:layman`, `diverse` → `pnpm bench:diverse`). If a name has
no script, stop and ask; never invent a dataset id.

Recurring mode: arm it with the `loop` skill / `ScheduleWakeup`, passing the same
`/harness-eval-loop <interval> <dataset>` prompt back. Before arming, check state (below) for
an existing loop and do not arm a second one. Pick `interval` no shorter than one full bench
run; a tick that finds the previous run still going records "run in progress" and does nothing else.

## Inferring the run from context (no arguments)

Read, in this order, and stop at the first that decides:

1. An open experiment ledger: newest `.libragent/work/harbor-harness-cycle/*/experiment.md` or
   `analysis.md` whose decision is `iterate` / `inconclusive` → reuse its dataset, tasks, attempts.
2. The current branch diff (`git diff main...HEAD --stat`, plus `git status`): changes under
   `benchmarks/harbor/<dir>` or a builtin tool/skill/prompt that a dataset's tasks exercise
   → pick the dataset that covers them (layman-intent for skills/delegation/scheduling,
   `fileop` for readFile/editing, `compaction` for context/compaction, etc.).
3. The user's last request in this conversation.

State the inferred dataset, tasks, attempts and baseline job in one short block before building.
If two datasets are equally plausible, ask once; otherwise proceed. Prefer the smallest slice that
exercises the change (`--include` / `--n-tasks`), then widen only if the slice is positive.

## Tick procedure

### 1. Freeze the experiment contract

Record in `.libragent/work/harbor-harness-cycle/<cycle-id>/experiment.md` (cycle-id =
`YYYY-MM-DD-cNN`): git revision + dirty flag, dataset/tasks, attempts, concurrency, assistant,
execution + workspace mode, timeout/verifier settings, and the **hypothesis and acceptance
criteria written before the run** (e.g. "t17 reaches `createAgent`; no task that passed regresses").

Choose the **baseline**: the newest _complete_ job under `jobs/` whose contract matches
(same dataset/tasks/attempts/concurrency/modes/verifier). If none matches, run one on the base
revision first (or mark the result `inconclusive` and say why). Incomplete or cancelled jobs
are never baselines.

### 2. Build

The bench launches the release binary (`src-tauri/target/release/libragent`) via
`--isolated`, so a stale binary silently benchmarks old code.

```bash
git rev-parse --short HEAD
pnpm tauri build --no-bundle      # frontend + release binary; heavy, run once per revision
```

- Skip the build only if the binary is newer than every file in `git diff --name-only <last-built-rev>`
  (store `lastBuiltRev` in state). Doc-only or `benchmarks/`-only changes never need a rebuild.
- Do **not** run `pnpm refactor:validate` or raw `cargo test` (project rule). For code changes run only
  the focused tests/lint for the changed files before building; for Rust use
  `pnpm rust:test --test <name>`.
- Build failure → stop, report the error, file nothing about the benchmark.
- Confirm Docker and `harbor` are available (`docker info`, `harbor --version`); missing → stop and say so.

### 3. Run the bench

Run in the background and wait for the completion notification; do not poll.

```bash
pnpm bench:<dataset> [--include '<glob>'] [--n-tasks N] [--n-attempts K]   # run_in_background
```

- Use the same flags as the baseline. Concurrency stays 1 in `--isolated` mode.
- Never change timeouts, resources, tasks or verifiers to get a result.
- Ctrl+C / cancellation leaves a partial job: analyze it for diagnosis only, never as a verdict.
- Print the job path as soon as it exists so the user can follow along.

### 4. Evaluate and decide

```bash
python .agents/skills/harbor-harness-improvement-loop/scripts/analyze_harbor_results.py \
  jobs/<baseline> jobs/<candidate> \
  --output .libragent/work/harbor-harness-cycle/<cycle-id>/summary.json
```

Then, per the improvement-loop rules:

1. **Comparability gate.** Any mismatch in the contract table → decision is `inconclusive`.
2. **Acceptance criteria.** Check each criterion written in step 1 against trial-level evidence
   (tool sequences, observations), not reward alone.
3. **Regression check.** Every task that passed in the baseline must still pass; a flipped
   task is read in the trajectory before it is called noise.
4. **Cost metrics.** Compare turns, tool calls, input/output/cache tokens, repeated and failed calls.
5. **Variance.** With 1 attempt, differences of ≤1 task are not evidence; say `inconclusive`
   or rerun with more attempts rather than claiming a win.

Decision, exactly one: **adopt** (criteria met, no regression, cost not worse) · **revert**
(regression traced to the change) · **iterate** (partial/new contract defect found) ·
**inconclusive** (not comparable / within variance / telemetry missing). Write
`analysis.md` using `harbor-harness-improvement-loop/references/report-template.md`; every
percentage carries its count.

Analysis scope applies: no model or serving-engine commentary. Findings are tool sequences,
observations, and the schema/handler/response/prompt/adapter/telemetry/verifier source.

### 5. Triage findings before filing

For every failed or degraded trial and every cost outlier, classify (this split is mandatory):

| Class                   | Meaning                                                                         | Goes to                                   |
| ----------------------- | ------------------------------------------------------------------------------- | ----------------------------------------- |
| product defect          | a tool/prompt/handler/response contract is broken or misleading                 | issue, label per layer                    |
| bench / verifier defect | the check can't be met by correct product behavior (e.g. wake flow vs idle end) | issue, label `bench`/`harbor`             |
| performance regression  | candidate worse than baseline on turns/calls/tokens/retries with a traced cause | issue, with the delta                     |
| pattern only            | odd tool order but no contract broken                                           | analysis.md only, no issue                |
| policy question         | needs an owner decision (e.g. act immediately on an explicit request)           | issue labelled `question`; change nothing |

Never fix product behavior to match a bench quirk, and never put task wording into a skill
trigger or example as the "fix". Single-trial findings are labelled hypotheses in the issue.

### 6. File issues

Authorized by this skill, but bounded:

1. **Dedupe first.** `gh issue list --state all --search "<tool or symptom> in:title,body" --limit 10`.
   If a match exists, add a comment with the new evidence (job path, trial id, tool sequence) instead
   of opening a new issue. Never reopen a closed `wontfix` issue.
2. **At most 5 new issues per tick.** Rank by breadth × confidence; the rest go to
   `analysis.md` under "backlog".
3. **Body** (Korean unless the repo thread is English): title `bench(harbor): <symptom>` or
   `fix(<tool>): <symptom>`; sections _Evidence_ (job, trial ids, git revision, tool sequence with
   args/observation sizes), _Class_ (from step 5), _Owning layer + inspected paths_,
   _Hypothesis / confidence_, _Proposed smallest intervention_ (not applied), _Acceptance criteria_,
   _Counter-example check_. Link the cycle's `analysis.md` path and the parent issue if any.
   Never paste secrets, wake URLs/tokens, or the user's email.
4. Labels: reuse existing ones only (`bug`, `enhancement`, `prompt`, `agent`, `question`, ...);
   check `gh label list` and never create labels.
5. `--dry-run`, or any doubt about whether a finding is outward-facing/sensitive → print the draft and ask.

Do not push, open PRs, or edit product/verifier/skill files from this loop. Applying a fix is a
separate, user-approved step.

### 7. Record and report

Update `.libragent/work/harness-eval-loop/state.json` (git-ignored):

```json
{
  "loopArmed": true,
  "interval": "2h",
  "dataset": "layman",
  "lastBuiltRev": "abc1234",
  "baselineJob": "jobs/…",
  "lastJob": "jobs/…",
  "openCycle": "2026-10-26-c01",
  "filedIssues": [2341],
  "lastDecision": "iterate"
}
```

End every tick with a short report: revision, job path, pass count vs baseline (numerator/denominator),
the **decision**, issues opened/commented (with numbers), and what the next tick will do. In recurring
mode, a tick with no change, no new findings and no decision change says so in one line.

## Stop conditions

Stop and report (do not file issues or decide) when: the build fails; Docker/harbor is unavailable;
the bench was cancelled or errored before any trial completed; the dataset name is unknown; the
baseline is not comparable and no rerun was approved; or the only evidence is hidden reasoning text.
In recurring mode also stop re-arming after 3 consecutive ticks that fail for the same environmental reason.
