# Harbor + LibrAgent adapter

Runs [Harbor](https://github.com/laude-institute/harbor) / Terminal-Bench tasks against a
live LibrAgent Session API (`POST /api/sessions` with `executionMode`).

Benchmark sessions default to **`unsafe`** so shell and other hard-approval tools run
without manual approval. Override with Harbor `--ak execution_mode=yolo|normal` or
`LIBRAGENT_EXECUTION_MODE`.

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
  -ExecutionMode unsafe

# Or set globally for the shell session
$env:LIBRAGENT_EXECUTION_MODE = "unsafe"
pnpm bench:terminal

# Dry-run (print harbor command only)
.\scripts\run-harbor-bench.ps1 -Preset hello -DryRun
```

Environment overrides:

- `LIBRAGENT_API_URL` (default `http://localhost:3030/api`)
- `LIBRAGENT_ASSISTANT_ID` (otherwise resolves assistant named `Coding Expert`)
- `LIBRAGENT_EXECUTION_MODE` (`normal` | `yolo` | `unsafe`, default `unsafe` in adapter/scripts)
- `LIBRAGENT_TIMEOUT_MULTIPLIER` / `LIBRAGENT_AGENT_TIMEOUT_MULTIPLIER` — Harbor timeouts (Terminal-Bench agent default is often ~360s; raise for long tasks)
- `LIBRAGENT_POLL_TIMEOUT_SEC` — optional adapter wall-clock poll budget; omit to wait until Harbor cancels

## Timeouts (important)

Harbor cancels the agent coroutine when the **agent timeout** elapses. The adapter
must **not** harvest workspace/messages after that cancel — incomplete harvests
were scoring unfinished runs as finished.

- Wait for session status `idle` or `error` only (`paused`/`busy` are not done).
- On Harbor cancel (`CancelledError`), the adapter re-raises and skips harvest.
- For long Terminal-Bench tasks, increase the agent budget, e.g.:

```powershell
.\scripts\run-harbor-bench.ps1 -Preset terminal-bench -NTasks 1 `
  -AgentTimeoutMultiplier 3
# or
$env:LIBRAGENT_AGENT_TIMEOUT_MULTIPLIER = "3"
pnpm bench:terminal
```

## Manual Harbor CLI

```powershell
$env:PYTHONPATH = (Get-Location).Path
$env:PYTHONUTF8 = "1"
harbor run `
  -a benchmarks.harbor.libragent_agent:LibrAgentHarborAdapter `
  --ak api_url=http://localhost:3030/api `
  --ak assistant_id=<CODING_EXPERT_UUID> `
  --ak execution_mode=unsafe `
  --agent-timeout-multiplier 3 `
  -d terminal-bench@2.0 `
  -l 1 `
  -n 1
```

## Success criteria

- Script health check prints `executionMode=unsafe` (or your override)
- Trial `verifier/reward.txt` is `1` (or job eval mean `1.0`)
- Agent logs show `Session workflow reached terminal state: idle|error` before harvest
  (not `Session polling cancelled ... Will still harvest`)

On Windows, Harbor may still exit non-zero due to console emoji encoding; trust
`reward.txt` / pytest output under `jobs/<timestamp>/`.
