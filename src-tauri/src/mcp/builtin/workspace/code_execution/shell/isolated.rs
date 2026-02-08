use std::collections::HashMap;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::mcp::builtin::error_guidance::{
    operation_failed_error, ErrorCategory, ErrorGuidance, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::session_isolation::{IsolatedProcessConfig, IsolationLevel};

use super::super::super::{terminal_manager, WorkspaceServer, PERSISTENT_SHELL_TOOL};
use super::super::{normalization, process, validation};

impl WorkspaceServer {
    /// Execute shell commands with isolation
    pub(crate) async fn execute_shell_with_isolation(
        &self,
        command: &str,
        isolation_level: IsolationLevel,
        timeout_secs: u64,
        session_id: &str,
        env_vars: HashMap<String, String>,
    ) -> Result<MCPResult, String> {
        let session_id = session_id.to_string();

        let workspace_path = self
            .session_manager
            .get_session_workspace_dir_by_id(&session_id);

        // Normalize shell command
        let normalized_command = normalization::normalize_shell_command(command);

        // Track execution time
        let execution_start = std::time::Instant::now();

        // Generate process ID for sync execution
        let process_id = cuid2::create_id();

        // Create temporary directory for output files
        let process_tmp_dir = workspace_path
            .join("tmp")
            .join(format!("sync_{process_id}"));

        if let Err(e) = tokio::fs::create_dir_all(&process_tmp_dir).await {
            return Ok(operation_failed_error(
                "Create temp directory",
                &e.to_string(),
                vec![
                    "Check workspace directory permissions".to_string(),
                    "Ensure sufficient disk space is available".to_string(),
                    format!(
                        "Verify tmp directory is writable: {}",
                        workspace_path.join("tmp").display()
                    ),
                ],
                ToolGroup::Workspace,
            ));
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
                return Ok(operation_failed_error(
                    "Create isolated shell command",
                    &e.to_string(),
                    vec![
                        "Verify shell environment is properly configured".to_string(),
                        "Check if required shell binary exists (bash/sh/PowerShell)".to_string(),
                        "Ensure workspace isolation level is valid".to_string(),
                    ],
                    ToolGroup::Workspace,
                ));
            }
        };

        // Create cancellation token
        let cancel_token = CancellationToken::new();

        // Register process in registry
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
        }

        // Execute command with timeout using common spawn+stream logic
        let timeout_duration = Duration::from_secs(timeout_secs);
        let execution_result = tokio::time::timeout(
            timeout_duration,
            process::spawn_and_stream_to_files(
                cmd,
                stdout_path.clone(),
                stderr_path.clone(),
                format!("sync:{process_id}"),
                cancel_token.clone(),
            ),
        )
        .await;

        // Update registry with result
        let mut reg = self.process_registry.write().await;

        match execution_result {
            Ok(Ok((pid, exit_code, stdout, stderr))) => {
                // Measure duration
                let duration_ms = execution_start.elapsed().as_millis() as u64;

                // Update registry entry
                if let Some(entry) = reg.entries.get_mut(&process_id) {
                    entry.pid = pid;
                    entry.exit_code = exit_code;
                    entry.status = if exit_code.unwrap_or(-1) == 0 {
                        terminal_manager::ProcessStatus::Finished
                    } else {
                        terminal_manager::ProcessStatus::Failed
                    };
                    entry.finished_at = Some(chrono::Utc::now());
                    entry.stdout_size = terminal_manager::get_file_size(&stdout_path).await;
                    entry.stderr_size = terminal_manager::get_file_size(&stderr_path).await;
                }

                // Remove cancellation token
                reg.cancellation_tokens.remove(&process_id);
                drop(reg);

                // Cleanup temp directory
                let _ = tokio::fs::remove_dir_all(&process_tmp_dir).await;

                let success = exit_code.unwrap_or(-1) == 0;
                let actual_exit_code = exit_code.unwrap_or(-1);

                // Construct JSON response with enhanced metadata
                let response = serde_json::json!({
                    "command": command,
                    "exit_code": actual_exit_code,
                    "stdout": stdout,
                    "stderr": stderr,
                    "status": if success { "finished" } else { "failed" },
                    "duration_ms": duration_ms,
                    "execution_type": "isolated"
                });

                info!(
                    "Isolated shell command executed: {} (session: {}, exit: {:?}, duration: {}ms)",
                    command, session_id, exit_code, duration_ms
                );

                // ✅ CRITICAL FIX: Handle non-zero exit codes as errors
                if !success {
                    let error_output = if !stderr.is_empty() {
                        format!("Error output:\n{}", stderr)
                    } else if !stdout.is_empty() {
                        format!("Command output:\n{}", stdout)
                    } else {
                        "No error output captured".to_string()
                    };

                    // Provide context-specific guidance based on exit code
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

                    return Ok(operation_failed_error(
                        "Execute shell command",
                        &format!(
                            "Command failed with exit code: {}\n\n{}",
                            actual_exit_code, error_output
                        ),
                        guidance,
                        ToolGroup::Workspace,
                    ));
                }

                // Enhanced text response with explicit status and output visibility
                let header = format!("Command executed in {}ms (exit code: 0)", duration_ms);

                // Include output in text message if available (CRITICAL FIX for sync visibility)
                let text_message = if !stdout.is_empty() {
                    format!("{}\n\nOutput:\n{}", header, stdout)
                } else if !stderr.is_empty() {
                    format!("{}\n\nStderr:\n{}", header, stderr)
                } else {
                    header
                };

                // ✅ ENHANCED: Detect signs of interactive prompts or cancelled operations
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

                // If interactive indicators detected, provide enhanced guidance
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
                        2. Or add non-interactive flags: --yes, --force, -y\n\
                        3. Or pipe input: echo y | command\n\n\
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
                            format!("Add non-interactive flags: {} --yes", command),
                            "Or use echo/stdin redirection for automated input".to_string(),
                        ],
                    );

                    return Ok(hint.to_mcp_result_with_data(Some(response)));
                }

                let hint = SuccessHint::new(
                    text_message,
                    SuccessHint::for_tool(PERSISTENT_SHELL_TOOL, ToolGroup::Workspace),
                );
                Ok(hint.to_mcp_result_with_data(Some(response)))
            }
            Ok(Err(e)) => {
                // Update registry entry to Failed
                if let Some(entry) = reg.entries.get_mut(&process_id) {
                    entry.status = terminal_manager::ProcessStatus::Failed;
                    entry.finished_at = Some(chrono::Utc::now());
                }
                reg.cancellation_tokens.remove(&process_id);
                drop(reg);

                // Cleanup temp directory
                let _ = tokio::fs::remove_dir_all(&process_tmp_dir).await;

                error!(
                    "Failed to execute isolated shell command '{}': {}",
                    command, e
                );
                Ok(operation_failed_error(
                    "Execute shell command",
                    &e.to_string(),
                    vec![
                        "Verify the command syntax is correct".to_string(),
                        "Check if required programs are installed".to_string(),
                        "Use listDirectory to verify file paths exist".to_string(),
                    ],
                    ToolGroup::Workspace,
                ))
            }
            Err(_) => {
                // Timeout - cancel the process
                cancel_token.cancel();

                // Update registry entry to Killed
                if let Some(entry) = reg.entries.get_mut(&process_id) {
                    entry.status = terminal_manager::ProcessStatus::Killed;
                    entry.finished_at = Some(chrono::Utc::now());
                }
                reg.cancellation_tokens.remove(&process_id);
                drop(reg);

                // Cleanup temp directory
                let _ = tokio::fs::remove_dir_all(&process_tmp_dir).await;

                error!(
                    "Isolated shell command '{}' timed out after {} seconds",
                    command, timeout_secs
                );

                // ✅ ENHANCED: Check if timeout might be due to waiting for interactive input
                let might_be_interactive = validation::is_likely_interactive_command(command);

                let error_message = if might_be_interactive {
                    format!(
                        "Command timed out after {} seconds (possibly waiting for interactive input)",
                        timeout_secs
                    )
                } else {
                    format!("Command timed out after {} seconds", timeout_secs)
                };

                let mut guidance = Vec::new();

                if might_be_interactive {
                    guidance.push(
                        "⚠️ This command may be waiting for interactive input (password, prompts, confirmations)".to_string()
                    );
                    guidance.push(format!(
                        "Use {} with requireUserInput: true for interactive commands",
                        PERSISTENT_SHELL_TOOL
                    ));
                    guidance.push(
                        "Or add non-interactive flags: --yes, --force, -y, --non-interactive"
                            .to_string(),
                    );
                    guidance
                        .push("Examples: npm init --yes, npx create-vite . --force".to_string());
                } else {
                    guidance.push(format!(
                        "Increase timeout parameter (current: {}s)",
                        timeout_secs
                    ));
                    guidance.push("Use spawnProcess for long-running background tasks".to_string());
                    guidance.push("Use pollProcess to check status of async commands".to_string());
                }

                Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::Timeout,
                    error_message,
                    guidance,
                    ToolGroup::Workspace,
                )
                .to_mcp_result())
            }
        }
    }
}
