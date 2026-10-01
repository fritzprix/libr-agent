#!/usr/bin/env bash
# Thin wrapper → bundled demo-play skill hero chrome beat.
# Prefer: @skill:demo-play / src-tauri/bundled_skills/demo-play/
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec bash "${ROOT}/src-tauri/bundled_skills/demo-play/scripts/play_hero_chrome.sh" "$@"
