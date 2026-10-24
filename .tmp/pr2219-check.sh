#!/bin/bash
cd /home/fritzprix/my_works/libr-agent
echo "=== PR #2219 file changes ==="
gh pr diff 2219 --repo fritzprix/libr-agent | grep "^diff --git" | head -20
echo "---"
echo "=== PR #2219: 'any' keyword ==="
gh pr diff 2219 --repo fritzprix/libr-agent | grep -n "any" | grep -v "^[+-]*any\[" | head -10
echo "---"
echo "=== PR #2219: std::fs ==="
gh pr diff 2219 --repo fritzprix/libr-agent | grep -n "std::fs\|std::io\|blocking\|fs::" | head -10
