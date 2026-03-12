#!/usr/bin/env bash
# Branch cleanup script — safe deletions confirmed 2026-03-12
# See docs/branch-cleanup-2026-03.md for full analysis.
#
# Usage: bash scripts/cleanup-branches.sh
# Requires: gh CLI authenticated with repo write access

set -euo pipefail

REPO="fritzprix/libr-agent"

# Verify gh CLI is available and authenticated
if ! gh auth status &>/dev/null; then
  echo "Error: gh CLI is not authenticated. Run 'gh auth login' first." >&2
  exit 1
fi

DELETED_COUNT=0

delete_branch() {
  local branch="$1"
  local reason="$2"
  # The GitHub refs API accepts slashes in the branch name directly as path components
  if gh api -X DELETE "repos/$REPO/git/refs/heads/$branch" 2>/dev/null; then
    echo "  ✓ deleted: $branch  ($reason)"
    DELETED_COUNT=$((DELETED_COUNT + 1))
  else
    echo "  ✗ skipped: $branch  (already gone or error)"
  fi
}

echo "=================================================="
echo " Branch cleanup: $REPO"
echo " Date: $(date -u '+%Y-%m-%d %H:%M UTC')"
echo "=================================================="

# -------------------------------------------------------
# GROUP 1: Merged into main (git ancestry confirmed)
# -------------------------------------------------------
echo ""
echo "--- Group 1: Merged into main (20 branches) ---"

delete_branch "atlas-cross-platform-path-fixes-5379120781736563232" "merged into main"
delete_branch "copilot-worktree-2026-02-08T13-15-43"               "merged into main (Copilot worktree)"
delete_branch "dev/0.1.1"                                           "merged into main (old release cycle)"
delete_branch "dev/0.2.0"                                           "merged into main (old release cycle)"
delete_branch "fix/batch-tool-calls"                                "merged into main"
delete_branch "fix/fluid-playbook-dnd-loading-states-13403884465222317889" "merged into main"
delete_branch "fluid-ui-loading-states-1022241757438928708"         "merged into main"
delete_branch "fractal-refactor-llm-response-17387151373517687981"  "merged into main"
delete_branch "hermes/logger-ipc-error-handling-6633559961318240156" "merged into main"
delete_branch "jules-docs-sync-1106456784670056104"                  "merged into main"
delete_branch "nexus-decouple-session-cleanup-8691370924490767940"   "merged into main"
delete_branch "nexus/extract-agent-session-manager-deletion-10334942398720138240" "merged into main"
delete_branch "palette-api-key-toggle-11503987520087105893"          "merged into main"
delete_branch "refactor/stdio-manager-split-7312783446801029112"     "merged into main"
delete_branch "release/0.3.1"                                        "merged into main (old release)"
delete_branch "resolve-content-store-validation-2055047095501464178" "merged into main"
delete_branch "sentinel-fix-isolation-env-clear-18348970828199946881" "merged into main"
delete_branch "sentinel-fix-mcp-env-leak-8266293857286341031"        "merged into main"
delete_branch "weaver-eradicate-use-effect-sync-3256216506919389302" "merged into main"
delete_branch "weaver/remove-derived-state-anti-pattern-14212238653961176358" "merged into main"

# -------------------------------------------------------
# GROUP 2: Squash-merged into dev/0.5.x (PR confirmed)
# -------------------------------------------------------
echo ""
echo "--- Group 2: Merged into dev/0.5.x via squash (3 branches) ---"

delete_branch "fluid-session-history-panel-performance-17819986540411602727" "PR #807 merged into dev/0.5.x"
delete_branch "optimize/prompt-assembly"                             "PR #808 merged into dev/0.5.x"
delete_branch "sonar/backend-wrapper-tests-13727110902053388345"     "PR #788 merged into dev/0.5.x"

# -------------------------------------------------------
# GROUP 3: Stale / abandoned (no PR, far behind main)
# -------------------------------------------------------
echo ""
echo "--- Group 3: Stale / abandoned (7 branches) ---"

delete_branch "dbg/message-streaming"                               "debug branch, no PR, 1453 behind main"
delete_branch "dev/0.3.x"                                           "old dev cycle, 1729 behind main"
delete_branch "dev/0.4.0"                                           "old dev cycle, 977 behind main"
delete_branch "dev/0.4.0-bot"                                       "old bot branch, 1261 behind main"
delete_branch "feat/docker-sandbox"                                  "abandoned feature, no PR, 1337 behind"
delete_branch "feat/headless-mode-v0.5.0"                           "abandoned feature, no PR, 1296 behind"
delete_branch "feature/interactive-shell-migration-and-test"         "abandoned, no PR, 1633 behind main"

echo ""
echo "=================================================="
echo " Done. $DELETED_COUNT branches deleted."
echo " See docs/branch-cleanup-2026-03.md for details"
echo " on the 9 branches that need manual review."
echo "=================================================="
