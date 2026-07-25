#!/usr/bin/env bash
# Run Harbor / Terminal-Bench tasks against a live LibrAgent Session API.
set -euo pipefail

PRESET="hello"
PRESET_EXPLICIT=0
DATASET="terminal-bench/terminal-bench-2-1"
DATASET_EXPLICIT=0
HARBOR_INDEX_DATASET="harbor-index/harbor-index-1.0"
PATH_ARG=""
INCLUDE=""
N_TASKS=0
N_ATTEMPTS="${LIBRAGENT_N_ATTEMPTS:-1}"
CONCURRENT=1
API_URL="${LIBRAGENT_API_URL:-http://localhost:3030/api}"
ASSISTANT_ID="${LIBRAGENT_ASSISTANT_ID:-}"
# Harbor -m. Prefer explicit CLI/env; otherwise read global preferredModel from LibrAgent.
HARBOR_MODEL="${LIBRAGENT_MODEL:-${LIBRAGENT_HARBOR_MODEL:-}}"
EXECUTION_MODE="${LIBRAGENT_EXECUTION_MODE:-unsafe}"
# Omitted by default (official submissions must not modify timeouts/resources).
# Set LIBRAGENT_* or pass CLI flags for local debugging only.
TIMEOUT_MULTIPLIER="${LIBRAGENT_TIMEOUT_MULTIPLIER:-}"
AGENT_TIMEOUT_MULTIPLIER="${LIBRAGENT_AGENT_TIMEOUT_MULTIPLIER:-}"
ASSISTANT_NAME="Coding Expert"
SKIP_HEALTH=0
DRY_RUN=0
DEBUG_HARBOR=0
VERIFIER_ENV=()

usage() {
  cat <<'EOF'
Usage: scripts/run-harbor-bench.sh [options]

Options:
  --preset hello|terminal-bench|harbor-index|path|dataset
                                       Default: hello
  --dataset NAME                       Default depends on preset
                                       (terminal-bench/terminal-bench-2-1 or
                                       harbor-index/harbor-index-1.0).
                                       Required for preset=dataset; omitting
                                       --preset with --dataset selects dataset.
  --path DIR                           Local task/dataset path (preset=path)
  --include GLOB                       Include task name pattern (-i)
  --n-tasks N                          Max tasks (-l)
  --n-attempts N                       Attempts per task (-k), default 1 (use 5 for official submission)
  --concurrent N                       Concurrent trials (-n), default 1
  --api-url URL                        Default: http://localhost:3030/api
  --assistant-id UUID                  Or set LIBRAGENT_ASSISTANT_ID
  --model NAME                         Harbor -m (provider/model). Default: LibrAgent
                                       global preferredModel via GET /api/settings/preferredModel
                                       (or LIBRAGENT_MODEL / LIBRAGENT_HARBOR_MODEL)
  --execution-mode yolo|unsafe|normal  Default: unsafe (or LIBRAGENT_EXECUTION_MODE)
  --timeout-multiplier N               Local debug only (omitted by default; submissions must not set this)
  --agent-timeout-multiplier N         Local debug only (omitted by default; submissions must not set this)
  --verifier-env KEY=VALUE             Pass environment variable to verifier (repeatable)
  --skip-health-check
  --dry-run
  --debug
  -h, --help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --preset) PRESET="$2"; PRESET_EXPLICIT=1; shift 2 ;;
    --dataset) DATASET="$2"; DATASET_EXPLICIT=1; shift 2 ;;
    --path) PATH_ARG="$2"; shift 2 ;;
    --include) INCLUDE="$2"; shift 2 ;;
    --n-tasks) N_TASKS="$2"; shift 2 ;;
    --n-attempts) N_ATTEMPTS="$2"; shift 2 ;;
    --concurrent) CONCURRENT="$2"; shift 2 ;;
    --api-url) API_URL="$2"; shift 2 ;;
    --assistant-id) ASSISTANT_ID="$2"; shift 2 ;;
    --model) HARBOR_MODEL="$2"; shift 2 ;;
    --execution-mode) EXECUTION_MODE="$2"; shift 2 ;;
    --timeout-multiplier) TIMEOUT_MULTIPLIER="$2"; shift 2 ;;
    --agent-timeout-multiplier) AGENT_TIMEOUT_MULTIPLIER="$2"; shift 2 ;;
    --verifier-env|--ve) VERIFIER_ENV+=("$2"); shift 2 ;;
    --skip-health-check) SKIP_HEALTH=1; shift ;;
    --dry-run) DRY_RUN=1; shift ;;
    --debug) DEBUG_HARBOR=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage; exit 1 ;;
  esac
done

# Convenience: if --dataset is set without an explicit --preset, treat as dataset preset.
if [[ "$PRESET_EXPLICIT" -eq 0 && "$DATASET_EXPLICIT" -eq 1 ]]; then
  PRESET="dataset"
fi

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

