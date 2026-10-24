#!/bin/bash
cd /home/fritzprix/my_works/libr-agent

# Run tests for each PR branch
# PR 2212 is the only test-related PR, so let's run its tests
echo "=== PR #2212: Run Vitest for AgentChatContext tests ==="
git fetch origin 2>/dev/null
git checkout -B pr-2212-test origin/pr-2212 2>/dev/null
pnpm vitest run src/context/__tests__/ 2>&1 | tail -30
echo "=== PR #2212 test result: $? ==="
git checkout - 2>/dev/null
