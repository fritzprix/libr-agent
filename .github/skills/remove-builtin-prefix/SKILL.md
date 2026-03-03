---
name: remove-builtin-prefix
description: >
  Resumable execution guide for the "remove builtin_ prefix" refactoring on the
  refactor/remove-builtin-prefix branch. Use this skill when asked to continue,
  resume, or execute any phase of this refactoring. The skill contains the
  current state of every phase, exact files to touch, and precise mechanical
  changes required so work can be resumed after a context reset.
---

# Remove builtin\_ Prefix Refactoring

## Context

Builtin MCP tool names currently use the pattern `builtin_<service>__<tool>`
(e.g. `builtin_browser__fetch`). The `builtin_` prefix is the sole routing
discriminator inside `routing.rs`. The LLM sometimes drops the prefix and
calls `browser__fetch`, which fails with "Server not found: browser".

Goal: Replace prefix-based routing with `BuiltinServiceId::from_alias()`
so names become `<service>__<tool>` (e.g. `browser__fetch`). The prerequisite
is guaranteeing that no external MCP server can ever be registered with a name
that collides with a builtin service alias.

Branch: `refactor/remove-builtin-prefix`

Full plan doc: `docs/refactoring/remove-builtin-prefix.md`

---

## Resume Protocol

Before doing anything else, run:

```powershell
git checkout refactor/remove-builtin-prefix
```

Then check which phases are still pending:

```powershell
Select-String -Path "src-tauri\src\**\*.rs" -Pattern '"builtin_[a-z]' -Recurse | Measure-Object | Select-Object Count
Select-String -Path "src\**\*.ts","src\**\*.tsx" -Pattern '"builtin_[a-z]' -Recurse | Measure-Object | Select-Object Count
```

Read the counts, then pick up from the first incomplete phase below.

---

## Phase Status Tracker

Update checkboxes as phases complete. Re-verify each phase with its grep check.

- [ ] Phase 1 - Reserved name enforcement
- [ ] Phase 2 - Core routing change
- [ ] Phase 3 - Builtin server dual match-arm cleanup
- [ ] Phase 4 - Hardcoded logic strings (Rust)
- [ ] Phase 5 - Error/hint message strings (Rust)
- [ ] Phase 6 - Integration tests (Rust)
- [ ] Phase 7 - Frontend TypeScript utilities
- [ ] Phase 8 - Frontend tests
- [ ] Final - Build and validate

---

## Phase 1 - Reserved Name Enforcement

Verify (pass = phase complete):

```powershell
Select-String -Path "src-tauri\src\mcp\builtin\mcp_manager\operations.rs" -Pattern "from_alias" | Select-Object LineNumber, Line
Select-String -Path "src-tauri\src\commands\mcp_server_config_commands.rs" -Pattern "from_alias" | Select-Object LineNumber, Line
```

Both must return a result.

### File: src-tauri/src/mcp/builtin/mcp_manager/operations.rs

Find `register_server` (and `update_server`). After the existing name-empty check, insert:

```rust
if BuiltinServiceId::from_alias(&name).is_some() {
    return Err(format!(
        "Server name '{}' is reserved for a builtin service. Choose a different name.",
        name
    ));
}
```

Add import at top of file if not present:

```rust
use crate::mcp::builtin::service_id::BuiltinServiceId;
```

### File: src-tauri/src/commands/mcp_server_config_commands.rs

Same guard in `create_mcp_server_config` and `update_mcp_server_config`:

```rust
if BuiltinServiceId::from_alias(&config.name).is_some() {
    return Err(format!(
        "Server name '{}' is reserved for a builtin service.",
        config.name
    ));
}
```

---

## Phase 2 - Core Routing Change

Verify (pass = phase complete):

```powershell
Select-String -Path "src-tauri\src\mcp\service_proxy\routing.rs" -Pattern "is_builtin_tool_name|BUILTIN_PREFIX" | Measure-Object | Select-Object Count
```

Must return Count 0.

### File: src-tauri/src/mcp/service_proxy/routing.rs

Replace the import at top:

```rust
// OLD:
// use crate::mcp::builtin::service_id::{is_builtin_tool_name, parse_builtin_tool_name};

// NEW:
use crate::mcp::builtin::service_id::BuiltinServiceId;
```

Replace the entire route_tool function body:

