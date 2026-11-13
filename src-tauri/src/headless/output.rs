//! Stdout formatting (`text`, `json`, `stream-json`) for `liblet` headless runs (#2703).
//!
//! Matches `docs/specs/liblet-cli-interface.md` §3.4.

use crate::agent::session_manager::PendingApprovalSummary;
use crate::agent::state::{
    PendingApprovalKind, PendingUserPromptSummary, UserPromptKind, UserPromptPayload,
};
use crate::execution_mode::ExecutionMode;
use crate::headless::cli::OutputFormat;
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;
use serde::{Deserialize, Serialize};
use std::io::Write;

/// Terminal status of a headless run (`docs/specs/liblet-cli-interface.md` §3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HeadlessStatus {
    Completed,
    Error,
    Blocked,
    Timeout,
    Interrupted,
}

/// Reason a headless run stopped in `status: "blocked"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HeadlessBlockReason {
    Approval,
    Question,
}

/// Pending approval entry in the `json` / `stream-json` result payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadlessPendingApproval {
    pub tool_call_id: String,
    pub tool_name: String,
    pub reason: String,
    pub command_preview: String,
    pub approval_kind: PendingApprovalKind,
}

impl From<PendingApprovalSummary> for HeadlessPendingApproval {
    fn from(summary: PendingApprovalSummary) -> Self {
        Self {
            tool_call_id: summary.tool_call_id,
            tool_name: summary.tool_name,
            reason: summary.reason,
            command_preview: summary.command_preview,
            approval_kind: summary.approval_kind,
        }
    }
}

/// Final result object emitted for `--output-format json` and as the closing
/// event of `--output-format stream-json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadlessResultPayload {
    #[serde(rename = "type")]
    pub event_type: String,
    pub status: HeadlessStatus,
    pub reason: Option<HeadlessBlockReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub workspace_path: String,
    pub execution_mode: ExecutionMode,
    pub pending_approvals: Vec<HeadlessPendingApproval>,
    pub exit_code: i32,
}

impl HeadlessResultPayload {
    pub fn completed(
        result: String,
        workspace_path: String,
        execution_mode: ExecutionMode,
    ) -> Self {
        Self {
            event_type: "result".to_string(),
            status: HeadlessStatus::Completed,
            reason: None,
            result: Some(result),
            error: None,
            workspace_path,
            execution_mode,
            pending_approvals: Vec::new(),
            exit_code: crate::headless::cli::exit_codes::SUCCESS,
        }
    }

    pub fn error(
        error_message: String,
        workspace_path: String,
        execution_mode: ExecutionMode,
    ) -> Self {
        Self {
            event_type: "result".to_string(),
            status: HeadlessStatus::Error,
            reason: None,
            result: None,
            error: Some(error_message),
            workspace_path,
            execution_mode,
            pending_approvals: Vec::new(),
            exit_code: crate::headless::cli::exit_codes::ERROR,
        }
    }

    pub fn blocked_on_approval(
        pending_approvals: Vec<HeadlessPendingApproval>,
        workspace_path: String,
        execution_mode: ExecutionMode,
    ) -> Self {
        Self {
            event_type: "result".to_string(),
            status: HeadlessStatus::Blocked,
            reason: Some(HeadlessBlockReason::Approval),
            result: None,
            error: None,
            workspace_path,
            execution_mode,
            pending_approvals,
            exit_code: crate::headless::cli::exit_codes::APPROVAL_REQUIRED,
        }
    }

    pub fn blocked_on_question(
        result_hint: Option<String>,
        workspace_path: String,
        execution_mode: ExecutionMode,
    ) -> Self {
        Self {
            event_type: "result".to_string(),
            status: HeadlessStatus::Blocked,
            reason: Some(HeadlessBlockReason::Question),
            result: result_hint,
            error: None,
            workspace_path,
            execution_mode,
            pending_approvals: Vec::new(),
            exit_code: crate::headless::cli::exit_codes::QUESTION_REQUIRED,
        }
    }

