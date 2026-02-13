# Error Handling Unification Refactoring Plan (Builtin + External MCP)

**Date:** 2026-02-12  
**Branch:** `resolve-assistant-id-14444470464130912136`  
**Scope:** Rust backend error handling for:

- Built-in MCP tool service framework (`src-tauri/src/mcp/builtin/**`)
- External MCP server integration (stdio + HTTP) (`src-tauri/src/mcp/**`)

This plan is based on repo architecture docs and code entrypoint discovery (notably `MCPServiceProxy`, external managers, and builtin error helper usage).

---

## 0) Goals and non-goals

### Goals

1. **Agent-recoverable errors**: Every tool failure should tell the agent what happened _and_ what to do next.
2. **Consistency**: Builtin + external tool failures should share a consistent error “shape” in **agent-visible text**.
3. **Normalization**: External errors (stdio/HTTP/protocol/remote-tool) should be mapped into a small, stable taxonomy.
4. **Guardrails**: Prevent regressions:
   - Recovery hints must stay in the same tool group.
   - Critical identifiers (IDs/handles) must appear in text (`content`) not only in JSON.
5. **No silent deprecations**: Deprecated external MCP command paths must not silently return empty lists or ambiguous results.

### Non-goals

- No major protocol changes to rmcp.
- No “did you mean” tool aliasing (canonical naming remains the rule).
- No sweeping rewrite of all tools at once; refactor in phases with tests and compatibility.

---

## 1) Current state summary (evidence)

### Builtin tools

- Multiple builtin services already use shared error helpers:
  - `missing_param_error(...)`
  - `operation_failed_error(...)`
  - `not_found_error(...)`
  - `ToolGroup::*` scoping

Examples found under:

- `src-tauri/src/mcp/builtin/ui/mod.rs`
- `src-tauri/src/mcp/builtin/browser/*`
- `src-tauri/src/mcp/builtin/playbook/*`
- `src-tauri/src/mcp/builtin/workspace/*`

**Strength:** Tools like Workspace implement layered validation + actionable recovery guidance.

**Risk:** Consistency is convention-based (easy to drift); some success/error patterns vary per tool.

### External MCP

Key routing/entrypoints:

- `src-tauri/src/mcp/service_proxy.rs` (`MCPServiceProxy::call_tool`)
- `src-tauri/src/mcp/server/mod.rs` and `src-tauri/src/mcp/server/tools.rs`
- Session isolation managers:
  - `src-tauri/src/mcp/session_isolation/http_manager.rs`
  - `src-tauri/src/mcp/session_isolation/stdio_manager.rs`

**Risk:** External error modes vary widely (transport/protocol/remote tool errors/HTTP 404). Without explicit normalization, agent recovery becomes inconsistent.

### Deprecated global MCP manager command path

In `src-tauri/src/commands/mcp_commands.rs`:

- warnings about deprecated global manager
- `println!` usage
- `list_tools_from_config` explicitly deprecated and returns empty list

**Risk:** Multiple public surfaces produce inconsistent behavior; silent empty results are particularly harmful.

---

## 2) Target contract: agent-visible error text

Agents primarily “see” tool outputs via the `content` field; JSON fields like `structured_content` are UI-only.

### Required agent-visible error sections

Every tool failure message should include:

- **Operation**: short label of what was attempted
- **Source**: `Builtin(<service>)` or `External(<server>)`
- **Category**: one of a stable set (see taxonomy)
- **Cause**: one-line summary
- **Recovery**: 1..N bullet steps with copy-pastable next calls when possible

### Example (target format)

```text
Operation: Call External Tool
Source: External(filesystem)
Category: Transport
Cause: failed to spawn stdio process (ENOENT)

Recovery:
- Verify the server command exists and is executable
- Restart the MCP server: startServer("filesystem")
- List tools to confirm availability: listTools("filesystem")
```

This is intentionally text-first. Structured JSON can still be included for UI rendering.

### UI contract (required for clean error grouping)

In addition to text formatting, the UI needs a **stable machine-readable signal** to:

1. start a new group when a tool result failed, and
2. group consecutive failures into a dedicated "error group" with warning/red semantics.

We will use the chat message metadata field:

- `message.role === 'tool'`
- `message.metadata.toolError === true` for failed tool result messages

This avoids brittle heuristics like parsing tool output text.

**Backend-first requirement:** the UI grouping behavior will not activate until the backend sets `metadata.toolError` on failed tool results.

#### Mapping rules (two classes of errors)

- **#1 Agent/tool recoverable errors** (invalid tool call, validation failures, tool-side logic errors)
  - Represent as normal tool result messages (`role: 'tool'`)
  - Set `message.metadata.toolError = true`
  - Put detailed guidance in tool result text (user can see tool calls/results)
  - Do **not** escalate to global "service error" UI

- **#2 User-recoverable provider/system errors** (LLM provider integration issues: broken JSON/tool-use format, empty response, auth, rate limits)
  - Represent as `Message.error` (or global error state) with a small taxonomy + retry
  - Keep `displayMessage` non-technical
  - Always show Retry button

---

## 3) Unified taxonomy (builtin + external)

### Proposed categories

#### Shared / general

- `InvalidInput`
- `NotFound`
- `PermissionDenied`
- `InvalidState`
- `Internal`

#### External-specific

- `Transport` (spawn/connect/broken pipe)
- `Protocol` (unexpected JSON-RPC / schema / decode)
- `RemoteToolError` (server returned `isError=true`)
- `SessionExpired` (HTTP 404/session invalidation)
- `Timeout`

