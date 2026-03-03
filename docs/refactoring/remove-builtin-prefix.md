# Refactoring: Remove `builtin_` Prefix from Tool Names

**Branch:** `refactor/remove-builtin-prefix`  
**Date:** 2026-03-03  
**Status:** 🔄 In Progress

---

## Background

Currently all builtin MCP tool names are exposed to the LLM with a `builtin_` prefix:

- `builtin_browser__fetch`
- `builtin_planning__addTodo`
- `builtin_workspace__readFile`

The prefix was used as the **sole routing discriminator** in `routing.rs`:

- starts with `builtin_` → `BuiltinServiceId` → builtin server
- otherwise → external MCP server

### Why Remove It?

1. **LLM hallucination**: AI sometimes drops the prefix and calls `browser__fetch` instead of
   `builtin_browser__fetch`, causing `Server not found: browser` errors (confirmed in prod trace).
2. **Redundant**: `BuiltinServiceId::from_alias()` already knows every builtin group name.
   If external server names are guaranteed to never collide with builtin names, routing can use
   `from_alias()` instead of the prefix.
3. **Cleaner API**: `browser__fetch` is simpler and less error-prone for the LLM.

### Prerequisite: Name Collision Must Be Prevented

Without the prefix, `browser__fetch` is ambiguous unless we guarantee no external server is
ever named `browser`, `planning`, `workspace`, etc.

**Three registration paths must all enforce this:**

| Path                                                                                | Status              |
| ----------------------------------------------------------------------------------- | ------------------- |
| UI Form (`useMCPServerForm.ts`) — `RESERVED_BUILTIN_NAMES`                          | ✅ Already enforced |
| AI agent `registerServer` tool (`mcp_manager/operations.rs`)                        | ❌ Must add         |
| Tauri command `create_mcp_server_config` (`commands/mcp_server_config_commands.rs`) | ❌ Must add         |

---

## New Routing Logic (after refactor)

```rust
// routing.rs — NEW
pub fn route_tool(tool_name: &str) -> Result<ToolRouting, String> {
    let (server_name, real_tool_name) = tool_name
        .split_once("__")
        .ok_or_else(|| format!("Invalid tool name format: {}", tool_name))?;

    if let Some(service_id) = BuiltinServiceId::from_alias(server_name) {
        Ok(ToolRouting::Builtin {
            server_id: service_id.name().to_string(),
            tool_name: real_tool_name.to_string(),
        })
    } else {
        Ok(ToolRouting::External {
            server_name: server_name.to_string(),
            tool_name: real_tool_name.to_string(),
        })
    }
}
```

---

## Execution Plan

### Phase 1 — Prerequisites: Enforce Reserved Names ⬜

**Goal:** Guarantee external servers can never be named after a builtin group.

- [ ] `src-tauri/src/mcp/builtin/mcp_manager/operations.rs`
  - `register_server()`: add `BuiltinServiceId::from_alias(&name)` check → return guided error
  - `update_server()`: same check
- [ ] `src-tauri/src/commands/mcp_server_config_commands.rs`
  - `create_mcp_server_config()`: add same check
  - `update_mcp_server_config()`: add same check (when name is being changed)

---

### Phase 2 — Core Routing Change ⬜

**Goal:** Replace prefix-based routing with `BuiltinServiceId::from_alias()`.

- [ ] `src-tauri/src/mcp/service_proxy/routing.rs`
  - Rewrite `route_tool()` as shown above
  - Remove `is_builtin_tool_name` / `parse_builtin_tool_name` dependency
  - Update tests (remove `builtin_` from test strings)

- [ ] `src-tauri/src/mcp/service_proxy/mod.rs`
  - `get_builtin_server_tools()`: remove `builtin_tool_name()` call, return `server_id__tool_name` directly
  - Remove `inject_routing_correction()` fallback (no longer needed — prefix hallucination is now handled naturally)
  - Update all doc comments referencing `builtin_xxx__yyy`

- [ ] `src-tauri/src/server/mcp_handler.rs`
  - Delete `strip_builtin_prefix()` function (no prefix to strip anymore)
  - Delete `resolve_tool_name()` function (no longer needed)
  - `handle_tools_list()`: remove `.map(|mut t| { t.name = strip_builtin_prefix(&t.name); t })`
  - `handle_tools_call()`: remove `resolve_tool_name()` call, use `call.name` directly

- [ ] `src-tauri/src/mcp/builtin/service_id.rs`
  - Remove `BUILTIN_PREFIX` constant
  - Remove `builtin_tool_name()` function
  - Remove `is_builtin_tool_name()` function
  - Remove `parse_builtin_tool_name()` function
  - Keep `BuiltinServiceId` enum, `from_alias()`, `name()` — these are still needed

---

### Phase 3 — Builtin Server `call_tool()` Match Cleanup ⬜

**Goal:** Remove the `"builtin_xxx__toolName" |` alias arms (they were defensive fallbacks).

Each builtin server's `call_tool()` currently has:

```rust
"addTodo" | "builtin_planning__addTodo" => { ... }
```

After the refactor `call_tool()` is always called with the short name, so aliases can be removed:

```rust
"addTodo" => { ... }
```

Files to update:

