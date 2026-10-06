#!/usr/bin/env bash
# Thin wrapper for Cursor → agy print-mode delegation.
# Usage:
#   scripts/run.sh [--timeout DUR] [--dir ABS]... -- TASK_TEXT
#   scripts/run.sh [--model ID] ... -- TASK   # only when user explicitly asked
# Env defaults:
#   AGY_MODEL=gemini-3.8-flash-high   # do not change unless user sets AGY_MODEL
#   AGY_TIMEOUT=300s
#   AGY_OUTPUT=text   # or json
set -euo pipefail

AGY_BIN="${AGY_BIN:-$(command -v agy || true)}"
MODEL="${AGY_MODEL:-gemini-3.8-flash-high}"
TIMEOUT="${AGY_TIMEOUT:-300s}"
OUTPUT="${AGY_OUTPUT:-text}"
DIRS=()
SKIP_PERMS=1

usage() {
  echo "Usage: $0 [--model ID] [--timeout DUR] [--dir ABS] [--no-skip-perms] [--json] -- TASK" >&2
  echo "  Default model: gemini-3.8-flash-high (pass --model only if the user requested it)" >&2
  exit 2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --model)
      MODEL="${2:?}"; shift 2 ;;
    --timeout)
      TIMEOUT="${2:?}"; shift 2 ;;
    --dir)
      DIRS+=("${2:?}"); shift 2 ;;
    --json)
      OUTPUT=json; shift ;;
    --no-skip-perms)
      SKIP_PERMS=0; shift ;;
    --)
      shift; break ;;
    -h|--help)
      usage ;;
    *)
      break ;;
  esac
done

TASK="${*:-}"
if [[ -z "$TASK" ]]; then
  usage
fi

if [[ -z "$AGY_BIN" || ! -x "$AGY_BIN" ]]; then
  echo "agy not found on PATH (set AGY_BIN)" >&2
  exit 1
fi

args=(-p "$TASK" --model "$MODEL" --print-timeout "$TIMEOUT" --output-format "$OUTPUT")
if [[ "$SKIP_PERMS" -eq 1 ]]; then
  args+=(--dangerously-skip-permissions)
fi
for d in "${DIRS[@]}"; do
  args+=(--add-dir "$d")
done

exec "$AGY_BIN" "${args[@]}"
