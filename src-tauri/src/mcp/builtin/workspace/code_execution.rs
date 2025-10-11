use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::session_isolation::{IsolatedProcessConfig, IsolationLevel};

use super::{terminal_manager, utils, WorkspaceServer};
use crate::mcp::MCPResponse;

#[allow(dead_code)]
impl WorkspaceServer {
    /// Spawn process and stream stdout/stderr to files (common logic for sync/async)
    /// Returns (pid, exit_code, stdout_content, stderr_content)
    /// Respects cancellation token for graceful shutdown
    async fn spawn_and_stream_to_files(
        mut cmd: tokio::process::Command,
        stdout_path: std::path::PathBuf,
        stderr_path: std::path::PathBuf,
        process_label: String,
        cancel_token: CancellationToken,
    ) -> Result<(Option<u32>, Option<i32>, String, String), String> {
        use std::process::Stdio;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn process: {e}"))?;

        let pid = child.id();
        info!("Process {} started with PID {:?}", process_label, pid);
        // Determine maximum output size from configuration (default 100MB)
        let max_output_size = crate::config::max_output_size();

        // Stream stdout to file
        let stdout_handle = if let Some(mut stdout) = child.stdout.take() {
            let stdout_path_clone = stdout_path.clone();
            let label = process_label.clone();
            let cancel_clone = cancel_token.clone();
            Some(tokio::spawn(async move {
                if let Ok(mut file) = tokio::fs::File::create(&stdout_path_clone).await {
                    let mut total_written = 0u64;
                    let mut buffer = [0u8; 8192];

                    loop {
                        tokio::select! {
                            _ = cancel_clone.cancelled() => {
                                info!("Process {} stdout streaming cancelled", label);
                                break;
                            }
                            result = stdout.read(&mut buffer) => {
                                match result {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        total_written += n as u64;
                                        if total_written > max_output_size {
                                            warn!(
                                                "Process {} stdout size limit exceeded, truncating",
                                                label
                                            );
                                            let _ = file
                                                .write_all(b"\n[Output truncated: size limit exceeded]\n")
                                                .await;
                                            break;
                                        }
                                        if file.write_all(&buffer[..n]).await.is_err() {
                                            break;
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                        }
                    }
                }
            }))
        } else {
            None
        };

        // Stream stderr to file
        let stderr_handle = if let Some(mut stderr) = child.stderr.take() {
            let stderr_path_clone = stderr_path.clone();
            let label = process_label.clone();
            let cancel_clone = cancel_token.clone();
            Some(tokio::spawn(async move {
                if let Ok(mut file) = tokio::fs::File::create(&stderr_path_clone).await {
                    let mut total_written = 0u64;
                    let mut buffer = [0u8; 8192];

                    loop {
                        tokio::select! {
                            _ = cancel_clone.cancelled() => {
                                info!("Process {} stderr streaming cancelled", label);
                                break;
                            }
                            result = stderr.read(&mut buffer) => {
                                match result {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        total_written += n as u64;
                                        if total_written > max_output_size {
                                            warn!(
                                                "Process {} stderr size limit exceeded, truncating",
                                                label
                                            );
                                            let _ = file
                                                .write_all(b"\n[Output truncated: size limit exceeded]\n")
                                                .await;
                                            break;
                                        }
                                        if file.write_all(&buffer[..n]).await.is_err() {
                                            break;
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                        }
                    }
                }
            }))
        } else {
            None
        };

        // Wait for process completion or cancellation
        let exit_code = tokio::select! {
            _ = cancel_token.cancelled() => {
                info!("Process {} cancellation requested, killing child", process_label);

                // Try graceful kill first
                let _ = child.kill().await;

                // Wait a bit for process to die (configurable graceful shutdown timeout)
                let graceful_secs = crate::config::graceful_shutdown_timeout();
                match tokio::time::timeout(Duration::from_secs(graceful_secs), child.wait()).await {
                    Ok(Ok(status)) => status.code(),
                    _ => {
                        warn!("Process {} did not terminate gracefully", process_label);
                        None
                    }
                }
            }
            result = child.wait() => {
                match result {
                    Ok(status) => status.code(),
                    Err(e) => {
                        error!("Process {} wait error: {}", process_label, e);
                        None
                    }
                }
            }
        };

        // Wait for streaming tasks to complete
        if let Some(h) = stdout_handle {
            let _ = h.await;
        }
        if let Some(h) = stderr_handle {
            let _ = h.await;
        }

        // Read output files
        let stdout_content = tokio::fs::read_to_string(&stdout_path)
            .await
            .unwrap_or_default();
        let stderr_content = tokio::fs::read_to_string(&stderr_path)
            .await
            .unwrap_or_default();

        info!(
            "Process {} completed with exit code {:?}",
            process_label, exit_code
        );

        Ok((pid, exit_code, stdout_content, stderr_content))
    }

    /// Execute shell commands with isolation
    async fn execute_shell_with_isolation(
        &self,
        command: &str,
        isolation_level: IsolationLevel,
        timeout_secs: u64,
    ) -> MCPResponse {
        let request_id = Self::generate_request_id();

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
            return Self::error_response(
                request_id,
                -32603,
                &format!("Failed to create temp directory: {e}"),
            );
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
                return Self::error_response(
                    request_id,
                    -32603,
                    &format!("Failed to create isolated shell command: {e}"),
                );
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
            Self::spawn_and_stream_to_files(
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

                let result_text = if success {
                    if stdout.trim().is_empty() && stderr.trim().is_empty() {
                        "Command executed successfully (no output)".to_string()
                    } else if stderr.trim().is_empty() {
                        format!("Command executed successfully:\n{}", stdout.trim())
                    } else {
                        format!(
                            "Command executed successfully:\nSTDOUT:\n{}\n\nSTDERR:\n{}",
                            stdout.trim(),
                            stderr.trim()
                        )
                    }
                } else {
                    format!(
                        "Command failed with exit code {}:\nSTDOUT:\n{}\n\nSTDERR:\n{}",
                        exit_code.unwrap_or(-1),
                        stdout.trim(),
                        stderr.trim()
                    )
                };

                info!(
                    "Isolated shell command executed: {} (session: {}, exit: {:?})",
                    command, session_id, exit_code
                );

                Self::success_response(request_id, &result_text)
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
                Self::error_response(request_id, -32603, &format!("Execution error: {e}"))
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
                Self::error_response(
                    request_id,
                    -32603,
                    &format!("Command timed out after {timeout_secs} seconds"),
                )
            }
        }
    }

    /// LLM이 생성한 Shell 명령어의 따옴표 문제를 자동으로 보정
    /// Bash와 POSIX sh 모두에서 동작하는 따옴표 보정
    fn normalize_shell_command(raw_command: &str) -> String {
        let mut normalized = raw_command.to_string();

        // 1. 불완전한 따옴표 쌍 감지 및 보정
        let double_quote_count = normalized.chars().filter(|&c| c == '"').count();
        let single_quote_count = normalized.chars().filter(|&c| c == '\'').count();

        // 2. 홀수 개의 따옴표가 있으면 마지막에 추가
        if double_quote_count % 2 != 0 {
            normalized.push('"');
            info!("Shell command: Added missing double quote");
        }
        if single_quote_count % 2 != 0 {
            normalized.push('\'');
            info!("Shell command: Added missing single quote");
        }

        // 3. 연속된 따옴표 보정 패턴들
        // "echo "hello"" -> "echo \"hello\""
        if normalized.contains("\"\"") {
            normalized = Self::fix_consecutive_quotes(&normalized);
        }

        normalized
    }

    /// 연속된 따옴표를 문맥에 따라 보정
    fn fix_consecutive_quotes(input: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if i + 1 < chars.len() && chars[i] == '"' && chars[i + 1] == '"' {
                // 연속된 따옴표 발견
                if i > 0 && chars[i - 1] != ' ' && chars[i - 1] != '=' {
                    // 앞에 공백이나 등호가 없으면 첫 번째는 이스케이프
                    result.push('\\');
                    result.push('"');
                    i += 1; // 두 번째 따옴표는 다음 루프에서 처리
                } else if i + 2 < chars.len() && chars[i + 2] != ' ' {
                    // 뒤에 공백이 없으면 두 번째는 이스케이프
                    result.push('"');
                    result.push('\\');
                    result.push('"');
                    i += 2;
                } else {
                    // 기본적으로 첫 번째만 남기고 두 번째 제거
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

    pub async fn handle_execute_shell(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let raw_command = match args.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: command",
                );
            }
        };

        // Check run_mode parameter
        let run_mode = args
            .get("run_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("sync");

        // Async mode: background execution
        if run_mode == "async" {
            return self.execute_shell_async(raw_command, &args).await;
        }

        // Sync mode: existing behavior
        let timeout_secs = utils::validate_timeout(args.get("timeout").and_then(|v| v.as_u64()));

        // Enforce maximum sync timeout. Read default/max values from runtime
        // configuration so the limit can be adjusted via environment variables
        // (see `src-tauri/src/config.rs`). `default_execution_timeout()` is the
        // recommended default; we treat it as the sync maximum here to keep
        // sync requests short-lived and encourage async for long-running work.
        let sync_max = crate::config::default_execution_timeout();
        if timeout_secs > sync_max {
            return Self::error_response(
                request_id,
                -32603,
                &format!(
                    "Sync mode supports a maximum timeout of {sync_max} seconds.\nFor longer-running commands, set \"run_mode\" to \"async\" so the command runs in background and can be polled.\nYou can adjust the default via the SYNAPTICFLOW_DEFAULT_EXECUTION_TIMEOUT environment variable.",
                ),
            );
        }

        // Determine isolation level based on arguments or default to Medium
        let isolation_level = args
            .get("isolation")
            .and_then(|v| v.as_str())
            .map(|level| match level {
                "basic" => IsolationLevel::Basic,
                "high" => IsolationLevel::High,
                _ => IsolationLevel::Medium,
            })
            .unwrap_or(IsolationLevel::Medium);

        // Use existing isolation-aware shell execution
        self.execute_shell_with_isolation(raw_command, isolation_level, timeout_secs)
            .await
    }

    /// Execute shell command asynchronously in background
    async fn execute_shell_async(&self, command: &str, args: &Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

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
                return Self::error_response(
                    request_id,
                    -32603,
                    &format!(
                        "Maximum concurrent processes limit reached ({MAX_CONCURRENT_PROCESSES})"
                    ),
                );
            }
        }

        // Generate process ID
        let process_id = cuid2::create_id();

        // Create process tmp directory
        let process_tmp_dir = workspace_path
            .join("tmp")
            .join(format!("process_{process_id}"));

        if let Err(e) = tokio::fs::create_dir_all(&process_tmp_dir).await {
            return Self::error_response(
                request_id,
                -32603,
                &format!("Failed to create process directory: {e}"),
            );
        }

        let stdout_path = process_tmp_dir.join("stdout");
        let stderr_path = process_tmp_dir.join("stderr");

        // Normalize command
        let normalized_command = Self::normalize_shell_command(command);

        // Determine isolation level
        let isolation_level = args
            .get("isolation")
            .and_then(|v| v.as_str())
            .map(|level| match level {
                "basic" => IsolationLevel::Basic,
                "high" => IsolationLevel::High,
                _ => IsolationLevel::Medium,
            })
            .unwrap_or(IsolationLevel::Medium);

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
                return Self::error_response(
                    request_id,
                    -32603,
                    &format!("Failed to create isolated command: {e}"),
                );
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
        };

        {
            let mut registry = self.process_registry.write().await;
            registry.entries.insert(process_id.clone(), entry.clone());
            registry
                .cancellation_tokens
                .insert(process_id.clone(), cancel_token.clone());
        }

        // Spawn monitoring task using common spawn+stream logic
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

            // Execute using common logic
            let result = Self::spawn_and_stream_to_files(
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
                    Ok((pid, exit_code, _, _)) => {
                        entry.pid = pid;
                        let code = exit_code.unwrap_or(-1);
                        entry.status = if code == 0 {
                            terminal_manager::ProcessStatus::Finished
                        } else {
                            terminal_manager::ProcessStatus::Failed
                        };
                        entry.exit_code = exit_code;
                    }
                    Err(e) => {
                        entry.status = terminal_manager::ProcessStatus::Failed;
                        error!("Process {} execution error: {}", pid_copy, e);
                    }
                }
                entry.finished_at = Some(chrono::Utc::now());

                // Update file sizes
                entry.stdout_size = terminal_manager::get_file_size(&stdout_path).await;
                entry.stderr_size = terminal_manager::get_file_size(&stderr_path).await;
            }

            // Remove cancellation token
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
                    return Self::error_response(request_id, -32603, "Process failed to start");
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

        Self::success_response(request_id, &response_msg)
    }
}
