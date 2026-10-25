#!/bin/bash
cd /home/fritzprix/my_works/libr-agent
echo "=== PR #2217 file changes ==="
gh pr diff 2217 --repo fritzprix/libr-agent | grep "^diff --git" | head -20
echo "---"
echo "=== PR #2217: 'any' keyword ==="
gh pr diff 2217 --repo fritzprix/libr-agent | grep -n "any" | grep -v "^[+-]*any\[" | head -10
echo "---"
echo "=== PR #2217: std::fs in async ==="
gh pr diff 2217 --repo fritzprix/libr-agent | grep -n "std::fs\|std::io\|blocking" | head -10
