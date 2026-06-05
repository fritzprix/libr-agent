# MCP Tool Response Design

When implementing MCP tools, understand that AI agents and UI components see different parts of `MCPResult`.

## Data Flow Architecture (LibrAgent-Specific)

```rust
pub struct MCPResult {
    content: Vec<MCPContent>,           // → Standard MCP: AI agents SEE this
    structured_content: Option<Value>,  // → LibrAgent extension: UI components only (agents DON'T)
    is_error: Option<bool>,             // → Standard MCP
}
```

**Important:** `structured_content` is a **non-standard LibrAgent internal extension**. The standard MCP protocol only defines `content` (array of MCPContent items) and `isError` (boolean). We added `structured_content` for LibrAgent's UI components to render rich data without parsing text. External MCP servers don't use this field.

## What Goes Where

| Information Type | Text Content (agents see) | structured_content (UI only) |
| ---------------- | ------------------------- | ---------------------------- |
| Process IDs      | ✅ **MUST include**       | ✅ Optional for UI parsing   |
| File paths       | ✅ **MUST include**       | ✅ Optional for UI parsing   |
| Status messages  | ✅ **MUST include**       | ✅ Optional for UI parsing   |
| Error details    | ✅ **MUST include**       | ✅ Optional for UI parsing   |
| Metadata         | ❌ Not critical           | ✅ For UI components         |
| Raw data arrays  | ❌ Summarize in text      | ✅ For UI rendering          |

## Anti-Patterns to Avoid

### ❌ Wrong: Critical ID only in structured_content

```rust
let result = MCPResult {
    content: vec![text("Background process started successfully")],
    structured_content: Some(json!({
        "process_id": "7573a69b",  // Agents can't see this!
        "status": "running"
    })),
    is_error: Some(false),
};
```

### ✅ Correct: ID visible in text output

```rust
let result = MCPResult {
    content: vec![text("Background process started (ID: 7573a69b)\n\nUse pollProcess(\"7573a69b\") to check status")],
    structured_content: Some(json!({
        "process_id": "7573a69b",  // Redundant but useful for UI
        "status": "running"
    })),
    is_error: Some(false),
};
```

### ❌ Wrong: IDs buried in JSON summary

```rust
let hint = SuccessHint::new(
    "Found 3 processes (1 running, 2 finished)",
    vec!["Use pollProcess to check status"],
);
```

### ✅ Correct: IDs visible for copy-paste

```rust
let process_list = processes.iter()
    .map(|p| format!("• {} [{}]: {}", p.id, p.status, p.command))
    .collect::<Vec<_>>()
    .join("\n");

let hint = SuccessHint::new(
    format!("Found 3 processes:\n\n{}", process_list),
    vec!["Use pollProcess(processId) to check status"],
);
```

### ❌ Wrong: Implicit state, only in JSON

```rust
let output = format!("Command executed\n{}", stdout);
let data = json!({"execution_type": "persistent", "cwd": "/project"});
```

### ✅ Correct: Explicit state in text

```rust
let output = format!(
    "Command executed\n\n{}\n\nPersistent shell state (maintained for next call):\n  Working directory: {}\n  Exit code: {}",
    stdout, cwd, exit_code
);
```

## Testing Your Tool Responses

Apply these checks to every tool implementation:

1. **Text-Only Test**: Read only the `content` field — can an agent understand what happened?
2. **ID Extraction**: Can an agent copy process IDs, file paths, session IDs from the text?
3. **Follow-up Actions**: Does the text contain enough info for the next tool call?
4. **State Clarity**: Is execution context (persistent vs isolated) clear from text alone?

## Core Rule

- **Agents ONLY see text content** — design for text-first readability
- `structured_content` is purely for UI components and external tooling
- If an agent needs to use a value in a follow-up call, it **MUST** be in text
- Test by reading only the text field — pretend JSON doesn't exist
