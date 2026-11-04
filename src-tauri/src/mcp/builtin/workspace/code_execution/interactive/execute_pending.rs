use std::time::Duration;
use tracing::{error, warn};

use crate::mcp::builtin::error_guidance::SuccessHint;
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::builtin::workspace::code_execution::normalization;
use crate::mcp::builtin::workspace::code_execution::shell::{
    format_command_io_message, format_duration_ms, shell_signal_interrupt_result,
    truncate_sync_shell_stream,
};
use crate::mcp::builtin::workspace::code_execution::validation;
use crate::mcp::builtin::workspace::{
    InteractiveShellInputType, InteractiveShellRequest, WorkspaceServer, PERSISTENT_SHELL_TOOL,
};
use crate::mcp::types::MCPResult;

impl WorkspaceServer {
    pub(crate) async fn execute_interactive_shell_with_input(
        &self,
        pending: InteractiveShellRequest,
        user_input: String,
    ) -> Result<MCPResult, String> {
        let session_id = pending.session_id.clone();
        let workspace_path = self
            .session_manager
            .get_session_workspace_dir_by_id(&session_id);
        let previous_cwd = self.shell_manager.get_shell_cwd(&session_id).await;
        let normalized_command =
            normalization::normalize_shell_command(&pending.executable_command);
        let execution_start = std::time::Instant::now();

        let timeout_duration = Duration::from_secs(pending.timeout);
        let execution_result = tokio::time::timeout(
            timeout_duration,
            self.shell_manager.execute_with_input(
                session_id.clone(),
                workspace_path.clone(),
                &normalized_command,
                &user_input,
                pending.stdin_delivery,
            ),
        )
        .await;

        let (stdout, stderr, exit_code, cwd) = match execution_result {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => return Err(error),
            Err(_) => {
                warn!(
                    "Interactive persistent shell execution timed out for session {}. Terminating shell to cleanup.",
                    session_id
                );

                if let Err(error) = self.shell_manager.restart_shell(&session_id).await {
                    error!(
                        "Failed to terminate timed out interactive shell for session {}: {}",
                        session_id, error
                    );
                }

                return Ok(guided_error(
                    ErrorCategory::Timeout,
                    format!(
                        "Interactive command timed out after {} seconds",
                        pending.timeout
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    format!(
                        "Re-run {} if you want to retry the interactive command",
                        PERSISTENT_SHELL_TOOL
                    ),
                    "The persistent shell was terminated to recover from the stuck command"
                        .to_string(),
                ])
                .to_mcp_result());
            }
        };

        let duration_ms = execution_start.elapsed().as_millis() as u64;
        let redact_output = pending.input_type == InteractiveShellInputType::Password;
        let output_redacted_notice =
            "Command output was redacted because this prompt used password-mode interactive input.";
        let interrupted = validation::is_signal_interrupt_exit(exit_code);
        let status = if exit_code == 0 {
            "finished"
        } else if interrupted {
            "interrupted"
        } else {
            "failed"
        };
        let structured_data = if redact_output {
            serde_json::json!({
                "command": pending.display_command,
                "exit_code": exit_code,
                "cwd": cwd,
                "status": status,
                "duration_ms": duration_ms,
                "execution_type": "persistent",
                "output_redacted": true
            })
        } else {
            serde_json::json!({
                "command": pending.display_command,
                "exit_code": exit_code,
                "stdout": stdout,
                "stderr": stderr,
                "cwd": cwd,
                "status": status,
                "duration_ms": duration_ms,
                "execution_type": "persistent"
            })
        };

        if interrupted {
            let (stdout_for_msg, stderr_for_msg) = if redact_output {
                (output_redacted_notice, "")
            } else {
                (stdout.as_str(), stderr.as_str())
            };
            return Ok(shell_signal_interrupt_result(
                exit_code,
                duration_ms,
                stdout_for_msg,
                stderr_for_msg,
                structured_data,
            ));
        }

        if exit_code != 0 {
            let mut error_sections = Vec::new();
            if redact_output {
                error_sections.push(output_redacted_notice.to_string());
            } else if !stdout.is_empty() {
                error_sections.push(format!("Output:\n{}", truncate_sync_shell_stream(&stdout)));
            }
            if !redact_output && !stderr.is_empty() {
                error_sections.push(format!("Stderr:\n{}", truncate_sync_shell_stream(&stderr)));
            }

            let error_output = if error_sections.is_empty() {
                "No output captured.".to_string()
            } else {
                error_sections.join("\n\n")
            };

            return Ok(guided_error(
                ErrorCategory::OperationFailed,
                format!(
                    "Command failed with exit code: {}\n\n{}",
                    exit_code, error_output
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Review the command output above for the exact failure reason".to_string(),
                format!(
                    "Re-run {} if the command needs another interactive attempt",
                    PERSISTENT_SHELL_TOOL
                ),
            ])
            .to_mcp_result());
        }

        self.invalidate_context_cache().await;

        let path_cwd = std::path::Path::new(&cwd);
        let relative_cwd = path_cwd
            .strip_prefix(&workspace_path)
            .unwrap_or(path_cwd)
            .to_string_lossy();
        let display_cwd = if relative_cwd.is_empty() {
            ".".to_string()
        } else if relative_cwd.starts_with(".")
            || relative_cwd.starts_with(std::path::MAIN_SEPARATOR)
            || relative_cwd.contains(":")
        {
            relative_cwd.to_string()
        } else {
            format!(".{}{}", std::path::MAIN_SEPARATOR, relative_cwd)
        };

        let previous_display_cwd = previous_cwd.as_deref().map(|previous| {
            let previous_path = std::path::Path::new(previous);
            let relative_previous = previous_path
                .strip_prefix(&workspace_path)
                .unwrap_or(previous_path)
                .to_string_lossy();

            if relative_previous.is_empty() {
                ".".to_string()
            } else if relative_previous.starts_with(".")
                || relative_previous.starts_with(std::path::MAIN_SEPARATOR)
                || relative_previous.contains(":")
            {
                relative_previous.to_string()
            } else {
                format!(".{}{}", std::path::MAIN_SEPARATOR, relative_previous)
            }
        });

        let cwd_changed = previous_display_cwd
            .as_deref()
            .map(|previous| previous != display_cwd)
            .unwrap_or(display_cwd != ".");

        let header = format!(
            "Interactive command executed in {} (exit code: 0)",
            format_duration_ms(duration_ms)
        );
        let shell_state = format!(
            "Persistent shell state (maintained for next {} call):\n  Working directory: {}\n  Exit code: {}",
            PERSISTENT_SHELL_TOOL, display_cwd, exit_code
        );
        let file_tools_warning = if display_cwd != "." && cwd_changed {
            "\n⚠️  workspace__readFile and workspace__listDirectory still use workspace root, not the shell CWD\n    Use absolute file-tool paths if you need the current shell directory"
        } else {
            ""
        };

        let text_message = if redact_output {
            format!("{header}\n\n{output_redacted_notice}\n\n{shell_state}{file_tools_warning}")
        } else {
            let mut io_message =
                format_command_io_message(&header, "Command output", &stdout, "Stderr", &stderr);
            if let Some(warning) = validation::exit_zero_pipeline_failure_warning(
                &pending.display_command,
                &stdout,
                &stderr,
            ) {
                io_message.push_str("\n\n");
                io_message.push_str(warning);
            }
            format!("{io_message}\n\n{shell_state}{file_tools_warning}")
        };

        let hint = SuccessHint::new(
            text_message,
            vec![
                "Command state (CWD, env vars) is preserved for the next call".to_string(),
                "The interactive input was handled locally and was not added to chat history"
                    .to_string(),
            ],
        );
        Ok(hint.to_mcp_result_with_data(Some(structured_data)))
    }
}
