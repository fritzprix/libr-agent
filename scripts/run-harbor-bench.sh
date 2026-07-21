#!/usr/bin/env bash
# Run Harbor / Terminal-Bench tasks against a live LibrAgent Session API.
set -euo pipefail

PRESET="hello"
DATASET="terminal-bench@2.0"
PATH_ARG=""
INCLUDE=""
N_TASKS=0
CONCURRENT=1
API_URL="${LIBRAGENT_API_URL:-http://localhost:3030/api}"
ASSISTANT_ID="${LIBRAGENT_ASSISTANT_ID:-}"
EXECUTION_MODE="${LIBRAGENT_EXECUTION_MODE:-unsafe}"
TIMEOUT_MULTIPLIER="${LIBRAGENT_TIMEOUT_MULTIPLIER:-1.0}"
AGENT_TIMEOUT_MULTIPLIER="${LIBRAGENT_AGENT_TIMEOUT_MULTIPLIER:-}"
ASSISTANT_NAME="Coding Expert"
SKIP_HEALTH=0
DRY_RUN=0
DEBUG_HARBOR=0

usage() {
  cat <<'EOF'
Usage: scripts/run-harbor-bench.sh [options]

Options:
  --preset hello|terminal-bench|path   Default: hello
  --dataset NAME@version               Default: terminal-bench@2.0
  --path DIR                           Local task/dataset path (preset=path)
  --include GLOB                       Include task name pattern (-i)
  --n-tasks N                          Max tasks (-l)
  --concurrent N                       Concurrent trials (-n), default 1
  --api-url URL                        Default: http://localhost:3030/api
  --assistant-id UUID                  Or set LIBRAGENT_ASSISTANT_ID
  --execution-mode yolo|unsafe|normal  Default: unsafe (or LIBRAGENT_EXECUTION_MODE)
  --timeout-multiplier N               Harbor task timeout multiplier (default: 1.0)
  --agent-timeout-multiplier N         Harbor agent-only timeout multiplier
  --skip-health-check
  --dry-run
  --debug
  -h, --help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --preset) PRESET="$2"; shift 2 ;;
    --dataset) DATASET="$2"; shift 2 ;;
    --path) PATH_ARG="$2"; shift 2 ;;
    --include) INCLUDE="$2"; shift 2 ;;
    --n-tasks) N_TASKS="$2"; shift 2 ;;
    --concurrent) CONCURRENT="$2"; shift 2 ;;
    --api-url) API_URL="$2"; shift 2 ;;
    --assistant-id) ASSISTANT_ID="$2"; shift 2 ;;
    --execution-mode) EXECUTION_MODE="$2"; shift 2 ;;
    --timeout-multiplier) TIMEOUT_MULTIPLIER="$2"; shift 2 ;;
    --agent-timeout-multiplier) AGENT_TIMEOUT_MULTIPLIER="$2"; shift 2 ;;
    --skip-health-check) SKIP_HEALTH=1; shift ;;
    --dry-run) DRY_RUN=1; shift ;;
    --debug) DEBUG_HARBOR=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage; exit 1 ;;
  esac
done

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"
export PYTHONUTF8=1
export PYTHONIOENCODING=utf-8
export PYTHONPATH="$REPO_ROOT${PYTHONPATH:+:$PYTHONPATH}"

need() {
  command -v "$1" >/dev/null 2>&1 || { echo "Missing required command: $1" >&2; exit 1; }
}

resolve_python() {
  if command -v python3 >/dev/null 2>&1; then
    command -v python3
  elif command -v python >/dev/null 2>&1; then
    command -v python
  else
    echo "Missing required command: python3 or python" >&2
    exit 1
  fi
}

PYTHON="$(resolve_python)"

ensure_harbor() {
  if command -v harbor >/dev/null 2>&1; then
    return
  fi

  echo "==> harbor command not found. Attempting to bootstrap/install..."

  if command -v uv >/dev/null 2>&1; then
    echo "Installing harbor and httpx using uv..."
    uv pip install harbor httpx --system
  else
    echo "Installing harbor and httpx using pip..."
    "$PYTHON" -m pip install harbor httpx
  fi

  if ! command -v harbor >/dev/null 2>&1; then
    scripts_dir="$("$PYTHON" -c "import sysconfig; print(sysconfig.get_path('scripts'))" 2>/dev/null || true)"
    if [[ -n "$scripts_dir" && -d "$scripts_dir" ]]; then
      echo "Adding $scripts_dir to PATH for this session" >&2
      export PATH="$scripts_dir:$PATH"
    fi
  fi

  if ! command -v harbor >/dev/null 2>&1; then
    echo "Could not bootstrap harbor. Please install manually (e.g. 'pip install harbor httpx')." >&2
    exit 1
  fi
  echo "==> harbor successfully bootstrapped and ready!"
}

need curl
ensure_harbor

resolve_assistant_id() {
  if [[ -n "$ASSISTANT_ID" ]]; then
    echo "$ASSISTANT_ID"
    return
  fi
  echo "==> Resolving assistant '$ASSISTANT_NAME'" >&2
  "$PYTHON" - "$API_URL" "$ASSISTANT_NAME" <<'PY'
import json, sys, urllib.request
api, name = sys.argv[1], sys.argv[2]
with urllib.request.urlopen(f"{api}/assistants", timeout=15) as resp:
    payload = json.load(resp)
assistants = payload if isinstance(payload, list) else payload.get("assistants", [])
match = next((a for a in assistants if a.get("name") == name), None)
if match is None:
    match = next((a for a in assistants if "Coding" in str(a.get("name", ""))), None)
if match is None:
    raise SystemExit("Could not resolve assistant id; pass --assistant-id")
print(match["id"])
PY
}