```rust
pub fn route_tool(tool_name: &str) -> Result<ToolRouting, String> {
    let (server_name, real_tool_name) = tool_name.split_once("__").ok_or_else(|| {
        format!("Invalid tool name format (expected server__tool): {}", tool_name)
    })?;

    if server_name.is_empty() {
        return Err(format!("Invalid tool name (empty server name): {}", tool_name));
    }
    if real_tool_name.is_empty() {
        return Err(format!("Invalid tool name (empty tool name): {}", tool_name));
    }

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

Update tests in the same file:

- `"builtin_attachments__addContent"` -> `"attachments__addContent"` (still expect Builtin)
- `test_invalid_tool_name`: remove the `"builtin_"` assertion; it is now valid External

### File: src-tauri/src/mcp/service_proxy/mod.rs

Find `get_builtin_server_tools`. Change tool name assembly:

```rust
// OLD:
// let full_name = builtin_tool_name(service_id.name(), &tool.name);

// NEW:
let full_name = format!("{}__{}", service_id.name(), &tool.name);
```

Remove any inject*routing_correction / fallback-retry block that re-adds `builtin*`
to a failed tool call.

### File: src-tauri/src/server/mcp_handler.rs

- Delete `strip_builtin_prefix()` function entirely.
- Delete `resolve_tool_name()` function entirely.
- In `handle_tools_list()`: remove the `.map(...)` call that applies strip_builtin_prefix.
- In `handle_tools_call()`: remove resolve_tool_name call; use `call.name` directly.

### File: src-tauri/src/mcp/builtin/service_id.rs

Remove these four items (keep enum, from_alias, name, Display impl):

- `pub const BUILTIN_PREFIX`
- `pub fn builtin_tool_name`
- `pub fn is_builtin_tool_name`
- `pub fn parse_builtin_tool_name`

---

## Phase 3 - Builtin Server Dual Match-Arm Cleanup

Verify (pass = phase complete):

```powershell
Select-String -Path "src-tauri\src\mcp\builtin\**\*.rs" -Pattern '"builtin_[a-z_]+__' -Recurse | Measure-Object | Select-Object Count
```

Must return Count 0.

Each builtin server's call_tool() has alias match arms like:

```rust
"addTodo" | "builtin_planning__addTodo" => { ... }
```

Remove the `| "builtin_planning__addTodo"` part everywhere. Find all occurrences:

```powershell
Select-String -Path "src-tauri\src\mcp\builtin\**\*.rs" -Pattern '"builtin_[a-z_]+__' -Recurse | Select-Object Filename, LineNumber, Line
```

Typical files: planning/mod.rs, ui/mod.rs, playbook/mod.rs, knowledge/mod.rs.

---

## Phase 4 - Hardcoded Logic Strings (Rust)

Verify (pass = phase complete):

```powershell
Select-String -Path "src-tauri\src\agent\**\*.rs" -Pattern '"builtin_[a-z]' -Recurse | Measure-Object | Select-Object Count
```

Must return Count 0.

File: src-tauri/src/agent/llm/circuit_breaker.rs

```text
"builtin_ui__circuitBreak"          -> "ui__circuitBreak"
"builtin_planning__clearScratchpad" -> "planning__clearScratchpad"
"builtin_planning__checkTodo"       -> "planning__checkTodo"
"builtin_swarm__healthCheck"        -> "swarm__healthCheck"
"builtin_workspace__readFile"       -> "workspace__readFile"
```

File: src-tauri/src/agent/llm/response.rs

```text
"builtin_ui__circuitBreak"  ->  "ui__circuitBreak"
```

---

## Phase 5 - Error/Hint Message Strings (Rust)

Verify (pass = phase complete):

```powershell
Select-String -Path "src-tauri\src\mcp\builtin\**\*.rs" -Pattern '"builtin_[a-z]' -Recurse | Measure-Object | Select-Object Count
```

Must return Count 0.

Find all remaining strings in builtin server files (AI-visible hints):

```powershell
Select-String -Path "src-tauri\src\mcp\builtin\**\*.rs" -Pattern '"builtin_[a-z]' -Recurse | Select-Object Filename, LineNumber, Line
```

Replace each `"builtin_xxx__yyy"` with `"xxx__yyy"`.

---

## Phase 6 - Integration Tests (Rust)

Verify (pass = phase complete):

```powershell
Select-String -Path "src-tauri\src\mcp\integration_tests.rs" -Pattern '"builtin_[a-z]' | Measure-Object | Select-Object Count
```

Must return Count 0.

All `call_tool("session-id", "builtin_xxx__yyy", ...)` -> `call_tool("session-id", "xxx__yyy", ...)`.

---

## Phase 7 - Frontend TypeScript Utilities

Verify (pass = phase complete):

```powershell
Select-String -Path "src\lib\tool-call-utils.ts","src\lib\utils.ts","src\lib\message-preprocessor.ts","src\features\agent\api\agent-backend.ts" -Pattern 'builtin_[a-z]' | Measure-Object | Select-Object Count
```

Must return Count 0.

### src/lib/tool-call-utils.ts

```typescript
// Canonical builtin service aliases -- must mirror BuiltinServiceId::from_alias() in Rust
const BUILTIN_SERVICE_NAMES = new Set([
  'planning',
  'workspace',
  'knowledge',
  'assistant',
  'skills',
  'playbook',
  'attachments',
  'content_store',
  'contentstore',
  'swarm',
  'session_api',
  'ui',
  'browser',
  'bootstrap',
  'mcp_manager',
]);

