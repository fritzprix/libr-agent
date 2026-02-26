use std::collections::HashMap;
use std::time::Duration;
use tracing::{error, info, warn};

use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, SuccessHint, ToolGroup};
use crate::mcp::types::MCPResult;

use super::super::super::{utils, WorkspaceServer, PERSISTENT_SHELL_TOOL};
use super::super::normalization;

impl WorkspaceServer {
    /// Execute command using persistent shell
    ///
    /// This method provides state preservation across commands (cd, export, venv)
    /// by reusing a single shell process per session.
    pub(crate) async fn execute_shell_persistent(
        &self,
        command: &str,
        timeout_secs: u64,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        let session_id = session_id.to_string();

        let workspace_path = self
            .session_manager
            .get_session_workspace_dir_by_id(&session_id);

        // Normalize command
        let normalized_command = normalization::normalize_shell_command(command);

        // Track execution time
        let execution_start = std::time::Instant::now();

        // Execute with timeout
        let timeout_duration = Duration::from_secs(timeout_secs);

        let execution_result = tokio::time::timeout(
            timeout_duration,
            self.shell_manager.execute(
                session_id.clone(),
                workspace_path.clone(),
                &normalized_command,
            ),
        )
        .await;

        match execution_result {
            Ok(Ok((stdout, stderr, exit_code, cwd))) => {
                // Measure duration
                let duration_ms = execution_start.elapsed().as_millis() as u64;

                // Success case - format result
                let success = exit_code == 0;

                info!(
                    "Persistent shell command executed: {} (session: {}, exit: {}, duration: {}ms)",
                    command, session_id, exit_code, duration_ms
                );

                let structured_data = serde_json::json!({
                    "command": command,
                    "exit_code": exit_code,
                    "stdout": stdout,
                    "stderr": stderr,
                    "cwd": cwd, // Return raw absolute path in data
                    "status": if success { "finished" } else { "failed" },
                    "duration_ms": duration_ms,
                    "execution_type": "persistent"
                });

                if success {
                    // Calculate relative path for display
                    let path_cwd = std::path::Path::new(&cwd);
                    let relative_cwd = path_cwd
                        .strip_prefix(&workspace_path)
                        .unwrap_or(path_cwd)
                        .to_string_lossy();

                    let display_cwd = if relative_cwd.is_empty() {
                        ".".to_string()
                    } else {
                        // Ensure it starts with ./ or .\ for clarity if it's relative
                        if relative_cwd.starts_with(".")
                            || relative_cwd.starts_with(std::path::MAIN_SEPARATOR)
                            || relative_cwd.contains(":")
                        {
                            relative_cwd.to_string()
                        } else {
                            format!(".{}{}", std::path::MAIN_SEPARATOR, relative_cwd)
                        }
                    };

                    // Invalidate service context cache to reflect CWD or status changes
                    self.invalidate_context_cache().await;

                    // Success - format with clear state reporting
                    let header = format!("Command executed successfully in {}ms", duration_ms);

                    // Clear shell state section with persistence indicator
                    let shell_state = format!(
                        "Persistent shell state (maintained for next {} call):\n  Working directory: {}\n  Exit code: {}",
                        PERSISTENT_SHELL_TOOL, display_cwd, exit_code
                    );

                    // Only show warning if shell CWD differs from workspace root for less noise
                    let file_tools_warning = if display_cwd != "." {
                        "\n⚠️  File tools (readFile, listDirectory) always use workspace root (.)\n    To list files in shell's current directory, use shell commands: ls or find"
                    } else {
                        ""
                    };

                    let text_message: String = if !stdout.is_empty() {
                        format!(
                            "{}\n\nCommand output:\n{}\n\n{}{}",
                            header, stdout, shell_state, file_tools_warning
                        )
                    } else {
                        format!("{}\n\n{}{}", header, shell_state, file_tools_warning)
                    };

                    let hint = SuccessHint::new(
                        text_message,
                        SuccessHint::for_tool(PERSISTENT_SHELL_TOOL, ToolGroup::Workspace),
                    );
                    Ok(hint.to_mcp_result_with_data(Some(structured_data)))
                } else {
                    // Failure - use ErrorGuidance format
                    let header = format!(
                        "Command failed in {}ms (exit code: {})",
                        duration_ms, exit_code
                    );

                    let error_message = if !stderr.is_empty() {
                        format!("{}\n\nstderr:\n{}", header, stderr)
                    } else {
                        header
                    };

                    Ok(guided_error(
                        ErrorCategory::OperationFailed,
                        error_message,
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Review the error message in stderr for details".to_string(),
                        "Check command syntax and file paths".to_string(),
                        "Use listDirectory to verify paths exist".to_string(),
                    ])
                    .to_mcp_result())
                }
            }
            Ok(Err(e)) => {
                // Execution error - shell crashed or command failed
                warn!(
                    "Persistent shell execution failed for session {}: {}. Falling back to one-shot.",
                    session_id, e
                );

                // Fallback to one-shot execution
                let isolation_level = utils::get_shell_isolation_level().await;
                self.execute_shell_with_isolation(
                    command,
                    isolation_level,
                    timeout_secs,
                    &session_id,
                    HashMap::new(), // Pass empty env vars for fallback
                )
                .await
            }
            Err(_) => {
                // Timeout
                warn!(
                    "Persistent shell execution timed out for session {}. Terminating shell to cleanup.",
                    session_id
                );

                // Cleanup: Terminate the stuck shell
                if let Err(e) = self.shell_manager.terminate_shell(&session_id).await {
                    error!(
                        "Failed to terminate stuck shell for session {}: {}",
                        session_id, e
                    );
                }

                // Return ErrorGuidance for timeout
                Ok(guided_error(
                    ErrorCategory::Timeout,
                    format!("Command execution timeout after {} seconds. The shell session has been reset.", timeout_secs),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Increase timeout value if the command needs more time".to_string(),
                    "Check if the command is stuck or waiting for input".to_string(),
                    "The shell session has been reset - execute the command again".to_string(),
                ])
                .to_mcp_result())
            }
        }
    }
}
