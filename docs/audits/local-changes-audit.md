# Code Audit Report — Local Uncommitted Changes

**Date:** 2025-07-24
**Scope:** 9 modified files (Rust backend + integration tests)
**Net Diff:** +34 / -27 lines
**Untracked:** `.agents/skills/lean-builtin-tool-auditor/`, `.kilo/`

---

## 1. Change Summary

| #   | File                                                              | Type    | Lines  |
| --- | ----------------------------------------------------------------- | ------- | ------ |
| 1   | `src-tauri/src/mcp/builtin/agent/handlers/sessions.rs`            | Logic   | -6/+5  |
| 2   | `src-tauri/src/mcp/builtin/agent/tools.rs`                        | Schema  | -2/+11 |
| 3   | `src-tauri/src/mcp/builtin/planning/goals.rs`                     | Cleanup | -4     |
| 4   | `src-tauri/src/mcp/builtin/planning/todos.rs`                     | Cleanup | -3     |
| 5   | `src-tauri/src/mcp/builtin/planning/tools.rs`                     | Schema  | -5/+10 |
| 6   | `src-tauri/src/mcp/builtin/tool/tools.rs`                         | Schema  | -1/+2  |
| 7   | `src-tauri/src/mcp/builtin/workspace/tools/file_tools.rs`         | Schema  | -3/+3  |
| 8   | `src-tauri/tests/integration/planning_todo_id_tests.rs`           | Test    | -2/+2  |
| 9   | `src-tauri/tests/integration/tool_schema_property_order_tests.rs` | Test    | -1/+1  |

---

## 2. Deep Analysis by Category

### 2.1 startSession Timeout Parameter (sessions.rs + agent/tools.rs)

**What changed:**

- Added `timeout` parameter to `startSession` tool schema (integer, range 1–3600, default 3600)
- `start_session_impl` now reads `timeout` from args and passes it to `check_session` when `waitForResult=true`
- Property order updated: `agentId` → `workspaceOverride` → `waitForResult` → `timeout` → `task`

**Assessment: ✅ GOOD**

- The `timeout` parameter was already supported by `check_session` internally (see `parse_message_to_session_wait_config` in sessions.rs). This change makes it **explicitly configurable at spawn time** instead of relying on the check_session default.
- Property order is correct: `task` stays last (longest/most variable field), `timeout` is a short numeric that naturally precedes it.
- The test `start_session_schema_property_order_puts_task_last` was updated to reflect the new order — good test hygiene.

**Risk: LOW** — This is additive. Existing callers that omit `timeout` get the same 3600s default.

---

### 2.2 Planning Response Cleanup (goals.rs + todos.rs)

**What changed:**

- Removed `let response_id = cuid2::create_id();` from `create_goal`, `update_goal`, `add_todo`, `check_todo`, `cancel_todo`
- Removed `"id": response_id` (or `"id": cuid2::create_id()`) from all `structured_content` JSON payloads

**Assessment: ✅ GOOD — Necessary cleanup**

- These synthetic `cuid2::create_id()` values were **never consumed by any consumer**. The real identifiers (`goalId`, `todoId`) were already in the response.
- The `createGoal` test (`planning_context_and_update_todo_use_todo_ids`) confirms agents use `goalId`/`todoId` from structured content, not a synthetic `id`.
- Reduces JSON payload noise and eliminates a meaningless UUID that could confuse agents.

**Risk: NONE** — Breaking change? No, because no consumer was using the removed `id` field.

---

### 2.3 Schema Tightening (planning/tools.rs + tests)

**What changed:**

- `updateTodo` `id` parameter: `minimum` set to `Some(1)` (was `None`)
- `getCurrentState` `include_checked`: added `default = true` (was implicit)
- Test renamed: `update_todo_schema_leaves_todo_id_unbounded` → `update_todo_schema_requires_positive_todo_id`
- Test assertion: `minimum` from `None` → `Some(1)`

**Assessment: ✅ GOOD — Defensive improvement**

- The runtime check `if todo_id <= 0` in `check_todo` already rejected negative IDs, but the **schema-level constraint** now catches this earlier and provides better guidance to LLM agents.
- This is a **schema-only change** — the runtime behavior is identical. No functional regression.
- The test rename accurately reflects the new intent.

