# Usage recipes — release → HEAD external E2E

The tester is the external multimodal agent that loads this skill. Two recipes.

## A. Plan only (`read-only` / `plan` scope)

**Goal:** pin SHAs, map the range to #2731 rows, dedupe prior tickets, draft the tracking
issue. No GitHub writes, no app testing.

### Bash (repo root)

```bash
export RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p ".libragent/work/release-head-self-e2e/${RUN_ID}"

python3 .agents/skills/release-head-self-e2e/scripts/pin_release_range.py \
  --repo . --json > ".libragent/work/release-head-self-e2e/${RUN_ID}/pins.json"

python3 .agents/skills/release-head-self-e2e/scripts/map_diff_to_cases.py \
  --repo . \
  --baseline "$(python3 -c 'import json; print(json.load(open(".libragent/work/release-head-self-e2e/'"${RUN_ID}"'/pins.json"))["baselineTag"])')" \
  --manifest benchmarks/harbor/libragent-session-e2e-matrix/coverage-manifest.json \
  --json > ".libragent/work/release-head-self-e2e/${RUN_ID}/case-hints.json"
```

Then write `plan.md` (row ID, why in range, risk, runner) and `dedupe.md` (kept/skipped
tracking tickets with links). Draft the tracking-issue body; post it only in
`github-write` scope.

### PowerShell

```powershell
$RunId = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
$Root = ".libragent/work/release-head-self-e2e/$RunId"
New-Item -ItemType Directory -Force -Path $Root | Out-Null

python3 .agents/skills/release-head-self-e2e/scripts/pin_release_range.py `
  --repo . --json | Set-Content -Encoding utf8 "$Root/pins.json"

$tag = (Get-Content "$Root/pins.json" | ConvertFrom-Json).baselineTag
python3 .agents/skills/release-head-self-e2e/scripts/map_diff_to_cases.py `
  --repo . --baseline $tag `
  --manifest benchmarks/harbor/libragent-session-e2e-matrix/coverage-manifest.json `
  --json | Set-Content -Encoding utf8 "$Root/case-hints.json"
```

## B. Full external E2E run (`execute-e2e` scope)

**Goal:** steps 1–5 end to end: pins → tracking issue → test every row → results
comment → bug issues.

### Linux (desktop debug build)

```bash
# Isolated profile — never the primary data dir.
export LIBRAGENT_DATA_DIR="$HOME/e2e-profile-$RUN_ID"
export LIBRAGENT_E2E_CONTROL=1
export LIBRAGENT_E2E_SYNTHETIC_SECRET="dummy-token-abcd"
# Start the debug app in another terminal: pnpm tauri dev
# Then read the capability token:  cat "$LIBRAGENT_DATA_DIR/e2e_control.cap"
```

For each planned row, in order:

1. **Session rows:** follow `@skill:libr-e2e-verify` (lifecycle → control-plane
   approvals/prompts → transcript → cleanup). Evidence = session id + transcript excerpt.
2. **GUI rows:** drive `/api/e2e-control/ui/*` (navigate → elements → input →
   screenshot), open the saved PNG, and judge pass/fail visually yourself. Evidence =
   screenshot file name.
3. Record `pass` / `fail` / `blocked` + evidence in `progress.jsonl` as you go.

After the queue: post the step-4 results comment, then file (or comment on) one bug
issue per new problem per the step-5 rules.

### Windows

Same recipe (`pnpm tauri dev`, `LIBRAGENT_DATA_DIR` to a throwaway dir). Linux-only
rows (sudo/askpass, X11) are `blocked (platform)` on Windows — not `pass`. The reverse
applies to Windows-only rows on Linux.

## C. What not to do

- Do not run this skill on install to auto-open GitHub issues.
- Do not enable `LIBRAGENT_E2E_CONTROL` on a shared production-profile debug build.
- Do not treat a green Harbor task as the verdict — recipes only; your evidence decides.
- Do not fall back to OS-level desktop automation when the control plane is absent;
  mark the row `blocked: no debug control plane`.
