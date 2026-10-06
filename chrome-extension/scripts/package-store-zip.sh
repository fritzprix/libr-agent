#!/usr/bin/env bash
# Build a Chrome Web Store ZIP with manifest.json at the archive root.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO_ROOT="$(cd "$ROOT/.." && pwd)"
VERSION="$(python3 - <<PY
import json
from pathlib import Path
print(json.loads(Path("${ROOT}/manifest.json").read_text())["version"])
PY
)"
OUT_DIR="${REPO_ROOT}/dist/chrome-extension"
ZIP_NAME="libragent-browser-bridge-${VERSION}.zip"
ZIP_PATH="${OUT_DIR}/${ZIP_NAME}"

mkdir -p "${OUT_DIR}"
rm -f "${ZIP_PATH}"

# Only ship runtime files (no STORE.md / README / pack scripts).
cd "${ROOT}"
zip -r "${ZIP_PATH}" \
  manifest.json \
  service_worker.js \
  popup.html \
  popup.js \
  options.html \
  options.js \
  icons/icon-16.png \
  icons/icon-48.png \
  icons/icon-128.png \
  -x '*.DS_Store' \
  >/dev/null

echo "Wrote ${ZIP_PATH}"
unzip -l "${ZIP_PATH}"
