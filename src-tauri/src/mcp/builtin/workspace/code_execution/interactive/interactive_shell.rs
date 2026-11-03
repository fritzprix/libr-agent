use serde_json::Value;
use tauri::Manager;

use crate::agent::state::{secret_outcome_message, UserPromptOutcome, UserPromptPayload};
use crate::agent::AgentSessionManager;
use crate::mcp::builtin::workspace::{
    InteractiveShellInputType, InteractiveShellRequest, WorkspaceServer,
    INTERACTIVE_SHELL_INPUT_MAX_BYTES, PERSISTENT_SHELL_TOOL,
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
                        "Use workspace__spawnProcess for commands longer than {} seconds",
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
        let request = InteractiveShellRequest {
            session_id: session_id.clone(),
            executable_command: command.to_string(),
            display_command: sanitized_command,
            run_mode,
            timeout,
            input_type,
            stdin_delivery: validation::resolve_stdin_delivery(command, args),
        };
        let variable_name = shell_input_variable_name(command);
        let payload =
            shell_prompt_payload(input_type, variable_name, prompt, &request.display_command);

        let manager = app_handle.state::<AgentSessionManager>();
        let outcome = match manager.request_user_prompt(&session_id, payload).await {
            Ok(outcome) => outcome,
            Err(error) => {
                return Ok(
                    guided_error(ErrorCategory::InvalidState, error, ToolGroup::Workspace)
                        .guidance(vec![
                    "Wait for the open prompt to close before running another interactive command"
                        .to_string(),
                    "Use a non-interactive command when no local UI is available".to_string(),
                ])
                        .to_mcp_result(),
                );
            }
        };

        let UserPromptOutcome::Answered(user_input) = outcome else {
            return Ok(unanswered_shell_result(
                &outcome,
                input_type,
                variable_name,
                &request.display_command,
            ));
        };

        if user_input.len() > INTERACTIVE_SHELL_INPUT_MAX_BYTES {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Interactive input exceeds the {} byte limit",
                    INTERACTIVE_SHELL_INPUT_MAX_BYTES
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Run the command again and submit a shorter input".to_string()
            ])
            .to_mcp_result());
        }

        let Some(user_input) = single_line_answer(&user_input) else {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Interactive input must be a single line".to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec!["Run the command again and submit one line".to_string()])
            .to_mcp_result());
        };

        self.execute_interactive_shell_with_input(request, user_input.to_string())
            .await
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

/// A password prompt is a masked `secret`; any other input is a `question`.
/// The tool result when the user gave no answer. A timeout is an error
/// (#2543 requirement 5); cancel and Stop stay informational, as before.
fn unanswered_shell_result(
    outcome: &UserPromptOutcome,
    input_type: InteractiveShellInputType,
    variable_name: &str,
    display_command: &str,
) -> MCPResult {
    let message = match input_type {
        InteractiveShellInputType::Password => secret_outcome_message(outcome, variable_name),
        InteractiveShellInputType::Text => outcome.caller_message(),
    };
    let text = format!("Interactive command not executed. {message}\n\nCommand: {display_command}");
    if *outcome != UserPromptOutcome::TimedOut {
        return MCPResult::informational(&text);
    }
    // `ErrorCategory::Timeout` renders a non-error notice; the prompt timeout
    // must be an error result.
    guided_error(ErrorCategory::OperationFailed, text, ToolGroup::Workspace)
        .guidance(vec![
            "Do not retry the command; the user did not answer the prompt".to_string(),
            "Ask the user in chat whether to run it again".to_string(),
        ])
        .to_mcp_result()
}

fn shell_prompt_payload(
    input_type: InteractiveShellInputType,
    variable_name: &str,
    prompt: String,
    display_command: &str,
) -> UserPromptPayload {
    match input_type {
        InteractiveShellInputType::Password => UserPromptPayload::Secret {
            variable_name: variable_name.to_string(),
            label: prompt,
            reuse_note: None,
        },
        InteractiveShellInputType::Text => UserPromptPayload::Question {
            title: prompt,
            body: Some(display_command.to_string()),
        },
    }
}

/// The shell reads one line, so a multi-line answer is refused.
fn single_line_answer(answer: &str) -> Option<&str> {
    let line = answer.trim_end_matches(['\r', '\n']);
    (!line.contains(['\r', '\n'])).then_some(line)
}

/// Name shown in the secret prompt; the shell has no variable of its own.
fn shell_input_variable_name(command: &str) -> &'static str {
    if validation::detect_privilege_escalation(command) {
        "SUDO_PASSWORD"
    } else {
        "SHELL_INPUT"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timed_out_prompt_is_an_error_result() {
        for input_type in [
            InteractiveShellInputType::Password,
            InteractiveShellInputType::Text,
        ] {
            let result = unanswered_shell_result(
                &UserPromptOutcome::TimedOut,
                input_type,
                "SUDO_PASSWORD",
                "sudo apt update",
            );
            assert_eq!(result.is_error, Some(true));
        }
    }

    #[test]
    fn a_cancelled_or_stopped_prompt_is_not_an_error_result() {
        for outcome in [UserPromptOutcome::Cancelled, UserPromptOutcome::Stopped] {
            let result = unanswered_shell_result(
                &outcome,
                InteractiveShellInputType::Password,
                "SUDO_PASSWORD",
                "sudo apt update",
            );
            assert_ne!(result.is_error, Some(true));
        }
    }

    #[test]
    fn sudo_password_becomes_a_secret_prompt() {
        let payload = shell_prompt_payload(
            InteractiveShellInputType::Password,
            shell_input_variable_name("sudo apt update"),
            "Enter your sudo password:".to_string(),
            "sudo apt update",
        );
        assert_eq!(
            payload,
            UserPromptPayload::Secret {
                variable_name: "SUDO_PASSWORD".to_string(),
                label: "Enter your sudo password:".to_string(),
                reuse_note: None,
            }
        );
    }

    #[test]
    fn text_input_becomes_a_question_showing_the_command() {
        let payload = shell_prompt_payload(
            InteractiveShellInputType::Text,
            shell_input_variable_name("read name"),
            "Enter input:".to_string(),
            "read name",
        );
        assert_eq!(
            payload,
            UserPromptPayload::Question {
                title: "Enter input:".to_string(),
                body: Some("read name".to_string()),
            }
        );
    }

    #[test]
    fn answers_must_be_one_line() {
        assert_eq!(single_line_answer("secret"), Some("secret"));
        assert_eq!(single_line_answer("secret\r\n"), Some("secret"));
        assert_eq!(single_line_answer("a\nb"), None);
    }
}
