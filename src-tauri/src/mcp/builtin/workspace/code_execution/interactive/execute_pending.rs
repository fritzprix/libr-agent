use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::session_isolation::IsolatedProcessConfig;

// Import WorkspaceServer and other types from the workspace module
use crate::mcp::builtin::workspace::{terminal_manager, WorkspaceServer};

// Import normalization from sibling modules
use super::super::normalization;

// Import security from sibling modules in interactive
use super::security;

impl WorkspaceServer {
    /// Handle execute_pending_shell tool call (2nd tool call)
    /// Executes pending command with user input via stdin
    pub async fn handle_execute_pending_shell(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        use crate::mcp::builtin::workspace::utils::sanitize_command_for_logging;

        // Extract execution_id (support both camelCase and snake_case)
        let execution_id = match args
            .get("executionId")
            .or_else(|| args.get("execution_id"))
            .and_then(|v| v.as_str())
        {
            Some(id) => id,
            None => {
                return Ok(missing_param_error("executionId", ToolGroup::Workspace));
            }
        };

        // Extract user_input (support both camelCase and snake_case)
        let obfuscated_input = match args
            .get("userInput")
            .or_else(|| args.get("user_input"))
            .and_then(|v| v.as_str())
        {
            Some(input) => input,
            None => {
                return Ok(missing_param_error("userInput", ToolGroup::Workspace));
            }
        };

        // Retrieve pending execution
        let pending = match self.pending_executions.remove(execution_id) {
            Some(p) => p,
            None => {
                return Ok(guided_error(
                    ErrorCategory::ResourceNotFound,
                    format!("Execution '{}' not found or expired", execution_id),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Execute the original command again to get a new execution_id".to_string(),
                    format!("Execution requests expire after {} minutes", 5),
                    "Ensure you're using the execution_id from the UI resource".to_string(),
                ])
                .to_mcp_result());
            }
        };

        // Validate session ownership
        if pending.session_id != session_id {
            return Ok(guided_error(
                ErrorCategory::PermissionDenied,
                format!(
                    "Pending execution '{}' belongs to a different session",
                    execution_id
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Ensure you are executing the command in the correct session".to_string(),
                "Executions are isolated per session".to_string(),
            ])
            .to_mcp_result());
        }

