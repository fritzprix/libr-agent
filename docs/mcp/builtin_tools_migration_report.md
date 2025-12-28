# Migration Verification Report: Built-in Tools (Rust vs TypeScript)

**Date**: 2025-12-28
**Scope**: Verification of "Web MCP Tools Migration to Rust Built-in Backend"
**Status**: ⚠️ **Significant Mismatches Identified**

## Executive Summary

A comprehensive comparison between the new Rust-based built-in tools (`src-tauri/src/mcp/builtin`) and the legacy TypeScript implementations (`src/lib/web-mcp`) has revealed several critical discrepancies. While the core functionality is largely present, inconsistencies in **API Schemas**, **Argument Types**, and **Tool Availability** pose a high risk of breaking existing agent workflows that rely on the TypeScript definitions.

**Key Findings**:

- **Missing Tools**: 4 tools are completely missing in the Rust backend (`readKnowledge`, `deleteKnowledge`, `searchAssistant`, `searchServer`).
- **Breaking Schema Changes**: `saveKnowledge` and `addScratchpad` expect different types for tags (String vs Array), which will cause runtime errors for existing clients. Use of "opaque" JSON objects in Rust for complex types reduces validation safety compared to detailed TypeScript schemas.
- **Inconsistent API Patterns**: Pagination parameters vary between modules (switching between `limit/offset` and `page/pageSize`).

## Detailed Findings by Module

### 1. Knowledge Module (High Risk)

**Location**: `src-tauri/src/mcp/builtin/knowledge/mod.rs` vs `src/lib/web-mcp/modules/knowledge-server/tools.ts`

| Discrepancy                     | Rust Implementation              | TypeScript Definition              | Impact                                                                      |
| :------------------------------ | :------------------------------- | :--------------------------------- | :-------------------------------------------------------------------------- |
| **Missing Tools**               | None                             | `readKnowledge`, `deleteKnowledge` | **Critical**. Agents cannot retrieve full details or cleanup data.          |
| **`saveKnowledge` Tags**        | `tags: string` (comma-separated) | `tags: string[]` (Array)           | **Breaking**. calls sending arrays will fail or be stringified incorrectly. |
| **`searchKnowledge` Query**     | `query` is **Required**          | `query` is **Optional**            | **Medium**. Restricts search flexibility.                                   |
| **`searchKnowledge` Filtering** | Missing `tags` filter            | Has `tags` filter (Array)          | **Medium**. Cannot filter search results by tag.                            |

**Code Evidence**:

```rust
// Rust: src-tauri/src/mcp/builtin/knowledge/mod.rs
props.insert("tags".to_string(), string_prop(..., "Optional comma-separated tags"));
// Implementation expects string:
let tags = args.get("tags").and_then(|v| v.as_str());
```

```typescript
// TypeScript: src/lib/web-mcp/modules/knowledge-server/tools.ts
tags: createArraySchema({ items: createStringSchema(...) })
```

### 2. Assistant Module (High Risk)

**Location**: `src-tauri/src/mcp/builtin/assistant/mod.rs` vs `src/lib/web-mcp/modules/assistant-manager/tools.ts`

| Discrepancy                  | Rust Implementation                 | TypeScript Definition                                    | Impact                                                                                                                   |
| :--------------------------- | :---------------------------------- | :------------------------------------------------------- | :----------------------------------------------------------------------------------------------------------------------- |
| **Missing Tools**            | None                                | `searchAssistant`                                        | **Medium**. Harder to find assistants in large lists.                                                                    |
| **`createAssistant` Schema** | `{ id, name, config: string/json }` | `{ name, systemPrompt, description, mcpServerIds, ... }` | **Critical**. Rust expects a raw config blob and manual ID, TS expects structured fields. Logic is completely different. |
| **Pagination**               | `limit`, `offset`                   | `page`, `pageSize`                                       | **Medium**. Inconsistent API style.                                                                                      |

**Code Evidence**:

```rust
// Rust: Generic config blob
props.insert("config".to_string(), string_prop_required("Assistant configuration (JSON object)"));
```

```typescript
// TypeScript: Structured fields
properties: {
  name: createStringSchema(...),
  systemPrompt: createStringSchema(...),
  mcpServerIds: createArraySchema(...)
}
```

### 3. Planning Module (Medium Risk)

