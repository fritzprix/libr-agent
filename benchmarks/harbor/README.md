# Harbor + LibrAgent adapter

Runs [Harbor](https://github.com/laude-institute/harbor) / Terminal-Bench tasks against a
live LibrAgent Session API (`POST /api/sessions` with `executionMode`).

## Prerequisites

1. `pnpm tauri dev` (HTTP API on `http://localhost:3030/api`)
2. `pip install harbor httpx` (or `uv tool install harbor`)
3. Docker available

## Quick start (recommended)

From the repo root, with LibrAgent already running:

```powershell
# Smoke: Harbor hello-world (expect reward=1)
pnpm bench:hello

# Terminal-Bench: first task only
pnpm bench:terminal

# Terminal-Bench: full dataset (long)
pnpm bench:terminal:all
```

Or call the script directly:

```powershell
# Named Terminal-Bench task(s)
.\scripts\run-harbor-bench.ps1 -Preset terminal-bench -Include "hello*" -NTasks 3

# Custom assistant / API
.\scripts\run-harbor-bench.ps1 -Preset terminal-bench -NTasks 1 `
  -ApiUrl http://localhost:3030/api `
  -AssistantId <uuid> `
  -ExecutionMode yolo

# Dry-run (print harbor command only)
.\scripts\run-harbor-bench.ps1 -Preset hello -DryRun
```

Environment overrides:

- `LIBRAGENT_API_URL` (default `http://localhost:3030/api`)
- `LIBRAGENT_ASSISTANT_ID` (otherwise resolves assistant named `Coding Expert`)

## Manual Harbor CLI

```powershell
$env:PYTHONPATH = (Get-Location).Path
$env:PYTHONUTF8 = "1"
harbor run `
  -a benchmarks.harbor.libragent_agent:LibrAgentHarborAdapter `
  --ak api_url=http://localhost:3030/api `
  --ak assistant_id=<CODING_EXPERT_UUID> `
  --ak execution_mode=yolo `
  -d terminal-bench@2.0 `
  -l 1 `
  -n 1
```

## Success criteria

- Script health check prints `executionMode=yolo`
- Trial `verifier/reward.txt` is `1` (or job eval mean `1.0`)

On Windows, Harbor may still exit non-zero due to console emoji encoding; trust
`reward.txt` / pytest output under `jobs/<timestamp>/`.
