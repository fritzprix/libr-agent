# Agent Hub Refactoring Plan (Phase 1)

> **Scope**: Tool deduplication and alias consolidation only.  
> Backend switching / multi-instance scenarios are explicitly out of scope.

## Background

Three builtin MCP servers currently operate as separate alias entries:

| Alias (`allowedBuiltInServiceAliases`) | Rust Module                | Prefix exposed to AI    |
| -------------------------------------- | -------------------------- | ----------------------- |
| `session_api`                          | `mcp/builtin/session_api/` | `builtin_session_api__` |
| `assistant`                            | `mcp/builtin/assistant/`   | `builtin_assistant__`   |
| `mcp_manager`                          | `mcp/builtin/mcp_manager/` | `builtin_mcp_manager__` |

## Problems Identified

### 1. Duplicate Tools

The same logical operation appears under two different prefixes:

| Tool name        | `session_api`                    | `assistant`                      | Notes                   |
| ---------------- | -------------------------------- | -------------------------------- | ----------------------- |
| `listAssistants` | ??(HTTP ??`/api/assistants`)     | ??(DB direct, pagination/search) | session_api is a subset |
| `getAssistant`   | ??(HTTP ??`/api/assistants/:id`) | ??(DB direct)                    | session_api is a subset |

When both are active in the same session, the AI sees two tools with identical names but different prefixes ??confusion.

### 2. Dependency Chain

The three servers form a natural workflow chain:

```
builtin_mcp_manager__listExternalServers
        ?? (get MCP server UUID)
builtin_assistant__createAssistant / updateAssistant
        ?? (get assistantId)
builtin_session_api__createChildSession
        ??builtin_session_api__waitForSessionIdle / getMessages / sendMessage
```

These are conceptually one "agent management" domain, split across three separate activation entries.

### 3. User-facing Friction

`allowedBuiltInServiceAliases` currently requires the user to know and list all three separately:

```json
["session_api", "assistant", "mcp_manager"]
```

---

## Proposed Solution: `agent_hub` Facade

### Concept

Introduce a single `AgentHubServer` that internally owns and delegates to the three servers.

```
agent_hub (new)
?��??� AssistantServer   (CRUD for assistants)
?��??� MCPManagerServer  (register/manage external MCP servers)
?��??� SessionApiServer  (child session lifecycle)
```

### Tool Namespace

All tools exposed under a single prefix: `builtin_agent_hub__`

Dedup policy:

- `listAssistants` ??use `assistant` version (DB-direct, supports pagination/search)
- `getAssistant` ??use `assistant` version (DB-direct)
- `session_api`'s versions are dropped

### Activation

Single alias instead of three:

```json
["agent_hub"]
```

### Backward Compatibility

`service_proxy.rs::create_builtin_server()` normalizes old aliases:

```rust
"session_api" | "assistant" | "mcp_manager" => "agent_hub"
```

This means existing assistant configs that list the old aliases continue to work.

---

## Implementation Scope

| File                                      | Change                                                |
| ----------------------------------------- | ----------------------------------------------------- |
| `src/mcp/builtin/agent_hub/mod.rs`        | New: `AgentHubServer` implementing `BuiltinMCPServer` |
| `src/mcp/builtin/mod.rs`                  | Add `pub mod agent_hub`                               |
| `src/mcp/service_proxy.rs`                | Add `"agent_hub"` case + normalize old aliases        |
| `src/mcp/builtin/session_api/tools.rs`    | Remove `listAssistants`, `getAssistant` (dedup)       |
| `src/mcp/builtin/session_api/handlers.rs` | Remove corresponding handler arms                     |

The three existing modules (`session_api`, `assistant`, `mcp_manager`) are **kept as-is** internally ??`AgentHubServer` simply delegates to them.

---

## Detailed Tool Inventory After Merge

### Tools dropped from `session_api` (deduplicated)

| Tool             | Reason                                                        |
| ---------------- | ------------------------------------------------------------- |
| `listAssistants` | `assistant` version is a strict superset (pagination, search) |
| `getAssistant`   | `assistant` version is a strict superset                      |

### Final tool list under `builtin_agent_hub__`

**From `session_api` (8 tools ??child session lifecycle):**

- `healthCheck`, `createChildSession`, `getSession`, `waitForSessionIdle`
- `getMessages`, `sendMessage`, `terminateSession`, `getChildSessions`

**From `assistant` (6 tools ??assistant CRUD):**

- `createAssistant`, `updateAssistant`, `deleteAssistant`
- `listAssistants`, `getAssistant`, `searchAssistant`

**From `mcp_manager` (7 tools ??external server management):**

- `listExternalServers`, `listInternalTools`, `searchServer`
- `registerServer`, `updateServer`, `deleteServer`, `verifyServer`

**Total: 21 tools under a single prefix**

---

## `AgentHubServer` Struct Shape

```rust
pub struct AgentHubServer {
    assistant_server: AssistantServer,
    mcp_manager_server: MCPManagerServer,
    session_api_server: SessionApiServer,
}
```

`call_tool()` routes by tool name ??delegates directly to the corresponding inner server's handler. No new dispatch logic beyond what each inner server already implements.

### Backward Compatibility in `service_proxy.rs`

```rust
"agent_hub" => AgentHubServer::new(...),
// normalize legacy aliases
"session_api" | "assistant" | "mcp_manager" => AgentHubServer::new(...),
```

Existing assistant configs listing the old aliases continue to work without data migration.

---

## Out of Scope

The following were analysed and explicitly deferred:

- Backend switching / multi-instance (home vs work LibrAgent)
- REST API completion (`POST/PUT/DELETE /api/assistants`, `/api/mcp-servers`)
- HTTP transport layer for `planning`, `playbook`, `knowledge`, `content_store`
- Session key portability across instances
