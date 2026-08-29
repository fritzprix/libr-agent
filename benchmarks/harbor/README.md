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
  `dockerConfig: { attachContainer, workdir, manageLifecycle: false }`
  - `workdir` is resolved **per task**, not a fixed path:
    1. task `[environment].workdir` when set
    2. else container image `WORKDIR` (`docker inspect`)
    3. else live `docker exec … pwd`
    4. else last-resort `/app` (legacy TB convention; logged as a warning)
- Shell and file tools run **inside** that container (`docker exec -w …` /
  `docker cp`). Absolute paths under that workdir are valid.
- LibrAgent does **not** create a second container and does **not** destroy
  Harbor’s container on session end.
- Host download/upload sync of the workdir is **skipped** on the attach path.

If the main container ID cannot be resolved (non-Docker Harbor providers such as
Modal/E2B, or missing Compose labels), the adapter **falls back** to the older
host-sync path: pull the container workdir to a host trial workspace, run a host
session, then push changes back. On that path, prefer relative paths under the
synced workspace rather than absolute container paths.

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

# Terminal-Bench: full dataset (n-attempts defaults to 1; add --n-attempts 5 for official submission)
pnpm bench:terminal:all

# Harbor Index: first task only
pnpm bench:harbor

# Harbor Index: full dataset (n-attempts defaults to 1; add --n-attempts 5 for official submission)
pnpm bench:harbor:all

# Upload latest job results to Harbor Hub (UTF-8 enabled automatically)
pnpm bench:upload
# Or upload a specific job folder:
pnpm bench:upload jobs/2026-08-16__14-25-18
```

Before uploading, `bench:upload` normalizes Hermes trial metadata to the stable
release line (for example, `Hermes Agent v0.19.0`). Hermes includes its install
path and Python version in the raw version output, but Harbor uses the complete
version string as the agent identity. Removing those environment-specific
details keeps all trials from one Hermes release in a single Harbor Hub result
group.

`pnpm bench:*` dispatches via `scripts/run-harbor-bench.cjs` to PowerShell on Windows
and bash on Linux/macOS. Defaults omit Harbor timeout/resource overrides so runs match
official submission rules (`submissions may not modify timeouts or resources`).
`bench:terminal:all` / `bench:harbor:all` now default to **1 attempt per task** (`-k 1`).
Pass `--n-attempts 5` explicitly for official leaderboard submissions:

```sh
pnpm bench:terminal:all --n-attempts 5
pnpm bench:harbor:all --n-attempts 5
```

Or call Harbor directly for submission-compatible runs:

```sh
harbor run -d terminal-bench/terminal-bench-2-1 -a <agent> -m <model> -k 5
harbor run -d harbor-index/harbor-index-1.0 -a <agent> -m <model> -k 5
```

`pnpm bench:*` resolves Harbor `-m` from LibrAgent's **global** `preferredModel`
setting (`GET /api/settings/preferredModel`) before the run starts — the same
source session creation uses when no per-session model is set. Override with
`--model provider/model` or `LIBRAGENT_MODEL`.

LibrAgent still runs the assistant's tools/API keys from the selected assistant;
`-m` is for Harbor reporting / Hub upload metadata (model_info + token rows).

Note: Harbor Index scoring may require judge API keys via `--verifier-env` /
`--ve` (e.g. `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GEMINI_API_KEY`) for LLM-judge tasks.

## Running any Harbor Hub dataset

Use `--dataset <org/name-version>` to run any dataset from the
[Harbor Hub registry](https://harbor.laude-institute.org) without touching the scripts.
`pnpm bench:registry` (and `pnpm xbench:registry` for Hermes) is a shortcut for `--preset dataset` — defaults to `NovitaAI/tb21-file-recovery`, or pass `--dataset` to run any other registry dataset (pnpm 9+ forwards unknown script flags without requiring `--`):

```sh
pnpm bench:registry                                            # runs default dataset: NovitaAI/tb21-file-recovery
pnpm xbench:registry                                           # runs Hermes on default dataset: NovitaAI/tb21-file-recovery
pnpm bench:registry --dataset swe-bench/swe-bench-verified-1.0 --n-tasks 1
pnpm bench:registry --dataset aider-bench/aider-bench-1.0
```

```sh
# One-shot via node dispatcher (cross-platform)
node scripts/run-harbor-bench.cjs --dataset swe-bench/swe-bench-verified-1.0 --n-tasks 1
node scripts/run-harbor-bench.cjs --preset dataset --dataset aider-bench/aider-bench-1.0

# bash
./scripts/run-harbor-bench.sh --dataset swe-bench/swe-bench-verified-1.0 --n-tasks 1

