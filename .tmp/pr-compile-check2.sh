#!/bin/bash
cd /home/fritzprix/my_works/libr-agent

# PR 2212: TypeScript test split
echo "=== PR #2212 compilation/lint check ==="
git fetch origin 2>/dev/null
git checkout -B pr-2212-check origin/pr-2212 2>/dev/null
pnpm lint 2>&1 | tail -20
echo "=== PR #2212 result: $? ==="
git checkout - 2>/dev/null
