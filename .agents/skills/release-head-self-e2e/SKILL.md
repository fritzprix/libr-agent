---
name: release-head-self-e2e
description: >
  Plan and run version-aware Session API self-E2E from the latest published release
  tag through current HEAD inside LibrAgent: pin baseline/head SHAs, map git diff to
  matrix cases, dedupe prior tracking tickets, optionally create/update a GitHub
  tracking issue when the user explicitly authorizes writes, then orchestrate parent→
  child delegates for zero-human execution and independent verification using real
  builtin tools and the debug-only E2E control plane (LIBRAGENT_E2E_CONTROL).
  Triggers: "HEAD부터 릴리스까지 E2E", "last release..HEAD E2E", "self e2e matrix",
  "#2565 자동", "릴리스 이후 회귀 E2E", "tracking ticket then self E2E".
  Not Harbor-only orchestration (Harbor is optional CI path); not Cursor libr-delegate
  unless the user runs from Cursor. Never invent tools; use agent__* and workspace__*.
---

# Release → HEAD self-E2E

Internal LibrAgent workflow to turn **last release..HEAD** into a **deduped E2E plan**,
an optional **GitHub tracking ticket**, and **delegated unattended runs** with
independent verifiers.

Read [references/scopes.md](references/scopes.md) before acting. Default is **read-only /
dry-run** until the user names a scope.

## Integrate — do not fork

| Need | Use |
| --- | --- |
| Child spawn / isolation | bundled **`delegate`** |
| Proof-before-done verifier child | **`delegation-eval-loop`** |
| Matrix + Harbor tasks | `benchmarks/harbor/libragent-session-e2e-matrix/` |
| Debug approvals / user prompts (`ui__askUser`, `storeAs`) | `LIBRAGENT_E2E_CONTROL=1` + `/api/e2e-control/*` (debug build only) |
| Optional external CI batch | `pnpm bench:session-e2e*` (Harbor); **secondary** to this skill |
| Cursor HTTP harness | `.agents/skills/libr-delegate` (out-of-app only) |

## Hard rules

- **Zero-human** for every case the repo can run unattended; otherwise mark
  `blocked` / `platform-unsupported` — never `pass` and never “needs-input”.
- **No rerun-until-green**; one honest attempt per case unless the user orders a rerun.
- **Failures get a diagnosis**, not silent retry loops.
- **No fabricated tools** — only names from the live tool list (`agent__*`, `workspace__*`, etc.).
- **Parent survives child stop** — `agent__stopSession` on children only; parent keeps orchestrating.
  Outer **app restart/crash** tests may use a watchdog; nothing else.
- Preserve protections: release has **no** E2E routes; dev needs **explicit opt-in**;
  **loopback** + capability header; **isolated** data dir — never production profile or real secrets.
- **Independent verifier** accepts only objective artifacts (API messages JSON, workspace files,
  verifier scripts, Harbor `session_e2e_evidence.json`) — not child prose alone.
- **No product code fix / merge / global settings** in this skill unless the user separately authorizes it.
- Creating this skill or loading it does **not** grant standing permission to open GitHub issues.

## Known product limits (honest)

These remain outside full #2565 unattended coverage (see PR #2632):

- E2E control drives **user prompts** (`ui__askUser` questions and `storeAs` /
  `requireUserInput` secret inputs, via the shared pending-user-prompt) and
  **tool approvals**. There is no separate interactive-shell control surface.
- Native GUI paths outside the app window are **not** drivable from the control plane.
- Full matrix still needs **Windows or Linux desktop debug** beside the app; cloud/Linux
  without the app cannot claim native GUI E2E.

Mark dependent checklist rows `blocked` with reason; continue independent cases.

## Phases

### 0. Confirm authorization scope

Parse the user message for one of:

| Scope | Allowed |
| --- | --- |
| `read-only` | Pin SHAs, diff, read manifest, read GitHub (no writes), write local plan under `.libragent/work/` |
| `plan` | Above + propose case list + ticket body draft (still no GitHub POST) |
| `github-write` | Create/update tracking issue **once** when user explicitly says so (e.g. “이슈 올려”, “create tracking issue”) |
| `execute-e2e` | Run delegated children + verifiers; requires debug app + `LIBRAGENT_E2E_CONTROL=1` on isolated profile |

If ambiguous, stay in `read-only` and ask one question.

### 1. Pin baseline and HEAD

From the **repo root** (absolute `workspacePath` / `workspaceOverride="current"`):

```bash
python3 .agents/skills/release-head-self-e2e/scripts/pin_release_range.py \
  --repo . --json > .libragent/work/release-head-self-e2e/<run-id>/pins.json
```

Script behavior:

- Picks latest **annotated/lightweight git tag** reachable as ancestor of `HEAD`
  (default: skips `-*alpha*`, `-*beta*`, `-*rc*`, `-*dev*` prerelease names).
