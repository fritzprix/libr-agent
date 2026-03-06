# Rust State Changes

To persist the context of a pending tool execution (tool name and arguments) alongside the oneshot channel sender, the `AgentSession` state and `ToolExecutionStarted` logic must be updated.

## 1. Update `AgentSession` and `PendingApprovalData`

In `src-tauri/src/agent/state.rs` (or where `AgentSession` is defined):

```rust
use tokio::sync::oneshot;

pub struct PendingApprovalData {
    pub sender: oneshot::Sender<bool>,
    pub tool_name: String,
    pub arguments: String,
}

pub struct AgentSession {
    // ... other fields ...
    pub pending_approvals: Arc<RwLock<std::collections::HashMap<String, PendingApprovalData>>>,
    // ... remaining fields ...
}
```

Make sure to search the codebase for all instances where `pending_approvals: Arc::new(RwLock::new(std::collections::HashMap::new()))` is initialized (e.g., `src-tauri/src/agent/lifecycle/creation.rs`, `recovery.rs`, etc.) to ensure no type errors occur. Since it initializes an empty generic `HashMap`, it usually compiles fine, but verify imports.

## 2. Update Tool Execution Logic

In `src-tauri/src/agent/llm/tool_execution.rs`, when a tool requires approval:

```rust
// Replace this:
// let mut approvals = session.pending_approvals.write().await;
// approvals.insert(tool_call_id.clone(), tx);

// With this:
let mut approvals = session.pending_approvals.write().await;
approvals.insert(
    tool_call_id.clone(),
    crate::agent::state::PendingApprovalData {
        sender: tx,
        tool_name: tool_name.clone(),
        arguments: args_str.clone(),
    },
);
```
