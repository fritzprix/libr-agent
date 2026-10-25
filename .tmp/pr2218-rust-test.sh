#!/bin/bash
cd /home/fritzprix/my_works/libr-agent
git fetch origin 2>/dev/null
git checkout -b pr-2218-check origin/fix/mcp-transport-oneof-2198 2>/dev/null
echo "=== Running builtin_service_registry_tests ==="
pnpm rust:test --test builtin_service_registry 2>&1 | tail -30
echo "=== Rust test result: $? ==="
git checkout - 2>/dev/null
git branch -D pr-2218-check 2>/dev/null
