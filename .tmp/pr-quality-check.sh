#!/bin/bash
cd /home/fritzprix/my_works/libr-agent

# Check for 'any' usage in new/modified code
for pr in 2207 2208 2209 2211 2212; do
    echo "=== PR #$pr: 'any' keyword search ==="
    gh pr diff $pr --repo fritzprix/libr-agent | grep -n "any" | grep -v "^[+-]*any\[" | head -10
    echo ""
done

# Check for test file additions/deletions
echo "=== Test file changes ==="
for pr in 2207 2208 2209 2211 2212; do
    echo "PR #$pr:"
    gh pr diff $pr --repo fritzprix/libr-agent | grep "^diff --git.*test" | head -5
    echo ""
done
