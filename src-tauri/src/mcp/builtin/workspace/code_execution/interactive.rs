use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use crate::mcp::types::MCPResult;
use crate::session_isolation::{IsolatedProcessConfig, IsolationLevel};

use super::super::{terminal_manager, PendingShellExecution, WorkspaceServer};

impl WorkspaceServer {
    /// Handle interactive shell execution (1st tool call)
    /// Returns UIResource with execution_id for user input
    pub(crate) async fn handle_interactive_shell(
        &self,
        command: &str,
        args: &Value,
    ) -> Result<MCPResult, String> {
        use super::super::utils::sanitize_command_for_logging;

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
        Ok(super::super::ui_resources::mcp_result_with_text_and_resource(
            &format!(
                "⏳ Waiting for user input\nExecution ID: {execution_id}\nCommand: {sanitized_command}"
            ),
            ui_resource,
        ))
    }

    /// Handle execute_pending_shell tool call (2nd tool call)
    /// Executes pending command with user input via stdin
    pub async fn handle_execute_pending_shell(&self, args: Value) -> Result<MCPResult, String> {
        use super::super::utils::sanitize_command_for_logging;

        let execution_id = match args.get("execution_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return Ok(MCPResult::error("Missing required parameter: execution_id"));
            }
        };

        let user_input = match args.get("user_input").and_then(|v| v.as_str()) {
            Some(input) => input,
            None => {
                return Ok(MCPResult::error("Missing required parameter: user_input"));
            }
        };

        // Retrieve pending execution
        let pending = match self.pending_executions.remove(execution_id) {
            Some(p) => p,
            None => {
                return Ok(MCPResult::error(&format!(
                    "Unknown or expired execution_id: {execution_id}"
                )));
            }
        };

        // Validate timeout (5 minutes for user input)
        const USER_INPUT_TIMEOUT_SECS: i64 = 300;
        let elapsed = chrono::Utc::now()
            .signed_duration_since(pending.created_at)
            .num_seconds();
        if elapsed > USER_INPUT_TIMEOUT_SECS {
            return Ok(MCPResult::error("Execution request expired. Please retry."));
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
                    workspace_path.clone(),
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
                    return Ok(MCPResult::error(&format!(
                        "Command execution timeout after {} seconds",
                        pending.timeout
                    )));
                }
            }
        }

        // FALLBACK: One-shot execution with stdin injection (original implementation)

        // Create isolation config
        let normalized_command = Self::normalize_shell_command(&final_command);
        let isolation_config = IsolatedProcessConfig {
            session_id: session_id.clone(),
            workspace_path: workspace_path.clone(),
            command: normalized_command,
            args: vec![],
            env_vars: HashMap::new(),
            isolation_level: IsolationLevel::Medium,
        };

        // Create isolated command
        let mut cmd = match self
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

        // Configure stdio pipes
        use std::process::Stdio;
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                return Ok(MCPResult::error(&format!("Failed to spawn process: {e}")));
            }
        };

        // Write user input to stdin
        if let Some(mut stdin) = child.stdin.take() {
            // CRITICAL: Write password and close stdin
            if let Err(e) = stdin.write_all(user_input.as_bytes()).await {
                return Ok(MCPResult::error(&format!("Failed to write to stdin: {e}")));
            }
            if let Err(e) = stdin.write_all(b"\n").await {
                return Ok(MCPResult::error(&format!("Failed to write newline: {e}")));
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
                    return Ok(MCPResult::error(&format!("Process error: {e}")));
                }
                Err(_) => {
                    let timeout_secs = pending.timeout;
                    return Ok(MCPResult::error(&format!(
                        "Command execution timeout after {timeout_secs} seconds"
                    )));
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
                return Ok(MCPResult::error(&format!(
                    "Failed to create process directory: {e}"
                )));
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
                return Ok(MCPResult::error("Missing required parameter: execution_id"));
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
