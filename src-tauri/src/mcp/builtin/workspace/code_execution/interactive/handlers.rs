use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use crate::mcp::builtin::error_guidance::{
    missing_param_error, operation_failed_error, ErrorCategory, ErrorGuidance, SuccessHint,
    ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::session_isolation::IsolatedProcessConfig;

// Imports from workspace module
use crate::mcp::builtin::workspace::{
    terminal_manager, utils, PendingShellExecution, WorkspaceServer, PERSISTENT_SHELL_TOOL,
};

// Relative imports
use super::super::normalization;
use super::{security, ui};

/// Handle interactive shell execution (1st tool call)
/// Returns UIResource with execution_id for user input
pub(crate) async fn handle_interactive_shell(
    server: &WorkspaceServer,
    command: &str,
    args: &Value,
    session_id: &str,
) -> Result<MCPResult, String> {
    use crate::mcp::builtin::workspace::utils::sanitize_command_for_logging;

    let execution_id = uuid::Uuid::new_v4().to_string();
    let session_id = session_id.to_string();

    // Sanitize command for storage/logging
    let sanitized_command = sanitize_command_for_logging(command);

    // Extract run_mode from 1st call (will be used in 2nd call)
    let run_mode = args
        .get("run_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("sync")
        .to_string();

    // Validate and enforce timeout limits
    let timeout = utils::validate_timeout(args.get("timeout").and_then(|v| v.as_u64()));

    // Generate nonce for client-side obfuscation
    let encryption_nonce = uuid::Uuid::new_v4().to_string();

    // Store pending execution
    let pending = PendingShellExecution {
        execution_id: execution_id.clone(),
        session_id,
        executable_command: command.to_string(), // Will be executed (may get -S flag)
        display_command: sanitized_command.clone(), // For logs/UI
        run_mode,                                // Store for 2nd call
        timeout,                                 // Validated timeout with enforced max
        encryption_nonce: encryption_nonce.clone(),
        created_at: chrono::Utc::now(),
    };

    server.pending_executions.insert(pending);

    // Build UIResource with platform-aware prompt
    let (prompt, input_type) = ui::get_prompt_config(command, args);
    let html = ui::build_shell_input_ui(&execution_id, prompt, input_type, &encryption_nonce);

    // Create UI resource JSON
    let _ui_resource = serde_json::json!({
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
    Ok(crate::mcp::builtin::utils::create_resource_response(
        &format!("ui://shell-input/{}", execution_id),
        "text/html",
        &html,
        "workspace",
        PERSISTENT_SHELL_TOOL,
        Some(&format!(
            "⏳ Waiting for user input\nExecution ID: {execution_id}\nCommand: {sanitized_command}"
        )),
    ))
}

/// Handle execute_pending_shell tool call (2nd tool call)
/// Executes pending command with user input via stdin
pub async fn handle_execute_pending_shell(
    server: &WorkspaceServer,
    args: Value,
    session_id: &str,
) -> Result<MCPResult, String> {
    use crate::mcp::builtin::workspace::utils::sanitize_command_for_logging;

    // Accept both camelCase (schema) and snake_case (legacy) for backward compatibility
    let execution_id = match args
        .get("executionId")
        .or_else(|| args.get("execution_id"))
        .and_then(|v| v.as_str())
    {
        Some(id) => id,
        None => {
            // Use the schema/documented name in the error
            return Ok(missing_param_error("executionId", ToolGroup::Workspace));
        }
    };

    let obfuscated_input = match args
        .get("userInput")
        .or_else(|| args.get("user_input"))
        .and_then(|v| v.as_str())
    {
        Some(input) => input,
        None => {
            // Use the schema/documented name in the error
            return Ok(missing_param_error("userInput", ToolGroup::Workspace));
        }
    };

    // Retrieve pending execution
    let pending = match server.pending_executions.remove(execution_id) {
        Some(p) => p,
        None => {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::ResourceNotFound,
                format!("Execution '{}' not found or expired", execution_id),
                vec![
                    "Execute the original command again to get a new execution_id".to_string(),
                    format!("Execution requests expire after {} minutes", 5),
                    "Ensure you're using the execution_id from the UI resource".to_string(),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }
    };

    // Validate session ownership
    if pending.session_id != session_id {
        return Ok(ErrorGuidance::with_guidance(
            ErrorCategory::PermissionDenied,
            format!(
                "Pending execution '{}' belongs to a different session",
                execution_id
            ),
            vec![
                "Ensure you are executing the command in the correct session".to_string(),
                "Executions are isolated per session".to_string(),
            ],
            ToolGroup::Workspace,
        )
        .to_mcp_result());
    }

    // De-obfuscate user input
    let user_input = match security::deobfuscate_input(obfuscated_input, &pending.encryption_nonce)
    {
        Ok(s) => s,
        Err(e) => {
            return Ok(operation_failed_error(
                "De-obfuscate user input",
                &e,
                vec![
                    "This is an internal error - the UI should handle obfuscation".to_string(),
                    "Try executing the command again".to_string(),
                    "Contact support if this persists".to_string(),
                ],
                ToolGroup::Workspace,
            ));
        }
    };
    let user_input = user_input.as_str();

    // Validate timeout (5 minutes for user input)
    const USER_INPUT_TIMEOUT_SECS: i64 = 300;
    let elapsed = chrono::Utc::now()
        .signed_duration_since(pending.created_at)
        .num_seconds();
    if elapsed > USER_INPUT_TIMEOUT_SECS {
        return Ok(ErrorGuidance::with_guidance(
            ErrorCategory::Timeout,
            format!("Execution request expired after {} seconds", elapsed),
            vec![
                "Execute the original command again to get a new execution_id".to_string(),
                format!(
                    "User input must be submitted within {} minutes",
                    USER_INPUT_TIMEOUT_SECS / 60
                ),
                "Respond more quickly to interactive prompts".to_string(),
            ],
            ToolGroup::Workspace,
        )
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
    let workspace_path = server.get_workspace_dir(&session_id);

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
            server.shell_manager.execute_with_input(
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
                let redacted_stdout = security::redact_sensitive_input(stdout.trim(), user_input);
                let redacted_stderr = security::redact_sensitive_input(stderr.trim(), user_input);

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
    let isolation_level = utils::get_shell_isolation_level().await;
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
    let mut cmd = match server
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

    // Configure stdio pipes
    use std::process::Stdio;
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return Ok(operation_failed_error(
                "Spawn process",
                &e.to_string(),
                vec![
                    "Verify the command syntax is correct".to_string(),
                    "Check if required programs are installed".to_string(),
                    "Ensure the command has execute permissions".to_string(),
                ],
                ToolGroup::Workspace,
            ));
        }
    };

    // Write user input to stdin
    if let Some(mut stdin) = child.stdin.take() {
        // CRITICAL: Write password and close stdin
        if let Err(e) = stdin.write_all(user_input.as_bytes()).await {
            return Ok(operation_failed_error(
                "Write to stdin",
                &e.to_string(),
                vec![
                    "The process may have crashed before accepting input".to_string(),
                    "Try executing the command again".to_string(),
                    "Check if the command expects input in a different format".to_string(),
                ],
                ToolGroup::Workspace,
            ));
        }
        if let Err(e) = stdin.write_all(b"\n").await {
            return Ok(operation_failed_error(
                "Write newline to stdin",
                &e.to_string(),
                vec![
                    "The process may have closed stdin unexpectedly".to_string(),
                    "Try executing the command again".to_string(),
                    "Verify the command is still running".to_string(),
                ],
                ToolGroup::Workspace,
            ));
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
                return Ok(operation_failed_error(
                    "Execute command with user input",
                    &e.to_string(),
                    vec![
                        "The command may have invalid syntax or crashed".to_string(),
                        "Verify the command works without user input first".to_string(),
                        "Check system logs for more details".to_string(),
                    ],
                    ToolGroup::Workspace,
                ));
            }
            Err(_) => {
                let timeout_secs = pending.timeout;
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::Timeout,
                    format!("Command execution timeout after {} seconds", timeout_secs),
                    vec![
                        format!("Increase timeout parameter (current: {}s)", timeout_secs),
                        "Use \"run_mode\": \"async\" for long-running commands".to_string(),
                        "Verify the command isn't hanging waiting for additional input"
                            .to_string(),
                    ],
                    ToolGroup::Workspace,
                )
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
            return Ok(operation_failed_error(
                "Create process directory",
                &e.to_string(),
                vec![
                    "Check workspace directory permissions".to_string(),
                    "Ensure sufficient disk space is available".to_string(),
                    "Verify tmp directory is writable".to_string(),
                ],
                ToolGroup::Workspace,
            ));
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
            let mut registry = server.process_registry.write().await;
            registry.entries.insert(process_id.clone(), entry);
            registry
                .cancellation_tokens
                .insert(process_id.clone(), cancel_token.clone());
        }

        // Spawn monitoring task with proper output streaming
        let registry = server.process_registry.clone();
        let pid_copy = process_id.clone();
        let stdout_path_copy = stdout_path.clone();
        let stderr_path_copy = stderr_path.clone();
        let user_input_copy = user_input.to_string();
        let display_command = pending.display_command.clone();

        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            
            // Take stdout and stderr pipes from child
            let stdout_pipe = child.stdout.take();
            let stderr_pipe = child.stderr.take();

            // Spawn tasks to stream stdout and stderr to files
            let stdout_task = if let Some(stdout) = stdout_pipe {
                let path = stdout_path_copy.clone();
                let input = user_input_copy.clone();
                Some(tokio::spawn(async move {
                    let reader = BufReader::new(stdout);
                    let mut lines = reader.lines();
                    let mut output_lines = Vec::new();

                    match tokio::fs::File::create(&path).await {
                        Ok(file) => {
                            let mut writer = tokio::io::BufWriter::new(file);
                            
                            while let Ok(Some(line)) = lines.next_line().await {
                                // Redact sensitive input from output
                                let redacted = super::security::redact_sensitive_input(&line, &input);
                                output_lines.push(redacted.clone());
                                
                                // Write to file
                                let _ = writer.write_all(redacted.as_bytes()).await;
                                let _ = writer.write_all(b"\n").await;
                            }
                            
                            let _ = writer.flush().await;
                            output_lines
                        }
                        Err(e) => {
                            tracing::error!("Failed to create stdout file: {}", e);
                            Vec::new()
                        }
                    }
                }))
            } else {
                None
            };

            let stderr_task = if let Some(stderr) = stderr_pipe {
                let path = stderr_path_copy.clone();
                let input = user_input_copy.clone();
                Some(tokio::spawn(async move {
                    let reader = BufReader::new(stderr);
                    let mut lines = reader.lines();
                    let mut output_lines = Vec::new();

                    match tokio::fs::File::create(&path).await {
                        Ok(file) => {
                            let mut writer = tokio::io::BufWriter::new(file);
                            
                            while let Ok(Some(line)) = lines.next_line().await {
                                // Redact sensitive input from output
                                let redacted = super::security::redact_sensitive_input(&line, &input);
                                output_lines.push(redacted.clone());
                                
                                // Write to file
                                let _ = writer.write_all(redacted.as_bytes()).await;
                                let _ = writer.write_all(b"\n").await;
                            }
                            
                            let _ = writer.flush().await;
                            output_lines
                        }
                        Err(e) => {
                            tracing::error!("Failed to create stderr file: {}", e);
                            Vec::new()
                        }
                    }
                }))
            } else {
                None
            };

            // Wait for process to complete
            let result = child.wait().await;

            // Wait for streaming tasks to complete
            if let Some(task) = stdout_task {
                let _ = task.await;
            }
            if let Some(task) = stderr_task {
                let _ = task.await;
            }

            // Update registry with final status
            let mut reg = registry.write().await;
            if let Some(entry) = reg.entries.get_mut(&pid_copy) {
                match result {
                    Ok(status) => {
                        entry.exit_code = status.code();
                        entry.status = if status.code().unwrap_or(-1) == 0 {
                            terminal_manager::ProcessStatus::Finished
                        } else {
                            terminal_manager::ProcessStatus::Failed
                        };
                        
                        // Update file sizes
                        entry.stdout_size = terminal_manager::get_file_size(&stdout_path_copy).await;
                        entry.stderr_size = terminal_manager::get_file_size(&stderr_path_copy).await;
                    }
                    Err(_) => {
                        entry.status = terminal_manager::ProcessStatus::Failed;
                    }
                }
                entry.finished_at = Some(chrono::Utc::now());
                
                // Log completion
                info!(
                    "Interactive shell async executed: {} (session: {}, status: {:?})",
                    display_command,
                    entry.session_id,
                    entry.status
                );
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
                    "Use pollProcess(\"{}\") to check status and completion",
                    process_id
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

/// Cancel a pending shell execution
/// Removes the pending execution from state without executing it
pub async fn handle_cancel_pending_execution(
    server: &WorkspaceServer,
    args: Value,
    session_id: &str,
) -> Result<MCPResult, String> {
    // Accept both camelCase (schema) and snake_case (legacy) for backward compatibility
    let execution_id = match args
        .get("executionId")
        .or_else(|| args.get("execution_id"))
        .and_then(|v| v.as_str())
    {
        Some(id) => id,
        None => {
            // Use the schema/documented name in the error
            return Ok(missing_param_error("executionId", ToolGroup::Workspace));
        }
    };

    // Remove pending execution
    match server.pending_executions.remove(execution_id) {
        Some(pending) => {
            // Validate session ownership
            if pending.session_id != session_id {
                server.pending_executions.insert(pending);

                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::PermissionDenied,
                    format!(
                        "Pending execution '{}' belongs to a different session",
                        execution_id
                    ),
                    vec![
                        "Ensure you are executing the command in the correct session"
                            .to_string(),
                        "Executions are isolated per session".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }

            let hint = SuccessHint::new(
                format!("Cancelled pending execution: {}", pending.display_command),
                vec!["Execute the command again if needed".to_string()],
            );

            let response_data = serde_json::json!({
                "execution_id": execution_id,
                "command": pending.display_command,
                "cancelled": true
            });

            Ok(hint.to_mcp_result_with_data(Some(response_data)))
        }
        None => Ok(ErrorGuidance::with_guidance(
            ErrorCategory::ResourceNotFound,
            format!("Pending execution '{}' not found", execution_id),
            vec![
                "The execution may have already been completed or cancelled".to_string(),
                "Verify the execution_id is correct".to_string(),
                format!("Executions expire after {} minutes", 5),
            ],
            ToolGroup::Workspace,
        )
        .to_mcp_result()),
    }
}
