use serde_json::Value;
use std::time::Duration;

use crate::agent::events::AgentEvent;
use crate::agent::tauri_events::emit_agent_event;
use crate::mcp::builtin::workspace::{
    InteractiveShellInputType, PendingShellExecution, PendingShellInputResolution, WorkspaceServer,
    INTERACTIVE_SHELL_INPUT_TIMEOUT_SECS, PERSISTENT_SHELL_TOOL,
};
use crate::mcp::builtin::{
    error_guidance::{guided_error, ErrorCategory, ToolGroup},
    workspace::utils,
};
use crate::mcp::types::MCPResult;

use super::super::validation;

impl WorkspaceServer {
    /// Start an interactive shell execution and wait for the local user to provide input.
    pub(crate) async fn handle_interactive_shell(
        &self,
        command: &str,
        args: &Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        use crate::mcp::builtin::workspace::utils::sanitize_command_for_logging;

        let execution_id = uuid::Uuid::new_v4().to_string();
        let session_id = session_id.to_string();
        let sanitized_command = sanitize_command_for_logging(command);

        let run_mode = args
            .get("run_mode")
            .and_then(|value| value.as_str())
            .unwrap_or("sync")
            .to_string();
        let requested_timeout = args.get("timeout").and_then(|value| value.as_u64());
        let timeout = match utils::resolve_sync_timeout(requested_timeout) {
            Ok(timeout) => timeout,
            Err(max_timeout) => {
                let attempted_timeout =
                    requested_timeout.unwrap_or_else(utils::default_sync_execution_timeout);
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Timeout ({} seconds) exceeds the sync execution limit ({} seconds)",
                        attempted_timeout, max_timeout
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    format!(
                        "Use spawnProcess for commands longer than {} seconds",
                        max_timeout
                    ),
                    "Background processes do not block the active agent workflow".to_string(),
                    format!(
                        "{} stays bounded because it executes synchronously",
                        PERSISTENT_SHELL_TOOL
                    ),
                ])
                .to_mcp_result());
            }
        };

        let Some(app_handle) = crate::state::get_app_handle() else {
            return Ok(guided_error(
                ErrorCategory::InternalError,
                "Interactive shell input requires the desktop UI runtime".to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Open the desktop app UI before retrying the interactive command".to_string(),
                "Use a non-interactive command when no local UI is available".to_string(),
            ])
            .to_mcp_result());
        };

        let (prompt, input_type) = self.get_prompt_config(command, args);
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        let pending = PendingShellExecution {
            execution_id: execution_id.clone(),
            session_id: session_id.clone(),
            executable_command: command.to_string(),
            display_command: sanitized_command.clone(),
            run_mode,
            timeout,
            created_at: chrono::Utc::now(),
            prompt: prompt.clone(),
            input_type: input_type.clone(),
            response_tx: Some(response_tx),
        };

        self.pending_executions.insert(pending);

        if let Err(error) = emit_agent_event(
            app_handle,
            AgentEvent::InteractiveShellInputRequested {
                session_id: session_id.clone(),
                execution_id: execution_id.clone(),
                prompt,
                input_type,
                command: sanitized_command.clone(),
            },
        ) {
            let _ = self.pending_executions.remove(&execution_id);
            return Ok(guided_error(
                ErrorCategory::InternalError,
                "Failed to open the interactive input prompt".to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Try the command again after the session UI is fully loaded".to_string(),
                format!("Internal error: {}", error),
            ])
            .to_mcp_result());
        }

        let resolution = tokio::time::timeout(
            Duration::from_secs(INTERACTIVE_SHELL_INPUT_TIMEOUT_SECS),
            response_rx,
        )
        .await;

        match resolution {
            Ok(Ok(PendingShellInputResolution::Submitted(user_input))) => {
                if let Some(pending) = self.pending_executions.remove(&execution_id) {
                    self.execute_interactive_shell_with_input(pending, user_input)
                        .await
                } else {
                    Ok(guided_error(
                        ErrorCategory::ResourceNotFound,
                        format!(
                            "Interactive execution '{}' is no longer available",
                            execution_id
                        ),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Run the original command again to open a fresh input prompt".to_string(),
                        "Avoid submitting the same prompt from multiple windows".to_string(),
                    ])
                    .to_mcp_result())
                }
            }
            Ok(Ok(PendingShellInputResolution::Cancelled)) => {
                Ok(MCPResult::informational(&format!(
                    "Interactive command cancelled before execution.\n\nCommand: {}",
                    sanitized_command
                )))
            }
            Ok(Err(_)) => {
                let _ = self.pending_executions.remove(&execution_id);
                Ok(guided_error(
                    ErrorCategory::InternalError,
                    "Interactive shell prompt closed unexpectedly".to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Run the original command again to reopen the prompt".to_string(),
                    "Keep the session window open while entering the requested input".to_string(),
                ])
                .to_mcp_result())
            }
            Err(_) => {
                let _ = self.pending_executions.remove(&execution_id);
                let _ = emit_agent_event(
                    app_handle,
                    AgentEvent::InteractiveShellInputResolved {
                        session_id,
                        execution_id,
                        outcome: "expired".to_string(),
                    },
                );

                Ok(guided_error(
                    ErrorCategory::Timeout,
                    format!(
                        "Interactive input timed out after {} minutes",
                        INTERACTIVE_SHELL_INPUT_TIMEOUT_SECS / 60
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Run the original command again to request a fresh prompt".to_string(),
                    "Submit the requested input before the prompt expires".to_string(),
                ])
                .to_mcp_result())
            }
        }
    }

    pub(crate) fn emit_interactive_shell_resolution(
        &self,
        session_id: &str,
        execution_id: &str,
        outcome: &str,
    ) -> Result<(), String> {
        let Some(app_handle) = crate::state::get_app_handle() else {
            return Err("AppHandle not available for interactive shell resolution".to_string());
        };

        emit_agent_event(
            app_handle,
            AgentEvent::InteractiveShellInputResolved {
                session_id: session_id.to_string(),
                execution_id: execution_id.to_string(),
                outcome: outcome.to_string(),
            },
        )
    }

    /// Get platform-aware prompt configuration for user input.
    fn get_prompt_config(
        &self,
        command: &str,
        args: &Value,
    ) -> (String, InteractiveShellInputType) {
        let is_privilege_cmd = validation::detect_privilege_escalation(command);

        if is_privilege_cmd {
            (
                "Enter your sudo password:".to_string(),
                InteractiveShellInputType::Password,
            )
        } else {
            let prompt = args
                .get("inputPrompt")
                .or_else(|| args.get("input_prompt"))
                .and_then(|value| value.as_str())
                .unwrap_or("Enter input:")
                .to_string();
            let input_type = match args
                .get("inputType")
                .or_else(|| args.get("input_type"))
                .and_then(|value| value.as_str())
                .unwrap_or("text")
                .to_ascii_lowercase()
                .as_str()
            {
                "password" => InteractiveShellInputType::Password,
                _ => InteractiveShellInputType::Text,
            };
            (prompt, input_type)
        }
    }
}
