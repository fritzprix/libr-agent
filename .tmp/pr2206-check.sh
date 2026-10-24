#!/bin/bash
cd /home/fritzprix/my_works/libr-agent
echo "=== PR #2206 file changes ==="
gh pr diff 2206 --repo fritzprix/libr-agent | grep "^diff --git" | head -20
echo "---"
echo "=== PR #2206: 'any' keyword ==="
gh pr diff 2206 --repo fritzprix/libr-agent | grep -n "any" | grep -v "^[+-]*any\[" | head -10
echo "---"
echo "=== PR #2206: std::fs ==="
gh pr diff 2206 --repo fritzprix/libr-agent | grep -n "std::fs\|std::io\|blocking" | head -10