    pub fn timeout(workspace_path: String, execution_mode: ExecutionMode) -> Self {
        Self {
            event_type: "result".to_string(),
            status: HeadlessStatus::Timeout,
            reason: None,
            result: None,
            error: None,
            workspace_path,
            execution_mode,
            pending_approvals: Vec::new(),
            exit_code: crate::headless::cli::exit_codes::TIMEOUT,
        }
    }

    pub fn interrupted(
        exit_code: i32,
        workspace_path: String,
        execution_mode: ExecutionMode,
    ) -> Self {
        Self {
            event_type: "result".to_string(),
            status: HeadlessStatus::Interrupted,
            reason: None,
            result: None,
            error: None,
            workspace_path,
            execution_mode,
            pending_approvals: Vec::new(),
            exit_code,
        }
    }
}

/// NDJSON trace events for `--output-format stream-json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HeadlessStreamEvent {
    #[serde(rename_all = "camelCase")]
    Init {
        workspace_path: String,
        execution_mode: ExecutionMode,
        model: String,
        provider: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        assistant_id: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Message {
        id: String,
        role: String,
        content: String,
    },
    #[serde(rename_all = "camelCase")]
    ToolUse {
        tool_call_id: String,
        tool_name: String,
        arguments: serde_json::Value,
    },
    #[serde(rename_all = "camelCase")]
    ToolResult {
        tool_call_id: String,
        content: String,
        is_error: bool,
    },
    #[serde(rename_all = "camelCase")]
    ApprovalRequest {
        tool_call_id: String,
        tool_name: String,
        reason: String,
        command_preview: String,
        approval_kind: PendingApprovalKind,
    },
    #[serde(rename_all = "camelCase")]
    Question {
        request_id: String,
        kind: UserPromptKind,
        #[serde(skip_serializing_if = "Option::is_none")]
        prompt: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Error { message: String },
}

impl HeadlessStreamEvent {
    pub fn from_pending_approval(approval: &HeadlessPendingApproval) -> Self {
        Self::ApprovalRequest {
            tool_call_id: approval.tool_call_id.clone(),
            tool_name: approval.tool_name.clone(),
            reason: approval.reason.clone(),
            command_preview: approval.command_preview.clone(),
            approval_kind: approval.approval_kind,
        }
    }

    pub fn from_pending_user_prompt(summary: &PendingUserPromptSummary) -> Self {
        let prompt = summarize_user_prompt_payload(&summary.payload);
        Self::Question {
            request_id: summary.request_id.clone(),
            kind: summary.kind,
            prompt,
        }
    }
}

/// Extract a human-readable prompt description from a `UserPromptPayload` without leaking secrets.
pub fn summarize_user_prompt_payload(payload: &UserPromptPayload) -> Option<String> {
    match payload {
        UserPromptPayload::Secret { label, .. } => Some(label.clone()),
        UserPromptPayload::Question { questions } => {
            let joined = questions
                .iter()
                .map(|q| q.question.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            if joined.trim().is_empty() {
                None
            } else {
                Some(joined)
            }
        }
    }
}

/// Extract concatenated plain text from a message's `MCPContent` blocks.
pub fn extract_message_text(message: &Message) -> String {
    message
        .content
        .iter()
        .filter_map(|item| match item {
            MCPContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Convert newly observed messages in a session into `stream-json` events.
pub fn stream_events_for_message(message: &Message) -> Vec<HeadlessStreamEvent> {
    let mut events = Vec::new();
    let text = extract_message_text(message);

    if message.role == "tool" {
        if let Some(tool_call_id) = message.tool_call_id.as_ref() {
            events.push(HeadlessStreamEvent::ToolResult {
                tool_call_id: tool_call_id.clone(),
                content: text,
                is_error: message.error.is_some(),
            });
        }
        return events;
    }

    if !text.trim().is_empty() && (message.role == "user" || message.role == "assistant") {
        events.push(HeadlessStreamEvent::Message {
            id: message.id.clone(),
            role: message.role.clone(),
            content: text,
        });
    }

    if let Some(tool_calls) = message.tool_calls.as_ref() {
        for call in tool_calls {
            let parsed_args = serde_json::from_str::<serde_json::Value>(&call.function.arguments)
                .unwrap_or_else(|_| serde_json::Value::String(call.function.arguments.clone()));
            events.push(HeadlessStreamEvent::ToolUse {
                tool_call_id: call.id.clone(),
                tool_name: call.function.name.clone(),
                arguments: parsed_args,
            });
        }
    }

    events
}

/// Write a single `HeadlessStreamEvent` line in `stream-json` format.
pub fn write_stream_event<W: Write>(
    writer: &mut W,
    event: &HeadlessStreamEvent,
) -> std::io::Result<()> {
    let line = serde_json::to_string(event)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    writeln!(writer, "{line}")?;
    writer.flush()
}

/// Write the final headless result to `stdout` (and/or `stderr` on error in `text` mode).
pub fn write_final_output<Out: Write, ErrOut: Write>(
    format: OutputFormat,
    payload: &HeadlessResultPayload,
    stdout: &mut Out,
    stderr: &mut ErrOut,
) -> std::io::Result<()> {
    match format {
        OutputFormat::Text => {
            if let Some(text) = payload.result.as_deref() {
                if !text.is_empty() {
                    if text.ends_with('\n') {
                        write!(stdout, "{text}")?;
                    } else {
                        writeln!(stdout, "{text}")?;
                    }
                    stdout.flush()?;
                }
            }
            match payload.status {
                HeadlessStatus::Completed => {}
                HeadlessStatus::Error => {
                    if let Some(err) = payload.error.as_deref() {
                        writeln!(stderr, "error: {err}")?;
                        stderr.flush()?;
                    }
                }
                HeadlessStatus::Blocked => match payload.reason {
                    Some(HeadlessBlockReason::Approval) => {
                        for approval in &payload.pending_approvals {
                            writeln!(
                                stderr,
                                "approval required ({:?}): {} — {} ({})",
                                approval.approval_kind,
                                approval.tool_name,
                                approval.reason,
                                approval.command_preview
                            )?;
                        }
                        stderr.flush()?;
                    }
                    Some(HeadlessBlockReason::Question) => {
                        writeln!(
                            stderr,
                            "blocked: agent requested interactive user input (question or secret)"
                        )?;
                        stderr.flush()?;
                    }
                    None => {}
                },
                HeadlessStatus::Timeout => {
                    writeln!(stderr, "error: headless execution timed out")?;
                    stderr.flush()?;
                }
                HeadlessStatus::Interrupted => {
                    writeln!(stderr, "error: headless execution interrupted")?;
                    stderr.flush()?;
                }
            }
            Ok(())
        }
        OutputFormat::Json | OutputFormat::StreamJson => {
            let line = serde_json::to_string(payload)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            writeln!(stdout, "{line}")?;
            stdout.flush()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_result_matches_spec_schema() {
        let payload = HeadlessResultPayload::blocked_on_approval(
            vec![HeadlessPendingApproval {
                tool_call_id: "call_1".to_string(),
                tool_name: "workspace__runShell".to_string(),
                reason: "Shell command requires approval".to_string(),
                command_preview: "rm -rf build".to_string(),
                approval_kind: PendingApprovalKind::Hard,
            }],
            "/workspace/project".to_string(),
            ExecutionMode::Normal,
        );

        let value = serde_json::to_value(&payload).expect("serialize result payload");
        assert_eq!(value["type"], "result");
        assert_eq!(value["status"], "blocked");
        assert_eq!(value["reason"], "approval");
        assert_eq!(value["workspacePath"], "/workspace/project");
        assert_eq!(value["executionMode"], "normal");
        assert_eq!(value["exitCode"], 3);
        assert_eq!(value["pendingApprovals"][0]["toolCallId"], "call_1");
        assert_eq!(
            value["pendingApprovals"][0]["toolName"],
            "workspace__runShell"
        );
        assert_eq!(value["pendingApprovals"][0]["approvalKind"], "hard");
    }

    #[test]
    fn text_mode_writes_only_final_result_to_stdout() {
        let payload = HeadlessResultPayload::completed(
            "42".to_string(),
            "/tmp/ws".to_string(),
            ExecutionMode::Normal,
        );
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        write_final_output(OutputFormat::Text, &payload, &mut stdout, &mut stderr)
            .expect("write text output");
        assert_eq!(String::from_utf8(stdout).unwrap(), "42\n");
        assert!(stderr.is_empty());
    }
}
