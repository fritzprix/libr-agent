#!/bin/bash
# check-entity-usage.sh
# Detects direct Entity usage and SQL queries for repository pattern migration
# Usage: ./scripts/check-entity-usage.sh

set -e

# ANSI color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
RESET='\033[0m'
BOLD='\033[1m'

# Tables to check with their migration status
declare -A TABLE_PHASE
declare -A TABLE_STATUS

TABLE_PHASE["settings"]="1 (DONE)"
TABLE_PHASE["mcp_server"]="1 (DONE)"
TABLE_PHASE["message_index_meta"]="1 (DONE)"
TABLE_PHASE["message"]="1 (DONE)"
TABLE_PHASE["session"]="1 (DONE)"
TABLE_PHASE["assistant"]="2"
TABLE_PHASE["playbook"]="2"
TABLE_PHASE["knowledge"]="2"
TABLE_PHASE["planning_task"]="2"
TABLE_PHASE["planning_reflection"]="2"

TABLE_STATUS["settings"]="✓"
TABLE_STATUS["mcp_server"]="✓"
TABLE_STATUS["message_index_meta"]="✓"
TABLE_STATUS["message"]="✓"
TABLE_STATUS["session"]="✓"
TABLE_STATUS["assistant"]="○"
TABLE_STATUS["playbook"]="○"
TABLE_STATUS["knowledge"]="○"
TABLE_STATUS["planning_task"]="○"
TABLE_STATUS["planning_reflection"]="○"

TABLES=("settings" "mcp_server" "message_index_meta" "message" "session" "assistant" "playbook" "knowledge" "planning_task" "planning_reflection")

# Directories to search
SEARCH_DIRS=("src-tauri/src")

# Directories to exclude
EXCLUDE_DIRS=(
    "target"
    "node_modules"
    ".git"
    "dist"
    "migration"
)

echo -e "${BOLD}${CYAN}╔════════════════════════════════════════════════════════════╗${RESET}"
echo -e "${BOLD}${CYAN}║  Entity & SQL Query Usage Detector (Repository Pattern)  ║${RESET}"
echo -e "${BOLD}${CYAN}╚════════════════════════════════════════════════════════════╝${RESET}\n"

TOTAL_ISSUES=0
declare -A ISSUES_BY_TABLE

# Build find exclude pattern
FIND_EXCLUDE=""
for exclude_dir in "${EXCLUDE_DIRS[@]}"; do
    FIND_EXCLUDE="$FIND_EXCLUDE -not -path '*/$exclude_dir/*'"
done

for table in "${TABLES[@]}"; do
    phase="${TABLE_PHASE[$table]}"
    status="${TABLE_STATUS[$table]}"
    
    echo -e "${BOLD}${BLUE}━━━ $table (Phase $phase) $status ${RESET}"
    
    ISSUE_COUNT=0
    
    # Pattern 1: Direct Entity usage
    ENTITY_PATTERN="${table}::Entity"
    
    # Search for Entity usage in Rust files
    for search_dir in "${SEARCH_DIRS[@]}"; do
        if [ -d "$search_dir" ]; then
            # Find Rust files excluding certain patterns
            while IFS= read -r file; do
                # Skip entity definition files
                if [[ "$file" == *"/entity.rs" ]] || [[ "$file" == *"/entities.rs" ]] || [[ "$file" == *"/entities/"*".rs" ]]; then
                    continue
                fi
                
                # Check for Entity usage
                if grep -q "$ENTITY_PATTERN" "$file"; then
                    while IFS= read -r line_info; do
                        line_num=$(echo "$line_info" | cut -d':' -f1)
                        line_content=$(echo "$line_info" | cut -d':' -f2-)
                        echo -e "  ${RED}●${RESET} ${BOLD}$file:$line_num${RESET}"
                        echo -e "    ${RED}[Entity]${RESET} $(echo "$line_content" | xargs)"
                        ((ISSUE_COUNT++))
                    done < <(grep -n "$ENTITY_PATTERN" "$file" || true)
                fi
                
                # Check for Entity::find operations
                for op in "find" "find_by_id" "insert" "update" "delete"; do
                    FIND_PATTERN="${table}::Entity::$op"
                    if grep -q "$FIND_PATTERN" "$file"; then
                        while IFS= read -r line_info; do
                            line_num=$(echo "$line_info" | cut -d':' -f1)
                            line_content=$(echo "$line_info" | cut -d':' -f2-)
                            echo -e "  ${YELLOW}●${RESET} ${BOLD}$file:$line_num${RESET}"
                            echo -e "    ${YELLOW}[Entity::$op]${RESET} $(echo "$line_content" | xargs)"
                            ((ISSUE_COUNT++))
                        done < <(grep -n "$FIND_PATTERN" "$file" || true)
                    fi
                done
                
                # Check for SQL queries
                for sql_op in "SELECT.*FROM[[:space:]]\+$table" "INSERT[[:space:]]\+INTO[[:space:]]\+$table" "UPDATE[[:space:]]\+$table" "DELETE[[:space:]]\+FROM[[:space:]]\+$table"; do
                    if grep -qi "$sql_op" "$file"; then
                        while IFS= read -r line_info; do
                            line_num=$(echo "$line_info" | cut -d':' -f1)
                            line_content=$(echo "$line_info" | cut -d':' -f2-)
                            echo -e "  ${MAGENTA}●${RESET} ${BOLD}$file:$line_num${RESET}"
                            echo -e "    ${MAGENTA}[SQL Query]${RESET} $(echo "$line_content" | xargs)"
                            ((ISSUE_COUNT++))
                        done < <(grep -ni "$sql_op" "$file" || true)
                    fi
                done
                
            done < <(eval "find $search_dir -type f -name '*.rs' $FIND_EXCLUDE")
        fi
    done
    
    if [ $ISSUE_COUNT -eq 0 ]; then
        echo -e "  ${GREEN}✓ No direct Entity or SQL usage found${RESET}"
    fi
    
    ISSUES_BY_TABLE[$table]=$ISSUE_COUNT
    ((TOTAL_ISSUES += ISSUE_COUNT))
    
    echo ""
done

# Summary
echo -e "${BOLD}${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
echo -e "${BOLD}${CYAN}SUMMARY${RESET}\n"

for table in "${TABLES[@]}"; do
    count="${ISSUES_BY_TABLE[$table]}"
    phase="${TABLE_PHASE[$table]}"
    
    if [ "$count" -eq 0 ]; then
        echo -e "  ${GREEN}✓${RESET} ${BOLD}$table${RESET} (Phase $phase): ${GREEN}$count issues${RESET}"
    else
        echo -e "  ${RED}✗${RESET} ${BOLD}$table${RESET} (Phase $phase): ${RED}$count issues${RESET}"
    fi
done

echo ""
echo -e "${BOLD}Total issues found: ${RESET}"
if [ $TOTAL_ISSUES -eq 0 ]; then
    echo -e "${GREEN}${BOLD}$TOTAL_ISSUES${RESET} ${GREEN}(All tables migrated to repository pattern!)${RESET}"
elif [ $TOTAL_ISSUES -lt 10 ]; then
    echo -e "${YELLOW}${BOLD}$TOTAL_ISSUES${RESET} ${YELLOW}(Almost there!)${RESET}"
else
    echo -e "${RED}${BOLD}$TOTAL_ISSUES${RESET} ${RED}(Migration needed)${RESET}"
fi

echo ""
echo -e "${BOLD}${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"

exit $([ $TOTAL_ISSUES -eq 0 ] && echo 0 || echo 1)