ASSISTANT_ID="$(resolve_assistant_id)"
echo "Using assistantId=$ASSISTANT_ID"

if [[ "$SKIP_HEALTH" -eq 0 ]]; then
  echo "==> Checking $API_URL/health"
  curl -fsS "$API_URL/health" >/dev/null
  echo "==> Smoke-checking executionMode=$EXECUTION_MODE"
  CREATE=$(curl -fsS -X POST "$API_URL/sessions" \
    -H 'Content-Type: application/json' \
    -d "$("$PYTHON" - <<PY
import json
print(json.dumps({
  "assistantId": "$ASSISTANT_ID",
  "name": "harbor-bench-smoke",
  "request": "Reply with exactly: ok",
  "executionMode": "$EXECUTION_MODE",
  "workspaceIsolation": "host",
}))
PY
)")
  SID=$("$PYTHON" -c 'import json,sys; print(json.load(sys.stdin)["id"])' <<<"$CREATE")
  sleep 1
  MODE=$(curl -fsS "$API_URL/sessions/$SID" | "$PYTHON" -c 'import json,sys; print(json.load(sys.stdin).get("executionMode"))')
  echo "  session=$SID executionMode=$MODE"
  if [[ "$EXECUTION_MODE" != "normal" && "$MODE" != "$EXECUTION_MODE" ]]; then
    echo "API did not apply executionMode=$EXECUTION_MODE (got $MODE)" >&2
    exit 1
  fi
fi

ARGS=(
  run
  -a benchmarks.harbor.libragent_agent:LibrAgentHarborAdapter
  --ak "api_url=$API_URL"
  --ak "assistant_id=$ASSISTANT_ID"
  --ak "execution_mode=$EXECUTION_MODE"
  -n "$CONCURRENT"
  --timeout-multiplier "$TIMEOUT_MULTIPLIER"
)
if [[ -n "$AGENT_TIMEOUT_MULTIPLIER" ]]; then
  ARGS+=(--agent-timeout-multiplier "$AGENT_TIMEOUT_MULTIPLIER")
fi

case "$PRESET" in
  hello)
    echo "==> Preset: hello-world"
    CACHED=$(find "${HOME}/.cache/harbor/tasks" -type d -name hello-world 2>/dev/null | head -n 1 || true)
    if [[ -n "$CACHED" ]]; then
      ARGS+=(-p "$CACHED")
    else
      ARGS+=(--task-git-url https://github.com/laude-institute/harbor.git -p examples/tasks/hello-world)
    fi
    ;;
  terminal-bench)
    echo "==> Preset: Terminal-Bench ($DATASET)"
    ARGS+=(-d "$DATASET")
    [[ -n "$INCLUDE" ]] && ARGS+=(-i "$INCLUDE")
    [[ "$N_TASKS" -gt 0 ]] && ARGS+=(-l "$N_TASKS")
    ;;
  path)
    [[ -n "$PATH_ARG" ]] || { echo "--path required for preset=path" >&2; exit 1; }
    ARGS+=(-p "$PATH_ARG")
    [[ -n "$INCLUDE" ]] && ARGS+=(-i "$INCLUDE")
    [[ "$N_TASKS" -gt 0 ]] && ARGS+=(-l "$N_TASKS")
    ;;
  *)
    echo "Unknown preset: $PRESET" >&2
    exit 1
    ;;
esac

[[ "$DEBUG_HARBOR" -eq 1 ]] && ARGS+=(--debug)

echo "==> Running: harbor ${ARGS[*]}"
if [[ "$DRY_RUN" -eq 1 ]]; then
  echo "Dry run only; not executing."
  exit 0
fi

set +e
harbor "${ARGS[@]}"
CODE=$?
set -e

LATEST=$(ls -1d jobs/*/ 2>/dev/null | sort | tail -n 1 || true)
if [[ -n "$LATEST" ]]; then
  echo "==> Latest job: $LATEST"
  if [[ -f "${LATEST}result.json" ]]; then
    "$PYTHON" - "${LATEST}result.json" <<'PY' || true
import json, sys
path = sys.argv[1]
try:
    job = json.load(open(path, encoding="utf-8"))
except Exception:
    print("  (could not parse job result.json)")
    raise SystemExit(0)
evals = (job.get("stats") or {}).get("evals") or {}
for name, value in evals.items():
    metrics = value.get("metrics") or []
    mean = metrics[0].get("mean") if metrics else None
    print(
        f"  eval {name}: mean={mean} trials={value.get('n_trials')} errors={value.get('n_errors')}"
    )
PY
  fi
  while IFS= read -r -d '' reward_file; do
    trial="$(basename "$(dirname "$(dirname "$reward_file")")")"
    reward="$(tr -d '[:space:]' <"$reward_file")"
    echo "  trial ${trial}: reward=${reward}"
  done < <(find "$LATEST" -path '*/verifier/reward.txt' -print0 2>/dev/null || true)
fi

exit "$CODE"
