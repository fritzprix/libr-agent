#!/bin/bash
cd /home/fritzprix/my_works/libr-agent

echo "=== 병합됨 (dev/* 제외, 삭제 가능) ==="
git branch -r --merged origin/dev/0.9.x 2>/dev/null | grep -v "origin/dev/" | grep -v "origin/main" | wc -l

echo ""
echo "=== 미병합 (dev/* 제외) ==="
git branch -r --no-merged origin/dev/0.9.x 2>/dev/null | grep -v "origin/dev/" | wc -l

echo ""
echo "--- 미병합 카테고리 (dev 제외) ---"
git branch -r --no-merged origin/dev/0.9.x 2>/dev/null | grep -v "origin/dev/" | sed 's|origin/||' | cut -d/ -f1 | sort | uniq -c | sort -rn
