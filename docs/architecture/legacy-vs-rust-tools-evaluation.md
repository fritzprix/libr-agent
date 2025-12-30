# Legacy (v1) vs Rust (v2) Built-in Tools Evaluation

**Evaluation Date:** December 30, 2025
**Scope:** Comparison between legacy TypeScript implementations (`src/features/tools/browser-tools`, `src/lib/web-mcp`) and new Rust built-in MCP servers (`src-tauri/src/mcp/builtin`).
**Focus:** Tool Descriptions, Input Parameters, and Result Structures.

---

## Executive Summary

The migration from TypeScript-based "Web MCP" and local tools to Rust-based native MCP servers represents a significant architectural shift. This evaluation compares the two implementations to ensure feature parity, identify improvements, and highlight any potential regressions.

**Key Finding:** The Rust v2 implementation offers superior performance, better data integrity (SQLite vs IndexedDB), and a more robust architecture. The Planning tools in v2 have surpassed v1 in terms of validation and error guidance. However, some specific error handling nuances present in v1 browser tools need to be fully ported to v2.

---

## Tool Comparison

### 1. Browser Automation Tools

| Tool               | Legacy (TypeScript)                                                                                                                                                    | New (Rust)                                                                                                                                                          | Status      |
| :----------------- | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------ | :---------- |
| **navigateToUrl**  | **Desc:** Navigates to a new URL in an existing browser session.<br>**Input:** `sessionId` (string), `url` (string)<br>**Result:** Text response with navigation hint. | **Desc:** Navigate to a URL<br>**Input:** `session_id` (string), `url` (string)<br>**Result:** `MCPResult` (success/error)                                          | ✅ Parity   |
| **extractContent** | **Desc:** Extracts content from the current page as Markdown.<br>**Input:** `sessionId` (string)<br>**Result:** Markdown string + metadata.                            | **Desc:** Extract content from the current page<br>**Input:** `session_id` (string)<br>**Result:** `MCPContent` (text/markdown)                                     | 🚀 Improved |
| **readWebContent** | **Desc:** Read a specific page of content extracted from a webpage.<br>**Input:** `sessionId`, `page`<br>**Result:** Paginated content.                                | **Desc:** Read web content with pagination support.<br>**Input:** `session_id`, `url`, `page` (opt), `page_size` (opt)<br>**Result:** Paginated content + metadata. | ✅ Parity   |

**Analysis:**

- **v2 Improvement:** `extractWebContent` in Rust is smarter; it automatically merges content if small (≤2 pages OR <5000 chars), whereas legacy just dumped everything.
- **Parity:** `readWebContent` existed in legacy (`ReadContentTool.ts`) and is preserved in v2.
- **v2 Gap:** Legacy `extractContent` used `TurndownService` for robust HTML-to-Markdown conversion. v2 uses `html2md` crate which needs to be verified for quality parity.

### 2. Planning Tools (Todos/Goals)

| Tool           | Legacy (TypeScript)                                                                                                                                                                               | New (Rust)                                                                                                                                                                                       | Status      |
| :------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | :----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :---------- |
| **addTodo**    | **Desc:** Add a todo item. Supports 1-level nesting.<br>**Input:** `title`, `description` (opt), `priority` (opt), `parentId` (opt), `subtasks` (opt array)<br>**Result:** JSON with new todo ID. | **Desc:** Add a new todo item<br>**Input:** `title`, `description` (opt), `priority` (opt), `parentId` (opt), `subtasks` (opt array)<br>**Result:** `MCPResult` with `SuccessHint` (next steps). | 🚀 Improved |
| **createGoal** | **Desc:** Create a single goal for the session.<br>**Input:** `goal` (string)<br>**Result:** JSON confirmation.                                                                                   | **Desc:** Create a new goal<br>**Input:** `goal` (string)<br>**Result:** `MCPResult` with `SuccessHint`.                                                                                         | 🚀 Improved |
| **checkTodo**  | **Desc:** Mark a todo as completed.<br>**Input:** `id` (number)<br>**Result:** JSON confirmation.                                                                                                 | **Desc:** Mark a todo as completed<br>**Input:** `id` (number)<br>**Result:** `MCPResult` with `SuccessHint`.                                                                                    | 🚀 Improved |

