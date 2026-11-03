#!/bin/sh
# Linux/macOS runner for the docs skill tests that are not Windows-specific.
# Windows runs test_convert.ps1, which calls the same test_media.py.
set -eu

script_dir=$(cd "$(dirname "$0")" && pwd)

echo "=== Docs skill doctor (documented Linux/macOS command) ==="
doctor_output=$(uv run --no-project --python 3.12 "$script_dir/convert.py" --doctor)
echo "$doctor_output"
venv_python=$(printf '%s\n' "$doctor_output" | sed -n 's/^VenvPython: //p')
if [ -z "$venv_python" ] || [ ! -x "$venv_python" ]; then
  echo "FAILED: doctor did not report a usable VenvPython" >&2
  exit 1
fi

work_dir=$(mktemp -d)
trap 'rm -rf "$work_dir"' EXIT

echo "=== Media test (#2530) ==="
"$venv_python" "$script_dir/test_media.py" "$work_dir" "$script_dir/convert.py"
echo "=== All docs skill tests passed successfully! ==="
