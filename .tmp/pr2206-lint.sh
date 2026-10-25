#!/bin/bash
cd /home/fritzprix/my_works/libr-agent
echo "=== PR #2206 lint check ==="
git fetch origin 2>/dev/null
git checkout -b pr-2206-check origin/refactor/session-history-panel-2192 2>/dev/null
pnpm lint 2>&1 | tail -10
echo "=== PR #2206 lint result: $? ==="
git checkout - 2>/dev/null
git branch -D pr-2206-check 2>/dev/null
