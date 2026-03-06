# Rust Session Resume Logic

To re-emit the `ToolExecutionRequiresApproval` event when a session is resumed or opened, hook into the `agent_resume_session` command.

## Update `resume_session` in `session_manager.rs` or `agent_commands.rs`

When the active session is fetched during resume, iterate through `pending_approvals` and emit the events:

```rust
// In src-tauri/src/agent/session_manager.rs or src-tauri/src/commands/agent_commands.rs
// wherever resume_session logic resides

let pending_events = {
    let mut evs = Vec::new();
    let active = active_sessions.read().await;
    if let Some(session) = active.get(&session_id) {
        let approvals = session.pending_approvals.read().await;
        for (tool_call_id, data) in approvals.iter() {
            evs.push(crate::agent::events::AgentEvent::ToolExecutionRequiresApproval {
                session_id: session_id.clone(),
                tool_call_id: tool_call_id.clone(),
                tool_name: data.tool_name.clone(),
                arguments: data.arguments.clone(),
            });
        }
    }
    evs
};

// Emit the existing pending approvals
for event in pending_events {
    if let Err(e) = crate::agent::events::emit_agent_event(&app_handle, event) {
        log::error!("Failed to re-emit pending approval event on resume: {}", e);
    }
}
```

This logic ensures that if the UI re-connects to a session that is stuck waiting for an approval, the UI will receive the event again and render the `PendingApprovalWidget`.