// OLD: name.startsWith('builtin_')
// NEW:
export function isBuiltinTool(name: string): boolean {
  const server = name.split('__')[0];
  return BUILTIN_SERVICE_NAMES.has(server);
}

// OLD: strips "builtin_" prefix then splits on "__"
// NEW: just splits on "__"
export function parseBuiltinToolName(
  name: string,
): { serviceId: string; toolName: string } | null {
  const idx = name.indexOf('__');
  if (idx === -1) return null;
  const serviceId = name.slice(0, idx);
  const toolName = name.slice(idx + 2);
  if (!BUILTIN_SERVICE_NAMES.has(serviceId) || !toolName) return null;
  return { serviceId, toolName };
}
```

### src/lib/utils.ts - extractBuiltInServiceAlias()

```typescript
// OLD: strips "builtin_" then takes part before "__"
// NEW: takes part before "__" directly
export function extractBuiltInServiceAlias(toolName: string): string | null {
  const server = toolName.split('__')[0];
  return BUILTIN_SERVICE_NAMES.has(server) ? server : null;
}
```

### src/features/agent/api/agent-backend.ts

```text
'builtin_attachments__addContent'  ->  'attachments__addContent'
```

### src/lib/message-preprocessor.ts

```text
'builtin_workspace__readFile'  ->  'workspace__readFile'
```

---

## Phase 8 - Frontend Tests

Verify (pass = phase complete):

```powershell
Select-String -Path "src\**\*.test.ts","src\**\*.test.tsx","src\**\*.spec.ts" -Pattern 'builtin_[a-z]' -Recurse | Measure-Object | Select-Object Count
```

Must return Count 0.

Find all occurrences:

```powershell
Select-String -Path "src\**\*.test.ts","src\**\*.spec.ts" -Pattern 'builtin_[a-z]' -Recurse | Select-Object Filename, LineNumber, Line
```

Key files:

- src/lib/**tests**/tool-call-utils.test.ts
- src/lib/**tests**/utils.extractBuiltInServiceAlias.test.ts
- src/lib/ai-service/**tests**/gemini.tool-pairing.test.ts
- src/lib/**tests**/message-preprocessor.test.ts

---

## Final Verification

```powershell
# 1. No more builtin_ in Rust logic
Select-String -Path "src-tauri\src\**\*.rs" -Pattern '"builtin_[a-z]' -Recurse

# 2. No more builtin_ in TS logic
Select-String -Path "src\**\*.ts","src\**\*.tsx" -Pattern '"builtin_[a-z]' -Recurse

# 3. Rust build
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml

# 4. Full stack validation
pnpm refactor:validate
```

---

## Key Invariants

1. `BuiltinServiceId::from_alias()` is the single source of truth for "is this a builtin?".
   Both Rust routing and TS `isBuiltinTool()` delegate to it (or a mirror set).
2. mcp_handler.rs external HTTP API: after Phase 2, no name translation needed.
   Internal and external names are now identical.
3. DB `allowedBuiltInServiceAliases` stores bare service names ("browser", "planning").
   Never stored the builtin\_ prefix -- no DB migration needed.
4. The Phase Status Tracker checkboxes are the authoritative progress state.
   Update them as each phase completes.
