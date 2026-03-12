# Branch Cleanup Analysis — March 2026

**Date:** 2026-03-12  
**Analyst:** Copilot Agent  
**Base branch:** `main` (`b63c4159`)  
**Total remote branches reviewed:** 46

---

## Summary

| Category | Count |
|---|---|
| ✅ Safe to delete — already merged | 23 |
| 🗑️ Safe to delete — stale/abandoned | 7 |
| 🔒 Keep — active open PR | 7 |
| ⚠️ Review needed — recent, no open PR | 9 |

---

## ✅ Safe to Delete — Already Merged

These branches are fully merged into `main` (git ancestry confirmed) or into `dev/0.5.x` (PR `merged_at` confirmed).

### Merged into `main`

| Branch | Last Commit | Notes |
|---|---|---|
| `atlas-cross-platform-path-fixes-5379120781736563232` | 2026-02-28 | Merged via dev/0.5.x |
| `copilot-worktree-2026-02-08T13-15-43` | 2026-02-08 | Copilot worktree (temp) |
| `dev/0.1.1` | 2025-10-19 | Old release prep branch |
| `dev/0.2.0` | 2025-11-18 | Old release prep branch |
| `fix/batch-tool-calls` | 2026-01-28 | Bug fix, merged |
| `fix/fluid-playbook-dnd-loading-states-13403884465222317889` | 2026-02-28 | Bug fix, merged |
| `fluid-ui-loading-states-1022241757438928708` | 2026-03-01 | UI enhancement, merged |
| `fractal-refactor-llm-response-17387151373517687981` | 2026-02-23 | Refactor, merged |
| `hermes/logger-ipc-error-handling-6633559961318240156` | 2026-03-05 | IPC fix, merged |
| `jules-docs-sync-1106456784670056104` | 2026-03-06 | Docs sync, merged |
| `nexus-decouple-session-cleanup-8691370924490767940` | 2026-03-05 | Refactor, merged |
| `nexus/extract-agent-session-manager-deletion-10334942398720138240` | 2026-03-09 | Refactor, merged |
| `palette-api-key-toggle-11503987520087105893` | 2026-02-27 | UI feature, merged |
| `refactor/stdio-manager-split-7312783446801029112` | 2026-02-28 | Refactor, merged |
| `release/0.3.1` | 2025-11-18 | Old release branch |
| `resolve-content-store-validation-2055047095501464178` | 2026-02-21 | Fix, merged |
| `sentinel-fix-isolation-env-clear-18348970828199946881` | 2026-03-07 | Security fix, merged |
| `sentinel-fix-mcp-env-leak-8266293857286341031` | 2026-03-02 | Security fix, merged |
| `weaver-eradicate-use-effect-sync-3256216506919389302` | 2026-02-28 | Refactor, merged |
| `weaver/remove-derived-state-anti-pattern-14212238653961176358` | 2026-03-06 | Refactor, merged |

### Merged into `dev/0.5.x` (squash-merged, PR confirmed)

| Branch | PR | Merged At | Notes |
|---|---|---|---|
| `fluid-session-history-panel-performance-17819986540411602727` | #807 | 2026-03-11 | Confirmed squash-merged into dev/0.5.x |
| `optimize/prompt-assembly` | #808 | 2026-03-11 | Confirmed squash-merged into dev/0.5.x |
| `sonar/backend-wrapper-tests-13727110902053388345` | #788 | 2026-03-10 | Confirmed squash-merged into dev/0.5.x |

---

## 🗑️ Safe to Delete — Stale / Abandoned

These branches have no open PR, are significantly behind `main` (>175 commits), and have not seen activity in weeks or months. They were likely superseded by newer work.

| Branch | Last Commit | Behind main | Notes |
|---|---|---|---|
| `dbg/message-streaming` | 2026-01-26 | 1453 | Debug branch, never had PR |
| `dev/0.3.x` | 2025-12-28 | 1729 | Old dev cycle, superseded |
| `dev/0.4.0` | 2026-02-20 | 977 | Old dev cycle, superseded |
| `dev/0.4.0-bot` | 2026-02-06 | 1261 | Old bot dev branch, superseded |
| `feat/docker-sandbox` | 2026-01-31 | 1337 | Feature not pursued |
| `feat/headless-mode-v0.5.0` | 2026-02-03 | 1296 | Feature not pursued |
| `feature/interactive-shell-migration-and-test` | 2026-01-13 | 1633 | Old feature branch |

---

## 🔒 Keep — Active Open PRs

Do **not** delete these branches; they have open pull requests under active review.

| Branch | PR | Status |
|---|---|---|
| `dev/0.5.x` | #805 → `main` | Open (release in progress) |
| `fluid-fix-alert-blocking-3754786199166178366` | #817 → `dev/0.5.x` | Open |
| `fractal-refactor-terminal-handlers-1076172785191062383` | #812 → `dev/0.5.x` | Open |
| `refactor/rust-compact-orchestration` | #818 → `dev/0.5.x` | Open |
| `scribe-fix-rustdoc-warnings-4909577484865410700` | #813 → `dev/0.5.x` | Open |
| `sentinel-fix-bootstrap-env-leak-2112041576729098921` | #814 → `dev/0.5.x` | Open |
| `weaver/refactor-assistant-list-skills-editor-7954413265639032412` | #816 → `dev/0.5.x` | Open |

---

## ⚠️ Review Needed — Recent but No Open PR

These branches were recently updated but have no corresponding open PR. Some had PRs that were closed without merging. Owner should confirm intent before deletion.

| Branch | Last Commit | Ahead/Behind | Notes |
|---|---|---|---|
| `atlas-terminal-pathing-15123600821244322832` | 2026-03-04 | +3 / -337 | No PR; partial cross-platform pathing work |
| `fluid-ux-playbook-chat-9664272304061576978` | 2026-02-27 | +2 / -505 | No open PR; playbook chat UX work |
| `hermes/ipc-logger-boundary-10504298861725865818` | 2026-03-03 | +3 / -335 | No PR; IPC logger boundary work |
| `hermes/ipc-settings-optimization-18327452379354135015` | 2026-02-25 | +1 / -657 | No PR; IPC settings work |
| `nexus-fat-handler-extraction-3878661276462811372` | 2026-03-07 | +1 / -175 | No PR; handler extraction work |
| `refactor/weaver-use-session-tools-319563503510520546` | 2026-03-02 | +1 / -371 | No PR; Weaver session tools refactor |
| `resolve-mcp-integration-tests-6672073701306610574` | 2026-02-23 | +1 / -743 | No PR; MCP integration test fix |
| `rosetta-mcp-server-tools-modal-14454677898218055760` | 2026-03-11 | +3 / -33 | PR #804 closed without merge 2026-03-11; may be superseded |
| `rosetta-scheduled-tasks-9828945659281041981` | 2026-03-05 | +1 / -240 | No PR; scheduled tasks localization |

---

## Cleanup Script

Run `scripts/cleanup-branches.sh` to delete all confirmed-safe branches (Groups 1–3 above).  
Review the **⚠️ Review Needed** section before deciding whether to delete those 9 branches manually.

```bash
# Requires: gh CLI authenticated with repo write access
bash scripts/cleanup-branches.sh
```

The script deletes 30 branches in total (20 merged to `main`, 3 merged to `dev/0.5.x`, 7 stale/abandoned) and prints a per-branch result with the reason for deletion.
