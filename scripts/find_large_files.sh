#!/bin/bash

# Script to find files with more than 500 lines in the codebase
# Excludes node_modules, dist, and other generated/dependency directories

# ANSI color codes
if [ -t 1 ] && command -v tput >/dev/null 2>&1 && [ "$(tput colors)" -ge 8 ]; then
  RED='\033[0;31m'
  YELLOW='\033[1;33m'
  GREEN='\033[0;32m'
  BLUE='\033[0;34m'
  NC='\033[0m' # No Color
else
  RED=''
  YELLOW=''
  GREEN=''
  BLUE=''
  NC=''
fi

echo "🔍 Finding files with more than 500 lines..."
echo "=========================================="

# Find and count lines for source files (excluding total lines)
find ./src ./src-tauri/src ./docs -type f \
  \( -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.jsx" -o -name "*.rs" -o -name "*.md" \) \
  ! -path "*/node_modules/*" \
  ! -path "*/dist/*" \
  ! -path "*/target/*" \
  ! -path "*/build/*" \
  ! -path "*/.next/*" \
  ! -path "*/.nuxt/*" \
  ! -path "*/coverage/*" \
  -exec wc -l {} + 2>/dev/null | \
  awk '$1 > 500 && $2 != "total" { printf "%6d lines: %s\n", $1, $2 }' | \
  sort -nr | \
  while read -r line; do
    lines=$(echo "$line" | awk '{print $1}')
    filename=$(echo "$line" | cut -d' ' -f3-)

    if [ "$lines" -ge 1000 ]; then
      echo -e "${RED}🔴 ${line}${NC}"
    elif [ "$lines" -ge 800 ]; then
      echo -e "${YELLOW}🟡 ${line}${NC}"
    else
      echo -e "${GREEN}🟢 ${line}${NC}"
    fi
  done

echo ""
echo "=========================================="
echo "✅ Scan complete!"

# Calculate and display total lines separately with emphasis
TOTAL_LINES=$(find ./src ./src-tauri/src ./docs -type f \
  \( -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.jsx" -o -name "*.rs" -o -name "*.md" \) \
  ! -path "*/node_modules/*" \
  ! -path "*/dist/*" \
  ! -path "*/target/*" \
  ! -path "*/build/*" \
  ! -path "*/.next/*" \
  ! -path "*/.nuxt/*" \
  ! -path "*/coverage/*" \
  -exec wc -l {} + 2>/dev/null | \
  awk '{sum += $1} END {print sum}')

echo ""
echo -e "${BLUE}📊 📈 TOTAL LINES IN CODEBASE: ${TOTAL_LINES}${NC}"

# Show total count of large files
TOTAL_COUNT=$(find ./src ./src-tauri/src ./docs -type f \
  \( -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.jsx" -o -name "*.rs" -o -name "*.md" \) \
  ! -path "*/node_modules/*" \
  ! -path "*/dist/*" \
  ! -path "*/target/*" \
  ! -path "*/build/*" \
  ! -path "*/.next/*" \
  ! -path "*/.nuxt/*" \
  ! -path "*/coverage/*" \
  -exec wc -l {} + 2>/dev/null | \
  awk '$1 > 500 && $2 != "total"' | wc -l)

echo "📊 Total files with >500 lines: $TOTAL_COUNT"

# Show color legend
echo ""
echo "🎨 Color Legend:"
echo -e "  ${RED}🔴 1000+ lines${NC}"
echo -e "  ${YELLOW}🟡 800-999 lines${NC}"
echo -e "  ${GREEN}🟢 500-799 lines${NC}"
echo -e "  ${BLUE}📈 Total lines (summary)${NC}"
