# Post-Update Migration Evaluation: Built-in Tools

**Date**: 2025-12-28
**Status**: ⚠️ **Partial Fixes (Mixed State)**

## Evaluation Summary

Following the code updates, a re-evaluation of the Rust built-in tools reveals that **the Planning module has been improved**, but **Knowledge and Assistant modules remain critically mismatched** against the TypeScript legacy definitions.

| Module          | Status          | Notes                                                                            |
| :-------------- | :-------------- | :------------------------------------------------------------------------------- |
| **Planning**    | 🟢 **Improved** | `addScratchpad` schema now correctly expects `tags: string[]`.                   |
| **Knowledge**   | 🔴 **Critical** | Still missing tools (`read/delete`). `tags` is still `string` (comma-separated). |
| **Assistant**   | 🔴 **Critical** | Still uses generic `config` blob instead of structured fields.                   |
| **MCP Manager** | 🟡 **Warning**  | Missing `searchServer` and metadata fields.                                      |

## Detailed Analysis

### 1. Planning Module (✅ Fixed)

- **Change**: The `tools()` definition for `PlanningServer` now explicitly defines `addScratchpad` using a `json!` macro that sets `tags` to `{ "type": "array", "items": { "type": "string" } }`.
- **Implementation**: The code stores this as a JSON string (`v.to_string()`) and correctly parses it back as `Vec<String>` in `listScratchpad`.
- **Verdict**: This module appears to be **aligned** for the `tags` issue.

### 2. Knowledge Module (❌ Unchanged)

- **Issue 1: Type Mismatch**: `saveKnowledge` still uses the helper `string_prop(...)`, creating a schema where `tags` must be a single string.
  - _Code:_ `props.insert("tags".to_string(), string_prop(..., "Optional comma-separated tags"));`
  - **Impact**: Agents sending `["tag1", "tag2"]` (as per TS definition) will fail validation or runtime processing.
- **Issue 2: Missing Tools**: `readKnowledge` and `deleteKnowledge` are still absent from the `tools()` list.

### 3. Assistant Module (❌ Unchanged)

- **Issue**: `createAssistant` still expects a generic `{ id, name, config }` object.
  - _Legacy TS_: Expects `{ name, systemPrompt, description, ... }`.
  - **Impact**: Completely incompatible. Existing agents attempting to create assistants will fail. The Rust implementation requires the client to pre-construct a `config` JSON object and manually generate an ID.

## Recommendations

1.  **Apply Planning Module's Fix to Knowledge Module**:
    - Update `KnowledgeServer::tools()` to use `json!` for schema definition (like Planning) or update the `schema_builder` helper to support arrays.
    - Change `saveKnowledge` implementation to handle `Value::Array` for tags.
2.  **Implement Missing Knowledge Tools**:
    - Add `read` and `delete` handlers.
3.  **Refactor Assistant Creation**:
    - Update `createAssistant` to accept structured arguments (`systemPrompt`) and build the config internally.

## Conclusion

The migration is **incomplete**. While the Planning module shows evidence of remediation, the Knowledge and Assistant modules require similar attention to prevent breaking changes.
