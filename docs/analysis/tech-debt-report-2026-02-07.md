# Technical Debt & Dead Code Analysis Report

**Date:** 2026-02-07
**Scope:** Full Codebase

## 1. Dead Code Analysis

- **Status:** ✅ Clean
- **Findings:** The `pnpm dead-code` check passed with zero unimported files.
- **Action Taken:** Removed one instance of unused/deprecated code in `src-tauri/src/mcp/builtin/mcp_manager/tools.rs` (function `list_builtin_tools_tool`).

## 2. Technical Debt

### A. Deprecated Type Definitions (High Priority)

The file `src/lib/mcp-types.ts` is marked `@deprecated` but is still imported by **15+ active files**.

- **Impact:** Prevents full migration to the new modular MCP library structure (`@/lib/mcp/*`).
- **Recommendation:** Schedule a refactoring sprint to migrate these imports to the new `src/lib/mcp/` modules.
- **Affected Files (Sample):**
  - `src/context/llm/types.ts`
  - `src/features/agent/components/AgentMessageRenderer.tsx`
  - `src/hooks/use-agent-tools.ts`
  - `src/lib/backend/builtin-tools.ts`

### B. Missing Tests (Medium Priority)

Several Rust modules have placeholders for tests that are not yet implemented.

- **`src-tauri/src/mcp/session_isolation/tests.rs`**: "TODO: Implement unit tests as specified in Phase 1.5"
- **`src-tauri/src/mcp/service_proxy.rs`**:
  - `// TODO: Test that builtin_ prefix routes correctly`
  - `// TODO: Test that non-builtin tools route to external manager`
  - `// TODO: Test error handling for invalid tool names`

### C. Code Duplication (Resolved)

- **Problem:** `register_server_tool` and `update_server_tool` in `mcp_manager` had 30+ lines of identical schema definition.
- **Resolution:** Refactored into a shared helper function `transport_config_schema()` to improve maintainability.

## 3. Summary of Cleanup Actions

1.  **Deleted** unused function `list_builtin_tools_tool`.
2.  **Refactored** `src-tauri/src/mcp/builtin/mcp_manager/tools.rs` to use a shared helper for transport schemas, reducing code duplication.

## 4. Recommendations

1.  **Refactor Sprint:** Allocate time to remove `src/lib/mcp-types.ts` and update all consumers.
2.  **Testing:** Implement the missing unit tests for `session_isolation` to ensure the security boundary is robust.

## 3. Suppressed Dead Code Analysis (Rust)

A review of `#[allow(dead_code)]` usages usage reveals several categories of code retention:

### A. Unnecessary Suppressions (Clean up candidates)

Code that is actually used but still carries the suppression attribute.

- **`src-tauri/src/mcp/utils/schema_builder.rs`**: `boolean_prop` is used in 14+ locations (tools definitions) but still annotated with `#[allow(dead_code)]`.
  - **Action:** Attribute should be removed to reflect reality.

### B. Trait Completeness (Keep)

Methods implemented to satisfy a trait or interface completeness, even if unused by the current implementation.

- **`mcp/builtin/mod.rs`**: `description()` and `version()` in `BuiltinMCPServer` trait.
- **`mcp/types.rs`**: Various fields in configuration structs that map to JSON/Database but aren't read by Rust logic yet.

### C. Future Feature Placeholders (Monitor)

Code meant for upcoming features.

- **`agent/lifecycle.rs::resume_session`**: Fully implemented logic for resuming sessions. Likely for "Phase 2" of session management.
- **`mcp/oauth.rs`**: Explicitly marked "Reserved for future dynamic client registration (RFC 7591)".

## 4. Summary of Cleanup Actions (Updated)

1.  **Deleted** unused function `list_builtin_tools_tool`.
2.  **Refactored** `mcp_manager/tools.rs` to reduce duplication.
3.  **Identified** `boolean_prop` in `schema_builder.rs` as having an unnecessary dead code suppression.

## 5. Updated Recommendations

1.  **Refactor Sprint:** Remove `src/lib/mcp-types.ts`.
2.  **Testing:** Implement missing tests.
3.  **Cleanup:** Remove `#[allow(dead_code)]` from `boolean_prop` in a future cleanup pass.
