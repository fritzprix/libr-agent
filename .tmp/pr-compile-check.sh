#!/bin/bash
cd /home/fritzprix/my_works/libr-agent

# PR 2207: browser/content.rs split
echo "=== PR #2207 compilation check ==="
git stash 2>/dev/null
git fetch origin 2>/dev/null
git checkout -B pr-2207-check origin/pr-2207 2>/dev/null
pnpm rust:check 2>&1 | tail -20
echo "=== PR #2207 result: $? ==="
git checkout - 2>/dev/null

echo ""
echo "=== PR #2208 compilation check ==="
git checkout -B pr-2208-check origin/pr-2208 2>/dev/null
pnpm rust:check 2>&1 | tail -20
echo "=== PR #2208 result: $? ==="
git checkout - 2>/dev/null

echo ""
echo "=== PR #2209 compilation check ==="
git checkout -B pr-2209-check origin/pr-2209 2>/dev/null
pnpm rust:check 2>&1 | tail -20
echo "=== PR #2209 result: $? ==="
git checkout - 2>/dev/null

echo ""
echo "=== PR #2211 compilation check ==="
git checkout -B pr-2211-check origin/pr-2211 2>/dev/null
pnpm rust:check 2>&1 | tail -20
echo "=== PR #2211 result: $? ==="
git checkout - 2>/dev/null
