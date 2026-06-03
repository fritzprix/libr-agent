use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, SuccessHint, ToolGroup};
use crate::mcp::types::MCPResult;
use crate::session_isolation::{IsolatedProcessConfig, IsolationLevel};

use super::super::super::{terminal_manager, WorkspaceServer, PERSISTENT_SHELL_TOOL};
use super::super::{normalization, process, validation};
use super::{format_command_io_message, format_duration_ms};

impl WorkspaceServer {
    /// Execute shell commands with isolation
    pub(crate) async fn execute_shell_with_isolation(
        &self,
        command: &str,
        tool_name: &str,
        isolation_level: IsolationLevel,
        timeout_secs: u64,
        session_id: &str,
        env_vars: HashMap<String, String>,
    ) -> Result<MCPResult, String> {
        let session_id = session_id.to_string();

        let workspace_path = self
            .session_manager
            .get_session_workspace_dir_by_id(&session_id);

        if let Some(result) = self.apply_shell_policy_block(
            tool_name,
            command,
            &workspace_path,
            None,
            Some(&env_vars),
        ) {
            return Ok(result);
        }

        // Normalize shell command
        let normalized_command = normalization::normalize_shell_command(command);

        // Track execution time
        let execution_start = std::time::Instant::now();

        const MAX_CONCURRENT_PROCESSES: usize = 20;
        {
            let registry = self.process_registry.read().await;
            let active_count = registry
                .entries
                .values()
                .filter(|e| e.session_id == session_id)
                .filter(|e| terminal_manager::is_active_process_status(&e.status))
                .count();

            if active_count >= MAX_CONCURRENT_PROCESSES {
                return Ok(guided_error(
                    ErrorCategory::InvalidState,
                    format!(
                        "Maximum concurrent processes limit reached ({}/{})",
                        active_count, MAX_CONCURRENT_PROCESSES
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Use listProcesses to see active processes".to_string(),
                    "Use stopProcess to cancel unnecessary processes".to_string(),
                    "Wait for some processes to finish before starting new ones".to_string(),
                ])
                .to_mcp_result());
            }
        }

        // Generate process ID for sync execution
        let process_id = cuid2::create_id();

        // Create temporary directory for output files
        let process_tmp_dir = workspace_path
            .join(".libragent/tmp")
            .join(format!("sync_{process_id}"));

        if let Err(e) = tokio::fs::create_dir_all(&process_tmp_dir).await {
            return Ok(guided_error(
                ErrorCategory::InternalError,
                "Create temp directory failed".to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Check workspace directory permissions".to_string(),
                "Ensure sufficient disk space is available".to_string(),
                format!(
                    "Verify tmp directory is writable: {}",
                    workspace_path.join(".libragent/tmp").display()
                ),
                format!("Error: {}", e),
            ])
            .to_mcp_result());
        }

        let stdout_path = process_tmp_dir.join("stdout");
        let stderr_path = process_tmp_dir.join("stderr");

