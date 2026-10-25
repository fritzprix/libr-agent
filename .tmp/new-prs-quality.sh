#!/bin/bash
cd /home/fritzprix/my_works/libr-agent

echo "=== PR #2213: 'any' keyword search ==="
gh pr diff 2213 --repo fritzprix/libr-agent | grep -n "any" | grep -v "^[+-]*any\[" | head -10

echo ""
echo "=== PR #2215: 'any' keyword search ==="
gh pr diff 2215 --repo fritzprix/libr-agent | grep -n "any" | grep -v "^[+-]*any\[" | head -10

echo ""
echo "=== PR #2213 mod.rs re-exports ==="
gh pr diff 2213 --repo fritzprix/libr-agent | sed -n '/diff --git.*tests\/mod\.rs/,/^diff --git/p' | head -40

echo ""
echo "=== PR #2215 file sizes ==="
gh pr diff 2215 --repo fritzprix/libr-agent | grep "^diff --git"
