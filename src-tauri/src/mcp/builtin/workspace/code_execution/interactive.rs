use base64::{engine::general_purpose, Engine as _};
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
use crate::session_isolation::{IsolatedProcessConfig, IsolationLevel};

use super::super::{
    terminal_manager, PendingShellExecution, WorkspaceServer, PERSISTENT_SHELL_TOOL,
};
use super::{normalization, validation};

impl WorkspaceServer {
    /// Redact sensitive input from output string
    ///
    /// Note: This uses simple string replacement which may result in over-redaction
    /// (e.g. "pass" will be redacted in "compass"). This is intentional for security
    /// as over-redaction is safer than under-redaction in this context.
    fn redact_sensitive_input(output: &str, sensitive: &str) -> String {
        if sensitive.is_empty() {
            return output.to_string();
        }
        output.replace(sensitive, "********")
    }

    /// De-obfuscate input using XOR and Base64
    fn deobfuscate_input(input_base64: &str, nonce: &str) -> Result<String, String> {
        // If input doesn't look like base64 (e.g. plain text fallback), return as is
        // But for security, we should expect base64 if nonce was provided.
        // For backward compatibility or direct tool calls, we might need to handle plain text.
        // However, since this is a security feature, we assume the UI sends obfuscated data.

        let input_bytes = match general_purpose::STANDARD.decode(input_base64) {
            Ok(b) => b,
            Err(e) => {
                if !nonce.is_empty() {
                    // Security: fail if nonce is present and decoding fails
                    return Err(format!(
                        "Input must be base64-obfuscated when nonce is provided. Decode error: {e}"
                    ));
                } else {
                    // For legacy/plain text, allow fallback but log a warning
                    warn!("Base64 decode failed, falling back to plain text input: {e}");
                    return Ok(input_base64.to_string());
                }
            }
        };

        let nonce_bytes = nonce.as_bytes();
        if nonce_bytes.is_empty() {
            return Ok(input_base64.to_string());
        }

        let xored: Vec<u8> = input_bytes
            .iter()
            .enumerate()
            .map(|(i, &b)| b ^ nonce_bytes[i % nonce_bytes.len()])
            .collect();

        String::from_utf8(xored).map_err(|e| format!("UTF-8 decode failed: {e}"))
    }

    /// Handle interactive shell execution (1st tool call)
    /// Returns UIResource with execution_id for user input
    pub(crate) async fn handle_interactive_shell(
        &self,
        command: &str,
        args: &Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        use super::super::utils::sanitize_command_for_logging;

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

        // Generate nonce for client-side obfuscation
        // SECURITY WARNING:
        // XOR-based obfuscation with a UUID nonce provides only limited security.
        // Since the nonce is transmitted in the HTML, an attacker who can intercept or observe
        // the HTML content can easily reverse the obfuscation by applying the same XOR operation.
        // This approach protects against casual logging but NOT against determined attackers.
        // If the threat model requires protection against active attackers who can observe the UI content,
        // consider using a stronger encryption method (e.g., AES with secure key exchange via Web Crypto API).
        let encryption_nonce = uuid::Uuid::new_v4().to_string();

        // Store pending execution
        let pending = PendingShellExecution {
            execution_id: execution_id.clone(),
            session_id,
            executable_command: command.to_string(), // Will be executed (may get -S flag)
            display_command: sanitized_command.clone(), // For logs/UI
            run_mode,                                // Store for 2nd call
            timeout: args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(30), // Command execution timeout
            encryption_nonce: encryption_nonce.clone(),
            created_at: chrono::Utc::now(),
        };

        self.pending_executions.insert(pending);

        // Build UIResource with platform-aware prompt
        let (prompt, input_type) = self.get_prompt_config(command, args);
        let html = self.build_shell_input_ui(&execution_id, prompt, input_type, &encryption_nonce);

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
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        use super::super::utils::sanitize_command_for_logging;

        let execution_id = match args.get("execution_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return Ok(missing_param_error("execution_id", ToolGroup::Workspace));
            }
        };

        let obfuscated_input = match args.get("user_input").and_then(|v| v.as_str()) {
            Some(input) => input,
            None => {
                return Ok(missing_param_error("user_input", ToolGroup::Workspace));
            }
        };

        // Retrieve pending execution
        let pending = match self.pending_executions.remove(execution_id) {
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
        let user_input = match Self::deobfuscate_input(obfuscated_input, &pending.encryption_nonce)
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
                    let redacted_stdout = Self::redact_sensitive_input(stdout.trim(), user_input);
                    let redacted_stderr = Self::redact_sensitive_input(stderr.trim(), user_input);

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
        let isolation_config = IsolatedProcessConfig {
            session_id: session_id.clone(),
            workspace_path: workspace_path.clone(),
            command: normalized_command,
            args: vec![],
            env_vars: HashMap::new(),
            isolation_level: IsolationLevel::Medium,
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
                            "Use \"runMode\": \"async\" for long-running commands".to_string(),
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
            let redacted_stdout = Self::redact_sensitive_input(stdout_str.trim(), user_input);
            let redacted_stderr = Self::redact_sensitive_input(stderr_str.trim(), user_input);

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
                        "Use pollProcess with process_id \"{}\" to check status",
                        process_id
                    ),
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
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        // Extract execution_id
        let execution_id = match args.get("execution_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return Ok(missing_param_error("execution_id", ToolGroup::Workspace));
            }
        };

