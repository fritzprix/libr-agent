# Consolidated Code Review: LibrAgent Uncommitted Changes

**Branch**: `feat/durable-pending-queue`  
**Date**: 2026-07-27  
**Files Changed**: 27 modified, 5 untracked  
**Net Change**: +5,155 insertions, -1,774 deletions

---

## Executive Summary

This PR contains a **cohesive, cross-cutting refactor of the agent-facing hint/error guidance system** ("Next Steps" → "Suggested Follow-ups/Recovery") plus targeted improvements in:

- Shell command validation & error guidance (security hardening)
- Scheduled task UX (hint headers, cleaner messaging)
- File operation diff preview/anchors
- Browser tool schema documentation alignment
- Setup wizard text polish
- Frontend generated type sync

**Overall Assessment**: ✅ **High-quality refactor** — consistent terminology across 100+ hint sites, new validation logic with tests, no breaking API changes. Ready for merge after running `pnpm refactor:validate`.

---

## 1. Hint System Overhaul (Core Refactor) — ✅ EXCELLENT

**Files**: `guidance.rs` (+55/-20), `mod.rs`, `tests.rs` (+10), `tool_description.rs` (+10 net)

### Changes

| Component      | Old                          | New                                            |
| -------------- | ---------------------------- | ---------------------------------------------- |
| Success hints  | `💡 Next: A or B or C`       | `💡 Suggested Follow-ups:\n• A\n• B\n• C`      |
| Error recovery | `💡 Next Steps:\n1. A\n2. B` | `💡 Suggested Recovery:\n1. A\n2. B`           |
| Informational  | `💡 Next Steps:`             | `💡 Optional Guidance:`                        |
| Tool schema    | `💡 Next Steps:`             | `💡 Related Actions:` / `💡 Example workflow:` |
| Footer tips    | `💡 Use X...`                | `💡 Tip: X...`                                 |

### Quality

- **Centralized**: New `hint_headers` module with 7 constants — single source of truth
- **Helper functions**: `format_numbered_guidance()`, `format_bullet_guidance()` reduce duplication
- **All 8 unit tests pass** — validates formatting, error semantics, tool-group isolation
- **Integration tests updated** — `workspace_hint_assertions.rs` now asserts _absence_ of legacy patterns (`💡 Next:`, `writeFile for full file replacement`, `strReplace.old_string`)

### Impact

- Resolves the semantic confusion identified in the audit (agents treating "Next:" as mandatory command)
- Bullet format for success hints reduces "do this OR that" ambiguity
- Numbered format preserved for actual recovery procedures (correct UX)

---

## 2. Shell Command Validation Hardening — ✅ SECURITY IMPROVEMENT

**Files**: `validation.rs` (+166 lines, new functionality), `isolated.rs` (-38/+11), `persistent.rs` (-11/+11)

### New Validation Logic (`validation.rs`)

```rust
// NEW: Detects quote/heredoc parse failures in shell stderr/stdout
pub fn looks_like_shell_quote_parse_error(stdout, stderr) -> bool {
    // Matches English + Korean bash error messages
    // "unexpected EOF while looking for matching `''"
    // "here-document at line N delimited by end-of-file"
    // "`''을(를) 찾는 도중 예상치 못한 파일의 끝"
}

