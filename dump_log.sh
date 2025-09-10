#!/usr/bin/env bash
# Usage: ./dump_log.sh [LINES] [SRC_LOG] [OUT_FILE]
LINES="${1:-10}"

# Try to get log path from running Tauri app first
get_tauri_log_path() {
    # Check if Tauri app is running and can provide log path
    if command -v pnpm >/dev/null 2>&1; then
        # Try to invoke the get_app_logs_dir command through Tauri (if app is running)
        # This would require a separate CLI tool or API endpoint
        echo ""
    else
        echo ""
    fi
}

# Fallback to standard dirs crate paths (matching Tauri log plugin implementation)
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS - using Tauri log plugin path
    DEFAULT_SRC="$HOME/Library/Logs/com.fritzprix.synapticflow/synaptic-flow.log"
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    # Linux - using Tauri log plugin path  
    DEFAULT_SRC="$HOME/.local/share/com.fritzprix.synapticflow/logs/synaptic-flow.log"
elif [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" ]]; then
    # Windows - using Tauri log plugin path
    DEFAULT_SRC="$APPDATA/com.fritzprix.synapticflow/logs/synaptic-flow.log"
else
    # Unknown OS, try Linux path as fallback
    DEFAULT_SRC="$HOME/.local/share/com.fritzprix.synapticflow/logs/synaptic-flow.log"
fi

SRC="${2:-$DEFAULT_SRC}"
OUT="${3:-./log.txt}"

# Check if log file exists
if [[ ! -f "$SRC" ]]; then
    echo "⚠️  로그 파일이 존재하지 않습니다: $SRC"
    echo "💡 Tauri 앱을 실행하여 로그를 생성하거나, 수동으로 로그 파일을 생성하세요."
    echo ""
    echo "빈 로그 파일을 생성하시겠습니까? (y/N): "
    read -r response
    if [[ "$response" =~ ^[Yy]$ ]]; then
        mkdir -p "$(dirname "$SRC")"
        touch "$SRC"
        echo "✅ 빈 로그 파일 생성됨: $SRC"
    else
        echo "❌ 로그 덤프를 취소합니다."
        exit 1
    fi
fi

# Extract logs
if tail -n "$LINES" -- "$SRC" > "$OUT" 2>/dev/null; then
    echo "✅ 로그가 $SRC 에서 $OUT 으로 저장되었습니다."
    echo "📊 추출된 라인 수: $(wc -l < "$OUT")"
else
    echo "❌ 로그 추출 실패"
    exit 1
fi