- Sets `baselineSha` = tag peel, `headSha` = `HEAD`, verifies ancestry; if no tag
  or tag not ancestor → explicit error object (no silent fallback).

### 2. Map diff → E2E cases (version-aware)

1. Load `benchmarks/harbor/libragent-session-e2e-matrix/coverage-manifest.json`.
2. Run hints (optional):

```bash
python3 .agents/skills/release-head-self-e2e/scripts/map_diff_to_cases.py \
  --repo . --baseline "$(jq -r .baselineTag pins.json)" \
  --manifest benchmarks/harbor/libragent-session-e2e-matrix/coverage-manifest.json \
  --json > case-hints.json
```

3. **Human/agent merge**: hints are suggestions only — ground each selected case in
   `git log` / `git diff` paths and the issue #2565 expectation text. Write
   `plan.md` with: case id, why in range, risk, runner (`harbor-task` | `delegate-brief` | `blocked`).

### 3. Dedupe prior tickets

- Search existing GitHub issues (title/body keywords: `release-head-self-e2e`, baseline tag, `#2565`).
- Merge with completed evidence paths cited in comments; do not re-run `pass` cases unless user requests.
- Record `dedupe.md` with kept/skipped cases and links.

**Requires `github-write`** to post; otherwise output draft only.

### 4. Tracking ticket (optional)

Only in **`github-write`** scope. Single tracking issue per pinned `baselineSha..headSha` pair.
Body must include: pins, plan table, artifact dir, explicit “no secrets” policy, blocked list.
Link confirmed product failures only within user-granted write scope.

### 5. Execute self-E2E (`execute-e2e`)

**Preconditions**

Follow [../libr-e2e-verify/references/e2e-build-precondition.md](../libr-e2e-verify/references/e2e-build-precondition.md).
The running app must be a debug build of `headSha`. Read `buildSha` from the status routes and confirm each PR in scope before executing cases.

- Debug LibrAgent (`pnpm tauri dev` or debug build), disposable data dir.
- `LIBRAGENT_E2E_CONTROL=1`. The capability token is **harness-side only**: the
  external harness reads it from the disposable instance data dir. Agents and
  child briefs never receive token material.
- `LIBRAGENT_E2E_SYNTHETIC_SECRET` for masked-value fixtures only.
- Serialize: **one case at a time** on one app instance (queue file `queue.json`).
- **Prompt resolution scope:** the only auto-resolver today is Harbor's trial
  poll loop, which watches **its own** session. Executor children spawned via
  `agent__spawnSession` have nothing answering their `requireUserInput`
  prompts — route those cases (e.g. B-3 interactive echo) to the Harbor
  runner, or mark them `blocked` with reason `no in-app prompt resolver`.

**Parent (orchestrator)**

1. Load `@skill:delegate` mechanics; keep workspace shared when children need repo + manifest.
2. For each runnable case:
   - Spawn **executor** child (`waitForResult=false` first if parallel planning; **serialize** runs).
   - Brief: self-contained task, exact matrix id, synthetic secret policy, deliverable = evidence paths + API excerpt pointers.
   - Poll `agent__checkSession` with backoff; on `paused`, one `agent__messageToSession` resume attempt.
3. Spawn **verifier** child with **no executor transcript** as sole proof — pass
   `references/verifier-brief.md` + paths to raw `messages.json`, workspace outputs, Harbor evidence.
4. Update `progress.jsonl` and per-case folder under `.libragent/work/release-head-self-e2e/<run-id>/cases/<id>/`.
5. On unsupported driver → `blocked` with reason; continue queue.
6. Cleanup: executor terminate/delete after evidence harvest; verifier separate session; parent remains.

**Negative controls** (delegate brief or Harbor `test_outputs.py` patterns):

- E2E routes 404 without opt-in; wrong capability header rejected; stale execution id rejected;
  synthetic secret absent from logs in verifier grep rules.

### 6. Closeout

- `summary.md`: pass / fail / blocked counts, diagnosis per fail, limits section.
- If `github-write`: comment on tracking issue with summary + artifact paths (no secret values).
- Do not merge or change default settings.

## Artifact layout

```
.libragent/work/release-head-self-e2e/<run-id>/
  pins.json
  case-hints.json
  plan.md
  dedupe.md
  queue.json
  progress.jsonl
  summary.md
  cases/<case-id>/
    executor-messages.json
    verifier-verdict.md
    evidence/...
```

## Usage examples

Full command recipes: [references/usage.md](references/usage.md).

## References

- [scopes.md](references/scopes.md) — authorization matrix
- [usage.md](references/usage.md) — release→HEAD ticket + full self-E2E examples (bash + PowerShell)
- [verifier-brief.md](references/verifier-brief.md) — independent child template
- [executor-brief.md](references/executor-brief.md) — unattended runner child template