resolve_harbor_model() {
  if [[ -n "$HARBOR_MODEL" ]]; then
    echo "$HARBOR_MODEL"
    return
  fi
  echo "==> Resolving Harbor -m from LibrAgent global preferredModel" >&2
  "$PYTHON" - "$API_URL" <<'PY'
import json, sys, urllib.error, urllib.request

api = sys.argv[1].rstrip("/")
url = f"{api}/settings/preferredModel"
try:
    with urllib.request.urlopen(url, timeout=15) as resp:
        payload = json.load(resp)
except urllib.error.HTTPError as e:
    body = e.read().decode("utf-8", errors="replace")
    raise SystemExit(
        f"Failed to read preferredModel from {url} (HTTP {e.code}): {body}\n"
        "Restart LibrAgent (pnpm tauri dev) so GET /api/settings/preferredModel is available, "
        "or pass --model provider/model."
    ) from e
except Exception as e:
    raise SystemExit(
        f"Failed to read preferredModel from {url}: {e}\n"
        "Is LibrAgent running? Or pass --model provider/model."
    ) from e

harbor_model = (payload.get("harborModel") or "").strip()
if not harbor_model:
    model = (payload.get("model") or "").strip()
    provider = (payload.get("provider") or "").strip()
    if model and provider and "/" not in model:
        harbor_model = f"{provider}/{model}"
    else:
        harbor_model = model
if not harbor_model:
    raise SystemExit(
        "preferredModel is empty. Set a preferred model in LibrAgent settings, "
        "or pass --model provider/model."
    )
print(harbor_model)
print(
    f"  preferredModel provider={payload.get('provider')!r} model={payload.get('model')!r}",
    file=sys.stderr,
)
PY
}

HARBOR_MODEL="$(resolve_harbor_model)"
echo "Using Harbor model (-m)=$HARBOR_MODEL"

if [[ "$SKIP_HEALTH" -eq 0 ]]; then
  echo "==> Checking $API_URL/health"
  curl -fsS "$API_URL/health" >/dev/null
  echo "==> Smoke-checking executionMode=$EXECUTION_MODE (create → verify mode → await idle → terminate)"
  # Start a real turn so we verify the session can run, but wait for idle before
  # cleanup — terminating ~0.5s into an LLM turn aborts the reply mid-flight.
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
  cleanup_smoke() {
    curl -fsS -X POST "$API_URL/sessions/$SID/terminate" >/dev/null 2>&1 || true
    echo "  smoke session terminated ($SID)"
  }
  trap cleanup_smoke EXIT
  SESSION_JSON=$(curl -fsS "$API_URL/sessions/$SID")
  MODE=$("$PYTHON" -c 'import json,sys; print(json.load(sys.stdin).get("executionMode"))' <<<"$SESSION_JSON")
  STATUS=$("$PYTHON" -c 'import json,sys; print(json.load(sys.stdin).get("status"))' <<<"$SESSION_JSON")
  LAST_MSG=$("$PYTHON" -c 'import json,sys; print(json.load(sys.stdin).get("lastMessageAt"))' <<<"$SESSION_JSON")
  echo "  session=$SID executionMode=$MODE status=$STATUS"
  if [[ "$EXECUTION_MODE" != "normal" && "$MODE" != "$EXECUTION_MODE" ]]; then
    echo "API did not apply executionMode=$EXECUTION_MODE (got $MODE)" >&2
    exit 1
  fi
  if [[ "$STATUS" == "idle" && "$LAST_MSG" == "None" ]]; then
    echo "Smoke session did not start a workflow (status=idle with no messages)." >&2
    exit 1
  fi
  for _ in $(seq 1 90); do
    if [[ "$STATUS" == "idle" || "$STATUS" == "error" || "$STATUS" == "paused" ]]; then
      break
    fi
    sleep 1
    SESSION_JSON=$(curl -fsS "$API_URL/sessions/$SID")
    STATUS=$("$PYTHON" -c 'import json,sys; print(json.load(sys.stdin).get("status"))' <<<"$SESSION_JSON")
  done
  echo "  settled status=$STATUS"
  if [[ "$STATUS" != "idle" && "$STATUS" != "error" && "$STATUS" != "paused" ]]; then
    echo "Smoke session did not settle within 90s (status=$STATUS)" >&2
    exit 1
  fi
  cleanup_smoke
  trap - EXIT
fi

ARGS=(
  run
  -a benchmarks.harbor.libragent_agent:LibrAgentHarborAdapter
  -m "$HARBOR_MODEL"
  --ak "api_url=$API_URL"
  --ak "assistant_id=$ASSISTANT_ID"
  --ak "execution_mode=$EXECUTION_MODE"
  -n "$CONCURRENT"
  -k "$N_ATTEMPTS"
)
if [[ -n "$TIMEOUT_MULTIPLIER" ]]; then
  ARGS+=(--timeout-multiplier "$TIMEOUT_MULTIPLIER")
fi
if [[ -n "$AGENT_TIMEOUT_MULTIPLIER" ]]; then
  ARGS+=(--agent-timeout-multiplier "$AGENT_TIMEOUT_MULTIPLIER")
fi
for ve in "${VERIFIER_ENV[@]}"; do
  ARGS+=(--ve "$ve")
done

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
  harbor-index)
    if [[ "$DATASET_EXPLICIT" -eq 0 ]]; then
      DATASET="$HARBOR_INDEX_DATASET"
    fi
    echo "==> Preset: Harbor Index ($DATASET)"
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
  dataset)
    [[ "$DATASET_EXPLICIT" -eq 1 ]] || {
      echo "--dataset required for preset=dataset (e.g. --dataset swe-bench/swe-bench-verified-1.0)" >&2
      exit 1
    }
    echo "==> Preset: dataset ($DATASET)"
    ARGS+=(-d "$DATASET")
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