# PowerShell
.\scripts\run-harbor-bench.ps1 -Dataset swe-bench/swe-bench-verified-1.0 -NTasks 1
```

### Adding a permanent alias to `package.json`

For frequently-run benchmarks, add a shortcut pair to the `scripts` section of
[`package.json`](../../package.json):

```jsonc
// package.json → scripts
"bench:swe":     "node scripts/run-harbor-bench.cjs --dataset swe-bench/swe-bench-verified-1.0 --n-tasks 1",
"bench:swe:all": "node scripts/run-harbor-bench.cjs --dataset swe-bench/swe-bench-verified-1.0",
"bench:aider":     "node scripts/run-harbor-bench.cjs --dataset aider-bench/aider-bench-1.0 --n-tasks 1",
"bench:aider:all": "node scripts/run-harbor-bench.cjs --dataset aider-bench/aider-bench-1.0"
```

Then run as usual:

```sh
pnpm bench:swe          # first task only
pnpm bench:swe:all      # full dataset, n-attempts=1 (default)
pnpm bench:swe:all --n-attempts 5   # leaderboard submission
```

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
- `LIBRAGENT_TIMEOUT_MULTIPLIER` / `LIBRAGENT_AGENT_TIMEOUT_MULTIPLIER` — **local debug only**.
  Official Terminal-Bench submissions must not modify timeouts or resources; `pnpm bench:*`
  omits these flags unless you set the env vars or pass CLI options.
- `LIBRAGENT_POLL_TIMEOUT_SEC` — optional adapter wall-clock poll budget; omit to wait until Harbor cancels

## Timeouts (important)

Official submissions use Harbor’s default task/agent timeouts (do **not** pass
`--timeout-multiplier` / `--agent-timeout-multiplier`).

Harbor cancels the agent coroutine when the **agent timeout** elapses. The adapter
must **not** treat that cancel as a successful completion (incomplete success
harvests used to score unfinished runs as finished).

- Wait for session status `idle` or `error` only (`paused`/`busy` are not done).
- On Harbor cancel (`CancelledError`) or poll-budget expiry, the adapter writes a
  **diagnostic** `agent/trajectory.json` + `agent/timeout_meta.json` (best-effort),
  then `DELETE`s the session and re-raises — it does **not** mark the trial completed.
- If the session reaches a terminal status with **zero tool calls**, the adapter
  still writes ATIF/meta, deletes the session, and raises `EmptyAgentWorkError`
  so the trial is an agent error rather than a silent verifier miss.
- On every terminal path the adapter calls `DELETE /sessions/{id}` so the
  LibrAgent session is removed instead of lingering as an orphan. DELETE also
  terminates any still-running workflow. Successful runs delete only **after**
  harvesting messages and writing `trajectory.json`. Delete (and diagnostic dump)
  are shielded so a Harbor cancel still completes teardown.
- For **local** debugging of long tasks only, you may raise the agent budget:

```sh
# Local debug only — not for official submissions
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
# Submission-compatible (no timeout/resource overrides)
harbor run \
  -a benchmarks.harbor.libragent_agent:LibrAgentHarborAdapter \
  --ak api_url=http://localhost:3030/api \
  --ak assistant_id=<CODING_EXPERT_UUID> \
  --ak execution_mode=unsafe \
  -d terminal-bench/terminal-bench-2-1 \
  -k 5 \
  -n 1
```

## Success criteria

- Script health check creates a short smoke session (`Reply with exactly: ok`),
  verifies `executionMode=unsafe` (or your override), **waits until idle**, then
  **deletes** it before `harbor run` starts (so smoke does not abort an in-flight
  LLM turn or leave a busy session while Docker builds the task environment)
- Harbor’s progress bar timer includes **environment build**, not only agent
  runtime; agent timeout starts when the adapter runs
- Trial `verifier/reward.txt` is `1` (or job eval mean `1.0`)
- Agent logs show `Session workflow reached terminal state: idle|error` before harvest
  (not `Session polling cancelled ... Will still harvest`)

On Windows, Harbor may still exit non-zero due to console emoji encoding; trust
`reward.txt` / pytest output under `jobs/<timestamp>/`.

### Windows: `FileNotFoundError` while extracting Harbor packages

Some `harbor-index` tasks (e.g. `spider2-dbt-*`) unpack nested
`dbt_packages/...` trees whose full path exceeds classic Windows `MAX_PATH`
(~260). Harbor always caches under `~/.cache/harbor/tasks/packages/...` (no
cache-dir flag). Even a short `USERPROFILE` like `C:\h` can still fail (paths
~270 chars). A previous `jobs/` summary with `mean=1.0` can still print — that
is not proof the current run succeeded.

`scripts/run-harbor-bench.ps1` runs Harbor through
`scripts/harbor_short_cache_run.py`, which patches `PACKAGE_CACHE_DIR` to a
short root (`C:\p` by default; override with `LIBRAGENT_HARBOR_CACHE`) before
the CLI starts. Prefer enabling OS long paths when you can (admin; reboot may
be required):

```powershell
New-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem' `
  -Name LongPathsEnabled -Value 1 -PropertyType DWORD -Force
```