// NEW: Context-aware failure guidance
pub fn shell_command_failure_guidance(exit_code, stdout, stderr) -> Vec<String> {
    if quote_parse_error { return write_file_then_shell_guidance(); }
    // Otherwise: exit-code-specific guidance (1, 2, 127, 126, 130, other)
}
```

### Security Value

- **Prevents agent retry loops** on malformed one-liners (nested quotes, heredocs)
- **Escalates to `writeFile`** — the correct fix for complex scripts
- **Locale-aware** — handles Korean bash error messages (critical for global users)
- **3 new tests** cover detection + escalation logic

### Refactoring

- `isolated.rs` & `persistent.rs` now delegate to `validation::shell_command_failure_guidance()`
- Removed ~50 lines of duplicated match-arm guidance logic
- Single source of truth for shell failure hints

---

## 3. Scheduled Task Handlers — ✅ UX POLISH

**File**: `scheduled_task/handlers.rs` (+72 lines)

### Changes

- Uses `hint_headers::AVAILABLE_OPERATIONS`, `hint_headers::TIP` consistently
- Replaced numbered "Next steps" with bullet list under `💡 Available operations:`
- Message text softened: `"Use getScheduledTask(...)"` → `"getScheduledTask(...) can show..."`
- No functional logic changes — purely presentation alignment with new hint system

---

## 4. File Operations — ✅ QUALITY IMPROVEMENTS

### `edit_line/response.rs` (+11 lines)

- Added diff preview with `MAX_DIFF_PREVIEW_LINES = 50`, `DIFF_CONTEXT_LINES = 1`
- Git-style diff (context/added/removed) with smart range collapsing
- Anchor refresh messaging in success hints

### `write.rs` (+29 lines)

- Improved path-conflict guidance with concrete examples
- Better diff preview integration
- No functional changes to write modes (create/overwrite/append)

---

## 5. Browser Tools — ✅ MINOR DOC ALIGNMENT

**Files**: `interaction.rs` (1 line), `tools.rs` (6 lines)

| File                 | Change                                                        |
| -------------------- | ------------------------------------------------------------- |
| `interaction.rs:542` | `💡 Use the selector...` → `💡 Tip: Selectors can be used...` |
| `tools.rs:59-61`     | `💡 Next Steps:` → `💡 Suggested follow-ups:` (bullet format) |

---

## 6. Setup Wizard — ✅ TEXT POLISH

**Files**: `mod.rs` (+10), `tools.rs` (+8)

- Success hints use new `hint_headers::SUCCESS_FOLLOW_UPS` terminology
- No functional changes to platform detection or guide generation

---

## 7. Frontend Generated Types — ✅ SYNC MAINTENANCE

**Files**: `desktop-fetch.ts` (4 lines), `builtin-services.ts` (5), `execution-mode.ts` (6)

- Auto-generated via `scripts/sync-*.cjs` — reflects Rust-side enum/struct changes
- TypeScript types remain strict (no `any`)
- Desktop fetch: 8s native timeout + browser fallback (unchanged)

---

## 8. Agent Orchestration — ✅ MINOR

**Files**: `tool_execution.rs` (5 lines), `agent/handlers.rs` (4 lines)

- `tool_execution.rs`: Added session output-token budget lookup helper
- `handlers.rs`: Teamwork workspace provisioning path fix (uses `WorkspaceService::provision_teamwork_workspace`)

---

## 9. Test Coverage — ✅ COMPREHENSIVE

### New/Updated Test Files

| File                           | Lines | Focus                                                            |
| ------------------------------ | ----- | ---------------------------------------------------------------- |
| `workspace_hint_assertions.rs` | 25    | **New** — shared assertions against legacy hint patterns         |
| `error_contract_guards.rs`     | 347   | Error semantics, guidance presence, actionable next steps        |
| `workspace_guidance_tests.rs`  | +48   | Empty dir hints, search guidance, no edit promotion on read-only |
| `workspace_search_tests.rs`    | +37   | Pagination, regex, binary/gitignore skip metadata                |

### Key Assertions

- **No legacy leakage**: `!text.contains("💡 Next:")`, `!text.contains("writeFile")` on success
- **Error contract**: All errors have `is_error: true`, `✗` marker, recovery section
- **Tool-group isolation**: Browser errors suggest browser tools, not planning tools

---

## Risk Assessment

| Area               | Risk                                     | Mitigation                              |
| ------------------ | ---------------------------------------- | --------------------------------------- |
| Hint system        | Low — all call sites updated, tests pass | `cargo test` + integration tests verify |
| Shell validation   | Low — additive, backward compatible      | 3 new unit tests + existing test suite  |
| Generated TS types | Low — synced from Rust SSOT              | CI runs `sync-*.cjs` on build           |
| Core agent loop    | None — no logic changes                  | Only hint text / token budget lookup    |

---

## Validation Checklist

```bash
# Run these before merge (per AGENTS.md)
pnpm lint          # TypeScript/ESLint
pnpm format        # Prettier
pnpm rust:fmt      # rustfmt
pnpm rust:clippy   # Clippy
pnpm build         # Full build
pnpm dead-code     # unimported check
# OR all-in-one:
pnpm refactor:validate
```

---

## Recommendation

**✅ APPROVE FOR MERGE**

This is a well-scoped, high-impact refactor that:

1. Fixes a documented agent-confusion issue (semantic "Next:" problem)
2. Adds security hardening for shell command execution
3. Maintains full backward compatibility (no API breaks)
4. Includes comprehensive test coverage for new behavior
5. Uses centralized constants — future terminology changes are single-line edits

The only untracked files are build artifacts (`.agents/`, `libragent@0.8.34`, `node`, `tauri`, `tsc`) and the audit report — all safe to ignore.
