use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::mcp::builtin::error_guidance::{
    missing_param_error, operation_failed_error, ErrorCategory, ErrorGuidance, SuccessHint,
    ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::session_isolation::{IsolatedProcessConfig, IsolationLevel};

use super::super::{terminal_manager, utils, WorkspaceServer, PERSISTENT_SHELL_TOOL};
use super::process;

// We need to implement methods on WorkspaceServer directly in the crate's context
// This file is included via `mod code_execution { pub mod shell; }` in `mod.rs`.
// So `impl WorkspaceServer` here adds methods to `WorkspaceServer`.

impl WorkspaceServer {
    /// Handle createInteractiveShell tool
    pub async fn handle_create_interactive_shell(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        // Extract shell ID (default to "default" if not provided, processed in manager)
        let shell_id = args
            .get("shellId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let workspace_path = self
            .session_manager
            .get_session_workspace_dir_by_id(session_id);

        // Create or reuse shell
        let shell = self
            .shell_manager
            .get_or_create_shell(session_id.to_string(), shell_id.clone(), workspace_path.clone())
            .await
            .map_err(|e| format!("Failed to create shell: {}", e))?;

        // Lock to get status, then drop immediately
        let (cwd, pid) = {
            let mut shell_guard = shell.lock().await;
            (shell_guard.get_cwd().to_string(), shell_guard.pid())
        };

        let actual_shell_id = shell_id.unwrap_or_else(|| "default".to_string());

        let text_message = format!(
            "[CWD: {}]\n[STATUS: Ready]\n[PID: {:?}]\n---\nInteractive shell created for session: {} (ID: {})",
            cwd, pid, session_id, actual_shell_id
        );

        let structured_data = serde_json::json!({
            "shell_id": actual_shell_id,
            "cwd": cwd,
            "pid": pid,
            "status": "ready",
            "session_id": session_id
        });

        Ok(MCPResult::success_with_data(&text_message, structured_data))
    }

    /// Handle writeToInteractiveShell tool
    pub async fn handle_write_to_interactive_shell(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        let shell_id = args
            .get("shellId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let input = match args.get("input").and_then(|v| v.as_str()) {
            Some(i) => i,
            None => {
                return Ok(missing_param_error("input", ToolGroup::Workspace));
            }
        };

        let send_newline = args
            .get("sendNewline")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let workspace_path = self
            .session_manager
            .get_session_workspace_dir_by_id(session_id);

        let shell = self
            .shell_manager
            .get_or_create_shell(session_id.to_string(), shell_id.clone(), workspace_path)
            .await
            .map_err(|e| format!("Shell not found: {}", e))?;

        let cwd = {
            let mut shell_guard = shell.lock().await;

            // Write input
            let full_input = if send_newline {
                format!("{}\n", input)
            } else {
                input.to_string()
            };

            shell_guard
                .write_stdin_raw(&full_input, true)
                .await
                .map_err(|e| format!("Failed to write to shell: {}", e))?;

            shell_guard.get_cwd().to_string()
        };

        let actual_shell_id = shell_id.unwrap_or_else(|| "default".to_string());

        let text_message = format!(
            "[CWD: {}]\n[STATUS: Input sent]\n---\n> {}",
            cwd, input
        );

        let structured_data = serde_json::json!({
            "shell_id": actual_shell_id,
            "input": input,
            "cwd": cwd
        });

        Ok(MCPResult::success_with_data(&text_message, structured_data))
    }

    /// Handle readFromInteractiveShell tool
    pub async fn handle_read_from_interactive_shell(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        let shell_id = args
            .get("shellId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let timeout_ms = args
            .get("timeoutMs")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);

        let wait_for_pattern = args.get("waitForPattern").and_then(|v| v.as_str());

        let workspace_path = self
            .session_manager
            .get_session_workspace_dir_by_id(session_id);

        let shell = self
            .shell_manager
            .get_or_create_shell(session_id.to_string(), shell_id.clone(), workspace_path)
            .await
            .map_err(|e| format!("Shell not found: {}", e))?;

        let (stdout, has_more, cwd) = {
            let mut shell_guard = shell.lock().await;

            // Read output
            let (stdout, _stderr, has_more) = if let Some(pattern) = wait_for_pattern {
                // Pattern-based read
                let timeout_secs = (timeout_ms / 1000).max(1);
                match shell_guard.read_until_pattern(pattern, timeout_secs).await {
                    Ok(output) => (output, String::new(), true),
                    Err(_) => {
                        shell_guard.read_output_nonblocking(100).await
                            .map_err(|e| format!("Failed to read after pattern timeout: {}", e))?
                    }
                }
            } else {
                // Timeout-based read
                shell_guard
                    .read_output_nonblocking(timeout_ms)
                    .await
                    .map_err(|e| format!("Failed to read output: {}", e))?
            };
            (stdout, has_more, shell_guard.get_cwd().to_string())
        };

        let status = if has_more { "Active" } else { "Finished" }; // PTY is always active unless closed

        let actual_shell_id = shell_id.unwrap_or_else(|| "default".to_string());

        let text_message = format!(
            "[CWD: {}]\n[STATUS: {}]\n---\n{}",
            cwd, status, stdout
        );

        let structured_data = serde_json::json!({
            "shell_id": actual_shell_id,
            "stdout": stdout,
            "stderr": "", // PTY merges streams
            "cwd": cwd,
            "status": status.to_lowercase(),
            "has_more": has_more
        });

        Ok(MCPResult::success_with_data(&text_message, structured_data))
    }

    /// Handle killInteractiveShell tool
    pub async fn handle_kill_interactive_shell(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        let shell_id = args
            .get("shellId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()); // Keeps Option<String>

        let actual_shell_id = shell_id.clone().unwrap_or_else(|| "default".to_string());

        self.shell_manager
            .terminate_shell(session_id, shell_id.as_deref())
            .await
            .map_err(|e| format!("Failed to terminate shell: {}", e))?;

        let text_message = format!(
            "[STATUS: Terminated]\n---\nShell session '{}' has been terminated",
            actual_shell_id
        );

        let structured_data = serde_json::json!({
            "shell_id": actual_shell_id,
            "status": "terminated"
        });

        Ok(MCPResult::success_with_data(&text_message, structured_data))
    }

    /// Execute command using persistent shell (Legacy)
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

        let normalized_command = Self::normalize_shell_command(command);
        let execution_start = std::time::Instant::now();
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
                let duration_ms = execution_start.elapsed().as_millis() as u64;
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
                    "cwd": cwd,
                    "status": if success { "finished" } else { "failed" },
                    "duration_ms": duration_ms,
                    "execution_type": "persistent"
                });

                if success {
                    let path_cwd = std::path::Path::new(&cwd);
                    let relative_cwd = path_cwd
                        .strip_prefix(&workspace_path)
                        .unwrap_or(path_cwd)
                        .to_string_lossy();

                    let display_cwd = if relative_cwd.is_empty() {
                        ".".to_string()
                    } else {
                        if relative_cwd.starts_with(".")
                            || relative_cwd.starts_with("/")
                            || relative_cwd.contains(":")
                        {
                            relative_cwd.to_string()
                        } else {
                            format!("./{}", relative_cwd)
                        }
                    };

                    let header = format!("✓ Command executed successfully in {}ms", duration_ms);

                    let shell_state = format!(
                        "Persistent shell state (maintained for next {} call):\n  Working directory: {}\n  Exit code: {}",
                        PERSISTENT_SHELL_TOOL, display_cwd, exit_code
                    );

                    let file_tools_warning = "⚠️  File tools (readFile, listDirectory) always use workspace root (.)\n    To list files in shell's current directory, use shell commands: ls or find";

                    let text_message: String = if !stdout.is_empty() {
                        format!(
                            "{}\n\nCommand output:\n{}\n\n{}\n\n{}",
                            header, stdout, shell_state, file_tools_warning
                        )
                    } else {
                        format!("{}\n\n{}\n\n{}", header, shell_state, file_tools_warning)
                    };
                    Ok(MCPResult::success_with_data(
                        text_message.as_str(),
                        structured_data,
                    ))
                } else {
                    let header = format!(
                        "Command failed in {}ms (exit code: {})",
                        duration_ms, exit_code
                    );

                    // Note: stderr is likely empty with PTY, so we check stdout as well for error indication context if needed
                    let error_message = if !stderr.is_empty() {
                        format!("{}\n\nstderr:\n{}", header, stderr)
                    } else if !stdout.is_empty() {
                         format!("{}\n\nOutput:\n{}", header, stdout)
                    } else {
                        header
                    };

                    Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::OperationFailed,
                        error_message,
                        vec![
                            "Review the error message in output for details".to_string(),
                            "Check command syntax and file paths".to_string(),
                            "Use listDirectory to verify paths exist".to_string(),
                        ],
                        ToolGroup::Workspace,
                    )
                    .to_mcp_result())
                }
            }
            Ok(Err(e)) => {
                warn!(
                    "Persistent shell execution failed for session {}: {}. Falling back to one-shot.",
                    session_id, e
                );

                let isolation_level = IsolationLevel::Medium;
                self.execute_shell_with_isolation(
                    command,
                    isolation_level,
                    timeout_secs,
                    &session_id,
                )
                .await
            }
            Err(_) => {
                warn!(
                    "Persistent shell execution timed out for session {}. Terminating shell to cleanup.",
                    session_id
                );

                if let Err(e) = self.shell_manager.terminate_shell(&session_id, None).await {
                    error!(
                        "Failed to terminate stuck shell for session {}: {}",
                        session_id, e
                    );
                }

                Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::Timeout,
                    format!("Command execution timeout after {} seconds. The shell session has been reset.", timeout_secs),
                    vec![
                        "Increase timeout value if the command needs more time".to_string(),
                        "Check if the command is stuck or waiting for input".to_string(),
                        "The shell session has been reset - execute the command again".to_string(),
                    ],
                    ToolGroup::Workspace,
                ).to_mcp_result())
            }
        }
    }

    /// Execute shell commands with isolation
    pub(crate) async fn execute_shell_with_isolation(
        &self,
        command: &str,
        isolation_level: IsolationLevel,
        timeout_secs: u64,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        let session_id = session_id.to_string();

        let workspace_path = self
            .session_manager
            .get_session_workspace_dir_by_id(&session_id);

        let normalized_command = Self::normalize_shell_command(command);
        let execution_start = std::time::Instant::now();
        let process_id = cuid2::create_id();

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
            env_vars: HashMap::new(),
            isolation_level,
        };

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

        let cancel_token = CancellationToken::new();

        let entry = terminal_manager::ProcessEntry {
            id: process_id.clone(),
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

        let mut reg = self.process_registry.write().await;

        match execution_result {
            Ok(Ok((pid, exit_code, stdout, stderr))) => {
                let duration_ms = execution_start.elapsed().as_millis() as u64;

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

                reg.cancellation_tokens.remove(&process_id);
                drop(reg);

                let _ = tokio::fs::remove_dir_all(&process_tmp_dir).await;

                let success = exit_code.unwrap_or(-1) == 0;

                let response = serde_json::json!({
                    "command": command,
                    "exit_code": exit_code.unwrap_or(-1),
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

                let header = format!(
                    "Command executed in {}ms (exit code: {})",
                    duration_ms,
                    exit_code.unwrap_or(-1)
                );

                let text_message = if !stdout.is_empty() {
                    format!("{}\n\nOutput:\n{}", header, stdout)
                } else if !stderr.is_empty() {
                    format!("{}\n\nStderr:\n{}", header, stderr)
                } else {
                    header
                };

                let hint = SuccessHint::new(
                    text_message,
                    SuccessHint::for_tool(PERSISTENT_SHELL_TOOL, ToolGroup::Workspace),
                );
                Ok(hint.to_mcp_result_with_data(Some(response)))
            }
            Ok(Err(e)) => {
                if let Some(entry) = reg.entries.get_mut(&process_id) {
                    entry.status = terminal_manager::ProcessStatus::Failed;
                    entry.finished_at = Some(chrono::Utc::now());
                }
                reg.cancellation_tokens.remove(&process_id);
                drop(reg);

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
                cancel_token.cancel();

                if let Some(entry) = reg.entries.get_mut(&process_id) {
                    entry.status = terminal_manager::ProcessStatus::Killed;
                    entry.finished_at = Some(chrono::Utc::now());
                }
                reg.cancellation_tokens.remove(&process_id);
                drop(reg);

                let _ = tokio::fs::remove_dir_all(&process_tmp_dir).await;

                error!(
                    "Isolated shell command '{}' timed out after {} seconds",
                    command, timeout_secs
                );
                Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::Timeout,
                    format!("Command timed out after {} seconds", timeout_secs),
                    vec![
                        format!("Increase timeout parameter (current: {}s)", timeout_secs),
                        "Use \"runMode\": \"async\" for long-running commands".to_string(),
                        "Use pollProcess to check status of async commands".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result())
            }
        }
    }

    /// Normalize shell command for proper execution
    pub(crate) fn normalize_shell_command(raw_command: &str) -> String {
        #[cfg(windows)]
        {
            info!("Windows command (no normalization): {}", raw_command);
            raw_command.to_string()
        }

        #[cfg(not(windows))]
        {
            let mut normalized = raw_command.to_string();

            let mut double_quote_count = 0;
            let mut single_quote_count = 0;
            let mut in_double_quote = false;
            let mut in_single_quote = false;
            let mut escaped = false;

            for c in normalized.chars() {
                if in_single_quote {
                    if c == '\'' {
                        in_single_quote = false;
                        single_quote_count += 1;
                    }
                } else if in_double_quote {
                    if escaped {
                        escaped = false;
                        continue;
                    }
                    if c == '\\' {
                        escaped = true;
                        continue;
                    }
                    if c == '"' {
                        in_double_quote = false;
                        double_quote_count += 1;
                    }
                } else {
                    if escaped {
                        escaped = false;
                        continue;
                    }
                    if c == '\\' {
                        escaped = true;
                        continue;
                    }
                    if c == '"' {
                        in_double_quote = true;
                        double_quote_count += 1;
                    } else if c == '\'' {
                        in_single_quote = true;
                        single_quote_count += 1;
                    }
                }
            }

            if double_quote_count % 2 != 0 {
                normalized.push('"');
                info!("Shell command: Added missing double quote");
            }
            if single_quote_count % 2 != 0 {
                normalized.push('\'');
                info!("Shell command: Added missing single quote");
            }

            if normalized.contains("\"\"") {
                normalized = Self::fix_consecutive_quotes(&normalized);
            }

            normalized
        }
    }

    /// Fix consecutive quotes based on context
    #[cfg(not(windows))]
    fn fix_consecutive_quotes(input: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if i + 1 < chars.len() && chars[i] == '"' && chars[i + 1] == '"' {
                let mut backslash_count = 0;
                let mut j = i;
                while j > 0 && chars[j - 1] == '\\' {
                    backslash_count += 1;
                    j -= 1;
                }

                if backslash_count % 2 != 0 {
                    result.push(chars[i]);
                    i += 1;
                    continue;
                }

                if i > 0 && chars[i - 1] != ' ' && chars[i - 1] != '=' {
                    result.push('\\');
                    result.push('"');
                    i += 1;
                } else if i + 2 < chars.len() && chars[i + 2] != ' ' {
                    result.push('"');
                    result.push('\\');
                    result.push('"');
                    i += 2;
                } else {
                    result.push('"');
                    i += 2;
                }
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }

        result
    }

    #[cfg(windows)]
    fn contains_unquoted_andand(input: &str) -> bool {
        let mut in_single = false;
        let mut in_double = false;
        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];

            if in_single {
                if ch == '\'' {
                    if i + 1 < chars.len() && chars[i + 1] == '\'' {
                        i += 2;
                        continue;
                    }
                    in_single = false;
                }
                i += 1;
                continue;
            }

            if in_double {
                if ch == '`' {
                    i += 2;
                    continue;
                }
                if ch == '"' {
                    in_double = false;
                }
                i += 1;
                continue;
            }

            if ch == '\'' {
                in_single = true;
                i += 1;
                continue;
            }

            if ch == '"' {
                in_double = true;
                i += 1;
                continue;
            }

            if ch == '&' && i + 1 < chars.len() && chars[i + 1] == '&' {
                return true;
            }

            i += 1;
        }

        false
    }

    pub async fn handle_execute_shell(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        let raw_command = match args.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => {
                return Ok(missing_param_error("command", ToolGroup::Workspace));
            }
        };

        #[cfg(windows)]
        {
            if Self::contains_unquoted_andand(raw_command) {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Invalid PowerShell syntax: '&&' is not supported by PowerShell 5.1"
                        .to_string(),
                    vec![
                        "Use ';' to chain commands in PowerShell".to_string(),
                        "Example: cd src; pnpm test".to_string(),
                        "If you need conditional execution, use 'if ($LASTEXITCODE -eq 0) { ... }'"
                            .to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        }

        let require_input = args
            .get("requireUserInput")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let auto_detect = self.detect_privilege_escalation(raw_command);

        if require_input || auto_detect {
            return self
                .handle_interactive_shell(raw_command, &args, session_id)
                .await;
        }

        let timeout_secs = utils::validate_timeout(args.get("timeout").and_then(|v| v.as_u64()));

        let sync_max = crate::config::default_execution_timeout();
        if timeout_secs > sync_max {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                format!(
                    "Timeout ({} seconds) exceeds maximum ({} seconds)",
                    timeout_secs, sync_max
                ),
                vec![
                    format!(
                        "Use spawnProcess for commands longer than {} seconds",
                        sync_max
                    ),
                    "spawnProcess runs in background and returns process_id".to_string(),
                    format!(
                        "Current maximum timeout: {}s (LIBRAGENT_DEFAULT_EXECUTION_TIMEOUT)",
                        sync_max
                    ),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        self.execute_shell_persistent(raw_command, timeout_secs, session_id)
            .await
    }

    pub async fn handle_run_shell(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        let raw_command = match args.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => {
                return Ok(missing_param_error("command", ToolGroup::Workspace));
            }
        };

        #[cfg(windows)]
        {
            if Self::contains_unquoted_andand(raw_command) {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Invalid PowerShell syntax: '&&' is not supported by PowerShell 5.1"
                        .to_string(),
                    vec![
                        "Use ';' to chain commands in PowerShell".to_string(),
                        "Example: cd src; pnpm test".to_string(),
                        "If you need conditional execution, use 'if ($LASTEXITCODE -eq 0) { ... }'"
                            .to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        }

        let timeout_secs = utils::validate_timeout(args.get("timeout").and_then(|v| v.as_u64()));

        let sync_max = crate::config::default_execution_timeout();
        if timeout_secs > sync_max {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                format!(
                    "Timeout ({} seconds) exceeds maximum ({} seconds)",
                    timeout_secs, sync_max
                ),
                vec![
                    format!(
                        "Use spawnProcess for commands longer than {} seconds",
                        sync_max
                    ),
                    "spawnProcess runs in background and returns process_id".to_string(),
                    format!(
                        "Current maximum timeout: {}s (LIBRAGENT_DEFAULT_EXECUTION_TIMEOUT)",
                        sync_max
                    ),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        self.execute_shell_with_isolation(
            raw_command,
            IsolationLevel::Medium,
            timeout_secs,
            session_id,
        )
        .await
    }

    pub async fn handle_spawn_process(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        let raw_command = match args.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => {
                return Ok(missing_param_error("command", ToolGroup::Workspace));
            }
        };

        #[cfg(windows)]
        {
            if Self::contains_unquoted_andand(raw_command) {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Invalid PowerShell syntax: '&&' is not supported by PowerShell 5.1"
                        .to_string(),
                    vec![
                        "Use ';' to chain commands in PowerShell".to_string(),
                        "Example: cd src; pnpm test".to_string(),
                        "If you need conditional execution, use 'if ($LASTEXITCODE -eq 0) { ... }'"
                            .to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        }

        let require_input = args
            .get("requireUserInput")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if require_input {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                "Background processes cannot prompt for interactive input".to_string(),
                vec![
                    format!(
                        "Use {} (sync mode) for commands requiring user input",
                        PERSISTENT_SHELL_TOOL
                    ),
                    format!(
                        "{} supports requireUserInput for sudo/interactive commands",
                        PERSISTENT_SHELL_TOOL
                    ),
                    "Async processes run in the background without user interaction".to_string(),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        self.execute_shell_async(raw_command, &args, session_id)
            .await
    }

    async fn execute_shell_async(
        &self,
        command: &str,
        _args: &Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        let session_id = session_id.to_string();

        let workspace_path = self
            .session_manager
            .get_session_workspace_dir_by_id(&session_id);

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
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidState,
                    format!(
                        "Maximum concurrent processes limit reached ({}/{})",
                        running_count, MAX_CONCURRENT_PROCESSES
                    ),
                    vec![
                        "Use listProcesses to see running processes".to_string(),
                        "Use stopProcess to cancel unnecessary processes".to_string(),
                        "Wait for some processes to finish before starting new ones".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        }

        let process_id = cuid2::create_id();

        let process_tmp_dir = workspace_path
            .join("tmp")
            .join(format!("process_{process_id}"));

        if let Err(e) = tokio::fs::create_dir_all(&process_tmp_dir).await {
            return Ok(operation_failed_error(
                "Create process directory",
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

        let normalized_command = Self::normalize_shell_command(command);
        let isolation_level = IsolationLevel::Medium;

        let isolation_config = IsolatedProcessConfig {
            session_id: session_id.clone(),
            workspace_path: workspace_path.clone(),
            command: normalized_command.clone(),
            args: vec![],
            env_vars: HashMap::new(),
            isolation_level,
        };

        let cmd = match self
            .isolation_manager
            .create_isolated_command(isolation_config)
            .await
        {
            Ok(cmd) => cmd,
            Err(e) => {
                return Ok(operation_failed_error(
                    "Create isolated command",
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

        let cancel_token = CancellationToken::new();

        let entry = terminal_manager::ProcessEntry {
            id: process_id.clone(),
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

        let registry = self.process_registry.clone();
        let pid_copy = process_id.clone();

        tokio::spawn(async move {
            {
                let mut reg = registry.write().await;
                if let Some(entry) = reg.entries.get_mut(&pid_copy) {
                    entry.status = terminal_manager::ProcessStatus::Running;
                }
            }

            let result = process::spawn_and_stream_hybrid(
                cmd,
                stdout_path.clone(),
                stderr_path.clone(),
                format!("async:{pid_copy}"),
                cancel_token,
            )
            .await;

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

                        entry.stdout_size = terminal_manager::get_file_size(&stdout_path).await;
                        entry.stderr_size = terminal_manager::get_file_size(&stderr_path).await;

                        reg.streaming_handles
                            .insert(pid_copy.clone(), streaming_handle);
                    }
                    Err(e) => {
                        entry.status = terminal_manager::ProcessStatus::Failed;
                        entry.finished_at = Some(chrono::Utc::now());
                        error!("Process {} execution error: {}", pid_copy, e);

                        entry.stdout_size = terminal_manager::get_file_size(&stdout_path).await;
                        entry.stderr_size = terminal_manager::get_file_size(&stderr_path).await;
                    }
                }
            }

            reg.cancellation_tokens.remove(&pid_copy);

            info!(
                "Process {} completed with status: {:?}",
                pid_copy,
                reg.entries.get(&pid_copy).map(|e| &e.status)
            );
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        {
            let registry = self.process_registry.read().await;
            if let Some(entry) = registry.entries.get(&process_id) {
                if matches!(entry.status, terminal_manager::ProcessStatus::Failed) {
                    return Ok(operation_failed_error(
                        "Start background process",
                        "Process failed to start",
                        vec![
                            "Verify the command syntax is correct".to_string(),
                            "Check if required programs are installed".to_string(),
                            "Use listProcesses to see failed process details".to_string(),
                        ],
                        ToolGroup::Workspace,
                    ));
                }
            }
        }

        let hint = SuccessHint::new(
            format!("Background process started (ID: {})", process_id),
            vec![
                format!(
                    "Use pollProcess with process_id \"{}\" to check status",
                    process_id
                ),
                "Use listProcesses to see all running processes".to_string(),
            ],
        );

        let response_data = serde_json::json!({
            "process_id": process_id,
            "command": command,
            "mode": "async",
            "note": "async mode is intended for long-running commands (over 30s)"
        });

        Ok(hint.to_mcp_result_with_data(Some(response_data)))
    }

    #[cfg(unix)]
    pub(crate) fn detect_privilege_escalation(&self, command: &str) -> bool {
        let trimmed = command.trim_start();
        let patterns = ["sudo ", "su ", "doas ", "pkexec "];
        patterns.iter().any(|p| trimmed.starts_with(p))
    }

    #[cfg(windows)]
    pub(crate) fn detect_privilege_escalation(&self, _command: &str) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(windows))]
    fn test_normalize_shell_command_unix() {
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo hello"),
            "echo hello"
        );
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo 'hello'"),
            "echo 'hello'"
        );
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo \"hello\""),
            "echo \"hello\""
        );
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo \"hello"),
            "echo \"hello\""
        );
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo 'hello"),
            "echo 'hello'"
        );
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo \"foo\\\"bar\""),
            "echo \"foo\\\"bar\""
        );
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo '\"hello\"'"),
            "echo '\"hello\"'"
        );
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo \"'hello'\""),
            "echo \"'hello'\""
        );
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo \"path: \\\"/tmp/foo\\\"\""),
            "echo \"path: \\\"/tmp/foo\\\"\""
        );
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo hello \\"),
            "echo hello \\"
        );
    }

    #[test]
    #[cfg(windows)]
    fn test_normalize_shell_command_windows() {
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo hello"),
            "echo hello"
        );
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo \"hello"),
            "echo \"hello"
        );
    }
}
