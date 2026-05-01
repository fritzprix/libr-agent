use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, SuccessHint, ToolGroup};
use crate::mcp::types::MCPResult;
use crate::session_isolation::IsolatedProcessConfig;

use super::super::super::{terminal_manager, utils, WorkspaceServer};
use super::super::{normalization, process};

impl WorkspaceServer {
    /// Execute shell command asynchronously in background
    pub(crate) async fn execute_shell_async(
        &self,
        command: &str,
        _args: &Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        // Get session info
        let session_id = session_id.to_string();

        // Extract optional name
        let process_name = _args
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let workspace_path = self
            .session_manager
            .get_session_workspace_dir_by_id(&session_id);

        // Check concurrent process limit (max 20 per session)
        const MAX_CONCURRENT_PROCESSES: usize = 20;

        {
            let registry = self.process_registry.read().await;
            let running_count = registry
                .entries
                .values()
                .filter(|e| e.session_id == session_id)
                .filter(|e| matches!(e.status, terminal_manager::ProcessStatus::Running))
                .count();

            if running_count >= MAX_CONCURRENT_PROCESSES {
                return Ok(guided_error(
                    ErrorCategory::InvalidState,
                    format!(
                        "Maximum concurrent processes limit reached ({}/{})",
                        running_count, MAX_CONCURRENT_PROCESSES
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Use listProcesses to see running processes".to_string(),
                    "Use stopProcess to cancel unnecessary processes".to_string(),
                    "Wait for some processes to finish before starting new ones".to_string(),
                ])
                .to_mcp_result());
            }
        }

        // Generate process ID
        let process_id = cuid2::create_id();

        // Create process tmp directory
        let process_tmp_dir = workspace_path
            .join(".libragent/tmp")
            .join(format!("process_{process_id}"));

        if let Err(e) = tokio::fs::create_dir_all(&process_tmp_dir).await {
            return Ok(guided_error(
                ErrorCategory::OperationFailed,
                e.to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Check workspace directory permissions".to_string(),
                "Ensure sufficient disk space is available".to_string(),
                format!(
                    "Verify tmp directory is writable: {}",
                    workspace_path.join(".libragent/tmp").display()
                ),
            ])
            .to_mcp_result());
        }

        let stdout_path = process_tmp_dir.join("stdout");
        let stderr_path = process_tmp_dir.join("stderr");

        // Normalize command
        let normalized_command = normalization::normalize_shell_command(command);

        // Use configured isolation level
        let isolation_level = utils::get_shell_isolation_level().await;

        // Create isolation config
        let isolation_config = IsolatedProcessConfig {
            session_id: session_id.clone(),
            workspace_path: workspace_path.clone(),
            command: normalized_command.clone(),
            args: vec![],
            env_vars: _args
                .get("env")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            isolation_level,
            shell_type: None, // Default to platform default shell
        };

