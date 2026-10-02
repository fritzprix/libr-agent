#!/usr/bin/env bash
# Call one App Control MCP tool on POST /mcp/control.
# Usage: app_control.sh <tool_name> '<json-arguments>'
set -euo pipefail

PORT="${LIBRAGENT_HTTP_PORT:-$(cat "${HOME}/.libragent/http_port" 2>/dev/null || echo 3030)}"
URL="http://127.0.0.1:${PORT}/mcp/control"

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <tool_name> ['{\"arg\":...}']" >&2
  echo "Tools: app__navigate | app__highlight | app__install_preset | app__focus_session | app__wait_ui" >&2
  exit 2
fi

NAME="$1"
if [[ $# -ge 2 ]]; then
  ARGS="$2"
else
  ARGS='{}'
fi

curl -sS -m 30 -X POST "$URL" \
  -H 'content-type: application/json' \
  --data-binary "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"${NAME}\",\"arguments\":${ARGS}}}"
echo
