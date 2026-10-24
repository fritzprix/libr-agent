#!/bin/bash
cd /home/fritzprix/my_works/libr-agent
echo "=== PR #2213 file changes ==="
gh pr diff 2213 --repo fritzprix/libr-agent | grep "^diff --git" | head -20
echo "---"
echo "=== PR #2215 file changes ==="
gh pr diff 2215 --repo fritzprix/libr-agent | grep "^diff --git" | head -20
