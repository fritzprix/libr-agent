#!/bin/bash
set -euo pipefail
{
  echo "# Summary"
  for f in /app/docs/doc*.md; do
    name=$(basename "$f" .md)
    echo "- ${name}: $(sed -n 3p "$f")"
  done
} > /app/summary.md
