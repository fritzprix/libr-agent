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
- **Convention:** Use `camelCase` and `verbNoun` structure (e.g., `addContent`, `keywordSimilaritySearch`).

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
- [ ] **Vocabulary:** Descriptions use "Extract/Target" instead of "Copy/Click".
- [ ] **Testing:** Unit tests exist for Error Guidance formatting.