        let isolation_config = IsolatedProcessConfig {
            session_id: session_id.clone(),
            workspace_path: workspace_path.clone(),
            command: normalized_command,
            args: vec![],
            env_vars,
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
                    ErrorCategory::InternalError,
                    "Create isolated shell command failed".to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Verify shell environment is properly configured".to_string(),
                    "Check if required shell binary exists (bash/sh/PowerShell)".to_string(),
                    "Ensure workspace isolation level is valid".to_string(),
                    format!("Error: {}", e),
                ])
                .to_mcp_result());
            }
        };

        let process_permit = crate::state::get_concurrency_gate()
            .acquire_active_process()
            .await?;
        let cancel_token = CancellationToken::new();
        let completion_notifier = Arc::new(tokio::sync::Notify::new());

        let entry = terminal_manager::ProcessEntry {
            id: process_id.clone(),
            name: None,
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

        let registry = self.process_registry.clone();
        let pid_copy = process_id.clone();
        let stdout_path_for_task = stdout_path.clone();
        let stderr_path_for_task = stderr_path.clone();

        tokio::spawn(async move {
            let _process_permit = process_permit;

            {
                let mut reg = registry.write().await;
                if let Some(entry) = reg.entries.get_mut(&pid_copy) {
                    entry.status = terminal_manager::ProcessStatus::Running;
                }
            }

            let result = process::spawn_and_stream_to_files(
                cmd,
                stdout_path_for_task.clone(),
                stderr_path_for_task.clone(),
                format!("sync:{pid_copy}"),
                cancel_token,
            )
            .await;

            let mut reg = registry.write().await;
            if let Some(entry) = reg.entries.get_mut(&pid_copy) {
                match result {
                    Ok((pid, exit_code, _, _)) => {
                        entry.pid = pid;
                        entry.exit_code = exit_code;
                        entry.finished_at.get_or_insert_with(chrono::Utc::now);
                        entry.stdout_size =
                            terminal_manager::get_file_size(&stdout_path_for_task).await;
                        entry.stderr_size =
                            terminal_manager::get_file_size(&stderr_path_for_task).await;

                        if entry.status != terminal_manager::ProcessStatus::Killed {
                            entry.status = if exit_code.unwrap_or(-1) == 0 {
                                terminal_manager::ProcessStatus::Finished
                            } else {
                                terminal_manager::ProcessStatus::Failed
                            };
                        }
                    }
                    Err(e) => {
                        let error_text = format!("Failed to execute isolated shell command: {e}");
                        let _ = tokio::fs::write(&stderr_path_for_task, format!("{error_text}\n"))
                            .await;

                        entry.finished_at.get_or_insert_with(chrono::Utc::now);
                        entry.stdout_size =
                            terminal_manager::get_file_size(&stdout_path_for_task).await;
                        entry.stderr_size =
                            terminal_manager::get_file_size(&stderr_path_for_task).await;

                        if entry.status != terminal_manager::ProcessStatus::Killed {
                            entry.status = terminal_manager::ProcessStatus::Failed;
                        }

                        error!("Process {} execution error: {}", pid_copy, e);
                    }
                }
            }

            reg.cancellation_tokens.remove(&pid_copy);
            let notifier = reg.completion_notifiers.get(&pid_copy).cloned();
            drop(reg);

            if let Some(notifier) = notifier {
                notifier.notify_waiters();
            }
        });

        let timeout_duration = Duration::from_secs(timeout_secs);
        let terminal_entry = loop {
            {
                let registry = self.process_registry.read().await;
                let Some(entry) = registry.entries.get(&process_id).cloned() else {
                    return Ok(guided_error(
                        ErrorCategory::InternalError,
                        format!("Process {} disappeared before completion", process_id),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Retry the command once".to_string(),
                        "If this persists, inspect workspace tool logs".to_string(),
                    ])
                    .to_mcp_result());
                };

                if terminal_manager::is_terminal_process_status(&entry.status) {
                    break Some(entry);
                }
            }

            let remaining = timeout_duration.saturating_sub(execution_start.elapsed());
            if remaining.is_zero() {
                break None;
            }

            let notifier = {
                let registry = self.process_registry.read().await;
                registry.completion_notifiers.get(&process_id).cloned()
            };

            let wait_slice = remaining.min(Duration::from_millis(100));
            if let Some(notifier) = notifier {
                let _ = tokio::time::timeout(wait_slice, notifier.notified()).await;
            } else {
                tokio::time::sleep(wait_slice).await;
            }
        };

        match terminal_entry {
            Some(entry) => {
                let duration_ms = execution_start.elapsed().as_millis() as u64;
                let stdout_result = process::read_output_file(&stdout_path).await;
                let stderr_result = process::read_output_file(&stderr_path).await;
                let actual_exit_code = entry.exit_code.unwrap_or(-1);
                let success = entry.status == terminal_manager::ProcessStatus::Finished
                    && actual_exit_code == 0;

                {
                    let mut reg = self.process_registry.write().await;
                    reg.entries.remove(&process_id);
                    reg.cancellation_tokens.remove(&process_id);
                    reg.completion_notifiers.remove(&process_id);
                    reg.streaming_handles.remove(&process_id);
                }

                let _ = tokio::fs::remove_dir_all(&process_tmp_dir).await;

                let stdout = match stdout_result {
                    Ok(stdout) => stdout,
                    Err(error) => {
                        return Ok(guided_error(
                            ErrorCategory::OperationFailed,
                            error,
                            ToolGroup::Workspace,
                        )
                        .guidance(vec![
                            "Retry the command once".to_string(),
                            "If this persists, inspect workspace process logs".to_string(),
                        ])
                        .to_mcp_result());
                    }
                };
                let stderr = match stderr_result {
                    Ok(stderr) => stderr,
                    Err(error) => {
                        return Ok(guided_error(
                            ErrorCategory::OperationFailed,
                            error,
                            ToolGroup::Workspace,
                        )
                        .guidance(vec![
                            "Retry the command once".to_string(),
                            "If this persists, inspect workspace process logs".to_string(),
                        ])
                        .to_mcp_result());
                    }
                };

                let response = serde_json::json!({
                    "command": command,
                    "exit_code": actual_exit_code,
                    "stdout": stdout,
                    "stderr": stderr,
                    "status": terminal_manager::process_status_label(&entry.status),
                    "duration_ms": duration_ms,
                    "execution_type": "isolated"
                });

                info!(
                    "Isolated shell command executed: {} (session: {}, status: {:?}, exit: {:?}, duration: {}ms)",
                    command, session_id, entry.status, entry.exit_code, duration_ms
                );

                if !success {
                    let error_output = if !stderr.is_empty() {
                        format!("Error output:\n{}", stderr)
                    } else if !stdout.is_empty() {
                        format!("Command output:\n{}", stdout)
                    } else {
                        "No error output captured".to_string()
                    };

                    let guidance = match actual_exit_code {
                        1 => vec![
                            "General command failure - review error output above".to_string(),
                            "Verify command syntax and required files exist".to_string(),
                            "Use listDirectory to check file paths".to_string(),
                        ],
                        2 => vec![
                            "Misuse of shell command or invalid arguments".to_string(),
                            "Check command syntax in tool documentation".to_string(),
                            "Verify all required parameters are provided".to_string(),
                        ],
                        127 => vec![
                            "Command not found - program is not installed or not in PATH"
                                .to_string(),
                            "Verify the program is installed on the system".to_string(),
                            "Check for typos in the command name".to_string(),
                        ],
                        126 => vec![
                            "Command found but not executable".to_string(),
                            "Check file permissions".to_string(),
                            "Verify the file is a valid executable".to_string(),
                        ],
                        130 => vec![
                            "Command terminated by Ctrl+C (SIGINT)".to_string(),
                            "Process was interrupted by user or system".to_string(),
                        ],
                        _ => vec![
                            format!("Command failed with exit code: {}", actual_exit_code),
                            "Review error output above for specific failure reasons".to_string(),
                            "Verify command syntax and required dependencies".to_string(),
                        ],
                    };

                    return Ok(guided_error(
                        ErrorCategory::OperationFailed,
                        format!(
                            "Command failed with exit code: {}\n\n{}",
                            actual_exit_code, error_output
                        ),
                        ToolGroup::Workspace,
                    )
                    .guidance(guidance)
                    .to_mcp_result());
                }

                let header = format!(
                    "Command executed in {} (exit code: 0)",
                    format_duration_ms(duration_ms)
                );
                let text_message =
                    format_command_io_message(&header, "Output", &stdout, "Stderr", &stderr);

                let output_lower = (stdout.clone() + &stderr).to_lowercase();
                let cancellation_indicators = [
                    "operation cancelled",
                    "operation canceled",
                    "aborted",
                    "user cancelled",
                    "user canceled",
                    "no changes made",
                    "skipping",
                ];
                let prompt_indicators = ["overwrite", "? ", "[y/n]", "[yes/no]", "confirm"];

                let detected_cancellation = cancellation_indicators
                    .iter()
                    .any(|indicator| output_lower.contains(indicator));
                let detected_prompt = prompt_indicators
                    .iter()
                    .any(|indicator| output_lower.contains(indicator));

                if detected_cancellation || detected_prompt {
                    let indicator_type = if detected_cancellation {
                        "operation was cancelled"
                    } else {
                        "interactive prompt detected"
                    };

                    let enhanced_message = format!(
                        "{}\n\n⚠️ NOTICE: Output indicates {} in non-interactive mode.\n\n\
                        If this command requires user input:\n\
                        1. Use {} with requireUserInput: true\n\
                        2. That keeps a single synchronous tool call: the backend pauses, UI collects the input, then the same call resumes with the final result\n\
                        3. Or add non-interactive flags: --yes, --force, -y\n\
                        4. Or pipe input: echo y | command\n\n\
                        Detected indicator: {}",
                        text_message, indicator_type, PERSISTENT_SHELL_TOOL, indicator_type
                    );

                    let hint = SuccessHint::new(
                        enhanced_message,
                        vec![
                            format!(
                                "For interactive commands, use {} with requireUserInput: true",
                                PERSISTENT_SHELL_TOOL
                            ),
                            "The agent still sees one synchronous call with a prompt-resume flow"
                                .to_string(),
                            format!("Add non-interactive flags: {} --yes", command),
                            "Or use echo/stdin redirection for automated input".to_string(),
                        ],
                    );

                    return Ok(hint.to_mcp_result_with_data(Some(response)));
                }

                let hint = SuccessHint::new(
                    text_message,
                    SuccessHint::for_tool(tool_name, ToolGroup::Workspace),
                );
                Ok(hint.to_mcp_result_with_data(Some(response)))
            }
            None => {
                self.invalidate_context_cache().await;

                let status = {
                    let registry = self.process_registry.read().await;
                    registry
                        .entries
                        .get(&process_id)
                        .map(|entry| terminal_manager::process_status_label(&entry.status))
                        .unwrap_or_else(|| "running".to_string())
                };

                warn!(
                    "Isolated shell command '{}' exceeded sync timeout after {} seconds; handing off to background process {}",
                    command, timeout_secs, process_id
                );

                let might_be_interactive = validation::is_likely_interactive_command(command);
                let mut message = format!(
                    "Command exceeded the synchronous wait window after {} seconds and is still running in background.\n\nProcess ID: {}\nStatus: {}\nExit code: pending",
                    timeout_secs, process_id, status
                );

                if might_be_interactive {
                    message.push_str(
                        "\n\nThe command also looks interactive, so verify it is not waiting for prompts or passwords.",
                    );
                }

                let mut next_actions = vec![
                    format!(
                        "Use waitForProcess(\"{}\", 0) to check current status",
                        process_id
                    ),
                    format!(
                        "Use readProcessOutput(\"{}\", \"both\") to inspect stdout and stderr",
                        process_id
                    ),
                    format!("Use stopProcess(\"{}\") to terminate it", process_id),
                ];

                if might_be_interactive {
                    next_actions.insert(
                        0,
                        format!(
                            "If the command needed input, rerun it with {} and requireUserInput: true",
                            PERSISTENT_SHELL_TOOL
                        ),
                    );
                }

                let response = serde_json::json!({
                    "command": command,
                    "process_id": process_id,
                    "status": status,
                    "timeout_seconds": timeout_secs,
                    "execution_type": "isolated_background_handoff"
                });

                Ok(SuccessHint::new(message, next_actions).to_mcp_result_with_data(Some(response)))
            }
        }
    }
}
