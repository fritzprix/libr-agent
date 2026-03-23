This is the **Built-in MCP Tool Design Standard (v4.0)**.

I have restored critical architectural patterns from the original document—specifically **Canonical Naming**, **Service Context Injection**, **Session Isolation**, and **Testing Protocols**—while maintaining the strict "Zero Trust / Dual Channel" core.

---

# LibrAgent Built-in MCP Tool Design Standard

**Version:** 4.0 (Final)
**Scope:** All built-in tools (Browser, Planning, Workspace, etc.)

## 1. Core Philosophy

We design tools for an entity that has **High Reasoning** capabilities but **Zero Memory**, **No Body**, and **Untrusted Output**.

1. **Cognitive Ergonomics:** The Agent is blind to JSON. It only "sees" text descriptions and text responses.
2. **Zero Trust Architecture:** The Agent is an unreliable user. Validate everything (especially IDs) against the database before execution.
3. **Course Correction:** Errors are not failures; they are navigational aids.

---

## 2. Architectural Foundations

### 2.1 Canonical Naming (The "No Alias" Rule)

**Rule:** Each tool must have exactly **ONE** canonical name.
**Anti-Pattern:** Using aliases like `search` | `find` | `lookup` for the same function.

- **Why:** Aliases dilute the semantic weight of the tool in the System Prompt and confuse the Agent's decision tree.
- **Convention:** Use `camelCase` and `verbNoun` structure (e.g., `add`, `search`).

#### 2.1.1 No Alias Error Hints (Simplicity Rule)

**Rule:** Do NOT provide custom error hints for unmatched tool names in `call_tool()`.

**Anti-Pattern (Over-Engineering):**

```rust
match tool_name {
    "readFile" => self.handle_read_file(...),
    "createFile" => self.handle_create_file(...),

    // ❌ AVOID: Explicit alias handling
    "read_file" | "read" => Ok(MCPResult::error(
        "Did you mean 'readFile'?"
    )),
    "write_file" | "writeFile" => Ok(MCPResult::error(
        "Did you mean 'createFile'?"
    )),

    _ => Err(format!("Tool '{}' not found", tool_name)),
}
```

**Correct Pattern (Simple & Maintainable):**

```rust
match tool_name {
    "readFile" => self.handle_read_file(...),
    "createFile" => self.handle_create_file(...),
    "editFile" => self.handle_edit_file(...),

    // ✅ CORRECT: Generic error for all unmatched names
    _ => Err(format!("Tool '{}' not found", tool_name)),
}
```

**Why This Is Anti-Pattern:**

1. **Maintenance Burden:** Adding 1 tool requires adding N alias error handlers
2. **False Negatives:** Rejects valid camelCase alternatives (`writeFile`, `replaceStringInFile`)
3. **AI Learning Interference:** Agents should learn from tool schemas, not ad-hoc hints
4. **Code Bloat:** 15-20 lines of error handling for what should be 1 line

**Correct Approach:**

- Let the agent receive generic `"Tool 'X' not found"` error
- Agent will consult available tools list (from `list_tools()`)
- Agent learns canonical names from schema definitions

#### 2.1.2 Current Canonical Builtin Service Names (Implemented)

As of the March 2026 cleanup, the active public builtin service canonicals are:

- `planning`
- `scratchpad`
- `workspace`
- `knowledge`
- `agent`
- `skills`
- `playbook`
- `attachments`
- `ui`
- `browser`
- `bootstrap`
- `tool`
- `media`

Legacy names such as `assistant`, `assistant_manager`, `swarm`, and `session_api` still resolve for backward compatibility, but they are **aliases only**. They must not be reintroduced as separate public builtin services.

### 2.2 Trait-Based Interface

All tools must implement the standard `BuiltinMCPServer` trait to ensure polymorphic handling by the Agent Runtime.

```rust
#[async_trait]
pub trait BuiltinMCPServer {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn call_tool(&self, name: &str, args: Value) -> Result<MCPResult, String>;
    // Injects tool state into the system prompt
    async fn get_service_context(&self) -> ServiceContext;
}

```

### 2.3 Session Isolation

