use std::collections::HashMap;
use std::time::Duration;
use tracing::{error, info, warn};

use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, SuccessHint, ToolGroup};
use crate::mcp::types::MCPResult;
use crate::models::workspace_isolation::WorkspaceIsolationMode;
use crate::repositories::SessionRepository;
use crate::session_isolation::PathMappingLayer;

use super::super::super::{utils, WorkspaceServer, PERSISTENT_SHELL_TOOL};
use super::super::normalization;
use super::format_duration_ms;

impl WorkspaceServer {
    /// Execute command using persistent shell
    ///
    /// This method provides state preservation across commands (cd, export, venv)
    /// by reusing a single shell process per session.
    pub(crate) async fn execute_shell_persistent(
        &self,
        command: &str,
        tool_name: &str,
        timeout_secs: u64,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        let session_id = session_id.to_string();

        let workspace_path = self
            .session_manager
            .get_session_workspace_dir_by_id(&session_id);
        let previous_cwd = self.shell_manager.get_shell_cwd(&session_id).await;
        let current_dir = previous_cwd.as_deref().map(std::path::Path::new);
        let docker_path_mapper = docker_path_mapper_for_session(&session_id).await?;

        if let Some(result) =
            self.apply_shell_policy_block(tool_name, command, &workspace_path, current_dir, None)
        {
            return Ok(result);
        }

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
                    let display_cwd =
                        display_shell_cwd(&cwd, &workspace_path, docker_path_mapper.as_ref());

                    // Invalidate service context cache to reflect CWD or status changes
                    self.invalidate_context_cache().await;

                    // Success - format with clear state reporting
                    let header = format!(
                        "Command executed in {} (exit code: 0)",
                        format_duration_ms(duration_ms)
                    );

                    // Clear shell state section with persistence indicator
                    let shell_state = format!(
                        "Persistent shell state (maintained for next {} call):\n  Working directory: {}\n  Exit code: {}",
                        PERSISTENT_SHELL_TOOL, display_cwd, exit_code
                    );

                    let previous_display_cwd = previous_cwd.as_deref().map(|previous| {
                        display_shell_cwd(previous, &workspace_path, docker_path_mapper.as_ref())
                    });

                    let cwd_changed = previous_display_cwd
                        .as_deref()
                        .map(|previous| previous != display_cwd)
                        .unwrap_or(display_cwd != ".");

                    // Only warn when this call moved the shell away from workspace root or to another directory.
                    let file_tools_warning = if docker_path_mapper
                        .as_ref()
                        .is_some_and(|mapper| mapper.container_to_host(&cwd).is_none())
                    {
                        "\n⚠️  Shell CWD is outside /workspace; workspace__readFile/workspace__listDirectory/workspace__writeFile cannot map this container path"
                    } else if display_cwd != "." && cwd_changed {
                        "\n⚠️  workspace__readFile and workspace__listDirectory still use workspace root, not the shell CWD\n    Use /workspace absolute paths, relative workspace paths, or shell commands like ls/find for the current shell directory"
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

                    let next_actions = if display_cwd == "." {
                        vec![]
                    } else {
                        vec![
                            format!(
                                "Use {} with shell commands like `pwd` or `ls` to inspect the current shell directory",
                                tool_name
                            ),
                            "Use absolute file-tool paths if you want them to target the shell's current directory"
                                .to_string(),
                        ]
                    };

                    let hint = SuccessHint::new(text_message, next_actions);
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
                    .guidance(super::super::validation::shell_command_failure_guidance(
                        Some(exit_code),
                        &stdout,
                        &stderr,
                    ))
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
                    tool_name,
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

async fn docker_path_mapper_for_session(
    session_id: &str,
) -> Result<Option<PathMappingLayer>, String> {
    let Some(session_repo) = crate::state::try_get_session_repository() else {
        return Ok(None);
    };
    let Some(session) = session_repo
        .get_session(session_id)
        .await
        .map_err(|e| format!("Failed to load session isolation metadata: {e}"))?
    else {
        return Ok(None);
    };

    if session.workspace_isolation != WorkspaceIsolationMode::Docker {
        return Ok(None);
    }

    let host_workspace = session
        .docker_host_workspace_path
        .as_ref()
        .ok_or_else(|| format!("Missing Docker host workspace path for session {session_id}"))?;

    let workdir = session
        .docker_config
        .as_ref()
        .map(|config| config.workdir().to_string())
        .unwrap_or_else(|| crate::models::workspace_isolation::DEFAULT_DOCKER_WORKDIR.to_string());

    Ok(Some(PathMappingLayer::with_container_root(
        std::path::PathBuf::from(host_workspace),
        workdir,
    )))
}

fn display_shell_cwd(
    cwd: &str,
    workspace_path: &std::path::Path,
    docker_path_mapper: Option<&PathMappingLayer>,
) -> String {
    if let Some(mapper) = docker_path_mapper {
        let cwd_path = std::path::Path::new(cwd);
        if cwd_path == mapper.container_workspace() {
            return ".".to_string();
        }
        if let Ok(relative_cwd) = cwd_path.strip_prefix(mapper.container_workspace()) {
            return display_relative_path(relative_cwd);
        }
        return cwd.to_string();
    }

    let path_cwd = std::path::Path::new(cwd);
    let relative_cwd = path_cwd.strip_prefix(workspace_path).unwrap_or(path_cwd);
    display_relative_path(relative_cwd)
}

fn display_relative_path(relative_cwd: &std::path::Path) -> String {
    let relative_cwd = relative_cwd.to_string_lossy();
    if relative_cwd.is_empty() {
        ".".to_string()
    } else if relative_cwd.starts_with(".")
        || relative_cwd.starts_with(std::path::MAIN_SEPARATOR)
        || relative_cwd.contains(":")
    {
        relative_cwd.to_string()
    } else {
        format!(".{}{}", std::path::MAIN_SEPARATOR, relative_cwd)
    }
}
