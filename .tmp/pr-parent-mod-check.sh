#!/bin/bash
cd /home/fritzprix/my_works/libr-agent

# Check if parent modules properly reference the new module directories
echo "=== PR #2207: parent module change (browser/mod.rs or browser.rs) ==="
gh pr diff 2207 --repo fritzprix/libr-agent | grep -A5 -B5 "mod content" | head -20

echo ""
echo "=== PR #2208: parent module change ==="
gh pr diff 2208 --repo fritzprix/libr-agent | grep -A5 -B5 "mod configs" | head -20

echo ""
echo "=== PR #2209: parent module change (services/mod.rs or similar) ==="
gh pr diff 2209 --repo fritzprix/libr-agent | grep -A5 -B5 "workspace_runtime_manager" | head -30

echo ""
echo "=== PR #2211: parent module change ==="
gh pr diff 2211 --repo fritzprix/libr-agent | grep -A5 -B5 "mod sessions" | head -20
