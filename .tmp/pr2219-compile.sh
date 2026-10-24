#!/bin/bash
cd /home/fritzprix/my_works/libr-agent
echo "=== PR #2219 compilation check ==="
git fetch origin 2>/dev/null
git checkout -b pr-2219-check origin/feat/list-servers-catalog-2199 2>/dev/null
pnpm rust:check 2>&1 | tail -10
echo "=== PR #2219 rust result: $? ==="
pnpm lint 2>&1 | tail -10
echo "=== PR #2219 lint result: $? ==="
git checkout - 2>/dev/null
git branch -D pr-2219-check 2>/dev/null