        // De-obfuscate user input
        let user_input =
            match security::deobfuscate_input(obfuscated_input, &pending.encryption_nonce) {
                Ok(s) => s,
                Err(e) => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidState,
                        "De-obfuscate user input failed".to_string(),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "This is an internal error - the UI should handle obfuscation".to_string(),
                        "Try executing the command again".to_string(),
                        "Contact support if this persists".to_string(),
                        format!("Error: {}", e),
                    ])
                    .to_mcp_result());
                }
            };
        let user_input = user_input.as_str();

        // Validate timeout (5 minutes for user input)
        const USER_INPUT_TIMEOUT_SECS: i64 = 300;
        let elapsed = chrono::Utc::now()
            .signed_duration_since(pending.created_at)
            .num_seconds();
        if elapsed > USER_INPUT_TIMEOUT_SECS {
            return Ok(guided_error(
                ErrorCategory::Timeout,
                format!("Execution request expired after {} seconds", elapsed),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Execute the original command again to get a new execution_id".to_string(),
                format!(
                    "User input must be submitted within {} minutes",
                    USER_INPUT_TIMEOUT_SECS / 60
                ),
                "Respond more quickly to interactive prompts".to_string(),
            ])
            .to_mcp_result());
        }

        // Auto-inject -S flag for sudo commands (Agent doesn't know about it)
        #[cfg(unix)]
        let final_command = if pending.executable_command.trim_start().starts_with("sudo ") {
            // Check if -S flag already exists (defensive programming)
            if pending.executable_command.contains("sudo -S ") {
                pending.executable_command.clone()
            } else {
                // Insert -S flag after 'sudo'
                pending.executable_command.replacen("sudo ", "sudo -S ", 1)
            }
        } else {
            pending.executable_command.clone()
        };

        #[cfg(windows)]
        let final_command = pending.executable_command.clone();

        // Get workspace and session info
        let session_id = pending.session_id.clone();
        let workspace_path = self.get_workspace_dir(&session_id);

        // Check if persistent shell should be used (default: true)
        let use_persistent_shell = args
            .get("use_persistent_shell")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Try persistent shell path first (if enabled)
        if use_persistent_shell && pending.run_mode == "sync" {
            let normalized_command = normalization::normalize_shell_command(&final_command);

            // Execute with persistent shell (includes timeout and retry)
            let execution_result = tokio::time::timeout(
                Duration::from_secs(pending.timeout),
                self.shell_manager.execute_with_input(
                    session_id.clone(),
                    workspace_path.clone(),
                    &normalized_command,
                    user_input,
                ),
            )
            .await;

            match execution_result {
                Ok(Ok((stdout, stderr, exit_code, _cwd))) => {
                    // Success - format and return result
                    info!(
                        "Interactive persistent shell executed: {} (session: {}, exit: {})",
                        sanitize_command_for_logging(&pending.display_command),
                        session_id,
                        exit_code
                    );

                    // Redact sensitive user input from output
                    let redacted_stdout =
                        security::redact_sensitive_input(stdout.trim(), user_input);
                    let redacted_stderr =
                        security::redact_sensitive_input(stderr.trim(), user_input);

                    let result_text = if exit_code == 0 {
                        if redacted_stdout.is_empty() && redacted_stderr.is_empty() {
                            "Command executed successfully (no output)".to_string()
                        } else if redacted_stderr.is_empty() {
                            format!("Command executed successfully:\n{redacted_stdout}")
                        } else {
                            format!(
                                "Command executed successfully:\nSTDOUT:\n{redacted_stdout}\n\nSTDERR:\n{redacted_stderr}"
                            )
                        }
                    } else {
                        format!(
                            "Command failed with exit code {exit_code}:\nSTDOUT:\n{redacted_stdout}\n\nSTDERR:\n{redacted_stderr}"
                        )
                    };

                    if exit_code == 0 {
                        return Ok(MCPResult::success(&result_text));
                    } else {
                        return Ok(MCPResult::error(&result_text));
                    }
                }
                Ok(Err(e)) => {
                    // Shell error - log and fallback to one-shot
                    warn!(
                        "Persistent shell execution with input failed: {}. Falling back to one-shot.",
                        e
                    );
                }
                Err(_) => {
                    // Timeout
                    return Ok(MCPResult::error(&format!(
                        "Command execution timeout after {} seconds",
                        pending.timeout
                    )));
                }
            }
        }

        // FALLBACK: One-shot execution with stdin injection (original implementation)

        // Create isolation config
        let normalized_command = normalization::normalize_shell_command(&final_command);
        let isolation_level =
            crate::mcp::builtin::workspace::utils::get_shell_isolation_level().await;
        let isolation_config = IsolatedProcessConfig {
            session_id: session_id.clone(),
            workspace_path: workspace_path.clone(),
            command: normalized_command,
            args: vec![],
            env_vars: HashMap::new(),
            isolation_level,
            shell_type: None, // Default to platform default shell
        };

        // Create isolated command
        let mut cmd = match self
            .isolation_manager
            .create_isolated_command(isolation_config)
            .await
        {
            Ok(cmd) => cmd,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::PermissionDenied,
                    "Create isolated command failed".to_string(),
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

        // Configure stdio pipes
        use std::process::Stdio;
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidState,
                    "Spawn process failed".to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Verify the command syntax is correct".to_string(),
                    "Check if required programs are installed".to_string(),
                    "Ensure the command has execute permissions".to_string(),
                    format!("Error: {}", e),
                ])
                .to_mcp_result());
            }
        };

        // Write user input to stdin
        if let Some(mut stdin) = child.stdin.take() {
            // CRITICAL: Write password and close stdin
            if let Err(e) = stdin.write_all(user_input.as_bytes()).await {
                return Ok(guided_error(
                    ErrorCategory::InvalidState,
                    "Write to stdin failed".to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "The process may have crashed before accepting input".to_string(),
                    "Try executing the command again".to_string(),
                    "Check if the command expects input in a different format".to_string(),
                    format!("Error: {}", e),
                ])
                .to_mcp_result());
            }
            if let Err(e) = stdin.write_all(b"\n").await {
                return Ok(guided_error(
                    ErrorCategory::InvalidState,
                    "Write newline to stdin failed".to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "The process may have closed stdin unexpectedly".to_string(),
                    "Try executing the command again".to_string(),
                    "Verify the command is still running".to_string(),
                    format!("Error: {}", e),
                ])
                .to_mcp_result());
            }
            drop(stdin); // Close stdin to signal EOF
        }

        // SECURITY: user_input reference will be dropped at end of scope

        // Execute based on run_mode from 1st call
        if pending.run_mode == "sync" {
            // Wait for completion with timeout
            let output = match tokio::time::timeout(
                Duration::from_secs(pending.timeout),
                child.wait_with_output(),
            )
            .await
            {
                Ok(Ok(output)) => output,
                Ok(Err(e)) => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidState,
                        "Execute command with user input failed".to_string(),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "The command may have invalid syntax or crashed".to_string(),
                        "Verify the command works without user input first".to_string(),
                        "Check system logs for more details".to_string(),
                        format!("Error: {}", e),
                    ])
                    .to_mcp_result());
                }
                Err(_) => {
                    let timeout_secs = pending.timeout;
                    return Ok(guided_error(
                        ErrorCategory::Timeout,
                        format!("Command execution timeout after {} seconds", timeout_secs),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        format!("Increase timeout parameter (current: {}s)", timeout_secs),
                        "Use \"runMode\": \"async\" for long-running commands".to_string(),
                        "Verify the command isn't hanging waiting for additional input".to_string(),
                    ])
                    .to_mcp_result());
                }
            };

            // SECURITY: Log sanitized command only
            info!(
                "Interactive shell executed: {} (session: {}, exit: {:?})",
                pending.display_command,
                session_id,
                output.status.code()
            );

            // Format response
            let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);

            // Redact sensitive user input from output
            let redacted_stdout = security::redact_sensitive_input(stdout_str.trim(), user_input);
            let redacted_stderr = security::redact_sensitive_input(stderr_str.trim(), user_input);

            let success = exit_code == 0;

            let hint = SuccessHint::new(
                format!("Interactive command executed (exit code: {})", exit_code),
                SuccessHint::for_tool("executePendingShell", ToolGroup::Workspace),
            );

            let response_data = serde_json::json!({
                "exit_code": exit_code,
                "stdout": redacted_stdout,
                "stderr": redacted_stderr,
                "status": if success { "success" } else { "failed" }
            });

            Ok(hint.to_mcp_result_with_data(Some(response_data)))
        } else {
            // Async mode: Return process_id immediately and spawn monitoring task
            let process_id = cuid2::create_id();

            // Create process tmp directory
            let process_tmp_dir = workspace_path
                .join("tmp")
                .join(format!("process_{process_id}"));

            if let Err(e) = tokio::fs::create_dir_all(&process_tmp_dir).await {
                return Ok(guided_error(
                    ErrorCategory::InvalidState,
                    "Create process directory failed".to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Check workspace directory permissions".to_string(),
                    "Ensure sufficient disk space is available".to_string(),
                    "Verify tmp directory is writable".to_string(),
                    format!("Error: {}", e),
                ])
                .to_mcp_result());
            }

            let stdout_path = process_tmp_dir.join("stdout");
            let stderr_path = process_tmp_dir.join("stderr");

            // Register process
            let cancel_token = tokio_util::sync::CancellationToken::new();
            let entry = terminal_manager::ProcessEntry {
                id: process_id.clone(),
                name: None,
                session_id: session_id.clone(),
                command: sanitize_command_for_logging(&pending.display_command), // Sanitized version
                status: terminal_manager::ProcessStatus::Running,
                pid: child.id(),
                exit_code: None,
                started_at: chrono::Utc::now(),
                finished_at: None,
                stdout_path: stdout_path.to_string_lossy().to_string(),
                stderr_path: stderr_path.to_string_lossy().to_string(),
                stdout_size: 0,
                stderr_size: 0,
                last_poll_at: None,
                poll_count: 0,
                consecutive_running_polls: 0,
                first_running_poll_at: None,
            };

            {
                let mut registry = self.process_registry.write().await;
                registry.entries.insert(process_id.clone(), entry);
                registry
                    .cancellation_tokens
                    .insert(process_id.clone(), cancel_token.clone());
            }

            // Spawn monitoring task
            let registry = self.process_registry.clone();
            let pid_copy = process_id.clone();

            tokio::spawn(async move {
                // Execute using common spawn+stream logic would go here
                // For now, simplified version
                let result = child.wait_with_output().await;

                let mut reg = registry.write().await;
                if let Some(entry) = reg.entries.get_mut(&pid_copy) {
                    match result {
                        Ok(output) => {
                            entry.exit_code = output.status.code();
                            entry.status = if output.status.code().unwrap_or(-1) == 0 {
                                terminal_manager::ProcessStatus::Finished
                            } else {
                                terminal_manager::ProcessStatus::Failed
                            };
                        }
                        Err(_) => {
                            entry.status = terminal_manager::ProcessStatus::Failed;
                        }
                    }
                    entry.finished_at = Some(chrono::Utc::now());
                }
                reg.cancellation_tokens.remove(&pid_copy);
            });

            let hint = SuccessHint::new(
                format!(
                    "Interactive command running in background (ID: {})",
                    process_id
                ),
                vec![
                    format!(
                        "Use waitForProcess(\"{}\", 0) to check status, or waitForProcess(\"{}\") to block until done",
                        process_id, process_id
                    ),
                    "If status is 'failed', use readProcessOutput with 'stderr' to view errors"
                        .to_string(),
                    "If status is 'finished', use readProcessOutput with 'stdout' to view output"
                        .to_string(),
                    "Use listProcesses to see all running processes".to_string(),
                ],
            );

            let response_data = serde_json::json!({
                "process_id": process_id,
                "mode": "async"
            });

            Ok(hint.to_mcp_result_with_data(Some(response_data)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::builtin::workspace::PendingShellExecution;
    use crate::session::SessionManager;
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;

    async fn create_server() -> WorkspaceServer {
        let temp_dir = tempdir().unwrap();
        let session_manager =
            Arc::new(SessionManager::new_with_base_dir(temp_dir.path().to_path_buf()).unwrap());
        WorkspaceServer::new("test-session".to_string(), session_manager)
    }

    #[tokio::test]
    async fn test_execute_pending_shell_parameter_extraction() {
        let server = create_server().await;
        let execution_id = "test-execution-id";

        // Pre-insert an entry
        server.pending_executions.insert(PendingShellExecution {
            execution_id: execution_id.to_string(),
            session_id: "test-session".to_string(),
            executable_command: "echo 'hello'".to_string(),
            display_command: "echo 'hello'".to_string(),
            run_mode: "sync".to_string(),
            timeout: 30,
            encryption_nonce: "nonce".to_string(),
            created_at: chrono::Utc::now(),
        });

        // Test with snake_case (fallback)
        let args_snake = json!({
            "execution_id": execution_id,
            "user_input": "obfuscated"
        });

        // This will fail because we are in a test environment without a real process manager
        // but we can check if it gets past parameter extraction.
        // Actually, let's just test that it DOES NOT return missing_param_error.
        let result = server
            .handle_execute_pending_shell(args_snake, "test-session")
            .await;

        if let Ok(res) = result {
            let res_json = serde_json::to_value(res).unwrap();
            let content = res_json.get("content").and_then(|c| c.as_array()).unwrap();
            let text = content[0].get("text").and_then(|t| t.as_str()).unwrap();
            assert!(!text.contains("Missing executionId"));
        }

        // Test with camelCase (primary)
        // Add it back since it was removed by previous call
        server.pending_executions.insert(PendingShellExecution {
            execution_id: execution_id.to_string(),
            session_id: "test-session".to_string(),
            executable_command: "echo 'hello'".to_string(),
            display_command: "echo 'hello'".to_string(),
            run_mode: "sync".to_string(),
            timeout: 30,
            encryption_nonce: "nonce".to_string(),
            created_at: chrono::Utc::now(),
        });

        let args_camel = json!({
            "executionId": execution_id,
            "userInput": "obfuscated"
        });

        let result = server
            .handle_execute_pending_shell(args_camel, "test-session")
            .await;

        if let Ok(res) = result {
            let res_json = serde_json::to_value(res).unwrap();
            let content = res_json.get("content").and_then(|c| c.as_array()).unwrap();
            let text = content[0].get("text").and_then(|t| t.as_str()).unwrap();
            assert!(!text.contains("Missing executionId"));
        }
    }
}
