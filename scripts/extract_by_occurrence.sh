#!/usr/bin/env bash
# Extract +/-100 lines around each [WARN] occurrence and write to app_1.log, app_2.log, ...
set -euo pipefail

infile="${1:-app.log}"
if [ ! -f "$infile" ]; then
  echo "Input file not found: $infile" >&2
  exit 2
fi

grep -n '\[WARN\]' "$infile" | cut -d: -f1 > /tmp/_warn_lines_$$ || true
if [ ! -s /tmp/_warn_lines_$$ ]; then
  echo "No [WARN] lines found in $infile"
  rm -f /tmp/_warn_lines_$$
  exit 0
fi

count=1
while IFS= read -r lineno; do
  start=$((lineno - 100))
  [ "$start" -lt 1 ] && start=1
  end=$((lineno + 100))

  outfile="app_${count}.log"
  sed -n "${start},${end}p" "$infile" > "$outfile"
  echo "wrote $outfile (lines ${start}-${end})"
  count=$((count + 1))
done < /tmp/_warn_lines_$$

rm -f /tmp/_warn_lines_$$
exit 0
