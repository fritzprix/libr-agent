#!/usr/bin/env bash
set -euo pipefail

# Copy refactoring.md into docs/history with a timestamp suffix.
# Usage: ./scripts/cp_refactoring_to_history.sh [source-file] [dest-dir]
# Defaults: source-file=refactoring.md  dest-dir=docs/history

SRC_FILE="${1:-refactoring.md}"
DEST_DIR="${2:-docs/history}"

if [ ! -f "$SRC_FILE" ]; then
  echo "Source file not found: $SRC_FILE" >&2
  exit 2
fi

mkdir -p "$DEST_DIR"

BASE_NAME=$(basename "$SRC_FILE")
EXT="${BASE_NAME##*.}"
NAME_NO_EXT="${BASE_NAME%.*}"
TS=$(date +"%Y%m%d_%H%M%S")
NEW_NAME="${NAME_NO_EXT}_${TS}.${EXT}"
DEST_PATH="$DEST_DIR/$NEW_NAME"

cp -- "$SRC_FILE" "$DEST_PATH"

echo "Copied '$SRC_FILE' -> '$DEST_PATH'"
exit 0
