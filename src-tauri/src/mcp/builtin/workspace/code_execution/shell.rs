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
        let normalized_command = Self::normalize_shell_command(command);

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
                        // Ensure it starts with ./ for clarity if it's relative
                        if relative_cwd.starts_with(".")
                            || relative_cwd.starts_with("/")
                            || relative_cwd.contains(":")
                        {
                            relative_cwd.to_string()
                        } else {
                            format!("./{}", relative_cwd)
                        }
                    };

                    // Invalidate service context cache to reflect CWD or status changes
                    self.invalidate_context_cache().await;

                    // Success - format with clear state reporting
                    let header = format!("✓ Command executed successfully in {}ms", duration_ms);

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
                    Ok(MCPResult::success_with_data(
                        text_message.as_str(),
                        structured_data,
                    ))
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

                    Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::OperationFailed,
                        error_message,
                        vec![
                            "Review the error message in stderr for details".to_string(),
                            "Check command syntax and file paths".to_string(),
                            "Use listDirectory to verify paths exist".to_string(),
                        ],
                        ToolGroup::Workspace,
                    )
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

        // Normalize shell command
        let normalized_command = Self::normalize_shell_command(command);

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
                let might_be_interactive = Self::is_likely_interactive_command(command);

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
                    // PowerShell single-quote escaping: '' inside single quotes
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
                // PowerShell escape inside double quotes via backtick
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

    /// Detect commands that commonly require interactive input
    ///
    /// This function checks for patterns indicating a command will wait for user input,
    /// such as npm init without --yes, npx create-* without --force, REPL modes, etc.
    ///
    /// Returns true if the command is likely to require interactive input.
    fn is_likely_interactive_command(command: &str) -> bool {
        let cmd_lower = command.to_lowercase();
        let cmd_trimmed = cmd_lower.trim();

        // Pattern 1: Package manager initialization without non-interactive flags
        let package_init_patterns = [
            ("npm init", &["--yes", "-y"] as &[&str]),
            ("pnpm init", &["--yes", "-y"]),
            ("yarn init", &["--yes", "-y", "--private"]),
            ("bun init", &["--yes", "-y"]),
        ];

        for (pattern, non_interactive_flags) in package_init_patterns {
            if cmd_lower.contains(pattern) {
                let has_flag = non_interactive_flags
                    .iter()
                    .any(|flag| cmd_lower.contains(flag));
                if !has_flag {
                    return true;
                }
            }
        }

        // Pattern 2: Scaffolding/creation tools without force flags
        let scaffolding_patterns = [
            ("npx create-", &["--force", "--yes", "-y"] as &[&str]),
            ("npm create", &["--force", "--yes", "-y"]),
            ("pnpm create", &["--force", "--yes", "-y"]),
            ("yarn create", &["--force", "--yes", "-y"]),
            ("npx degit", &[]),
        ];

        for (pattern, non_interactive_flags) in scaffolding_patterns {
            if cmd_lower.contains(pattern) {
                if non_interactive_flags.is_empty() {
                    return true;
                }
                let has_flag = non_interactive_flags
                    .iter()
                    .any(|flag| cmd_lower.contains(flag));
                if !has_flag {
                    return true;
                }
            }
        }

        // Pattern 3: PowerShell interactive cmdlets (always interactive)
        let ps_interactive_cmdlets = ["read-host", "get-credential", "out-gridview"];
        for cmdlet in ps_interactive_cmdlets {
            if cmd_lower.contains(cmdlet) {
                return true;
            }
        }

        // Pattern 4: REPL mode detection (executable without arguments)
        // Check for bare executables that start interactive sessions
        let repl_executables = [
            "python",
            "python3",
            "py",
            "node",
            "irb",
            "ruby",
            "psql",
            "mysql",
            "mongosh",
            "redis-cli",
        ];

        for exec in repl_executables {
            // Match pattern: command starts with executable and has no script argument
            if cmd_trimmed == exec {
                // Exact match - definitely REPL
                return true;
            }

            // Check if it's "executable" followed only by flags (no positional args)
            if let Some(rest) = cmd_trimmed.strip_prefix(exec) {
                let rest = rest.trim();

                // Exception: "python -c", "python -m", "node -e" are NOT REPL (check first)
                // These execute code or modules non-interactively
                if rest.starts_with("-c ")
                    || rest.starts_with("-m ")
                    || rest.starts_with("-e ")
                    || rest.starts_with("--eval ")
                    || rest.starts_with("-c\t")
                    || rest.starts_with("-m\t")
                    || rest.starts_with("-e\t")
                    || rest.starts_with("--eval\t")
                {
                    continue;
                }

                // If rest is empty or only contains flags starting with -, it's likely REPL
                if rest.is_empty()
                    || (rest.starts_with('-') && !rest.contains(".py") && !rest.contains(".js"))
                {
                    return true;
                }
            }
        }

        // Pattern 5: Git interactive commands
        let git_interactive = [
            "git add -p",
            "git add --patch",
            "git rebase -i",
            "git rebase --interactive",
        ];
        for pattern in git_interactive {
            if cmd_lower.contains(pattern) {
                return true;
            }
        }

        // Pattern 6: Interactive shells invoked directly
        let interactive_shells = [
            "bash\n",
            "bash\r",
            "bash ",
            "sh\n",
            "sh\r",
            "sh ",
            "powershell\n",
            "powershell\r",
            "pwsh\n",
            "pwsh\r",
        ];
        for shell_pattern in interactive_shells {
            if cmd_lower.ends_with(shell_pattern.trim()) || cmd_trimmed == shell_pattern.trim() {
                return true;
            }
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

        // Check for requireUserInput parameter or auto-detect privilege escalation
        let require_input = args
            .get("requireUserInput")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let auto_detect = self.detect_privilege_escalation(raw_command);

        // If user input required, return UIResource for interactive execution
        if require_input || auto_detect {
            return self
                .handle_interactive_shell(raw_command, &args, session_id)
                .await;
        }

        // Sync mode: persistent shell execution
        let timeout_secs = utils::validate_timeout(args.get("timeout").and_then(|v| v.as_u64()));

        // Enforce maximum sync timeout
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

        // Execute with persistent shell (state preservation)
        self.execute_shell_persistent(raw_command, timeout_secs, session_id)
            .await
    }

    /// Handle primary isolated shell execution (new tool)
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

        // ✅ ENHANCED: Detect commands requiring interactive input BEFORE execution
        if Self::is_likely_interactive_command(raw_command) {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                "This command likely requires interactive input (prompts, passwords, confirmations)",
                vec![
                    format!("Detected interactive pattern in: {}", raw_command),
                    format!("Use {} with requireUserInput: true for interactive commands", PERSISTENT_SHELL_TOOL),
                    "Or add non-interactive flags to your command:".to_string(),
                    "  • npm init → npm init --yes".to_string(),
                    "  • npx create-vite → npx create-vite . --force".to_string(),
                    "  • python (REPL) → python script.py".to_string(),
                    "  • Read-Host → Use config files or environment variables".to_string(),
                    format!("See {} tool description for requireUserInput usage", PERSISTENT_SHELL_TOOL),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        // Get timeout (use default if not specified)
        let timeout_secs = utils::validate_timeout(args.get("timeout").and_then(|v| v.as_u64()));

        // Enforce maximum sync timeout
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

        // Execute with Medium isolation (always workspace root anchored)
        self.execute_shell_with_isolation(
            raw_command,
            IsolationLevel::Medium,
            timeout_secs,
            session_id,
        )
        .await
    }

    /// Handle async shell execution (separate tool)
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

        // Async mode does not support interactive input
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

        // Execute in background
        self.execute_shell_async(raw_command, &args, session_id)
            .await
    }

    /// Execute shell command asynchronously in background
    async fn execute_shell_async(
        &self,
        command: &str,
        _args: &Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        // Get session info
        let session_id = session_id.to_string();

        let workspace_path = self
            .session_manager
            .get_session_workspace_dir_by_id(&session_id);

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

        // Generate process ID
        let process_id = cuid2::create_id();

        // Create process tmp directory
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

        // Invalidate service context cache to reflect new process
        self.invalidate_context_cache().await;

        // Return immediate response with process_id
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

    #[test]
    fn test_is_likely_interactive_command() {
        // ✅ Python -m commands should NOT be interactive
        assert!(!WorkspaceServer::is_likely_interactive_command(
            "python -m unittest discover tests"
        ));
        assert!(!WorkspaceServer::is_likely_interactive_command(
            "python -m pytest"
        ));
        assert!(!WorkspaceServer::is_likely_interactive_command(
            "python3 -m pip install requests"
        ));
        assert!(!WorkspaceServer::is_likely_interactive_command(
            "py -m venv env"
        ));

        // ✅ Python -c commands should NOT be interactive
        assert!(!WorkspaceServer::is_likely_interactive_command(
            "python -c 'print(123)'"
        ));
        assert!(!WorkspaceServer::is_likely_interactive_command(
            "python3 -c \"import sys; print(sys.version)\""
        ));

        // ✅ Node -e commands should NOT be interactive
        assert!(!WorkspaceServer::is_likely_interactive_command(
            "node -e \"console.log('test')\""
        ));

        // ❌ Bare Python should be interactive (REPL)
        assert!(WorkspaceServer::is_likely_interactive_command("python"));
        assert!(WorkspaceServer::is_likely_interactive_command("python3"));
        assert!(WorkspaceServer::is_likely_interactive_command("node"));

        // ❌ npm init without flags should be interactive
        assert!(WorkspaceServer::is_likely_interactive_command("npm init"));
        // ✅ npm init with --yes should NOT be interactive
        assert!(!WorkspaceServer::is_likely_interactive_command(
            "npm init --yes"
        ));

        // ❌ npx create-* without flags should be interactive
        assert!(WorkspaceServer::is_likely_interactive_command(
            "npx create-vite my-app"
        ));
        // ✅ npx create-* with --force should NOT be interactive
        assert!(!WorkspaceServer::is_likely_interactive_command(
            "npx create-vite my-app --force"
        ));

        // ❌ Read-Host should be interactive
        assert!(WorkspaceServer::is_likely_interactive_command(
            "Read-Host 'Enter password'"
        ));

        // ✅ Normal scripts should NOT be interactive
        assert!(!WorkspaceServer::is_likely_interactive_command(
            "python script.py"
        ));
        assert!(!WorkspaceServer::is_likely_interactive_command(
            "node index.js"
        ));
        assert!(!WorkspaceServer::is_likely_interactive_command(
            "cargo test"
        ));
    }
}
