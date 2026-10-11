#!/usr/bin/env bash
# Serial knowledge-bridge suite: wipe → research-distill → cold-solve (n=1).
#
# Sources:
#   local (default): .libragent/work/harbor-datasets/libragent-knowledge-bridge-2
#   hub:             harbor download fritzprix/libragent-knowledge-bridge-2@v0.1
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HUB_DATASET="${LIBRAGENT_KNOWLEDGE_BRIDGE_DATASET:-fritzprix/libragent-knowledge-bridge-2@v0.1}"
LOCAL_DATASET_ROOT="$REPO_ROOT/.libragent/work/harbor-datasets/libragent-knowledge-bridge-2"
HUB_CACHE_ROOT="${LIBRAGENT_KNOWLEDGE_BRIDGE_CACHE:-$REPO_ROOT/.libragent/cache/harbor-datasets}"
WIPE="$REPO_ROOT/scripts/harbor-knowledge-bridge-wipe.sh"
SKIP_WIPE="${LIBRAGENT_KNOWLEDGE_BRIDGE_SKIP_WIPE:-0}"
SOURCE="${LIBRAGENT_KNOWLEDGE_BRIDGE_SOURCE:-local}"
DRY_RUN=0
EXTRA_ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=1; shift ;;
    --skip-wipe) SKIP_WIPE=1; shift ;;
    --hub) SOURCE=hub; shift ;;
    --local) SOURCE=local; shift ;;
    --dataset)
      HUB_DATASET="$2"
      SOURCE=hub
      shift 2
      ;;
    *) EXTRA_ARGS+=("$1"); shift ;;
  esac
done

resolve_dataset_root() {
  if [[ "$SOURCE" == "hub" ]]; then
    mkdir -p "$HUB_CACHE_ROOT"
    local dest="$HUB_CACHE_ROOT"
    echo "==> Downloading Hub dataset: $HUB_DATASET" >&2
    if [[ "$DRY_RUN" -eq 1 ]]; then
      echo "  (dry-run) would run: harbor download $HUB_DATASET -o $dest --overwrite" >&2
      # Prefer previously cached tree for dry-run path resolution.
      if [[ -d "$dest/libragent-knowledge-bridge-2" ]]; then
        echo "$dest/libragent-knowledge-bridge-2"
        return
      fi
      echo "$LOCAL_DATASET_ROOT"
      return
    fi
    harbor download "$HUB_DATASET" -o "$dest" --overwrite
    echo "$dest/libragent-knowledge-bridge-2"
    return
  fi
  echo "$LOCAL_DATASET_ROOT"
}

DATASET_ROOT="$(resolve_dataset_root)"
TASK_A="$DATASET_ROOT/kb-research-distill-v1"
TASK_B="$DATASET_ROOT/kb-cold-solve-v1"

for p in "$TASK_A" "$TASK_B" "$WIPE"; do
  [[ -e "$p" ]] || { echo "missing: $p" >&2; exit 1; }
done

chmod +x "$WIPE" \
  "$TASK_A/tests/test.sh" "$TASK_A/solution/solve.sh" \
  "$TASK_B/tests/test.sh" "$TASK_B/solution/solve.sh" 2>/dev/null || true

if [[ "$SKIP_WIPE" != "1" ]]; then
  echo "==> Preflight knowledge wipe"
  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "  (dry-run) would run: $WIPE"
  else
    bash "$WIPE"
  fi
else
  echo "==> Skipping knowledge wipe (LIBRAGENT_KNOWLEDGE_BRIDGE_SKIP_WIPE=1)"
fi

run_task() {
  local path="$1"
  local label="$2"
  echo "==> Harbor task: $label ($path)"
  local args=(
    --preset path
    --path "$path"
    --concurrent 1
    --n-attempts 1
    # Suite already talks to the API for wipe; avoid per-task smoke waits.
    --skip-health-check
  )
  if [[ "$DRY_RUN" -eq 1 ]]; then
    args+=(--dry-run)
  fi
  # shellcheck disable=SC2068
  node "$REPO_ROOT/scripts/run-harbor-bench.cjs" "${args[@]}" ${EXTRA_ARGS[@]+"${EXTRA_ARGS[@]}"}
}

echo "==> Knowledge-bridge source=$SOURCE dataset_root=$DATASET_ROOT"
run_task "$TASK_A" "kb-research-distill-v1"
run_task "$TASK_B" "kb-cold-solve-v1"

echo "==> Knowledge-bridge serial suite finished"
if [[ "$DRY_RUN" -eq 0 ]]; then
  echo "Inspect latest jobs/*/agent/trajectory.json for knowledge__recordKnowledge then knowledge__searchKnowledge"
fi
