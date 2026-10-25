#!/bin/bash
cd /home/fritzprix/my_works/libr-agent

echo "=== PR #2213 compilation check ==="
git fetch origin 2>/dev/null
git checkout -B pr-2213-check origin/pr-2213 2>/dev/null
pnpm rust:check 2>&1 | tail -10
echo "=== PR #2213 result: $? ==="
git checkout - 2>/dev/null

echo ""
echo "=== PR #2215 lint check ==="
git checkout -B pr-2215-check origin/pr-2215 2>/dev/null
pnpm lint 2>&1 | tail -10
echo "=== PR #2215 result: $? ==="
git checkout - 2>/dev/null
