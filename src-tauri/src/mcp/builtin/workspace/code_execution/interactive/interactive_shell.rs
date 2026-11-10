use serde_json::Value;
use tauri::Manager;

use crate::agent::state::{secret_outcome_message, UserPromptOutcome, UserPromptPayload};
use crate::agent::AgentSessionManager;
use crate::mcp::builtin::workspace::code_execution::shell::persistent::prepend_result_note;
use crate::mcp::builtin::workspace::persistent_shell::secrets::validate_secret_name;
use crate::mcp::builtin::workspace::{
    WorkspaceServer, INTERACTIVE_SHELL_INPUT_MAX_BYTES, PERSISTENT_SHELL_TOOL,
};
use crate::mcp::builtin::{
    error_guidance::{guided_error, ErrorCategory, ToolGroup},
    workspace::utils,
};
use crate::mcp::types::MCPResult;

impl WorkspaceServer {
    /// Ask the local user for a value, store it as a session secret, then run the command.
    ///
    /// The shell has no terminal, so user input is never written to it. The command
    /// reads the stored value from `$<storeAs>` instead.
    pub(crate) async fn handle_interactive_shell(
        &self,
        command: &str,
        args: &Value,
        session_id: &str,
        tool_name: &str,
    ) -> Result<MCPResult, String> {
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

        let name = match self
            .prompt_and_store_secret(command, args, session_id)
            .await
        {
            Ok(name) => name,
            Err(result) => return Ok(result),
        };
        let mut result = self
            .execute_shell_persistent(command, PERSISTENT_SHELL_TOOL, timeout, session_id)
            .await?;
        prepend_stored_note(&mut result, &name, tool_name);
        Ok(result)
    }