**Analysis:**

- **v2 Improvement:** The `SuccessHint` system in v2 provides actionable next steps (e.g., "Todo added. Next: Use list_todos to see..."), which helps the LLM stay on track.
- **Parity:** Input schemas are identical, ensuring drop-in compatibility for LLMs.

### 3. Assistant Management

| Tool                | Legacy (TypeScript)                                                                                                                                                                      | New (Rust)                                                                                                                                                                                    | Status           |
| :------------------ | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :--------------- |
| **listAssistants**  | **Desc:** List available assistants with pagination.<br>**Input:** `page` (opt), `pageSize` (opt)<br>**Result:** JSON array of assistants.                                               | **Desc:** List all assistants<br>**Input:** `limit` (opt), `offset` (opt)<br>**Result:** JSON array + total count.                                                                            | ⚠️ Schema Change |
| **createAssistant** | **Desc:** Create a new assistant.<br>**Input:** `name`, `systemPrompt`, `description` (opt), `mcpServerIds` (opt), `allowedBuiltInServiceAliases` (opt)<br>**Result:** JSON with new ID. | **Desc:** Create a new assistant<br>**Input:** `id`, `name`, `systemPrompt`, `modelProvider`, `modelName`, `temperature`, `maxTokens`, `tools`, `mcpServers`<br>**Result:** JSON with new ID. | ⚠️ Schema Change |

**Analysis:**

- **Schema Change:** `listAssistants` uses `limit`/`offset` in Rust instead of `page`/`pageSize`.
- **Schema Change:** `createAssistant` in Rust has different parameter names (e.g., `mcpServers` instead of `mcpServerIds`) and includes model configuration fields directly.

### 4. Content Store (Knowledge/Files)

| Tool                | Legacy (TypeScript)                                                                                                                    | New (Rust)                                                                                                                                                            | Status           |
| :------------------ | :------------------------------------------------------------------------------------------------------------------------------------- | :-------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :--------------- |
| **searchKnowledge** | **Desc:** Search by query string and/or tags.<br>**Input:** `query` (string), `tags` (opt array)<br>**Result:** JSON array of matches. | **Desc:** Search content using BM25 (Tool name: `keywordSimilaritySearch`)<br>**Input:** `query` (string)<br>**Result:** JSON array of matches with relevance scores. | 🚀 Improved      |
| **saveKnowledge**   | **Desc:** Save a piece of knowledge.<br>**Input:** `title`, `content`, `tags` (opt)<br>**Result:** JSON confirmation.                  | **Desc:** Add content to store (Tool name: `addContent`)<br>**Input:** `content` (string), `metadata` (opt object)<br>**Result:** JSON confirmation.                  | ⚠️ Schema Change |

**Analysis:**

- **v2 Improvement:** BM25 search provides relevance scores, making retrieval much more effective than simple string matching.
- **Schema Change:** `saveKnowledge` (Legacy) vs `addContent` (Rust). The Rust tool is more generic (`addContent` handles files and text), whereas legacy was specific to "knowledge".
- **Name Change:** `searchKnowledge` is now `keywordSimilaritySearch`.

### 5. MCP Manager

| Tool             | Legacy (TypeScript)                                                                                                                                                                               | New (Rust)                                                                                                                                       | Status        |
| :--------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | :----------------------------------------------------------------------------------------------------------------------------------------------- | :------------ |
| **listServers**  | **Desc:** List all registered MCP servers with pagination.<br>**Input:** `page` (opt), `pageSize` (opt), `filterByAssistant` (opt), `includeInactive` (opt)<br>**Result:** JSON array of servers. | **Desc:** List all MCP servers<br>**Input:** `page`, `pageSize`, `filterByAssistant`, `includeInactive`<br>**Result:** Text summary + JSON data. | ✅ Parity     |
| **searchServer** | **Desc:** Search servers by name/desc/tags.<br>**Input:** `query` (string), `page` (opt)<br>**Result:** JSON array of matches.                                                                    | **Desc:** Search MCP servers<br>**Input:** `query` (string)<br>**Result:** Text summary + JSON data.                                             | ⚠️ Regression |

