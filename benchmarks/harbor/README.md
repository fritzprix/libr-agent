# Harbor + LibrAgent adapter

Runs [Harbor](https://github.com/laude-institute/harbor) / Terminal-Bench tasks against a
live LibrAgent Session API (`POST /api/sessions` with `executionMode`).

Benchmark sessions default to **`unsafe`** so shell and other hard-approval tools run
without manual approval. Override with Harbor `--ak execution_mode=yolo|normal` or
`LIBRAGENT_EXECUTION_MODE`.

## Workspace modes (Docker attach vs host sync)

For **Docker-backed Harbor trials**, the adapter prefers attaching LibrAgent to
Harbor’s existing Compose `main` container:

- Session uses `workspaceIsolation: "docker"` with
  `dockerConfig: { attachContainer, workdir: "/app" (or task workdir), manageLifecycle: false }`
- Shell and file tools run **inside** that container (`docker exec -w …` /
  `docker cp`). Absolute paths like `/app/gpt2.c` are valid.
- LibrAgent does **not** create a second container and does **not** destroy
  Harbor’s container on session end.
- Host download/upload sync of `/app` is **skipped** on the attach path.

If the main container ID cannot be resolved (non-Docker Harbor providers such as
Modal/E2B, or missing Compose labels), the adapter **falls back** to the older
host-sync path: pull `/app` to a host trial workspace, run a host session, then
push changes back. On that path, prefer relative paths under the synced
workspace rather than absolute `/app/...`.

## Prerequisites

1. `pnpm tauri dev` (HTTP API on `http://localhost:3030/api`)
2. `pip install harbor httpx` (or `uv tool install harbor`)
3. Docker available

## Quick start (recommended)

From the repo root, with LibrAgent already running (`pnpm` works on Windows, Linux, and macOS):

```sh
# Smoke: Harbor hello-world (expect reward=1)
pnpm bench:hello

# Terminal-Bench: first task only
pnpm bench:terminal

# Terminal-Bench: full dataset (long)
pnpm bench:terminal:all
```

`pnpm bench:*` dispatches via `scripts/run-harbor-bench.cjs` to PowerShell on Windows
and bash on Linux/macOS.

Or call the platform script directly:

```sh
# Cross-platform (Node dispatcher)
node scripts/run-harbor-bench.cjs --preset terminal-bench --include "hello*" --n-tasks 3
node scripts/run-harbor-bench.cjs --preset hello --dry-run

# Linux / macOS
./scripts/run-harbor-bench.sh --preset terminal-bench --n-tasks 1 \
  --api-url http://localhost:3030/api \
  --assistant-id <uuid> \
  --execution-mode unsafe

# Windows PowerShell
.\scripts\run-harbor-bench.ps1 -Preset terminal-bench -NTasks 1 `
  -ApiUrl http://localhost:3030/api `
  -AssistantId <uuid> `
  -ExecutionMode unsafe
```

Environment override example:

```sh
# bash / zsh
export LIBRAGENT_EXECUTION_MODE=unsafe
pnpm bench:terminal

# PowerShell
$env:LIBRAGENT_EXECUTION_MODE = "unsafe"
pnpm bench:terminal
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
- On every terminal path (success after harvest, poll-budget/agent timeout,
  cancel, or error) the adapter calls `POST /sessions/{id}/terminate` so the
  LibrAgent session is torn down instead of running on as an orphan. On success
  it terminates only **after** harvesting messages; on abort it terminates
  before re-raising. The terminate request is shielded so a Harbor cancel still
  completes the teardown.
- For long Terminal-Bench tasks, increase the agent budget, e.g.:

```sh
# Cross-platform
node scripts/run-harbor-bench.cjs --preset terminal-bench --n-tasks 1 \
  --agent-timeout-multiplier 3
# or
export LIBRAGENT_AGENT_TIMEOUT_MULTIPLIER=3   # bash
# $env:LIBRAGENT_AGENT_TIMEOUT_MULTIPLIER = "3"  # PowerShell
pnpm bench:terminal
```

## Manual Harbor CLI

```sh
export PYTHONPATH="$(pwd)"
export PYTHONUTF8=1
harbor run \
  -a benchmarks.harbor.libragent_agent:LibrAgentHarborAdapter \
  --ak api_url=http://localhost:3030/api \
  --ak assistant_id=<CODING_EXPERT_UUID> \
  --ak execution_mode=unsafe \
  --agent-timeout-multiplier 3 \
  -d terminal-bench@2.0 \
  -l 1 \
  -n 1
```

## Success criteria

- Script health check prints `executionMode=unsafe` (or your override)
- Trial `verifier/reward.txt` is `1` (or job eval mean `1.0`)
- Agent logs show `Session workflow reached terminal state: idle|error` before harvest
  (not `Session polling cancelled ... Will still harvest`)

On Windows, Harbor may still exit non-zero due to console emoji encoding; trust
`reward.txt` / pytest output under `jobs/<timestamp>/`.