**Rule:** One Agent, One State.

- State must be encapsulated in `Arc<RwLock<T>>`.
- Resources (browsers, file handles) must be keyed by `agent_session_id`. **Never** leak state between parallel agent sessions.

---

## 3. Schema & Data Integrity (The "Zero Trust" Rules)

### 3.1 The Immutable ID Rule

**Rule:** The Agent does not define reality; the System does.

- **Creation Schemas:** The `id` field is **STRICTLY FORBIDDEN**.
- _Correct:_ Agent sends `{ "name": "Project X" }` → System returns `{ "id": "proj_123" }`.

- **Update/Delete Schemas:** The `id` field is **MANDATORY**.

### 3.2 The Hallucination Firewall

**Rule:** A foreign key provided by the Agent is guilty until proven innocent.

- **Protocol:** Perform a lightweight `exists()` check on all IDs _before_ any write operation.
- **Response:** If the check fails, return a "Logic Error" with a recovery hint, not a database exception.

```rust
// 🛡️ The Firewall Pattern
if !db.projects.exists(&args.project_id).await? {
    return Ok(operation_failed_error(
        "Create Task",
        &format!("Project ID '{}' invalid", args.project_id),
        vec!["Use 'list_projects' to find the valid ID".to_string()], // Recovery Hint
        ToolGroup::Planning
    ));
}

```

---

## 4. The Response Standard (Text vs. UI)

### 4.1 The Dual-Channel Rule

Every tool returns an `MCPResult` serving two masters:

| Channel       | Field                | Audience      | Rule                                                                            |
| ------------- | -------------------- | ------------- | ------------------------------------------------------------------------------- |
| **Reasoning** | `content`            | **AI Agent**  | **Must be self-sufficient.** Must explicitly state IDs, Status, and Next Steps. |
| **Rendering** | `structured_content` | **Client UI** | Raw JSON for tables/graphs. **Agent never sees this.**                          |

### 4.2 The Narrative Requirement

The `content` text must tell the full story.

- ✅ "Created Project Alpha (ID: `proj_882`)"
- ❌ "Project Created" (Agent asks: "What is the ID?")

---

## 5. Service Context (State Injection)

### 5.1 Context Injection

Tools often hold state (e.g., current directory, active URL) that the Agent needs to know _before_ acting. Use `get_service_context` to inject this into the System Prompt.

**Format:**

```text
## Browser Tool
Active Session: sess_9982
Current URL: https://example.com/docs
Status: Ready

```

### 5.2 Caching Strategy

Context generation (e.g., DOM scraping) can be expensive.

- **Rule:** Implement a short TTL cache (e.g., 5 seconds) for `get_service_context`.
- **Invalidation:** Invalidate cache immediately after any state-changing tool call (e.g., `Maps_to`).

---

## 6. Error Handling: The "Success Hint" Pattern

### 6.1 The Detour Principle

**Rule:** Never return a raw error. Always pair an error with a solution.

### 6.2 Tool Group Isolation

**Rule:** Only suggest tools from the **same domain** (Tool Group).

- _Correct:_ Browser Error → Suggest `scrollPage`.
- _Incorrect:_ Browser Error → Suggest `createTodo` (Planning).

### 6.3 Success/Error Channel Separation

**Rule:** Never mix success hints with error responses.

**Two Distinct Channels:**

- **Error Response:** Recovery guidance only ("How to fix this")
- **Success Response:** Next-action hints only ("What to do next")

**Anti-Pattern (causes misleading hints):**

```rust
// ❌ WRONG: Success hints appear even after operation failure
let result = match execute_script(...) {
    Err(e) => { return Ok(operation_failed_error(...)) }
    Ok(res) => res
};

let hint = SuccessHint::new(result, vec!["Check for page changes"]); // ⚠️ After error!
```

**Correct Pattern:**

```rust
// ✅ CORRECT: Success hints ONLY in success branch
match execute_script(...) {
    Ok(res) => {
        if res.contains("Error") {
            return Ok(operation_failed_error(...)); // ❌ Recovery hints only
        }
        // ✅ Success path: Return next-action hints
        let hint = SuccessHint::new(res, vec!["Check for page changes"]);
        Ok(hint.to_mcp_result())
    }
    Err(e) => Ok(operation_failed_error(...)) // ❌ Recovery only
}
```

