#!/usr/bin/env bash
# Primary hero chrome beat: highlight → wait → one-click install → wait.
# Requires LibrAgent with --mcp --app-control. Desktop UI must be open.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PRESET="${1:-hn}"
CALL="${SCRIPT_DIR}/app_control.sh"

case "$PRESET" in
  serena|comfyui)
    echo "Refusing noisy preset '${PRESET}' for hero chrome (opens external UI/browser)." >&2
    echo "Use: hn | arxiv | ddg-search | docx | yahoo-finance" >&2
    exit 2
    ;;
esac

echo "Hero chrome beat (preset=${PRESET})"
bash "$CALL" app__highlight "{\"target\":\"preset\",\"name\":\"${PRESET}\",\"ms\":2500}"
bash "$CALL" app__wait_ui '{"ms":800}'
bash "$CALL" app__install_preset "{\"name\":\"${PRESET}\"}"
bash "$CALL" app__wait_ui '{"ms":1500}'
echo "Chrome beat done. Next: POST /api/sessions + app__focus_session + deliverable prompt."
echo "Quiet presets only (hn, arxiv, ddg-search, …). Never serena — dashboard browser steals focus."