    /// Ask the local user for the `storeAs` value and keep it as a session secret.
    ///
    /// Every shell this session starts afterwards (one-off or persistent) gets it as
    /// `$<storeAs>`. Returns the stored name, or the tool result to return when
    /// nothing was stored (invalid name, no UI, cancel, timeout, bad answer).
    pub(crate) async fn prompt_and_store_secret(
        &self,
        command: &str,
        args: &Value,
        session_id: &str,
    ) -> Result<String, MCPResult> {
        use crate::mcp::builtin::workspace::utils::sanitize_command_for_logging;

        let display_command = sanitize_command_for_logging(command);

        let Some(name) = args.get("storeAs").and_then(|value| value.as_str()) else {
            return Err(missing_store_as_error());
        };
        if let Err(message) = validate_secret_name(name) {
            return Err(
                guided_error(ErrorCategory::InvalidInput, message, ToolGroup::Workspace)
                    .guidance(vec![
                        "Pick an uppercase name such as API_TOKEN that the app and the shell do not use"
                            .to_string(),
                    ])
                    .to_mcp_result(),
            );
        }

        let Some(app_handle) = crate::state::get_app_handle() else {
            return Err(guided_error(
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

        let prompt =
            prompt_override(args).unwrap_or_else(|| format!("Enter the value for {name}:"));
        let manager = app_handle.state::<AgentSessionManager>();
        let outcome = match manager
            .request_user_prompt(session_id, stored_secret_payload(name, prompt))
            .await
        {
            Ok(outcome) => outcome,
            Err(error) => {
                return Err(
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
            return Err(unanswered_shell_result(&outcome, name, &display_command));
        };

        if user_input.len() > INTERACTIVE_SHELL_INPUT_MAX_BYTES {
            return Err(guided_error(
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
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                "Interactive input must be a single line".to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec!["Run the command again and submit one line".to_string()])
            .to_mcp_result());
        };

        if let Err(error) = self
            .shell_manager
            .store_secret(session_id, name, user_input.to_string())
            .await
        {
            tracing::warn!(
                "Shell restart after storing {} failed for session {}: {}",
                name,
                session_id,
                error
            );
        }
        Ok(name.to_string())
    }
}

/// How a command run by `tool_name` reads the env var `name`.
pub(crate) fn env_var_ref(tool_name: &str, name: &str) -> String {
    if tool_name.contains("PowerShell") {
        format!("$env:{name}")
    } else {
        format!("${name}")
    }
}

/// Tells the agent which variable now holds the value (never the value itself).
pub(crate) fn prepend_stored_note(result: &mut MCPResult, name: &str, tool_name: &str) {
    prepend_result_note(
        result,
        &format!(
            "Stored the value as {} for this session.",
            env_var_ref(tool_name, name)
        ),
    );
}

/// `storeAs` without `requireUserInput=true` would ask nothing and store nothing.
pub(crate) fn store_as_without_prompt_error(args: &Value) -> Option<MCPResult> {
    args.get("storeAs").filter(|value| !value.is_null())?;
    Some(
        guided_error(
            ErrorCategory::InvalidInput,
            "storeAs needs requireUserInput=true. Nothing was asked or stored, and the command did not run".to_string(),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "Set requireUserInput=true with storeAs=NAME to ask the user for the value".to_string(),
            "A value stored earlier in this session is already available; drop storeAs to reuse it"
                .to_string(),
        ])
        .to_mcp_result(),
    )
}

/// `requireUserInput` without `storeAs`: the shell cannot receive typed input.
fn missing_store_as_error() -> MCPResult {
    guided_error(
        ErrorCategory::InvalidInput,
        "requireUserInput needs storeAs. The shell cannot send input to a command".to_string(),
        ToolGroup::Workspace,
    )
    .guidance(vec![
        "Set storeAs=NAME so the user's value is stored as $NAME, then read $NAME in the command"
            .to_string(),
        "For sudo, use inputType=password and storeAs=SUDO_PASSWORD, then run plain `sudo`"
            .to_string(),
    ])
    .to_mcp_result()
}

/// The tool result when the user gave no answer. A timeout is an error
/// (#2543 requirement 5); cancel and Stop stay informational, as before.
fn unanswered_shell_result(
    outcome: &UserPromptOutcome,
    variable_name: &str,
    display_command: &str,
) -> MCPResult {
    let message = secret_outcome_message(outcome, variable_name);
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

fn prompt_override(args: &Value) -> Option<String> {
    args.get("inputPrompt")
        .or_else(|| args.get("input_prompt"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

/// A `storeAs` prompt: a masked secret that the session keeps.
fn stored_secret_payload(name: &str, label: String) -> UserPromptPayload {
    UserPromptPayload::Secret {
        variable_name: name.to_string(),
        label,
        reuse_note: Some("This value is reused for this session.".to_string()),
    }
}

/// A stored value is one line, so a multi-line answer is refused.
fn single_line_answer(answer: &str) -> Option<&str> {
    let line = answer.trim_end_matches(['\r', '\n']);
    (!line.contains(['\r', '\n'])).then_some(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timed_out_prompt_is_an_error_result() {
        let result = unanswered_shell_result(
            &UserPromptOutcome::TimedOut,
            "SUDO_PASSWORD",
            "sudo apt update",
        );
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn a_cancelled_or_stopped_prompt_is_not_an_error_result() {
        for outcome in [UserPromptOutcome::Cancelled, UserPromptOutcome::Stopped] {
            let result = unanswered_shell_result(&outcome, "SUDO_PASSWORD", "sudo apt update");
            assert_ne!(result.is_error, Some(true));
        }
    }

    #[test]
    fn require_user_input_without_store_as_is_an_error() {
        assert_eq!(missing_store_as_error().is_error, Some(true));
    }

    #[test]
    fn store_as_becomes_a_secret_prompt_with_the_reuse_note() {
        assert_eq!(
            stored_secret_payload("API_TOKEN", "Enter the value for API_TOKEN:".to_string()),
            UserPromptPayload::Secret {
                variable_name: "API_TOKEN".to_string(),
                label: "Enter the value for API_TOKEN:".to_string(),
                reuse_note: Some("This value is reused for this session.".to_string()),
            }
        );
    }

    #[test]
    fn powershell_tools_name_the_env_drive_form() {
        for tool in ["runPowerShell", "runInPersistentPowerShell"] {
            assert_eq!(env_var_ref(tool, "API_TOKEN"), "$env:API_TOKEN");
        }
        for tool in ["runShell", "runInPersistentShell"] {
            assert_eq!(env_var_ref(tool, "API_TOKEN"), "$API_TOKEN");
        }
    }

    #[test]
    fn store_as_alone_is_rejected_but_null_is_ignored() {
        let rejected = store_as_without_prompt_error(&serde_json::json!({ "storeAs": "X" }));
        assert_eq!(rejected.and_then(|result| result.is_error), Some(true));
        assert!(store_as_without_prompt_error(&serde_json::json!({ "storeAs": null })).is_none());
        assert!(store_as_without_prompt_error(&serde_json::json!({})).is_none());
    }

    #[test]
    fn answers_must_be_one_line() {
        assert_eq!(single_line_answer("secret"), Some("secret"));
        assert_eq!(single_line_answer("secret\r\n"), Some("secret"));
        assert_eq!(single_line_answer("a\nb"), None);
    }
}
