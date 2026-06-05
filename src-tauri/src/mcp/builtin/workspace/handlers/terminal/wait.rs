use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::builtin::workspace::terminal_manager;
use crate::mcp::builtin::workspace::WorkspaceServer;
use crate::mcp::types::MCPResult;
use serde_json::Value;

impl WorkspaceServer {
    pub async fn handle_wait_for_process(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        let process_id = match args.get("processId").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return Ok(missing_param_error("processId", ToolGroup::Workspace));
            }
        };

        let timeout_secs = args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(30);
        let timeout = std::time::Duration::from_secs(timeout_secs);
        let start_time = std::time::Instant::now();
        let is_polling_mode = timeout_secs == 0;

        // Loop for blocking wait (or single iteration for polling)
        loop {
            // Check process status and update usage statistics if polling
            let (status, entry_data, should_show_guidance, notifier) = {
                let mut registry = self.process_registry.write().await;

                if let Some(entry) = registry.entries.get_mut(process_id) {
                    if entry.session_id != session_id {
                        return Ok(guided_error(
                            ErrorCategory::PermissionDenied,
                            format!("Process '{}' not found in current session", process_id),
                            ToolGroup::Workspace,
                        )
                        .guidance(vec!["Process belongs to another session".to_string()])
                        .to_mcp_result());
                    }

                    // Poll tracking logic (migrated from pollProcess)
                    // We interpret every check as a "poll" for statistical purposes
                    let now = chrono::Utc::now();
                    entry.last_poll_at = Some(now);
                    entry.poll_count += 1;

                    let is_running = terminal_manager::is_active_process_status(&entry.status);
                    if is_running {
                        if entry.first_running_poll_at.is_none() {
                            entry.first_running_poll_at = Some(now);
                        }
                        entry.consecutive_running_polls += 1;
                    } else {
                        entry.consecutive_running_polls = 0;
                        entry.first_running_poll_at = None;
                    }

                    // Strict polling guidance only for 0-timeout calls to avoid blocking long-waits
                    let threshold = crate::config::poll_threshold();
                    let guidance = is_polling_mode
                        && is_running
                        && entry.consecutive_running_polls >= threshold;

                    // Clone entry data before the mutable borrow of `entry` ends so we can
                    // subsequently take an immutable borrow for the completion notifier.
                    // (NLL ends the mutable borrow after the last use of `entry`.)
                    let status_clone = entry.status.clone();
                    let entry_clone = entry.clone();

                    // Grab the completion notifier while still holding the write lock.
                    let notifier = registry.completion_notifiers.get(process_id).cloned();

                    (status_clone, entry_clone, guidance, notifier)
                } else {
                    // Check available processes for error recovery
                    let available: Vec<String> = registry
                        .entries
                        .values()
                        .filter(|e| e.session_id == session_id)
                        .take(5)
                        .map(|e| format!("{} [{}]", e.id, e.command))
                        .collect();

                    let available_text = if available.is_empty() {
                        "No processes found in this session".to_string()
                    } else {
                        format!("Available processes: {}", available.join(", "))
                    };

                    return Ok(guided_error(
                        ErrorCategory::ResourceNotFound,
                        format!("Process '{}' not found in session", process_id),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        available_text,
                        "Use listProcesses() to see all active processes".to_string(),
                    ])
                    .to_mcp_result());
                }
            };

            // 1. Success Condition: Process Finished (Always return immediately)
            if matches!(
                status,
                terminal_manager::ProcessStatus::Finished
                    | terminal_manager::ProcessStatus::Failed
                    | terminal_manager::ProcessStatus::Killed
            ) {
                let response = serde_json::json!({
                    "process_id": process_id,
                    "status": terminal_manager::process_status_label(&status),
                    "command": entry_data.command,
                    "exit_code": entry_data.exit_code,
                    "pid": entry_data.pid,
                    "started_at": entry_data.started_at.to_rfc3339(),
                    "finished_at": entry_data.finished_at.map(|t| t.to_rfc3339()),
                });

                return Ok(SuccessHint::new(
                    format!("Process {} finished with status: {:?}", process_id, status),
                    SuccessHint::for_tool("waitForProcess", ToolGroup::Workspace),
                )
                .to_mcp_result_with_data(Some(response)));
            }

            // 2. Polling Mode: Return current status immediately (even if Running)
            if is_polling_mode {
                let response = serde_json::json!({
                    "process_id": process_id,
                    "status": terminal_manager::process_status_label(&status),
                    "command": entry_data.command,
                    "exit_code": entry_data.exit_code,
                    "pid": entry_data.pid,
                    "started_at": entry_data.started_at.to_rfc3339(),
                    // finished_at is None if running
                    "finished_at": entry_data.finished_at.map(|t| t.to_rfc3339()),
                });

                if should_show_guidance {
                    return Ok(guided_error(
                        ErrorCategory::InvalidState,
                        "Excessive polling detected".to_string(),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Wait a few seconds before checking again".to_string(),
                        "Or use waitForProcess with a non-zero timeout".to_string(),
                    ])
                    .to_mcp_result());
                }

                return Ok(SuccessHint::new(
                    format!("Process {} is currently {:?}", process_id, status),
                    SuccessHint::for_tool("pollProcess", ToolGroup::Workspace),
                )
                .to_mcp_result_with_data(Some(response)));
            }

            // 3. Timeout Check (Blocking Mode only)
            if start_time.elapsed() >= timeout {
                return Ok(guided_error(
                    ErrorCategory::Timeout,
                    format!(
                        "Timeout waiting for process {} ({}s)",
                        process_id, timeout_secs
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Process is still running in background".to_string(),
                    "Use waitForProcess(timeout=0) to check status without waiting".to_string(),
                    "Increase timeout parameter if needed".to_string(),
                ])
                .to_mcp_result());
            }

            // 4. Wait before next loop iteration.
            // Use push-notification (notifier) with a 30s heartbeat fallback so the loop
            // wakes up immediately when the process finishes instead of busy-polling every 100ms.
            let remaining = timeout.saturating_sub(start_time.elapsed());
            let wait_slice = remaining.min(tokio::time::Duration::from_secs(30));
            match notifier {
                Some(n) => {
                    tokio::select! {
                        _ = n.notified() => {}
                        _ = tokio::time::sleep(wait_slice) => {}
                    }
                }
                None => {
                    // Defensive fallback: notifier missing (shouldn't happen for valid processes).
                    tokio::time::sleep(wait_slice.min(std::time::Duration::from_millis(500))).await;
                }
            }
        }
    }
}
