# Post-Update Migration Evaluation V4: Built-in Tools

**Date**: 2025-12-28
**Status**: ✅ **Fully Compliant / Ready for Migration**

## Evaluation Summary

This final evaluation confirms that the Rust implementation of the built-in MCP tools is now fully compliant with the legacy TypeScript specifications. All previously identified critical issues, feature gaps, and schema mismatches have been resolved.

| Module          | Status       | Notes                                                                                                  |
| :-------------- | :----------- | :----------------------------------------------------------------------------------------------------- |
| **Planning**    | 🟢 **Fixed** | Full parity. `tags` argument correctly handled as `string[]`.                                          |
| **Knowledge**   | 🟢 **Fixed** | Full parity. `read`/`delete` added. `tags` fixed. `searchKnowledge` improved (tags support).           |
| **Assistant**   | 🟢 **Fixed** | Full parity. `createAssistant` uses structured fields. `searchAssistant` added.                        |
| **Playbook**    | 🟢 **Fixed** | Full parity. Input validation strengthened with detailed schemas for `workflow` and `successCriteria`. |
| **MCP Manager** | 🟢 **Fixed** | Full parity. `searchServer` added.                                                                     |

## Detailed Analysis

### 1. Planning Module (✅ Fully Compliant)

- **Schema**: `addScratchpad` correctly accepts `tags` as `string[]`, matching TypeScript.
- **Tools**: All tools (`createGoal`, `updateGoal`, `clearGoal`, `addTodo`, `checkTodo`, `cancelTodo`, `clearSession`, `addScratchpad`, `listScratchpad`, `readScratchpad`, `clearScratchpad`, `getCurrentState`, `pauseAndThink`, `critiqueAndReflection`) are implemented and verified.
- **Note**: `clearTodos` renamed to `cancelTodo` for better semantic clarity (January 2026).

### 2. Knowledge Module (✅ Fully Compliant)

- **Tools**: `readKnowledge` and `deleteKnowledge` are implemented.
- **Search**: `searchKnowledge` now supports direct FTS5 `query` and/or `tags` filtering. Input schema is robust.
- **Data Handling**: `tags` are correctly serialized/deserialized as arrays.

### 3. Assistant Module (✅ Fully Compliant)

- **Schema**: `createAssistant` accepts individual property arguments (`systemPrompt`, `modelName`, etc.) as defined in TypeScript, mapping them internally to the config object.
- **Search**: `searchAssistant` tool has been implemented to allow searching by name or configuration content.
- **Constraint**: The `id` parameter is required, which is a minor but acceptable requirement for the client to generate its own IDs.

### 4. Playbook Module (✅ Fully Compliant)

- **Validation**: The `createPlaybook` and `updatePlaybook` tools now use strict JSON Schemas (`playbook_step_schema` and `success_criteria_schema`) to validate the structure of `workflow` and `successCriteria` arguments. This ensures that only valid playbook steps are accepted, mirroring or exceeding the type safety of the TypeScript version.
- **Helpers**: Dedicated helper functions were added to the module to generate these specific schemas.

### 5. MCP Manager Module (✅ Fully Compliant)

- **Tools**: `searchServer` is implemented, closing the feature gap.
- **Functionality**: Core server management (list, connect, disconnect, create) is fully operational.

## Conclusion

The Rust built-in tools are now **strict replacements** for the legacy Web MCP modules. The migration risk is minimal, as the API contract (tool names, input schemas) has been rigorously aligned.

**Recommended Action**:

- Proceed immediately with the frontend migration to use the Rust built-in backend.
- Ensure the frontend generates a UUID for the `id` field when calling `createAssistant`, `createPlaybook`, etc., as the backend expects this.

---

## Implementation Update (2026-03-15)

The migration status above is still broadly useful, but the active implementation has moved beyond the older Assistant/MCP Manager naming used in this document.

### 1. Agent domain is now canonical

The public builtin surface now uses `agent` as the canonical domain.

- `assistant`
- `assistant_manager`
- `swarm`
- `session_api`

are retained only as compatibility aliases that resolve to `agent`.

This means new code, docs, and UI surfaces should refer to:

- `agent / create`
- `agent / list`
- `agent / update`
- `agent / startSession`
- `agent / messageToSession`
- `agent / checkSession`
- `agent / stopSession`

and should not expose separate public Assistant/Swarm builtin services again.

### 2. Tool domain is now canonical

The public builtin surface now uses `tool` as the canonical domain.

- Legacy `mcp_manager` naming still resolves as a compatibility alias where needed.
- The active runtime naming contract remains `server__tool`, so the final exposed builtin symbol is `tool__list`, not nested names like `tool__tool__list`.

### 3. Builtin error semantics were narrowed

SP22-1 is now implemented in code.

Only agent-fault conditions keep hard tool-error semantics (`is_error: true`), such as:

- missing required params
- invalid input
- missing or wrong IDs
- duplicate conflicts
- explicit permission/ownership violations

Timeouts and backend/internal execution failures are now emitted as informational non-error tool results (`is_error: false`), including:

- builtin proxy timeouts
- builtin internal exceptions routed through shared guidance
- interactive shell timeout / non-zero exit reporting
- UI prompt cancellation

### 4. Regression coverage exists

These semantics are pinned by integration tests in:

- `src-tauri/tests/error_contract_guards.rs`

That file covers:

- timeout guided errors as non-error
- internal guided errors as non-error
- session wait timeout conversion to success
- UI cancellation as informational
