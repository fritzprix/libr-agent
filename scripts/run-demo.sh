#!/usr/bin/env bash
# Launch LibrAgent in demo profile (isolated DB + env LLM seed + app-control).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ -f "$ROOT/.env.demo" ]]; then
  set -a
  # shellcheck disable=SC1091
  source "$ROOT/.env.demo"
  set +a
else
  echo "⚠️  Missing .env.demo — copy from .env.example and fill VPN inference settings" >&2
  export LIBRAGENT_PROFILE=demo
  export LIBRAGENT_MCP_ENABLE=1
  export LIBRAGENT_APP_CONTROL=1
fi

export LIBRAGENT_PROFILE="${LIBRAGENT_PROFILE:-demo}"

# Prefer release binary if present; otherwise tauri dev.
BIN="$ROOT/src-tauri/target/release/libragent"
DEV_BIN="$ROOT/src-tauri/target/debug/libragent"

if [[ -x "$BIN" ]]; then
  exec "$BIN" --demo --mcp --app-control "$@"
elif [[ -x "$DEV_BIN" ]]; then
  exec "$DEV_BIN" --demo --mcp --app-control "$@"
fi

echo "📦 No built binary; starting via pnpm tauri dev…"
exec pnpm tauri dev -- --demo --mcp --app-control "$@"