**Location**: `src-tauri/src/mcp/builtin/planning/mod.rs` vs `src/lib/web-mcp/modules/planning-server/tools.ts`

| Discrepancy              | Rust Implementation                        | TypeScript Definition                                 | Impact                                                                           |
| :----------------------- | :----------------------------------------- | :---------------------------------------------------- | :------------------------------------------------------------------------------- |
| **`addScratchpad` Tags** | `tags: string` (comma-separated)           | `tags: string[]` (Array)                              | **Breaking**. Same issue as Knowledge module.                                    |
| **`addTodo` Subtasks**   | Loose validation (missing required fields) | Strict validation (`title` required, `priority` enum) | **Low**. Rust accepts more valid inputs, but might accept invalid data silently. |

### 4. Playbook Module (Medium Risk)

**Location**: `src-tauri/src/mcp/builtin/playbook/mod.rs` vs `src/lib/web-mcp/modules/playbook-store/tools.ts`

| Discrepancy         | Rust Implementation                                               | TypeScript Definition                      | Impact                                                                                                                     |
| :------------------ | :---------------------------------------------------------------- | :----------------------------------------- | :------------------------------------------------------------------------------------------------------------------------- |
| **Complex Objects** | `workflow` & `successCriteria` are opaque `object`/`array` types. | Detailed schemas with required sub-fields. | **Medium**. Loss of validation. Agents might generate malformed playbooks that fail at runtime instead of validation time. |

### 5. MCP Manager (Medium Risk)

**Location**: `src-tauri/src/mcp/builtin/mcp_manager/mod.rs` vs `src/lib/web-mcp/modules/mcp-manager/tools.ts`

| Discrepancy                 | Rust Implementation                   | TypeScript Definition     | Impact                  |
| :-------------------------- | :------------------------------------ | :------------------------ | :---------------------- |
| **Missing Tools**           | None                                  | `searchServer`            | **Low**.                |
| **`createServer` Metadata** | Missing `description`, `tags` fields. | Fields present in schema. | **Low**. Metadata loss. |

### 6. UI Module (Low Risk)

**Location**: `src-tauri/src/mcp/builtin/ui/mod.rs` vs `src/lib/web-mcp/modules/ui-tools/tools.ts`

| Discrepancy     | Rust Implementation                | TypeScript Definition            | Impact                                                                                                                                                                       |
| :-------------- | :--------------------------------- | :------------------------------- | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Tool Naming** | `snake_case` (e.g., `prompt_user`) | `camelCase` (e.g., `promptUser`) | **Potential Breaking**. Depends on how the dispatch layer aliases these. If agents are prompted with snake_case but code expects camelCase (or vice versa), tools will fail. |

## Remediation Plan

To ensure a successful migration and maintain backward compatibility, the following actions are recommended:

### Phase 1: Critical Fixes (Must Do)

1.  **Implement Missing Knowledge Tools**:
    - Add `read_knowledge` (get by ID).
    - Add `delete_knowledge` (delete by ID).
2.  **Fix Tag Types**:
    - Update `saveKnowledge` and `addScratchpad` in Rust to accept `Vec<String>` (arrays) instead of comma-separated strings. This aligns with the "Agentic" nature of passing structured data.
3.  **Align Assistant Creation**:
    - Refactor `createAssistant` in Rust to accept flattened fields (`systemPrompt`, `description`) and construct the internal `config` JSON object automatically.
    - Make `id` optional in Rust (auto-generate UUID if missing) to match TS behavior.

### Phase 2: Consistency & Validation (Should Do)

1.  **Standardize Pagination**:
    - Adopt `page`/`pageSize` across all modules for consistency with the frontend UI patterns, or strictly enforce `limit`/`offset` and update the frontend client. (Recommendation: `page`/`pageSize` is more user-friendly for UI).
2.  **Enhance Schemas**:
    - Update `Playbook` and `Planning` Rust schemas to fully define nested objects (`subtasks`, `workflow steps`) using `serde_json` schemas, rather than generic objects.

### Phase 3: Missing Features (Nice to Do)

1.  **Add Search Tools**: Implement `searchAssistant` and `searchServer` using simplistic name matching or FTS if supported by the backing store.
2.  **Metadata Fields**: Add `description` and `tags` support to `MCPManager`.

## Next Steps

1.  Approve this remediation plan.
2.  Begin implementation of Phase 1 fixes in `src-tauri/src/mcp/builtin/`.