        // Create isolated command
        let cmd = match self
            .isolation_manager
            .create_isolated_command(isolation_config)
            .await
        {
            Ok(cmd) => cmd,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::OperationFailed,
                    e.to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Verify shell environment is properly configured".to_string(),
                    "Check if required shell binary exists (bash/sh/PowerShell)".to_string(),
                    "Ensure workspace isolation level is valid".to_string(),
                ])
                .to_mcp_result());
            }
        };

        // Acquire a global active-process slot (SP2).  Called after all early-return
        // guards so no registry cleanup is needed if this blocks or fails.
        let process_permit = crate::state::get_concurrency_gate()
            .acquire_active_process()
            .await?;

        // Register process in registry (Starting status)
        let cancel_token = CancellationToken::new();
        let completion_notifier = Arc::new(tokio::sync::Notify::new());

        let entry = terminal_manager::ProcessEntry {
            id: process_id.clone(),
            name: process_name.clone(),
            session_id: session_id.clone(),
            command: command.to_string(),
            status: terminal_manager::ProcessStatus::Starting,
            pid: None,
            exit_code: None,
            started_at: chrono::Utc::now(),
            finished_at: None,
            stdout_path: stdout_path.to_string_lossy().to_string(),
            stderr_path: stderr_path.to_string_lossy().to_string(),
            stdout_size: 0,
            stderr_size: 0,
            // Initialize poll tracking fields
            last_poll_at: None,
            poll_count: 0,
            consecutive_running_polls: 0,
            first_running_poll_at: None,
        };

        {
            let mut registry = self.process_registry.write().await;
            registry.entries.insert(process_id.clone(), entry.clone());
            registry
                .cancellation_tokens
                .insert(process_id.clone(), cancel_token.clone());
            registry
                .completion_notifiers
                .insert(process_id.clone(), completion_notifier.clone());
        }

        // Spawn monitoring task using hybrid streaming
        let registry = self.process_registry.clone();
        let pid_copy = process_id.clone();

        tokio::spawn(async move {
            let _process_permit = process_permit;
            // Update registry: starting -> running
            {
                let mut reg = registry.write().await;
                if let Some(entry) = reg.entries.get_mut(&pid_copy) {
                    entry.status = terminal_manager::ProcessStatus::Running;
                }
            }

            // Execute using hybrid streaming
            let result = process::spawn_and_stream_hybrid(
                cmd,
                stdout_path.clone(),
                stderr_path.clone(),
                format!("async:{pid_copy}"),
                cancel_token,
            )
            .await;

            // Update registry: finished
            let mut reg = registry.write().await;
            if let Some(entry) = reg.entries.get_mut(&pid_copy) {
                match result {
                    Ok((pid, exit_code, streaming_handle)) => {
                        entry.pid = pid;
                        let code = exit_code.unwrap_or(-1);
                        entry.status = if code == 0 {
                            terminal_manager::ProcessStatus::Finished
                        } else {
                            terminal_manager::ProcessStatus::Failed
                        };
                        entry.exit_code = exit_code;
                        entry.finished_at = Some(chrono::Utc::now());

                        // Update file sizes
                        entry.stdout_size = terminal_manager::get_file_size(&stdout_path).await;
                        entry.stderr_size = terminal_manager::get_file_size(&stderr_path).await;

                        // Store streaming handle for real-time access (after entry mutations)
                        reg.streaming_handles
                            .insert(pid_copy.clone(), streaming_handle);
                    }
                    Err(e) => {
                        entry.status = terminal_manager::ProcessStatus::Failed;
                        entry.finished_at = Some(chrono::Utc::now());
                        error!("Process {} execution error: {}", pid_copy, e);

                        // Update file sizes even on error
                        entry.stdout_size = terminal_manager::get_file_size(&stdout_path).await;
                        entry.stderr_size = terminal_manager::get_file_size(&stderr_path).await;
                    }
                }
            }

            // Remove cancellation token (keep streaming handle for 5 minutes)
            reg.cancellation_tokens.remove(&pid_copy);

            // Extract notifier before dropping write lock, then wake waiters.
            let notifier = reg.completion_notifiers.get(&pid_copy).cloned();

            info!(
                "Process {} completed with status: {:?}",
                pid_copy,
                reg.entries.get(&pid_copy).map(|e| &e.status)
            );
            drop(reg);

            // Wake any handle_wait_for_process callers blocked on this process.
            if let Some(n) = notifier {
                n.notify_waiters();
            }
        });

        // Wait briefly to detect immediate failures
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check if process failed to start
        {
            let registry = self.process_registry.read().await;
            if let Some(entry) = registry.entries.get(&process_id) {
                if matches!(entry.status, terminal_manager::ProcessStatus::Failed) {
                    return Ok(guided_error(
                        ErrorCategory::OperationFailed,
                        "Process failed to start",
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Verify the command syntax is correct".to_string(),
                        "Check if required programs are installed".to_string(),
                        "Use listProcesses to see failed process details".to_string(),
                    ])
                    .to_mcp_result());
                }
            }
        }

        // Invalidate service context cache to reflect new process
        self.invalidate_context_cache().await;

        // Return immediate response with process_id
        let hint = SuccessHint::new(
            format!(
                "Background process started successfully

• Process ID: {}
• Command: {}
• Mode: Asynchronous (non-blocking)",
                process_id, command
            ),
            vec![
                format!(
                    "Use waitForProcess(\"{}\", 0) to check status and completion",
                    process_id
                ),
                "Use readProcessOutput with 'both' to inspect stdout and stderr".to_string(),
                "Use listProcesses to see all running processes".to_string(),
            ],
        );

        let response_data = serde_json::json!({
            "process_id": process_id,
            "command": command,
            "mode": "async",
            "note": "use waitForProcess or readProcessOutput to retrieve output"
        });

        Ok(hint.to_mcp_result_with_data(Some(response_data)))
    }
}
