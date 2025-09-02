#!/usr/bin/env bash
# Usage: ./dump_log.sh [LINES] [SRC_LOG] [OUT_FILE]
LINES="${1:-10}"

# OS 감지 및 기본 로그 경로 설정
if [[ "$OSTYPE" == "darwin"* ]]; then
  # macOS
  DEFAULT_SRC="$HOME/Library/Logs/com.fritzprix.synapticflow/synaptic-flow.log"
else
  # Linux (Ubuntu 등)
  DEFAULT_SRC="$HOME/.local/share/com.fritzprix.synapticflow/logs/synaptic-flow.log"
fi

SRC="${2:-$DEFAULT_SRC}"
OUT="${3:-./log.txt}"

tail -n "$LINES" -- "$SRC" > "$OUT"

echo "로그가 $SRC 에서 $OUT 으로 저장되었습니다."