#!/usr/bin/env bash
# Usage: ./dump_log.sh [LINES] [SRC_LOG] [OUT_FILE]
LINES="${1:-10}"
SRC="${2:-$HOME/.local/share/com.fritzprix.synapticflow/logs/synaptic-flow.log}"
OUT="${3:-./log.txt}"
tail -n "$LINES" -- "$SRC" > "$OUT"