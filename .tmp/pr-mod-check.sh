#!/bin/bash
cd /home/fritzprix/my_works/libr-agent

# Check mod.rs files for proper re-exports
echo "=== PR #2207 content/mod.rs ==="
gh pr diff 2207 --repo fritzprix/libr-agent | sed -n '/diff --git.*content\/mod\.rs/,/^diff --git/p' | head -50

echo ""
echo "=== PR #2208 configs/mod.rs ==="
gh pr diff 2208 --repo fritzprix/libr-agent | sed -n '/diff --git.*configs\/mod\.rs/,/^diff --git/p' | head -50

echo ""
echo "=== PR #2209 workspace_runtime_manager/mod.rs ==="
gh pr diff 2209 --repo fritzprix/libr-agent | sed -n '/diff --git.*workspace_runtime_manager\/mod\.rs/,/^diff --git/p' | head -50

echo ""
echo "=== PR #2211 sessions/mod.rs ==="
gh pr diff 2211 --repo fritzprix/libr-agent | sed -n '/diff --git.*sessions\/mod\.rs/,/^diff --git/p' | head -50