### Mapping

- Builtin `ErrorCategory::*` should map into the shared/general buckets.
- External errors should be normalized into the external-specific buckets.

---

## 4) Phased refactoring plan

### Phase 0 — Add contract + formatting utilities (no behavior change)

**Goal:** Establish reusable formatters and the taxonomy types.

#### Phase 0 Deliverables

- Add a small module for error formatting and taxonomy (location TBD; likely `src-tauri/src/mcp/errors/` or `src-tauri/src/mcp/error_normalization.rs`).
- Provide functions like:
  - `format_tool_error_text(...)`
  - `format_external_error_text(...)`

#### Phase 0 Acceptance criteria

- No tool logic changes yet.
- Can generate canonical error text from inputs.

---

### Phase 1 — Builtin framework hardening

**Goal:** Make builtin tools consistently emit contract-compliant errors.

#### Phase 1 Work items

1. **Centralize builtin error construction**
   - Introduce a single “canonical” builtin error helper that wraps existing helpers but standardizes text.
   - Keep existing helper functions for compatibility, but migrate call sites incrementally.

2. **Guardrail: ToolGroup hint isolation**
   - Add a debug/test-time validator that checks recovery hints only reference tools expected for that `ToolGroup`.
   - Lightweight string checks are sufficient.

3. **Guardrail: IDs must be in agent-visible text**
   - For operations that return important IDs in JSON, ensure the ID is also included in `content`.
   - Add unit tests to prevent regression.

4. **Service context cache invalidation**
   - Standardize: any state change in a builtin server should invalidate service context cache.
   - Workspace already has strong patterns; extend consistency.

#### Phase 1 Acceptance criteria

- Builtin errors always include Recovery section.
- No cross-tool-group hint leakage in tests.

---

### Phase 2 — External MCP error normalization

**Goal:** Normalize stdio/HTTP/protocol/tool errors into a stable taxonomy and consistent text contract.

#### Phase 2 Work items

1. **Normalization layer**
   - Add a single normalization function that takes (server, tool, error) and returns:
     - category
     - canonical agent-visible text
     - optional structured JSON

2. **HTTP SessionExpired policy (404 / invalid session)**
   - Centralize a bounded retry policy:
     1. detect session expired
     2. mark invalid
     3. reconnect/re-init once
     4. retry original operation once
     5. if still failing, return `SessionExpired` error with recovery steps

3. **Tool-not-found ergonomics**
   - Ensure external tool-not-found errors include:
     - attempted server/tool
     - explicit recovery: list tools for that server

#### Phase 2 Acceptance criteria

- All external errors include `(server, tool)` in text.
- SessionExpired includes bounded retry and clear recovery steps.

---

### Phase 3 — Deprecated command surface quarantine/removal

**Goal:** Remove or harden deprecated global MCP manager paths so they can’t silently mislead.

#### Phase 3 Options (pick one)

- **A (breaking):** remove deprecated commands.
- **B (compat):** route deprecated commands through the same normalization and session-isolated managers.
- **C (safe):** hard-error with explicit migration guidance (no silent empty list).

#### Phase 3 Must-fix regardless

- Replace `println!` with logger
- Never return empty list as a deprecated placeholder

#### Phase 3 Acceptance criteria

- No deprecated MCP command returns “success-like” empty outputs.

---

### Phase 4 — Tests

**Goal:** Treat error text as a public API for the agent.

#### Phase 4 Builtin test cases

- missing parameter -> includes `Category:` and `Recovery:`
- invalid ID -> includes correct “list…” hint in same ToolGroup
- critical IDs appear in text

#### Phase 4 External test cases

- transport failure normalized as `Transport`
- tool not found normalized with list-tools recovery
- SessionExpired policy: categorized and bounded retry

#### Phase 4 Acceptance criteria

- Tests fail if contract is violated.

---

### Phase 5 — Validation gate

- Run `pnpm refactor:validate`
- Run targeted `cargo test` (mcp-related tests)

#### Phase 5 Acceptance criteria

- Lint/build/tests pass.

---

## 5) Clarifications needed (decisions that affect implementation)

1. **External errors as tool-shaped results vs Rust `Err(String)`**
   - Should external failures be converted into an MCP tool-style error result whenever possible (preferred for agent recovery), or should they abort the call chain?

2. **HTTP re-init mechanism**
   - Do we have a supported rmcp-level “initialize/reinitialize session” hook we should call, or should we implement reconnect as “recreate transport/client and retry once”?

3. **Deprecated command path strategy**
   - For `src-tauri/src/commands/mcp_commands.rs`, should we remove, hard-error, or transparently route through the new session-isolated path?

---

## 6) Suggested implementation order (small PRs)

1. Phase 0 utilities + taxonomy types
2. One builtin server migration as exemplar (Workspace or UI)
3. External normalization layer + tests
4. Migrate remaining builtin servers incrementally
5. Deprecation cleanup in `mcp_commands.rs`

---

## Appendix: High-leverage files

- Builtin tool implementations:
  - `src-tauri/src/mcp/builtin/**`

- External routing:
  - `src-tauri/src/mcp/service_proxy.rs`
  - `src-tauri/src/mcp/server/mod.rs`
  - `src-tauri/src/mcp/server/tools.rs`
  - `src-tauri/src/mcp/session_isolation/http_manager.rs`
  - `src-tauri/src/mcp/session_isolation/stdio_manager.rs`

- Deprecated command path:
  - `src-tauri/src/commands/mcp_commands.rs`