**Analysis:**

- **Parity:** `listServers` in Rust supports the same filtering options (`filterByAssistant`, `includeInactive`) as legacy.
- **Regression:** `searchServer` in Rust is simpler (only `query`). Legacy had advanced options like `searchMode` (BM25 vs simple) and field weights.

### 6. Playbook Store

| Tool               | Legacy (TypeScript)                                                                                                                              | New (Rust)                                                                                                                                      | Status      |
| :----------------- | :----------------------------------------------------------------------------------------------------------------------------------------------- | :---------------------------------------------------------------------------------------------------------------------------------------------- | :---------- |
| **createPlaybook** | **Desc:** Create a new playbook.<br>**Input:** `goal`, `initialCommand`, `workflow` (array), `successCriteria`<br>**Result:** JSON confirmation. | **Desc:** Create a new playbook<br>**Input:** `goal`, `initialCommand`, `workflow` (array), `successCriteria`<br>**Result:** JSON confirmation. | ✅ Parity   |
| **selectPlaybook** | **Desc:** Select a playbook by ID.<br>**Input:** `id` (string)<br>**Result:** Formatted text + agent prompt.                                     | **Desc:** Select a playbook<br>**Input:** `id` (string)<br>**Result:** UI Resource (HTML) + Text.                                               | 🚀 Improved |

**Analysis:**

- **v2 Improvement:** `selectPlaybook` in v2 returns a UI resource (HTML) which allows for a richer, interactive selection experience compared to just text.

### 7. UI Tools

| Tool              | Legacy (TypeScript)                                                                                                                    | New (Rust)                                                                                                              | Status    |
| :---------------- | :------------------------------------------------------------------------------------------------------------------------------------- | :---------------------------------------------------------------------------------------------------------------------- | :-------- |
| **promptUser**    | **Desc:** Display interactive prompt (text/select).<br>**Input:** `prompt`, `type`, `options` (opt)<br>**Result:** UI Resource (HTML). | **Desc:** Display interactive prompt<br>**Input:** `prompt`, `type`, `options` (opt)<br>**Result:** UI Resource (HTML). | ✅ Parity |
| **visualizeData** | **Desc:** Create bar/line chart.<br>**Input:** `type`, `data` (array)<br>**Result:** UI Resource (HTML).                               | **Desc:** Create data visualization<br>**Input:** `type`, `data` (array)<br>**Result:** UI Resource (HTML).             | ✅ Parity |

---

## Migration Gaps & Recommendations

### 1. Browser Error Handling

**Gap:** The legacy `handleBrowserError` utility provided user-friendly messages for common browser errors. The v2 implementation often returns raw error strings.
**Recommendation:** Port the logic from `src/features/tools/browser-tools/error-utils.ts` to the new `error_guidance.rs` system in Rust.

### 2. UI Feedback

**Gap:** Legacy tools running in the main process could easily trigger UI toasts or updates. Native tools rely on the MCP protocol response.
**Recommendation:** Ensure the frontend correctly interprets MCP `notifications` or specific response formats to provide user feedback.

### 3. Validation Nuances

**Gap:** Legacy planning tools had specific checks for "corrupted" states (e.g., missing parent IDs).
**Recommendation:** While SQLite constraints prevent most of this, ensure that migration scripts (if any) handle legacy data cleanup.

---

## Conclusion

The v2 Rust implementation is a superior foundation for LibrAgent. It addresses the core stability and performance issues of the v1 Web MCP architecture. The Planning module is the gold standard for the new implementation. The primary focus now should be bringing the Browser and Workspace tools up to the same standard of error guidance and validation as the Planning tools.
