#!/bin/bash
cd /home/fritzprix/my_works/libr-agent
git fetch origin 2>/dev/null
git checkout -b pr-2206-test origin/refactor/session-history-panel-2192 2>/dev/null
echo "=== Running tests ==="
pnpm vitest run src/features/agent/components/__tests__ --reporter=verbose 2>&1 | tail -30
echo "=== PR #2206 test result: $? ==="
git checkout - 2>/dev/null
git branch -D pr-2206-test 2>/dev/null
