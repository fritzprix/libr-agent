#!/bin/bash
cd /home/fritzprix/my_works/libr-agent
echo "=== PR #2218 file changes ==="
gh pr diff 2218 --repo fritzprix/libr-agent | grep "^diff --git" | head -20
echo "---"
echo "=== PR #2218: 'any' keyword ==="
gh pr diff 2218 --repo fritzprix/libr-agent | grep -n "any" | grep -v "^[+-]*any\[" | head -10
echo "---"
echo "=== PR #2218: std::fs ==="
gh pr diff 2218 --repo fritzprix/libr-agent | grep -n "std::fs\|std::io\|blocking\|fs::" | head -10
echo "---"
echo "=== PR #2218: Rust type changes ==="
gh pr diff 2218 --repo fritzprix/libr-agent | grep "^+\|^-" | grep -v "^+++\|^---" | grep -i "struct\|enum\|type\|McpTransport\|McpSchema" | head -20