**Risk: NONE** — Schema tightening only.

---

### 2.4 Schema Examples & Limits (tool/tools.rs + file_tools.rs)

**What changed:**

- `registerServer` `name` field: added `examples: ["github", "local-fs"]`
- `readFile` `offset` description: removed "Alias to startLine" text
- `listDirectory` `limit` max: changed from `Some(1000)` to `Some(500)`

**Assessment: ⚠️ MIXED**

| Sub-change                      | Verdict                   | Notes                                                                                                                                                                                    |
| ------------------------------- | ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `registerServer` examples       | ✅ GOOD                   | Helps LLM agents choose valid slugs                                                                                                                                                      |
| `readFile` offset alias removal | ✅ GOOD                   | "Alias to startLine" was misleading — `offset` is the actual parameter name                                                                                                              |
| `listDirectory` limit 1000→500  | ⚠️ **NEEDS VERIFICATION** | This is a **breaking limit change**. If any code path or consumer depends on 1000 items per page, this will silently truncate. The description says "max: 500" which implies a hard cap. |

**Risk: MEDIUM** for `listDirectory` limit change. The `offset` parameter has no max, so pagination still works — but single-page callers expecting 1000 items now get at most 500.

---

## 3. Architectural Assessment

### 3.1 Consistency with LibrAgent Architecture

| Principle                      | Status       | Notes                                               |
| ------------------------------ | ------------ | --------------------------------------------------- |
| **Session Isolation**          | ✅ Preserved | No changes to session-scoping logic                 |
| **Tool Schema Discipline**     | ✅ Improved  | Better constraints, examples, and defaults          |
| **Response Data Minimization** | ✅ Improved  | Removed meaningless synthetic IDs                   |
| **Test Coverage**              | ✅ Updated   | Both integration tests updated to match new schemas |
| **DRY**                        | ✅ Improved  | Removed redundant `cuid2::create_id()` calls        |

### 3.2 Dependency Impact Graph

```
agent/tools.rs (schema)
  └── agent/handlers/sessions.rs (implementation)
        └── check_session (existing, already supports timeout)

planning/tools.rs (schema)
  └── planning/todos.rs (runtime: check_todo already has `if todo_id <= 0`)
  └── planning/goals.rs (no schema-logic coupling)

tool/tools.rs (schema only)
  └── No downstream implementation impact

file_tools.rs (schema only)
  └── No downstream implementation impact (limit enforced at schema level)
```

**No circular dependencies introduced. No cross-cutting concerns affected.**

---

## 4. Risk Assessment

| Risk                                                   | Severity | Likelihood | Mitigation                                                    |
| ------------------------------------------------------ | -------- | ---------- | ------------------------------------------------------------- |
| `listDirectory` limit 1000→500 breaks callers          | Medium   | Low        | Verify no consumer expects 1000 items/page                    |
| `timeout` parameter not documented in user-facing docs | Low      | Low        | Internal tool schema change; docs updated in tool description |
| Synthetic `id` field removal confuses legacy consumers | None     | None       | No evidence of any consumer using it                          |

---

## 5. Recommendations

### P0 — Must Fix

- **None.** All changes are sound.

### P1 — Should Consider

1. **`listDirectory` limit change**: Add a TODO comment explaining why 1000→500, or verify this is intentional. If it's for performance, document the rationale.
2. **Untracked directories**: `.agents/skills/lean-builtin-tool-auditor/` and `.kilo/` — confirm these are intentional additions or should be gitignored.

### P2 — Nice to Have

1. Consider adding `timeout` examples to the `startSession` schema (e.g., `examples: [300, 3600]`) for LLM agents.
2. The `parse_message_to_session_wait_config` function in `sessions.rs` already handles `timeout` for `messageToSession` — consider unifying the timeout handling path between `startSession` and `messageToSession` for consistency.

---

## 6. Overall Verdict

**Score: 8.5/10 — Clean, well-scoped changes with no structural issues.**

The changes are surgical, well-tested, and improve both the schema quality (constraints, examples, defaults) and response data cleanliness (removing synthetic IDs). The `startSession` timeout addition fills a real gap. The only item worth double-checking is the `listDirectory` limit reduction from 1000 to 500.

**Ready to commit after verifying the `listDirectory` limit change is intentional.**
