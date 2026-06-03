use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::builtin::workspace::terminal_manager;
use crate::mcp::builtin::workspace::WorkspaceServer;
use crate::mcp::types::MCPResult;
use serde_json::Value;

impl WorkspaceServer {
    pub async fn handle_stop_process(
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

        let mut registry = self.process_registry.write().await;

        // Check if process exists and belongs to session
        if let Some(entry) = registry.entries.get(process_id) {
            if entry.session_id != session_id {
                // ✅ ENHANCED: Process-specific error for session mismatch
                return Ok(guided_error(
                    ErrorCategory::PermissionDenied,
                    format!("Process '{}' not found in current session", process_id),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Process may belong to a different session".to_string(),
                    "Use listProcesses() to see processes in your session".to_string(),
                ])
                .to_mcp_result());
            }
        } else {
            // ✅ ENHANCED: Process-specific error with running process IDs
            let running: Vec<String> = registry
                .entries
                .values()
                .filter(|e| e.session_id == session_id)
                .filter(|e| terminal_manager::is_active_process_status(&e.status))
                .take(5)
                .map(|e| format!("{} [{}]", e.id, e.command))
                .collect();

            let running_text = if running.is_empty() {
                "No running processes found in this session".to_string()
            } else {
                format!("Running processes: {}", running.join(", "))
            };

            return Ok(guided_error(
                ErrorCategory::ResourceNotFound,
                format!("Process '{}' not found", process_id),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                running_text,
                "Use listProcesses() to see all processes".to_string(),
                "Only running processes can be stopped".to_string(),
            ])
            .to_mcp_result());
        }

        // Cancel process via token
        if let Some(token) = registry.cancellation_tokens.get(process_id) {
            token.cancel();
        }

        // Update status and kill process
        if let Some(entry) = registry.entries.get_mut(process_id) {
            // Check if process is already terminated
            if matches!(
                entry.status,
                terminal_manager::ProcessStatus::Finished
                    | terminal_manager::ProcessStatus::Failed
                    | terminal_manager::ProcessStatus::Killed
            ) {
                return Ok(guided_error(
                    ErrorCategory::InvalidState,
                    format!(
                        "Process {} has already terminated with status: {:?}",
                        process_id, entry.status
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Use listProcesses to see running processes".to_string(),
                    "Only running processes can be stopped".to_string(),
                ])
                .to_mcp_result());
            }

            // Kill process if running
            if let Some(pid) = entry.pid {
                if matches!(
                    entry.status,
                    terminal_manager::ProcessStatus::Running
                        | terminal_manager::ProcessStatus::Starting
                ) {
                    tracing::info!("Force-killing process {} (PID {})", process_id, pid);

                    #[cfg(unix)]
                    {
                        use std::process::Command;
                        let mut cmd = Command::new("kill");
                        cmd.arg("-TERM").arg(pid.to_string());
                        crate::utils::env::apply_isolated_env(&mut cmd);
                        let _ = cmd.output();
                    }

                    #[cfg(windows)]
                    {
                        use std::os::windows::process::CommandExt;
                        use std::process::Command;
                        let mut cmd = Command::new("taskkill");
                        cmd.args(["/PID", &pid.to_string(), "/F"]);
                        crate::utils::env::apply_isolated_env(&mut cmd);
                        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                        let _ = cmd.output();
                    }
                }
            }

            entry.status = terminal_manager::ProcessStatus::Killed;
            entry.finished_at = Some(chrono::Utc::now());
        }

        // Extract notifier before releasing the write lock.
        let notifier = registry.completion_notifiers.get(process_id).cloned();

        // Remove cancellation token
        registry.cancellation_tokens.remove(process_id);
        drop(registry); // Release write lock before firing notification

        // Invalidate service context cache
        self.invalidate_context_cache().await;

        // Wake any waiters blocked in handle_wait_for_process.
        if let Some(n) = notifier {
            n.notify_waiters();
        }

        let hint = SuccessHint::new(
            format!("Process {} stopped successfully", process_id),
            SuccessHint::for_tool("stopProcess", ToolGroup::Workspace),
        );

        let response = serde_json::json!({
            "process_id": process_id,
            "stopped": true
        });

        Ok(hint.to_mcp_result_with_data(Some(response)))
    }
}
