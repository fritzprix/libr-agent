use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::mcp::types::MCPResult;
use crate::session_isolation::{IsolatedProcessConfig, IsolationLevel};

use super::super::{terminal_manager, utils, WorkspaceServer};
use super::process;

impl WorkspaceServer {
    /// Execute command using persistent shell
    ///
    /// This method provides state preservation across commands (cd, export, venv)
    /// by reusing a single shell process per session.
    pub(crate) async fn execute_shell_persistent(
        &self,
        command: &str,
        timeout_secs: u64,
    ) -> Result<MCPResult, String> {
        let session_id = self
            .session_manager
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());

        let workspace_path = self.get_workspace_dir();

        // Normalize command
        let normalized_command = Self::normalize_shell_command(command);

        // Execute with timeout
        let timeout_duration = Duration::from_secs(timeout_secs);

        // Reset working directory to workspace root before executing command
        // This ensures predictable behavior (always starting from root) while preserving env vars
        // Note: We use to_string_lossy() which should be safe for most paths, but might be an issue for some non-UTF8 paths on Unix
        let full_command = if cfg!(windows) {
            format!(
                "Set-Location -Path \"{}\"; {}",
                workspace_path.to_string_lossy(),
                normalized_command
            )
        } else {
            format!(
                "cd \"{}\" && {}",
                workspace_path.to_string_lossy(),
                normalized_command
            )
        };

        // Pass workspace_path to execute (it is used for creation if shell doesn't exist)
        let execution_result = tokio::time::timeout(
            timeout_duration,
            self.shell_manager
                .execute(session_id.clone(), workspace_path, &full_command),
        )
        .await;

