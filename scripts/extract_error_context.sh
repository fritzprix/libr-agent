#!/usr/bin/env bash
# extract_error_context.sh
# Wrapper that calls the Python extractor with sensible defaults for this repo.

set -euo pipefail

LOG_FILE="${1:-log.txt}"
PATTERN="[ERROR]"
CONTEXT=${2:-5}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PY_SCRIPT="$SCRIPT_DIR/extract_context.py"

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 not found in PATH" >&2
  exit 1
fi

python3 "$PY_SCRIPT" "$LOG_FILE" --pattern "$PATTERN" --context "$CONTEXT"
