#!/bin/bash
cd /home/fritzprix/my_works/libr-agent
git fetch origin 2>/dev/null

for pr in 2207 2208 2209 2211 2212; do
    echo "=== PR #$pr ==="
    gh pr diff $pr --repo fritzprix/libr-agent 2>/dev/null | head -50
    echo "... ($(gh pr diff $pr --repo fritzprix/libr-agent 2>/dev/null | wc -l) lines total)"
    echo ""
done
