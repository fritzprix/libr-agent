# Phase 0 Discovery: V2 Tool Architecture

**Date**: 2026-01-10  
**Status**: ✅ Complete - No Implementation Needed

---

## Summary

**Original Goal**: Create AgentBuiltInToolProvider for V2 to fix tool context switching

**Discovery**: V2 doesn't need frontend tool providers at all!

---

## Architecture Analysis

### V1 Chat (Legacy) - Frontend Tool Registration

```typescript
App (Global)
├─ WebMCPProvider → Browser-based MCP servers
├─ BuiltInToolProvider (App-scoped) → Tool registry
├── WebMCPServiceRegistry → Registers Web MCP as builtin tools
├── BrowserToolProvider → Registers browser automation tools
└── RustMCPToolProvider → Registers Rust MCP stdio servers

/chat/single → Uses frontend-registered tools
```

**Pattern**: Frontend components register services with BuiltInToolProvider

### V2 Agent - Backend Tool Management

```rust
// src-tauri/src/agent/

AgentSessionManager
├─ MCPServiceProxy (per-session, isolated)
│  ├─ builtin_servers: HashMap<String, BuiltinServer>
│  │  ├─ Planning
│  │  ├─ Knowledge
│  │  ├─ Browser
│  │  └─ ... (configured in agent_config.tools)
│  └─ external_servers: Vec<MCPServer> (stdio)
│
└─ Tool Execution Flow:
   1. Frontend: agent_session_call_tool(sessionId, toolCall)
   2. Backend: MCPServiceProxy routes to correct builtin/external server
   3. Backend: Service returns MCPResponse
   4. Frontend: Receives result via event bus
```

**Pattern**: Backend manages all tools, frontend only triggers execution

---

## Key Differences

| Aspect                | V1 Chat                                | V2 Agent                             |
| --------------------- | -------------------------------------- | ------------------------------------ |
| **Tool Registration** | Frontend (React Context)               | Backend (Rust)                       |
| **Tool Source**       | WebMCP Workers                         | Rust Builtin Servers                 |
| **Session Scope**     | Global "current session"               | Per-session proxy                    |
| **Context Switching** | Frontend effect watches SessionContext | Backend manages per session          |
| **Service Contexts**  | Built in frontend buildToolPrompt()    | Built in backend get_system_prompt() |
| **MCP Servers**       | Web Workers (JS/TS)                    | Rust native + stdio                  |

---

## Code Evidence

### V2 Tool Execution (Rust Backend)

```rust
// src-tauri/src/agent/tools.rs

pub async fn collect_builtin_tools(session_id: &str) -> Vec<MCPTool> {
    if let Some(proxy) = AGENT_SESSION_MANAGER.get_proxy(session_id).await {
        let builtin_tool_ids = proxy.builtin_tool_ids();

        for tool_id in builtin_tool_ids {
            let server_tools = proxy.get_builtin_server_tools(&tool_id);
            tools.extend(server_tools);
        }
    }
    tools
}
```

### V2 Service Context (Rust Backend)

```rust
// src-tauri/src/agent/llm.rs

fn build_system_prompt() -> String {
    let contexts = service_proxy.collect_service_contexts().await;

    for (tool_id, service_context) in contexts {
        parts.push(service_context.context_prompt);  // From Rust builtin server
    }

    parts.join("\n\n")
}
```

---

## Decision

**No AgentBuiltInToolProvider needed** because:

1. ✅ V2 tools are configured in `agent_config.tools` during session creation
2. ✅ Tools are executed via `agent_session_call_tool(sessionId, toolCall)` Rust command
3. ✅ Service contexts are built by backend via `MCPServiceProxy::collect_service_contexts()`
4. ✅ Each session has isolated `MCPServiceProxy` with its own builtin servers
5. ✅ No frontend tool registration required

**WebMCP services are V1-only**:

- `WebMCPServiceRegistry` → V1 only, remove with V1 Chat
- `BrowserToolProvider` → V1 only (V2 uses Rust browser builtin)
- `RustMCPToolProvider` → V1 only (V2 has direct backend access)

---

## Next Steps

**Phase 1-7 (V1 Chat Removal)** can proceed without Phase 0:

1. ✅ V2 already has complete tool infrastructure
2. ✅ V2 and V1 use completely separate tool systems
3. ✅ No shared dependencies between V1 and V2 tool providers
4. ✅ V1 removal will not affect V2 functionality

**Simply remove V1 components:**

- `src/features/chat/` (entire feature)
- `src/context/ChatContext.tsx`
- `src/context/SessionContext.tsx` (after confirming no V2 usage)
- `src/features/tools/` (all V1-only providers)

---

**Estimated Time Saved**: 4-6 hours (Phase 0 not needed)  
**Risk**: None - V2 already production-ready  
**Status**: ✅ Ready to proceed with V1 removal
