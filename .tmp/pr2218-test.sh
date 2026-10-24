#!/bin/bash
cd /home/fritzprix/my_works/libr-agent
git fetch origin 2>/dev/null
git checkout -b pr-2218-check origin/fix/mcp-transport-oneof-2198 2>/dev/null
echo "=== Running gemini schema tests ==="
pnpm vitest run src/lib/ai-service/__tests__/gemini.schema-mapper.test.ts --reporter=verbose 2>&1 | tail -20
echo "=== gemini test result: $? ==="
git checkout - 2>/dev/null
git branch -D pr-2218-check 2>/dev/null