        match execution_result {
            Ok(Ok((stdout, stderr, exit_code))) => {
                // Success case - format result
                let success = exit_code == 0;

                // Construct JSON response
                let response = serde_json::json!({
                    "command": command,
                    "exit_code": exit_code,
                    "stdout": stdout,
                    "stderr": stderr,
                    "status": if success { "finished" } else { "failed" }
                });
                let result_text = response.to_string();

                info!(
                    "Persistent shell command executed: {} (session: {}, exit: {})",
                    command, session_id, exit_code
                );

                if success {
                    Ok(MCPResult::success(&result_text))
                } else {
                    Ok(MCPResult::error(&result_text))
                }
            }
            Ok(Err(e)) => {
                // Execution error - shell crashed or command failed
                warn!(
                    "Persistent shell execution failed for session {}: {}. Falling back to one-shot.",
                    session_id, e
                );

                // Fallback to one-shot execution
                let isolation_level = IsolationLevel::Medium;
                self.execute_shell_with_isolation(command, isolation_level, timeout_secs)
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

                // Return structured JSON error for consistency
                let response = serde_json::json!({
                    "command": command,
                    "exit_code": -1,
                    "stdout": "",
                    "stderr": format!("Command execution timeout after {timeout_secs} seconds. The shell session has been reset."),
                    "status": "timeout"
                });

                Ok(MCPResult::error(&response.to_string()))
            }
        }
    }

    /// Execute shell commands with isolation
    pub(crate) async fn execute_shell_with_isolation(
        &self,
        command: &str,
        isolation_level: IsolationLevel,
        timeout_secs: u64,
    ) -> Result<MCPResult, String> {
        let session_id = self
            .session_manager
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());

        let workspace_path = self.get_workspace_dir();

        // Normalize shell command
        let normalized_command = Self::normalize_shell_command(command);

        // Generate process ID for sync execution
        let process_id = cuid2::create_id();

        // Create temporary directory for output files
        let process_tmp_dir = workspace_path
            .join("tmp")
            .join(format!("sync_{process_id}"));

        if let Err(e) = tokio::fs::create_dir_all(&process_tmp_dir).await {
            return Ok(MCPResult::error(&format!(
                "Failed to create temp directory: {e}"
            )));
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

        // Create isolated command
        let cmd = match self
            .isolation_manager
            .create_isolated_command(isolation_config)
            .await
        {
            Ok(cmd) => cmd,
            Err(e) => {
                return Ok(MCPResult::error(&format!(
                    "Failed to create isolated shell command: {e}"
                )));
            }
        };

        // Create cancellation token
        let cancel_token = CancellationToken::new();

        // Register process in registry
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

                // Construct JSON response
                let response = serde_json::json!({
                    "command": command,
                    "exit_code": exit_code.unwrap_or(-1),
                    "stdout": stdout,
                    "stderr": stderr,
                    "status": if success { "finished" } else { "failed" }
                });
                let result_text = response.to_string();

                info!(
                    "Isolated shell command executed: {} (session: {}, exit: {:?})",
                    command, session_id, exit_code
                );

                Ok(MCPResult::success(&result_text))
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
                Ok(MCPResult::error(&format!("Execution error: {e}")))
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
                Ok(MCPResult::error(&format!(
                    "Command timed out after {timeout_secs} seconds"
                )))
            }
        }
    }

    /// Normalize shell command for proper execution
    /// Handles platform-specific quoting and escaping rules
    pub(crate) fn normalize_shell_command(raw_command: &str) -> String {
        #[cfg(windows)]
        {
            // Windows: PowerShell handles both single and double quotes correctly
            // No normalization needed - pass command as-is to avoid breaking nested quotes
            // in Python/Node.js inline commands like: python -c "print('Hello')"
            info!("Windows command (no normalization): {}", raw_command);
            raw_command.to_string()
        }

        #[cfg(not(windows))]
        {
            // Unix shell quoting normalization (existing logic)
            let mut normalized = raw_command.to_string();

            // 1. Detect incomplete quote pairs using a state machine
            let mut double_quote_count = 0;
            let mut single_quote_count = 0;
            let mut in_double_quote = false;
            let mut in_single_quote = false;
            let mut escaped = false;

            for c in normalized.chars() {
                if in_single_quote {
                    // Inside single quotes, backslash is literal, only single quote escapes
                    if c == '\'' {
                        in_single_quote = false;
                        single_quote_count += 1;
                    }
                } else if in_double_quote {
                    // Inside double quotes, backslash escapes next char
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
                    // Normal state
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

            // 2. Add missing closing quotes
            if double_quote_count % 2 != 0 {
                normalized.push('"');
                info!("Shell command: Added missing double quote");
            }
            if single_quote_count % 2 != 0 {
                normalized.push('\'');
                info!("Shell command: Added missing single quote");
            }

            // 3. Fix consecutive quote patterns
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
                // Consecutive quotes found

                // Check if the first quote is escaped (preceded by odd number of backslashes)
                let mut backslash_count = 0;
                let mut j = i;
                while j > 0 && chars[j - 1] == '\\' {
                    backslash_count += 1;
                    j -= 1;
                }

                if backslash_count % 2 != 0 {
                    // It is an escaped quote (e.g. \"), so it's not a start of consecutive quotes
                    result.push(chars[i]);
                    i += 1;
                    continue;
                }

                if i > 0 && chars[i - 1] != ' ' && chars[i - 1] != '=' {
                    // If no space or equals before, escape the first one
                    result.push('\\');
                    result.push('"');
                    i += 1; // Second quote processed in next loop
                } else if i + 2 < chars.len() && chars[i + 2] != ' ' {
                    // If no space after, escape the second one
                    result.push('"');
                    result.push('\\');
                    result.push('"');
                    i += 2;
                } else {
                    // Default: keep one, remove one
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

    pub async fn handle_execute_shell(&self, args: Value) -> Result<MCPResult, String> {
        let raw_command = match args.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => {
                return Ok(MCPResult::error("Missing required parameter: command"));
            }
        };

        // Check for require_user_input parameter or auto-detect privilege escalation
        let require_input = args
            .get("require_user_input")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let auto_detect = self.detect_privilege_escalation(raw_command);

        // If user input required, return UIResource for interactive execution
        if require_input || auto_detect {
            return self.handle_interactive_shell(raw_command, &args).await;
        }

        // Check run_mode parameter
        let run_mode = args
            .get("run_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("sync");

        // Async mode: background execution
        if run_mode == "async" {
            return self.execute_shell_async(raw_command, &args).await;
        }

        // Sync mode: check persistent shell preference
        let timeout_secs = utils::validate_timeout(args.get("timeout").and_then(|v| v.as_u64()));

        // Enforce maximum sync timeout
        let sync_max = crate::config::default_execution_timeout();
        if timeout_secs > sync_max {
            return Ok(MCPResult::error(&format!(
                "Sync mode supports a maximum timeout of {sync_max} seconds.\nFor longer-running commands, set \"run_mode\" to \"async\" so the command runs in background and can be polled.\nYou can adjust the default via the LIBRAGENT_DEFAULT_EXECUTION_TIMEOUT environment variable.",
            )));
        }

        // Check persistent shell preference (default: enabled)
        let use_persistent_shell = args
            .get("use_persistent_shell")
            .and_then(|v| v.as_bool())
            .unwrap_or(true); // Default enabled per Q1 decision

        if use_persistent_shell {
            // NEW PATH: Persistent shell execution (state preservation)
            return self
                .execute_shell_persistent(raw_command, timeout_secs)
                .await;
        }

        // FALLBACK PATH: One-shot isolation execution
        let isolation_level = IsolationLevel::Medium;

        #[cfg(windows)]
        info!(
            "execute_windows_cmd invoked: command='{}' run_mode='{}' require_input='{}' timeout={}",
            raw_command, run_mode, require_input, timeout_secs
        );
        self.execute_shell_with_isolation(raw_command, isolation_level, timeout_secs)
            .await
    }

    /// Execute shell command asynchronously in background
    async fn execute_shell_async(&self, command: &str, _args: &Value) -> Result<MCPResult, String> {
        // Get session info
        let session_id = self
            .session_manager
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());

        let workspace_path = self.get_workspace_dir();

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
                return Ok(MCPResult::error(&format!(
                    "Maximum concurrent processes limit reached ({MAX_CONCURRENT_PROCESSES})"
                )));
            }
        }

        // Generate process ID
        let process_id = cuid2::create_id();

        // Create process tmp directory
        let process_tmp_dir = workspace_path
            .join("tmp")
            .join(format!("process_{process_id}"));

        if let Err(e) = tokio::fs::create_dir_all(&process_tmp_dir).await {
            return Ok(MCPResult::error(&format!(
                "Failed to create process directory: {e}"
            )));
        }

        let stdout_path = process_tmp_dir.join("stdout");
        let stderr_path = process_tmp_dir.join("stderr");

        // Normalize command
        let normalized_command = Self::normalize_shell_command(command);

        // Always use Medium isolation
        let isolation_level = IsolationLevel::Medium;

        // Create isolation config
        let isolation_config = IsolatedProcessConfig {
            session_id: session_id.clone(),
            workspace_path: workspace_path.clone(),
            command: normalized_command.clone(),
            args: vec![],
            env_vars: HashMap::new(),
            isolation_level,
        };

        // Create isolated command
        let cmd = match self
            .isolation_manager
            .create_isolated_command(isolation_config)
            .await
        {
            Ok(cmd) => cmd,
            Err(e) => {
                return Ok(MCPResult::error(&format!(
                    "Failed to create isolated command: {e}"
                )));
            }
        };

        // Register process in registry (Starting status)
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

        // Spawn monitoring task using hybrid streaming
        let registry = self.process_registry.clone();
        let pid_copy = process_id.clone();

        tokio::spawn(async move {
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

            info!(
                "Process {} completed with status: {:?}",
                pid_copy,
                reg.entries.get(&pid_copy).map(|e| &e.status)
            );
        });

        // Wait briefly to detect immediate failures
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check if process failed to start
        {
            let registry = self.process_registry.read().await;
            if let Some(entry) = registry.entries.get(&process_id) {
                if matches!(entry.status, terminal_manager::ProcessStatus::Failed) {
                    return Ok(MCPResult::error("Process failed to start"));
                }
            }
        }

        // Return immediate response with process_id
        let response_msg = format!(
            "Process started in background.\n\
             Process ID: {process_id}\n\
             Command: {command}\n\
             \n\
             Use 'poll_process' to check status and view output:\n\
             poll_process(process_id: \"{process_id}\", tail: {{src: \"stdout\", n: 20}})"
        );

        // Clarify that async is intended for long-running commands
        let response_msg = format!(
            "{response_msg}\n\nNote: async mode is intended for long-running commands (over 30s)."
        );

        Ok(MCPResult::success(&response_msg))
    }

    /// Platform-specific privilege detection for Unix systems
    /// Detects commands that require elevated privileges (sudo, su, doas, pkexec)
    #[cfg(unix)]
    pub(crate) fn detect_privilege_escalation(&self, command: &str) -> bool {
        let trimmed = command.trim_start();
        let patterns = ["sudo ", "su ", "doas ", "pkexec "];
        patterns.iter().any(|p| trimmed.starts_with(p))
    }

    /// Platform-specific privilege detection for Windows
    /// Windows UAC cannot be detected from command string
    /// Agent must explicitly set require_user_input=true
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
        // Basic cases
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

        // Missing quotes
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo \"hello"),
            "echo \"hello\""
        );
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo 'hello"),
            "echo 'hello'"
        );

        // Escaped quotes (should NOT be counted as closing quotes)
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo \"foo\\\"bar\""),
            "echo \"foo\\\"bar\""
        );

        // Nested quotes
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo '\"hello\"'"),
            "echo '\"hello\"'"
        );
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo \"'hello'\""),
            "echo \"'hello'\""
        );

        // Complex case with multiple escapes
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo \"path: \\\"/tmp/foo\\\"\""),
            "echo \"path: \\\"/tmp/foo\\\"\""
        );

        // Trailing backslash (should be preserved)
        assert_eq!(
            WorkspaceServer::normalize_shell_command("echo hello \\"),
            "echo hello \\"
        );
    }

    #[test]
    #[cfg(windows)]
    fn test_normalize_shell_command_windows() {
        // Windows should pass through everything as-is
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
