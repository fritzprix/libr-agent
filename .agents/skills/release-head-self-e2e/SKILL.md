---
name: release-head-self-e2e
description: >
  The one workflow skill for the external E2E agent: run LibrAgent end-to-end from the
  last release tag (or last E2E run) to current dev/* HEAD. Five steps: check history
  and pin the tested SHA, create or reuse one GitHub tracking issue, test the app
  (session work through libr-e2e-verify, GUI work through the #2902 debug control API),
  post one results comment, and file one bug issue per new problem. The tester is the
  external multimodal agent that follows this skill; LibrAgent never judges results and
  Harbor tasks are optional scenario recipes only. Triggers: "HEAD부터 릴리스까지 E2E",
  "last release..HEAD E2E", "external e2e run", "#2731 재실행", "릴리스 이후 회귀 E2E".
  Not for a single session smoke run (libr-e2e-verify), a builtin-tool improvement loop
  (libr-tool-e2e-harness-loop), or Harbor benchmarking (bench).
---

# Release → HEAD external E2E

One workflow, one launcher: **you, the external agent**, are the tester. You drive the
app over HTTP, look at the evidence (traces, transcripts, screenshots) yourself, and
decide pass / fail / blocked. LibrAgent does not judge; Harbor does not judge.

Read [references/scopes.md](references/scopes.md) first. Default is **read-only /
dry-run** until the user names a scope. GitHub writes (tracking issue, results comment,
bug issues) happen only inside `github-write` / `execute-e2e` scopes.

## Hard rules

- **Zero-human** for every row the repo can run unattended; otherwise record `blocked`
  with the blocking issue — never `pass`, never "needs input".
- **One honest attempt per row.** No rerun-until-green. A `fail` gets a diagnosis.
- **Isolated profile only.** Debug build, explicit `LIBRAGENT_DATA_DIR` (or `--isolated`
  plus throwaway `HOME`/XDG). Never the primary profile, never real secrets.
- **Dummy secrets only** (for example `dummy-token-abcd`). No secret value in any text,
  log excerpt, or screenshot.
- **No product-code fix, merge, or settings change** from this workflow.
- **No standing GitHub permission**: loading this skill does not authorize writes; the
  scope rules in `references/scopes.md` do.
- **No GitHub CI step.** No LLM keys or `SESSION_E2E_*` secrets in CI; local LLM keys
  live only in the isolated profile.

## Step 1 — Check history

1. Pin the range: baseline = latest release tag reachable from `HEAD` (prerelease names
   skipped), head = current `dev/*` HEAD (or the last E2E run's SHA if the user names one).

   ```bash
   python3 .agents/skills/release-head-self-e2e/scripts/pin_release_range.py \
     --repo . --json > .libragent/work/release-head-self-e2e/<run-id>/pins.json
   ```

   No tag / no ancestry → explicit error, stop. No silent fallback.

2. Map merged PRs and the git diff to **#2731 rows** (IDs like `2565 P-1`, `2672 S-3`) and
   to matrix cases. Hints are suggestions — ground every selection in `git log` /
   `git diff` paths and the row text in #2731:

   ```bash
   python3 .agents/skills/release-head-self-e2e/scripts/map_diff_to_cases.py \
     --repo . --baseline "$(jq -r .baselineTag pins.json)" \
     --manifest benchmarks/harbor/libragent-session-e2e-matrix/coverage-manifest.json \
     --json > case-hints.json
   ```

3. Write `plan.md`: row ID, why in range, risk, runner (`session-api` | `gui` |
   `harbor-recipe` | `blocked`). **Record the pinned `headSha` — it is the tested SHA
   for every later artifact.**

## Step 2 — Create the tracking issue

1. **Search first** (open and recently closed): title/body keywords `release-head-self-e2e`,
   the baseline tag, `#2731`. Reuse one open tracking issue if it matches the pinned range.
2. Otherwise create **one** tracking issue for the run. Body: pins (`baselineSha..headSha`),
   the planned rows, the artifact dir, the no-secrets policy, and the blocked list.
3. List the planned rows there before testing starts.

## Step 3 — Test the app

### 3a. Isolated instance (precondition for every row)

Follow [../libr-e2e-verify/references/e2e-build-precondition.md](../libr-e2e-verify/references/e2e-build-precondition.md):
debug build of the pinned `headSha`, `LIBRAGENT_DATA_DIR` to a throwaway dir,
`LIBRAGENT_E2E_CONTROL=1`. The capability token is harness-side only: read
`e2e_control.cap` from the isolated data dir; never pass it into app prompts or logs.
Serialize: **one row at a time** on one instance.

### 3b. Session part — call `libr-e2e-verify`

Load `@skill:libr-e2e-verify` and use it for all session work: lifecycle, control-plane
approvals/prompts, polling, transcript extraction, cleanup. Do not copy its steps here.

### 3c. GUI part — the #2902 debug control API only

Operate the app **only** through `/api/e2e-control/ui/*` (screenshot, navigate, elements,
input, wait, window, restart — see the GUI table in `libr-e2e-verify`). Take window-only
screenshots, look at the saved PNGs yourself, and make the pass/fail decision from what
you see. If the control plane is absent (release build or no opt-in), GUI rows are
`blocked: no debug control plane` — do not fall back to OS-level desktop automation.

Harbor tasks under `benchmarks/harbor/libragent-session-e2e-matrix/` are **optional
scenario recipes and data probes** — reusable steps when a row matches a task. They are
not the verdict engine; your evidence and your judgment are.

## Step 4 — Upload the report

Post **one** results comment on the tracking issue:

- Tested SHA (or app version), OS, build type, profile mode (`isolated`).
- One row per #2731 item: **ID, result (`pass` / `fail` / `blocked`), evidence, and the
  linked bug issue** for every `fail` or `blocked`.
- Evidence per row: session id, trace or log excerpt, or screenshot (attached image or
  file name). Dummy secrets only — never a real secret value in any text or image.
- A short list of the new bug issues filed in step 5.

## Step 5 — File bugs

- **Search open and recently closed issues first.** If a match exists, comment there
  instead of filing a duplicate.
- **One problem per issue**: steps, expected, actual, tested SHA, evidence.
- Link each bug from the results comment and from the #2731 row.
- Never reopen the tracking issue or #2731 for fixes — failures live in their own issues.

## Classification rules

- `[auto]` — you can reach and capture/check the item through the Session API or the
  debug control API on a debug build with an isolated profile.
- `blocked` — cannot be automated yet; record the linked issue that tracks the missing
  piece. **`[manual]` is not a valid final state.**
- Platform-specific rows stay `blocked (platform)` on the other OS — not `pass`.

## Known product limits (honest)

- GUI input: element targets (`selector`/`testId`) deliver DOM events; raw `x`/`y` fall
  back to OS input only when the isolated window is focused and topmost (`409`
  otherwise). There is no `drag` action.
- Native dialogs outside the app window are limited to the documented `POST .../picker`
  one-shot route.
- No separate interactive-shell control surface: `storeAs` / `requireUserInput` prompts
  are answered through the shared pending-user-prompt route.

Mark dependent rows `blocked` with the reason; continue the queue.

## Artifact layout

```
.libragent/work/release-head-self-e2e/<run-id>/
  pins.json  case-hints.json  plan.md  dedupe.md  queue.json  progress.jsonl
  cases/<row-id>/  (transcript excerpt, screenshots, evidence pointers)
  results-comment.md   (the step-4 body before posting)
```

## Reuse — do not fork

| Need                                                                       | Use                                                                                                                                                               |
| -------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Session lifecycle, approvals/prompts, polling, evidence                    | **`libr-e2e-verify`** (this skill calls it)                                                                                                                       |
| GUI operation and visual verdict                                           | `/api/e2e-control/ui/*` routes (#2902), per `libr-e2e-verify`                                                                                                     |
| Control-plane gating, isolated runner, mock keychain, privileged container | carried from the closed sub-issues of #2900 — see `benchmarks/harbor/libragent-session-e2e-matrix/README.md` (§ privileged-path container & mock keychain, #2904) |
| Optional scenario recipes / data probes                                    | `benchmarks/harbor/libragent-session-e2e-matrix/` — recipes only, never the verdict                                                                               |
| Out-of-app session creation from Cursor etc.                               | `.agents/skills/libr-delegate`                                                                                                                                    |

## References

- [scopes.md](references/scopes.md) — authorization matrix
- [usage.md](references/usage.md) — run recipes (plan-only and full execute)
