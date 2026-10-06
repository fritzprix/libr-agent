#!/usr/bin/env bash
# Poll Session API until terminal status. Wire values are lowercase
# (serde rename_all = "lowercase"): idle|busy|paused|error|queued|provisioning
set -euo pipefail

SID="${1:-}"
BASE="${2:-}"
MAX_POLLS="${3:-40}"

if [[ -z "$SID" ]]; then
  echo "usage: $0 <sessionId> [baseUrl] [maxPolls]" >&2
  exit 2
fi

if [[ -z "$BASE" ]]; then
  PORT="$(cat "${HOME}/.libragent/http_port" 2>/dev/null || echo 3030)"
  BASE="http://127.0.0.1:${PORT}"
fi

normalize() {
  # trim + lowercase — never match PascalCase wire values
  printf '%s' "$1" | tr '[:upper:]' '[:lower:]' | tr -d '[:space:]'
}

for ((i = 1; i <= MAX_POLLS; i++)); do
  raw="$(curl -sS --max-time 10 "${BASE}/api/sessions/${SID}")"
  status="$(printf '%s' "$raw" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("status",""))')"
  status="$(normalize "$status")"
  echo "poll=${i} status=${status}"

  case "$status" in
    idle|error|paused)
      printf '%s\n' "$raw"
      exit 0
      ;;
    busy|provisioning|queued)
      ;;
    *)
      echo "unknown status '${status}' — treating as non-terminal" >&2
      ;;
  esac

  if ((i < 5)); then
    sleep 2
  elif ((i < 15)); then
    sleep 4
  else
    sleep 6
  fi
done

echo "timeout: session ${SID} still non-terminal after ${MAX_POLLS} polls" >&2
exit 1
