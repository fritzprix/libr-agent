#!/bin/bash
cd /home/fritzprix/my_works/libr-agent
gh pr diff 2207 --repo fritzprix/libr-agent | grep "^diff --git"
echo "---"
gh pr diff 2208 --repo fritzprix/libr-agent | grep "^diff --git"
echo "---"
gh pr diff 2209 --repo fritzprix/libr-agent | grep "^diff --git"
echo "---"
gh pr diff 2211 --repo fritzprix/libr-agent | grep "^diff --git"
echo "---"
gh pr diff 2212 --repo fritzprix/libr-agent | grep "^diff --git"