        // Remove pending execution
        match self.pending_executions.remove(execution_id) {
            Some(pending) => {
                // Validate session ownership
                if pending.session_id != session_id {
                    // Restore it if session mismatch (although it's already removed... wait.
                    // remove() takes it out. Ideally we check first.
                    // But HashMap only supports remove or get (ref).
                    // We should use entries or re-insert if invalid.
                    // Or, simpler: just error out and don't care about restoring it for invalid requester?
                    // Actually, if an attacker tries to cancel someone else's, implementation details matter.
                    // Since we already removed it, let's just re-insert it if validation fails.
                    self.pending_executions.insert(pending);

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

    /// Get platform-aware prompt configuration for user input
    /// Returns (prompt, input_type) tuple
    fn get_prompt_config<'a>(&self, command: &str, args: &'a Value) -> (&'a str, &'a str) {
        // Check if privilege escalation detected (Unix only)
        let is_privilege_cmd = validation::detect_privilege_escalation(command);

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
    fn build_shell_input_ui(
        &self,
        execution_id: &str,
        prompt: &str,
        input_type: &str,
        nonce: &str,
    ) -> String {
        // Use constants to ensure tool names match definition
        use crate::mcp::builtin::workspace::tools::code_tools::{
            CANCEL_PENDING_EXECUTION, EXECUTE_PENDING_SHELL,
        };

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
      const nonce = '{}';

      function obfuscate(input, nonce) {{
        const textEncoder = new TextEncoder();
        const inputBytes = textEncoder.encode(input);
        const nonceBytes = textEncoder.encode(nonce);
        const xored = new Uint8Array(inputBytes.length);
        for (let i = 0; i < inputBytes.length; i++) {{
          xored[i] = inputBytes[i] ^ nonceBytes[i % nonceBytes.length];
        }}
        // Convert to Base64 more safely (avoid stack overflow)
        let binary = '';
        for (let i = 0; i < xored.length; i++) {{
          binary += String.fromCharCode(xored[i]);
        }}
        return btoa(binary);
      }}

      document
        .getElementById('inputForm')
        .addEventListener('submit', async (e) => {{
          e.preventDefault();
          const userInput = document.getElementById('userInput').value;
          const obfuscatedInput = obfuscate(userInput, nonce);

          // Send to parent window (MCP Worker) - triggers 2nd tool call
          // IMPORTANT: Use window.parent.postMessage to send to parent frame
          // Using MCP-UI protocol format: type='tool' with payload wrapper
          window.parent.postMessage(
            {{
              type: 'tool',
              payload: {{
                toolName: '{}',
                params: {{
                  execution_id: executionId,
                  user_input: obfuscatedInput,
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
              toolName: '{}',
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
            execution_id,
            nonce,
            EXECUTE_PENDING_SHELL,
            CANCEL_PENDING_EXECUTION
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_sensitive_input() {
        let input = "password123";
        let output = "Enter password: password123\nAccess granted";
        let redacted = WorkspaceServer::redact_sensitive_input(output, input);
        assert_eq!(redacted, "Enter password: ********\nAccess granted");

        // Test multiple occurrences
        let output2 = "password123 is the password123";
        let redacted2 = WorkspaceServer::redact_sensitive_input(output2, input);
        assert_eq!(redacted2, "******** is the ********");

        // Test empty input (should not change output)
        let output3 = "normal output";
        let redacted3 = WorkspaceServer::redact_sensitive_input(output3, "");
        assert_eq!(redacted3, "normal output");
    }
    #[test]
    fn test_deobfuscate_input() {
        // "password123" XOR "nonce" -> base64
        // nonce = "nonce" (5 bytes)
        // p (112) ^ n (110) = 2
        // a (97) ^ o (111) = 14
        // s (115) ^ n (110) = 29
        // s (115) ^ c (99) = 16
        // w (119) ^ e (101) = 22
        // o (111) ^ n (110) = 1
        // r (114) ^ o (111) = 29
        // d (100) ^ n (110) = 10
        // 1 (49) ^ c (99) = 82
        // 2 (50) ^ e (101) = 87
        // 3 (51) ^ n (110) = 93

        let nonce = "nonce";
        let original = "password123";

        // Manual XOR for verification
        let original_bytes = original.as_bytes();
        let nonce_bytes = nonce.as_bytes();
        let xored: Vec<u8> = original_bytes
            .iter()
            .enumerate()
            .map(|(i, &b)| b ^ nonce_bytes[i % nonce_bytes.len()])
            .collect();
        let encoded = general_purpose::STANDARD.encode(&xored);

        // Test deobfuscation
        let decoded = WorkspaceServer::deobfuscate_input(&encoded, nonce).unwrap();
        assert_eq!(decoded, original);

        // Test with empty nonce (should return input as is)
        let decoded_empty = WorkspaceServer::deobfuscate_input(original, "").unwrap();
        assert_eq!(decoded_empty, original);

        // Test with invalid base64 (should return error when nonce is provided)
        let result_invalid = WorkspaceServer::deobfuscate_input("not base64", nonce);
        assert!(result_invalid.is_err());
        assert!(result_invalid
            .unwrap_err()
            .contains("Input must be base64-obfuscated"));
    }
}
