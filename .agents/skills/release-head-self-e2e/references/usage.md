# Usage examples — release → HEAD self-E2E

## A. Latest release → HEAD: ticket generation only (`plan` + optional `github-write`)

**Goal:** Pin SHAs, map diff to matrix rows, dedupe, draft or file a tracking issue. **Do not** run E2E.

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

Then load `@skill:release-head-self-e2e` in a LibrAgent session with workspace at repo root.
Ask: “scope `plan`: write `plan.md` and `dedupe.md` from pins and manifest; no GitHub POST.”

For an issue: “scope `github-write`: create one tracking issue from `plan.md`.”

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

## B. Full self-E2E execution (`execute-e2e`)

**Goal:** Parent orchestrator delegates executor + verifier children; zero-human where supported.

### Windows (primary)

```powershell
# Isolated profile — not production %LOCALAPPDATA%
$env:LIBRAGENT_E2E_CONTROL = "1"
$env:LIBRAGENT_E2E_SYNTHETIC_SECRET = "synthetic-e2e-fixture-only"
# Start debug app in another terminal: pnpm tauri dev

# Optional Harbor single-task smoke (secondary to delegate path):
pnpm bench:session-e2e:n1
```

In LibrAgent (Coding Expert or harness assistant), with repo workspace:

```
Load @skill:release-head-self-e2e and @skill:delegate.
Scope execute-e2e. Run-id <RUN_ID>. Serialize cases.
Use references/executor-brief.md and references/verifier-brief.md for children.
Blocked only for native GUI paths outside the app window; storeAs/uiAskUser
prompts are answered by the harness via the debug E2E control plane.
```

Parent keeps running; harvest `GET /api/sessions/:id/messages?limit=500` before child delete when using Session API helpers from briefs.

### Linux desktop debug

Same env vars; matrix rows marked `platform-specific` for Windows-only PowerShell paths stay `blocked` on Linux — not pass.

## C. What not to do

- Do not run this skill on install to auto-open GitHub issues.
- Do not enable `LIBRAGENT_E2E_CONTROL` on a shared production-profile debug build without isolation.
- Do not substitute Harbor green for delegate evidence when the skill scope is internal self-E2E.