**Why This Matters:** If `clickElement` fails due to invalid CSS selector, suggesting "check for page changes" wastes AI tokens on non-existent changes.

### 6.4 Implemented Builtin Error Semantics (March 2026)

The current builtin behavior is now stricter than the older generic "everything failed = error" approach.

#### What uses hard tool errors (`is_error: true`)

Use hard error semantics only when the agent can reasonably fix the problem by changing the tool call:

- missing required parameters
- invalid input / invalid format
- missing resources / wrong IDs
- duplicate-resource conflicts
- explicit permission / ownership violations
- invalid agent-visible state such as "already terminated" or "process belongs to another session"

#### What uses informational non-error results (`is_error: false`)

These cases should stay visible to the agent, but they are **not** agent-fault errors:

- builtin proxy execution timeouts
- builtin internal exceptions wrapped through `guided_error(...)`
- backend operation failures such as temp directory creation, ZIP finalization, process spawn/stdin failures
- user-driven cancellation flows such as `ui / getUserAnswer(cancelled=true)`
- interactive command non-zero exits where the command failed, but the tool call itself was valid

#### Implemented code paths

The current implementation is centered in:

- `src-tauri/src/mcp/types.rs`
  - `MCPResult::informational(...)`
  - `MCPResult::informational_with_data(...)`
- `src-tauri/src/mcp/builtin/error_guidance.rs`
  - `ErrorCategory::uses_error_semantics()`
  - `ErrorGuidance::to_mcp_result()`
- `src-tauri/src/mcp/service_proxy/mod.rs`
  - builtin timeout conversion to non-error tool results

#### Regression coverage

This contract is enforced by integration tests in:

- `src-tauri/tests/error_contract_guards.rs`

Those tests currently pin:

- `guided_error(ErrorCategory::Timeout, ...)` → non-error
- `guided_error(ErrorCategory::InternalError, ...)` → non-error
- session wait timeout conversion → success/informational result
- UI prompt cancellation → non-error informational result

---

## 7. Description Engineering (Input)

### 7.1 AI-Native Vocabulary

Speak to the Agent's functions, not human actions.

| ❌ Avoid (Human) | ✅ Use (AI)   | Reason          |
| ---------------- | ------------- | --------------- |
| "Copy/Paste"     | "Extract/Use" | No clipboard.   |
| "Remember"       | "Reference"   | No memory bank. |
| "Click"          | "Target"      | No mouse.       |

### 7.2 The Prerequisite Contract

Explicitly document dependencies in the description.

> "⚠️ MANDATORY: Call `get_file_info` FIRST. Extract the `file_hash`. Use that hash here."

---

## 8. Performance & Safety

### 8.1 Async Discipline

**Rule:** Never block the runtime.

- Use `spawn_blocking` for CPU-intensive tasks (parsing, crypto).
- Enforce strict input size limits (e.g., max 10MB string input).

### 8.2 Context Economy (Pagination)

**Rule:** Respect the Context Window.

- **Pagination:** Never dump >50 lines of data. Return Page 1 and a prompt: _"Use `read_next_page(2)` for more."_

---

## 9. Testing & Validation Checklist

Before deploying a tool, validation against these checks is mandatory:

- [ ] **Canonical Naming:** Tool has one unique name (no aliases) in camelCase.
- [ ] **Schema Safety:** `id` is removed from all Create schemas.
- [ ] **Firewall:** All input IDs are validated (`db.exists()`) before use.
- [ ] **Dual Channel:** Text output contains all IDs/Status needed for reasoning.
- [ ] **Hints:** Errors provide actionable next steps.
- [ ] **Isolation:** Hints only suggest tools from the same Tool Group.
- [ ] **Channel Separation:** Success hints only appear after confirmed operation success, never in error responses.
- [ ] **Vocabulary:** Descriptions use "Extract/Target" instead of "Copy/Click".
- [ ] **Testing:** Unit tests exist for Error Guidance formatting.
