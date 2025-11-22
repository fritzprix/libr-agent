use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::mcp::types::MCPResult;
use crate::session_isolation::{IsolatedProcessConfig, IsolationLevel};

use super::{terminal_manager, utils, WorkspaceServer};

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

        // Configure stdio pipes - critical for capturing output
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.stdin(Stdio::null()); // Explicitly close stdin to prevent blocking

        // Windows-specific: Ensure handle inheritance is enabled for stdio pipes
        #[cfg(target_os = "windows")]
        {
            info!(
                "Spawning Windows process with stdio redirection: {}",
                process_label
            );
            // Note: tokio::process::Command automatically handles handle inheritance
            // on Windows when Stdio::piped() is used, but we log for diagnostics

            // Log the full command for debugging Windows execution issues
            info!("Windows process command debug: {:?}", cmd.as_std());

            // Also log important environment variables that affect Windows process
            // execution (PATH, SystemRoot, COMSPEC, PSModulePath, ProgramFiles).
            // We avoid logging full PATH contents to reduce noise; instead log
            // lengths and key variable presence.
            let path_len = std::env::var("PATH").map(|p| p.len()).unwrap_or(0);
            let system_root =
                std::env::var("SystemRoot").unwrap_or_else(|_| "<not-set>".to_string());
            let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "<not-set>".to_string());
            let psmodulepath =
                std::env::var("PSModulePath").unwrap_or_else(|_| "<not-set>".to_string());
            let program_files =
                std::env::var("ProgramFiles").unwrap_or_else(|_| "<not-set>".to_string());

            info!("Windows env: PATH.len={}, SystemRoot={}, COMSPEC={}, PSModulePath.present={}, ProgramFiles={}",
                path_len, system_root, comspec, !psmodulepath.is_empty(), program_files);
        }

        let mut child = cmd.spawn().map_err(|e| {
            error!(
                "Failed to spawn process {}: {}. Check command path and permissions.",
                process_label, e
            );
            format!("Failed to spawn process: {e}")
        })?;

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
                if let Ok(file) = tokio::fs::File::create(&stdout_path_clone).await {
                    let mut writer = tokio::io::BufWriter::new(file);
                    info!(
                        "Process {} stdout streaming started to {:?}",
                        label, stdout_path_clone
                    );
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
                                            let _ = writer
                                                .write_all(b"\n[Output truncated: size limit exceeded]\n")
                                                .await;
                                            break;
                                        }
                                        if writer.write_all(&buffer[..n]).await.is_err() {
                                            break;
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                        }
                    }

                    // Explicit flush before drop
                    let _ = writer.flush().await;
                    drop(writer);

                    info!(
                        "Process {} stdout streaming completed, total bytes: {}",
                        label, total_written
                    );
                } else {
                    error!(
                        "Process {} failed to create stdout file: {:?}",
                        label, stdout_path_clone
                    );
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
                if let Ok(file) = tokio::fs::File::create(&stderr_path_clone).await {
                    let mut writer = tokio::io::BufWriter::new(file);
                    info!(
                        "Process {} stderr streaming started to {:?}",
                        label, stderr_path_clone
                    );
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
                                            let _ = writer
                                                .write_all(b"\n[Output truncated: size limit exceeded]\n")
                                                .await;
                                            break;
                                        }
                                        if writer.write_all(&buffer[..n]).await.is_err() {
                                            break;
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                        }
                    }

                    // Explicit flush before drop
                    let _ = writer.flush().await;
                    drop(writer);

                    info!(
                        "Process {} stderr streaming completed, total bytes: {}",
                        label, total_written
                    );
                } else {
                    error!(
                        "Process {} failed to create stderr file: {:?}",
                        label, stderr_path_clone
                    );
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

        // Add small delay for Windows file system sync to ensure data is fully written
        #[cfg(windows)]
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Read output files
        // Read output files using lossy UTF-8 conversion to handle non-UTF8 output (e.g. CP949)
        let stdout_bytes = tokio::fs::read(&stdout_path).await.unwrap_or_default();
        let stdout_content = String::from_utf8_lossy(&stdout_bytes).to_string();

        let stderr_bytes = tokio::fs::read(&stderr_path).await.unwrap_or_default();
        let stderr_content = String::from_utf8_lossy(&stderr_bytes).to_string();

        info!(
            "Process {} completed with exit code {:?}",
            process_label, exit_code
        );

        // Log sizes of captured outputs for debugging intermittent missing output
        // on Windows; this helps determine whether the process produced no data
        // or whether it failed before output was written.
        info!(
            "Process {} output sizes: stdout={} bytes, stderr={} bytes",
            process_label,
            stdout_content.len(),
            stderr_content.len()
        );

        // If both stdout and stderr are empty but process exit code is non-zero,
        // log a warning and point back to the command label so operations team
        // can correlate with Windows process details.
        if stdout_content.is_empty() && stderr_content.is_empty() {
            if let Some(code) = exit_code {
                if code != 0 {
                    warn!(
                        "Process {} returned non-zero exit code {} but both stdout and stderr are empty",
                        process_label, code
                    );
                }
            }
        }

        Ok((pid, exit_code, stdout_content, stderr_content))
    }

    /// Spawn process and stream output to both in-memory buffers and files (for async/long-running processes)
    /// Returns (pid, exit_code, streaming_handle)
    /// Provides real-time access to output through broadcast channels and circular buffers
    async fn spawn_and_stream_hybrid(
        mut cmd: tokio::process::Command,
        stdout_path: std::path::PathBuf,
        stderr_path: std::path::PathBuf,
        process_label: String,
        cancel_token: CancellationToken,
    ) -> Result<
        (
            Option<u32>,
            Option<i32>,
            std::sync::Arc<terminal_manager::StreamingHandle>,
        ),
        String,
    > {
        use std::process::Stdio;
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        // Configure stdio pipes
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.stdin(Stdio::null());

        #[cfg(target_os = "windows")]
        {
            info!(
                "Spawning Windows process with hybrid streaming: {}",
                process_label
            );
        }

        let mut child = cmd.spawn().map_err(|e| {
            error!(
                "Failed to spawn process {}: {}. Check command path and permissions.",
                process_label, e
            );
            format!("Failed to spawn process: {e}")
        })?;

        let pid = child.id();
        info!(
            "Process {} started with PID {:?} (hybrid streaming)",
            process_label, pid
        );

        // Create streaming handle with 1000 line buffer
        let streaming = std::sync::Arc::new(terminal_manager::StreamingHandle::new(1000));
        let max_output_size = crate::config::max_output_size();

        // Stdout streaming: line-by-line with broadcast and file
        let stdout_handle = if let Some(stdout) = child.stdout.take() {
            let streaming_clone = streaming.clone();
            let stdout_path_clone = stdout_path.clone();
            let label = process_label.clone();
            let cancel_clone = cancel_token.clone();

            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();

                match tokio::fs::File::create(&stdout_path_clone).await {
                    Ok(file) => {
                        let mut writer = tokio::io::BufWriter::new(file);
                        let mut total_bytes = 0u64;
                        let mut lines_since_flush = 0;
                        const FLUSH_INTERVAL: usize = 10;

                        loop {
                            tokio::select! {
                                _ = cancel_clone.cancelled() => {
                                    info!("Process {} stdout streaming cancelled", label);
                                    break;
                                }
                                line_result = lines.next_line() => {
                                    match line_result {
                                        Ok(Some(line)) => {
                                            let line_bytes = line.len() as u64 + 1; // +1 for newline
                                            total_bytes += line_bytes;

                                            if total_bytes > max_output_size {
                                                warn!("Process {} stdout size limit exceeded", label);
                                                let _ = writer.write_all(b"\n[Output truncated: size limit exceeded]\n").await;
                                                break;
                                            }

                                            // 1. Send to broadcast channel + buffer
                                            streaming_clone.push_stdout(line.clone()).await;

                                            // 2. Write to file with periodic flush
                                            if writer.write_all(line.as_bytes()).await.is_ok() {
                                                let _ = writer.write_all(b"\n").await;
                                                lines_since_flush += 1;
                                                if lines_since_flush >= FLUSH_INTERVAL {
                                                    let _ = writer.flush().await;
                                                    lines_since_flush = 0;
                                                }
                                            }
                                        }
                                        Ok(None) => break, // EOF
                                        Err(e) => {
                                            warn!("Process {} stdout read error: {}", label, e);
                                            break;
                                        }
                                    }
                                }
                            }
                        }

                        let _ = writer.flush().await;
                        drop(writer);
                        info!(
                            "Process {} stdout hybrid streaming completed ({} bytes)",
                            label, total_bytes
                        );
                    }
                    Err(e) => {
                        error!("Process {} failed to create stdout file: {}", label, e);
                    }
                }
            })
        } else {
            tokio::spawn(async {})
        };

        // Stderr streaming: same pattern
        let stderr_handle = if let Some(stderr) = child.stderr.take() {
            let streaming_clone = streaming.clone();
            let stderr_path_clone = stderr_path.clone();
            let label = process_label.clone();
            let cancel_clone = cancel_token.clone();

            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();

                match tokio::fs::File::create(&stderr_path_clone).await {
                    Ok(file) => {
                        let mut writer = tokio::io::BufWriter::new(file);
                        let mut total_bytes = 0u64;
                        let mut lines_since_flush = 0;
                        const FLUSH_INTERVAL: usize = 10;

                        loop {
                            tokio::select! {
                                _ = cancel_clone.cancelled() => {
                                    info!("Process {} stderr streaming cancelled", label);
                                    break;
                                }
                                line_result = lines.next_line() => {
                                    match line_result {
                                        Ok(Some(line)) => {
                                            let line_bytes = line.len() as u64 + 1;
                                            total_bytes += line_bytes;

                                            if total_bytes > max_output_size {
                                                warn!("Process {} stderr size limit exceeded", label);
                                                let _ = writer.write_all(b"\n[Output truncated: size limit exceeded]\n").await;
                                                break;
                                            }

                                            // 1. Send to broadcast channel + buffer
                                            streaming_clone.push_stderr(line.clone()).await;

                                            // 2. Write to file with periodic flush
                                            if writer.write_all(line.as_bytes()).await.is_ok() {
                                                let _ = writer.write_all(b"\n").await;
                                                lines_since_flush += 1;
                                                if lines_since_flush >= FLUSH_INTERVAL {
                                                    let _ = writer.flush().await;
                                                    lines_since_flush = 0;
                                                }
                                            }
                                        }
                                        Ok(None) => break, // EOF
                                        Err(e) => {
                                            warn!("Process {} stderr read error: {}", label, e);
                                            break;
                                        }
                                    }
                                }
                            }
                        }

                        let _ = writer.flush().await;
                        drop(writer);
                        info!(
                            "Process {} stderr hybrid streaming completed ({} bytes)",
                            label, total_bytes
                        );
                    }
                    Err(e) => {
                        error!("Process {} failed to create stderr file: {}", label, e);
                    }
                }
            })
        } else {
            tokio::spawn(async {})
        };

        // Wait for process completion or cancellation
        let exit_code = tokio::select! {
            _ = cancel_token.cancelled() => {
                info!("Process {} cancellation requested, killing child", process_label);
                let _ = child.kill().await;

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
        let _ = stdout_handle.await;
        let _ = stderr_handle.await;

        info!(
            "Process {} completed with exit code {:?} (hybrid streaming)",
            process_label, exit_code
        );

        Ok((pid, exit_code, streaming))
    }

    /// Execute command using persistent shell
    ///
    /// This method provides state preservation across commands (cd, export, venv)
    /// by reusing a single shell process per session.
    async fn execute_shell_persistent(
        &self,
        command: &str,
        timeout_secs: u64,
    ) -> Result<MCPResult, String> {
        let session_id = self
            .session_manager
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());

        // Normalize command
        let normalized_command = Self::normalize_shell_command(command);

        // Execute with timeout
        let timeout_duration = Duration::from_secs(timeout_secs);

        let execution_result = tokio::time::timeout(
            timeout_duration,
            self.shell_manager
                .execute(session_id.clone(), &normalized_command),
        )
        .await;

        match execution_result {
            Ok(Ok((stdout, stderr, exit_code))) => {
                // Success case - format result
                let success = exit_code == 0;

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
                        exit_code,
                        stdout.trim(),
                        stderr.trim()
                    )
                };

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
                Err(format!(
                    "Command execution timeout after {} seconds",
                    timeout_secs
                ))
            }
        }
    }

    /// Execute shell commands with isolation
    async fn execute_shell_with_isolation(
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
            return Err(format!("Failed to create temp directory: {e}"));
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
                return Err(format!("Failed to create isolated shell command: {e}"));
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
                Err(format!("Execution error: {e}"))
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
                Err(format!("Command timed out after {timeout_secs} seconds"))
            }
        }
    }

    /// Normalize shell command for proper execution
    /// Handles platform-specific quoting and escaping rules
    fn normalize_shell_command(raw_command: &str) -> String {
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

            // 1. Detect incomplete quote pairs
            let double_quote_count = normalized.chars().filter(|&c| c == '"').count();
            let single_quote_count = normalized.chars().filter(|&c| c == '\'').count();

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

    pub async fn handle_execute_shell(&self, args: Value) -> Result<MCPResult, String> {
        let raw_command = match args.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => {
                return Err("Missing required parameter: command".to_string());
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

        // Enforce maximum sync timeout. Read default/max values from runtime
        // configuration so the limit can be adjusted via environment variables
        // (see `src-tauri/src/config.rs`). `default_execution_timeout()` is the
        // recommended default; we treat it as the sync maximum here to keep
        // sync requests short-lived and encourage async for long-running work.
        let sync_max = crate::config::default_execution_timeout();
        if timeout_secs > sync_max {
            return Err(format!(
                "Sync mode supports a maximum timeout of {sync_max} seconds.\nFor longer-running commands, set \"run_mode\" to \"async\" so the command runs in background and can be polled.\nYou can adjust the default via the LIBRAGENT_DEFAULT_EXECUTION_TIMEOUT environment variable.",
            ));
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
        // Always use Medium isolation for security (isolation parameter removed from tool)
        // Medium provides good balance between security and compatibility
        let isolation_level = IsolationLevel::Medium;

        // Use existing isolation-aware shell execution
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
                return Err(format!(
                    "Maximum concurrent processes limit reached ({MAX_CONCURRENT_PROCESSES})"
                ));
            }
        }

        // Generate process ID
        let process_id = cuid2::create_id();

        // Create process tmp directory
        let process_tmp_dir = workspace_path
            .join("tmp")
            .join(format!("process_{process_id}"));

        if let Err(e) = tokio::fs::create_dir_all(&process_tmp_dir).await {
            return Err(format!("Failed to create process directory: {e}"));
        }

        let stdout_path = process_tmp_dir.join("stdout");
        let stderr_path = process_tmp_dir.join("stderr");

        // Normalize command
        let normalized_command = Self::normalize_shell_command(command);

        // Always use Medium isolation (isolation parameter removed from tool)
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
                return Err(format!("Failed to create isolated command: {e}"));
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
            let result = Self::spawn_and_stream_hybrid(
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
                    return Err("Process failed to start".to_string());
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

    /// Handle interactive shell execution (1st tool call)
    /// Returns UIResource with execution_id for user input
    async fn handle_interactive_shell(
        &self,
        command: &str,
        args: &Value,
    ) -> Result<MCPResult, String> {
        use super::{utils::sanitize_command_for_logging, PendingShellExecution};

        let execution_id = uuid::Uuid::new_v4().to_string();
        let session_id = self
            .session_manager
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());

        // Sanitize command for storage/logging
        let sanitized_command = sanitize_command_for_logging(command);

        // Extract run_mode from 1st call (will be used in 2nd call)
        let run_mode = args
            .get("run_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("sync")
            .to_string();

        // Store pending execution
        let pending = PendingShellExecution {
            execution_id: execution_id.clone(),
            session_id,
            executable_command: command.to_string(), // Will be executed (may get -S flag)
            display_command: sanitized_command.clone(), // For logs/UI
            run_mode,                                // Store for 2nd call
            timeout: args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(30), // Command execution timeout
            created_at: chrono::Utc::now(),
        };

        self.pending_executions.insert(pending);

        // Build UIResource with platform-aware prompt
        let (prompt, input_type) = self.get_prompt_config(command, args);
        let html = self.build_shell_input_ui(&execution_id, prompt, input_type);

        // Create UI resource JSON
        let ui_resource = serde_json::json!({
            "uri": format!("ui://shell-input/{}", execution_id),
            "mimeType": "text/html",
            "text": html,
            "_meta": {
                "title": "Shell Command Input",
                "execution_id": execution_id,
                "created_at": chrono::Utc::now().to_rfc3339()
            }
        });

        // Return response with text and resource
        Ok(super::ui_resources::mcp_result_with_text_and_resource(
            &format!(
                "⏳ Waiting for user input\nExecution ID: {execution_id}\nCommand: {sanitized_command}"
            ),
            ui_resource,
        ))
    }

    /// Handle execute_pending_shell tool call (2nd tool call)
    /// Executes pending command with user input via stdin
    pub async fn handle_execute_pending_shell(&self, args: Value) -> Result<MCPResult, String> {
        use super::utils::sanitize_command_for_logging;
        use std::collections::HashMap;
        use tokio::io::AsyncWriteExt;

        let execution_id = match args.get("execution_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return Err("Missing required parameter: execution_id".to_string());
            }
        };

        let user_input = match args.get("user_input").and_then(|v| v.as_str()) {
            Some(input) => input,
            None => {
                return Err("Missing required parameter: user_input".to_string());
            }
        };

        // Retrieve pending execution
        let pending = match self.pending_executions.remove(execution_id) {
            Some(p) => p,
            None => {
                return Err(format!("Unknown or expired execution_id: {execution_id}"));
            }
        };

        // Validate timeout (5 minutes for user input)
        const USER_INPUT_TIMEOUT_SECS: i64 = 300;
        let elapsed = chrono::Utc::now()
            .signed_duration_since(pending.created_at)
            .num_seconds();
        if elapsed > USER_INPUT_TIMEOUT_SECS {
            return Err("Execution request expired. Please retry.".to_string());
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
        let workspace_path = self.get_workspace_dir();
        let session_id = pending.session_id.clone();

        // Check if persistent shell should be used (default: true)
        let use_persistent_shell = args
            .get("use_persistent_shell")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Try persistent shell path first (if enabled)
        if use_persistent_shell && pending.run_mode == "sync" {
            let normalized_command = Self::normalize_shell_command(&final_command);

            // Execute with persistent shell (includes timeout and retry)
            let execution_result = tokio::time::timeout(
                Duration::from_secs(pending.timeout),
                self.shell_manager.execute_with_input(
                    session_id.clone(),
                    &normalized_command,
                    user_input,
                ),
            )
            .await;

            match execution_result {
                Ok(Ok((stdout, stderr, exit_code))) => {
                    // Success - format and return result
                    info!(
                        "Interactive persistent shell executed: {} (session: {}, exit: {})",
                        sanitize_command_for_logging(&pending.display_command),
                        session_id,
                        exit_code
                    );

                    let result_text = if exit_code == 0 {
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
                            exit_code,
                            stdout.trim(),
                            stderr.trim()
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
                    return Err(format!(
                        "Command execution timeout after {} seconds",
                        pending.timeout
                    ));
                }
            }
        }

        // FALLBACK: One-shot execution with stdin injection (original implementation)

        // Create isolation config
        let normalized_command = Self::normalize_shell_command(&final_command);
        let isolation_config = crate::session_isolation::IsolatedProcessConfig {
            session_id: session_id.clone(),
            workspace_path: workspace_path.clone(),
            command: normalized_command,
            args: vec![],
            env_vars: HashMap::new(),
            isolation_level: crate::session_isolation::IsolationLevel::Medium,
        };

        // Create isolated command
        let mut cmd = match self
            .isolation_manager
            .create_isolated_command(isolation_config)
            .await
        {
            Ok(cmd) => cmd,
            Err(e) => {
                return Err(format!("Failed to create isolated command: {e}"));
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
                return Err(format!("Failed to spawn process: {e}"));
            }
        };

        // Write user input to stdin
        if let Some(mut stdin) = child.stdin.take() {
            // CRITICAL: Write password and close stdin
            if let Err(e) = stdin.write_all(user_input.as_bytes()).await {
                return Err(format!("Failed to write to stdin: {e}"));
            }
            if let Err(e) = stdin.write_all(b"\n").await {
                return Err(format!("Failed to write newline: {e}"));
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
                    return Err(format!("Process error: {e}"));
                }
                Err(_) => {
                    let timeout_secs = pending.timeout;
                    return Err(format!(
                        "Command execution timeout after {timeout_secs} seconds"
                    ));
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

            let result_text = if exit_code == 0 {
                if stdout_str.trim().is_empty() && stderr_str.trim().is_empty() {
                    "Command executed successfully (no output)".to_string()
                } else if stderr_str.trim().is_empty() {
                    format!("Command executed successfully:\n{}", stdout_str.trim())
                } else {
                    format!(
                        "Command executed successfully:\nSTDOUT:\n{}\n\nSTDERR:\n{}",
                        stdout_str.trim(),
                        stderr_str.trim()
                    )
                }
            } else {
                format!(
                    "Command failed with exit code {}:\nSTDOUT:\n{}\n\nSTDERR:\n{}",
                    exit_code,
                    stdout_str.trim(),
                    stderr_str.trim()
                )
            };

            Ok(MCPResult::success(&result_text))
        } else {
            // Async mode: Return process_id immediately and spawn monitoring task
            let process_id = cuid2::create_id();

            // Create process tmp directory
            let process_tmp_dir = workspace_path
                .join("tmp")
                .join(format!("process_{process_id}"));

            if let Err(e) = tokio::fs::create_dir_all(&process_tmp_dir).await {
                return Err(format!("Failed to create process directory: {e}"));
            }

            let stdout_path = process_tmp_dir.join("stdout");
            let stderr_path = process_tmp_dir.join("stderr");

            // Register process
            let cancel_token = tokio_util::sync::CancellationToken::new();
            let entry = terminal_manager::ProcessEntry {
                id: process_id.clone(),
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

            Ok(MCPResult::success(&format!(
                "Command running in background.\nProcess ID: {process_id}\n\nUse 'poll_process' to check status."
            )))
        }
    }

    /// Cancel a pending shell execution
    /// Removes the pending execution from state without executing it
    pub async fn handle_cancel_pending_execution(&self, args: Value) -> Result<MCPResult, String> {
        // Extract execution_id
        let execution_id = match args.get("execution_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return Err("Missing required parameter: execution_id".to_string());
            }
        };

        // Remove pending execution
        match self.pending_executions.remove(execution_id) {
            Some(pending) => {
                let message = format!(
                    "✅ Cancelled pending command execution\n\nExecution ID: {}\nCommand: {}",
                    execution_id, pending.display_command
                );
                Ok(MCPResult::success(&message))
            }
            None => Err(format!(
                "No pending execution found with ID: {execution_id}"
            )),
        }
    }

    /// Platform-specific privilege detection for Unix systems
    /// Detects commands that require elevated privileges (sudo, su, doas, pkexec)
    #[cfg(unix)]
    fn detect_privilege_escalation(&self, command: &str) -> bool {
        let trimmed = command.trim_start();
        let patterns = ["sudo ", "su ", "doas ", "pkexec "];
        patterns.iter().any(|p| trimmed.starts_with(p))
    }

    /// Platform-specific privilege detection for Windows
    /// Windows UAC cannot be detected from command string
    /// Agent must explicitly set require_user_input=true
    #[cfg(windows)]
    fn detect_privilege_escalation(&self, _command: &str) -> bool {
        false
    }

    /// Get platform-aware prompt configuration for user input
    /// Returns (prompt, input_type) tuple
    fn get_prompt_config<'a>(&self, command: &str, args: &'a Value) -> (&'a str, &'a str) {
        // Check if privilege escalation detected (Unix only)
        let is_privilege_cmd = self.detect_privilege_escalation(command);

        if is_privilege_cmd {
            ("Enter your sudo password:", "password")
        } else {
            // Use custom prompt from args
            let prompt = args
                .get("input_prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("Enter input:");
            let input_type = args
                .get("input_type")
                .and_then(|v| v.as_str())
                .unwrap_or("text");
            (prompt, input_type)
        }
    }

    /// Build UIResource HTML for shell input form
    /// Returns HTML string with embedded execution_id, prompt, and input type
    fn build_shell_input_ui(&self, execution_id: &str, prompt: &str, input_type: &str) -> String {
        format!(
            r#"<!DOCTYPE html>
<html>
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <style>
      body {{
        font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
        padding: 20px;
        background: #1e1e1e;
        color: #d4d4d4;
        margin: 0;
      }}
      .container {{
        max-width: 500px;
        margin: 0 auto;
      }}
      h3 {{
        margin-top: 0;
        color: #e0e0e0;
      }}
      input {{
        width: 100%;
        padding: 10px;
        margin: 10px 0;
        background: #2d2d2d;
        color: #d4d4d4;
        border: 1px solid #444;
        border-radius: 4px;
        box-sizing: border-box;
        font-size: 14px;
      }}
      input:focus {{
        outline: none;
        border-color: #0e639c;
      }}
      button {{
        padding: 10px 20px;
        margin: 5px 5px 5px 0;
        background: #0e639c;
        color: white;
        border: none;
        border-radius: 4px;
        cursor: pointer;
        font-size: 14px;
      }}
      button:hover {{
        background: #1177bb;
      }}
      .cancel {{
        background: #6c757d;
      }}
      .cancel:hover {{
        background: #5a6268;
      }}
    </style>
  </head>
  <body>
    <div class="container">
      <h3>{}</h3>
      <form id="inputForm">
        <input
          type="{}"
          id="userInput"
          placeholder="Enter {}..."
          required
          autofocus
        />
        <div>
          <button type="submit">Submit</button>
          <button type="button" class="cancel" onclick="handleCancel()">
            Cancel
          </button>
        </div>
      </form>
    </div>

    <script>
      const executionId = '{}';

      document
        .getElementById('inputForm')
        .addEventListener('submit', async (e) => {{
          e.preventDefault();
          const userInput = document.getElementById('userInput').value;

          // Send to parent window (MCP Worker) - triggers 2nd tool call
          // IMPORTANT: Use window.parent.postMessage to send to parent frame
          // Using MCP-UI protocol format: type='tool' with payload wrapper
          window.parent.postMessage(
            {{
              type: 'tool',
              payload: {{
                toolName: 'execute_pending_shell',
                params: {{
                  execution_id: executionId,
                  user_input: userInput,
                }},
              }},
            }},
            '*',
          );

          // Clear input immediately
          document.getElementById('userInput').value = '';
          document.body.innerHTML =
            '<p style="text-align:center; color:#d4d4d4;">⏳ Executing command...</p>';
        }});

      function handleCancel() {{
        // Send to parent window (MCP Worker) - triggers cancel tool call
        // IMPORTANT: Use window.parent.postMessage to send to parent frame
        // Using MCP-UI protocol format: type='tool' with payload wrapper
        window.parent.postMessage(
          {{
            type: 'tool',
            payload: {{
              toolName: 'cancel_pending_execution',
              params: {{
                execution_id: executionId,
              }},
            }},
          }},
          '*',
        );

        document.body.innerHTML =
          '<p style="text-align:center; color:#d4d4d4;">❌ Cancelled</p>';
      }}
    </script>
  </body>
</html>"#,
            html_escape::encode_safe(prompt),
            input_type,
            input_type,
            execution_id
        )
    }
}