- [ ] `src-tauri/src/mcp/builtin/planning/mod.rs`
- [ ] `src-tauri/src/mcp/builtin/ui/mod.rs`
- [ ] `src-tauri/src/mcp/builtin/playbook/mod.rs`
- [ ] `src-tauri/src/mcp/builtin/knowledge/mod.rs`
- [ ] `src-tauri/src/mcp/builtin/browser/mod.rs` (already short names only — verify)
- [ ] `src-tauri/src/mcp/builtin/workspace/mod.rs` (already short names only — verify)
- [ ] `src-tauri/src/mcp/builtin/assistant/mod.rs` (verify)
- [ ] `src-tauri/src/mcp/builtin/mcp_manager/mod.rs` (verify)
- [ ] `src-tauri/src/mcp/builtin/content_store/mod.rs` (verify)
- [ ] `src-tauri/src/mcp/builtin/skills/mod.rs` (verify)
- [ ] `src-tauri/src/mcp/builtin/bootstrap/mod.rs` (verify)
- [ ] `src-tauri/src/mcp/builtin/session_api/mod.rs` (verify)

---

### Phase 4 — Hardcoded Tool Name Strings (Rust) ⬜

**Goal:** Update all hardcoded `"builtin_xxx__yyy"` string literals in logic code.

- [ ] `src-tauri/src/agent/llm/circuit_breaker.rs`
  - `"builtin_ui__circuitBreak"` → `"ui__circuitBreak"`
  - `"builtin_planning__clearScratchpad"` → `"planning__clearScratchpad"`
  - `"builtin_planning__checkTodo"` → `"planning__checkTodo"`
  - `"builtin_swarm__healthCheck"` → `"swarm__healthCheck"`
  - `"builtin_workspace__readFile"` → `"workspace__readFile"`

- [ ] `src-tauri/src/agent/llm/response.rs`
  - `"builtin_ui__circuitBreak"` → `"ui__circuitBreak"`

---

### Phase 5 — Error/Hint Message Strings ⬜

**Goal:** Update AI-visible hint messages so the AI learns the new names.

- [ ] `src-tauri/src/mcp/builtin/error_guidance.rs`
  - All `"builtin_assistant__xxx"` references in hint strings
- [ ] `src-tauri/src/mcp/builtin/assistant/operations.rs`
  - All hint strings referencing `builtin_xxx__yyy`
- [ ] `src-tauri/src/mcp/builtin/assistant/queries.rs`
  - All hint strings referencing `builtin_xxx__yyy`

> **Note:** This is a mechanical string replacement. Use grep to find all instances.
> `grep -rn "builtin_[a-z_]*__" src-tauri/src --include="*.rs" | grep -v "//\s*builtin_" | grep "\""`

---

### Phase 6 — Integration Tests (Rust) ⬜

**Goal:** Update test call_tool() invocations to use new names.

- [ ] `src-tauri/src/mcp/integration_tests.rs`
  - All `call_tool("...", "builtin_xxx__yyy", ...)` → `call_tool("...", "xxx__yyy", ...)`

---

### Phase 7 — Frontend TypeScript ⬜

**Goal:** Update frontend utilities that parse/detect builtin tool names.

- [ ] `src/lib/tool-call-utils.ts`
  - `isBuiltinTool(name)`: change from `name.startsWith("builtin_")` to `BuiltinServiceId`-style set check
  - `parseBuiltinToolName()`: update parsing logic
  - Update JSDoc examples

- [ ] `src/lib/utils.ts`
  - `extractBuiltInServiceAlias()`: update to handle new naming scheme

- [ ] `src/features/agent/api/agent-backend.ts`
  - `'builtin_attachments__addContent'` → `'attachments__addContent'`

- [ ] `src/lib/message-preprocessor.ts`
  - Hint string: `builtin_workspace__readFile` → `workspace__readFile`

- [ ] `src/features/mcp-servers/hooks/useMCPServerForm.ts`
  - `RESERVED_BUILTIN_NAMES` set — no change needed (still needed for collision prevention)
  - Error message text if it references `builtin_` prefix

---

### Phase 8 — Frontend Tests ⬜

**Goal:** Update test fixtures and assertions.

- [ ] `src/lib/__tests__/tool-call-utils.test.ts`
- [ ] `src/lib/__tests__/utils.extractBuiltInServiceAlias.test.ts`
- [ ] `src/lib/ai-service/__tests__/gemini.tool-pairing.test.ts`
- [ ] `src/lib/__tests__/message-preprocessor.test.ts`

---

## Verification Checklist

After all phases:

```sh
# Rust: no more builtin_ in logic (only in comments/docs is OK)
grep -rn '"builtin_[a-z]' src-tauri/src --include="*.rs"

# TS: no more builtin_ in logic
grep -rn '"builtin_[a-z]' src --include="*.ts" --include="*.tsx"

# Build
pnpm build
cargo clippy -- -D warnings
cargo test

# Full validation
pnpm refactor:validate
```

---

## Key Invariants to Preserve

1. **`BuiltinServiceId::from_alias()` is the single source of truth** for "is this a builtin?"
2. **All three registration paths must reject reserved names** (UI form, AI tool, Tauri command)
3. **`mcp_handler.rs` external API** (`/mcp/{session_id}`) — after refactor, no translation needed since internal and external names are identical
4. **`service_id.rs`** keeps `BuiltinServiceId` enum and `from_alias()` / `name()` — only the prefix-related helpers are removed

---

## Files NOT Changing

- `src-tauri/src/mcp/builtin/service_id.rs` — `BuiltinServiceId` enum itself (only helpers removed)
- DB schema — no migration needed, `allowedBuiltInServiceAliases` stores service names like `"browser"`, `"planning"` (no prefix)
- `src/lib/assistant/runtime-builtins.ts` — already uses unprefixed service aliases
