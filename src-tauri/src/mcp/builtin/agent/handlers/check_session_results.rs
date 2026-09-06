use serde_json::{json, Value};

use super::super::utils::{
    build_agent_session_tool_data, fetch_session_messages_for_result, latest_session_output,
    CHECK_SESSION_RESULT_MESSAGE_LIMIT,
};
use super::enrichment::{
    append_check_session_context_to_message, apply_check_session_enrichment, CheckSessionEnrichment,
};
use crate::mcp::builtin::error_guidance::SuccessHint;
use crate::mcp::types::MCPResult;

fn default_session_recovery_message(status: &str) -> String {
    match status.to_ascii_lowercase().as_str() {
        "paused" => "Resume after interruption. Continue the delegated task from the last completed step, reassess any interrupted tool work, and then proceed to the final answer.".to_string(),
        "error" | "failed" => "Retry after failure. Inspect the previous error, preserve any completed work, fix the immediate cause if needed, and continue the delegated task from the safest next step.".to_string(),
        "terminated" => "Restart the delegated task from the latest conversation state and continue from the most sensible next step.".to_string(),
        _ => "Continue the delegated task from the latest stable point and report the final answer.".to_string(),
    }
}

fn recovery_action_for_session(session_id: &str, status: &str, reason: &str) -> Value {
    let display_id = crate::utils::session_id::display_session_id(session_id);
    json!({
        "toolName": "agent__messageToSession",
        "reason": reason,
        "args": {
            "sessionId": display_id,
            "message": default_session_recovery_message(status),
        }
    })
}

pub fn build_paused_check_session_result_from_messages(
    session_id: &str,
    turn_count: usize,
    messages_value: &[Value],
    enrichment: Option<&CheckSessionEnrichment>,
) -> MCPResult {
    let display_id = crate::utils::session_id::display_session_id(session_id);
    let latest_output = latest_session_output(messages_value);
    let recovery_reason =
        "Wake the paused child session so it can continue from the last stable step.";
    let mut message = format!(
        "Session {} is paused and will not make progress on its own.\n\nLast known output:\n{}\n\nRecovery: send a follow-up message with agent__messageToSession(...) to restart the child workflow.",
        display_id, latest_output
    );
    if let Some(enrichment) = enrichment {
        message = append_check_session_context_to_message(&message, enrichment);
    }
    let next_actions = vec![
        recovery_action_for_session(session_id, "paused", recovery_reason),
        json!({
            "toolName": "agent__checkSession",
            "reason": "Check again after sending a recovery message.",
            "args": {
                "sessionId": display_id,
                "wait": true
            }
        }),
    ];
    let hint = SuccessHint::new(message.clone(), vec![]);
    let mut response_data = build_agent_session_tool_data(
        "agent__checkSession",
        session_id,
        &message,
        "paused",
        "paused",
        turn_count,
        next_actions,
    );
    response_data.insert("recoverable".to_string(), Value::Bool(true));
    response_data.insert(
        "recoveryStrategy".to_string(),
        Value::String("agent__messageToSession".to_string()),
    );
    response_data.insert(
        "recoveryMessage".to_string(),
        Value::String(default_session_recovery_message("paused")),
    );
    response_data.insert("abnormalTermination".to_string(), Value::Bool(false));
    response_data.insert("result".to_string(), Value::String(latest_output));
    if let Some(enrichment) = enrichment {
        apply_check_session_enrichment(&mut response_data, enrichment);
    }

    hint.to_mcp_result_with_data(Some(Value::Object(response_data)))
}

/// Build a terminal checkSession MCP result from pre-fetched messages.
///
/// `session_id` must be the opaque storage key (`SessionMetadata.id`). It is
/// only used to derive the agent-facing display token — messages must already
/// have been loaded via `fetch_session_messages_for_result` with a
/// `StorageSessionId`.
pub fn build_terminal_check_session_result_from_messages(
    session_id: &str,
    status: &str,
    turn_count: usize,
    messages_value: &[Value],
    enrichment: Option<&CheckSessionEnrichment>,
) -> MCPResult {
    let display_id = crate::utils::session_id::display_session_id(session_id);
    let assistant_text = latest_session_output(messages_value);
    let normalized_status = status.to_ascii_lowercase();
    let is_abnormal = matches!(normalized_status.as_str(), "error" | "failed");
    let is_recoverable = matches!(
        normalized_status.as_str(),
        "error" | "failed" | "terminated"
    );
    let next_actions = if is_recoverable {
        vec![
            recovery_action_for_session(
                session_id,
                status,
                "Retry the child session explicitly after abnormal termination.",
            ),
            json!({
                "toolName": "agent__checkSession",
                "reason": "Check again after sending a recovery message.",
                "args": {
                    "sessionId": display_id,
                    "wait": true
                }
            }),
        ]
    } else {
        vec![json!({
            "toolName": "agent__messageToSession",
            "reason": "Request the child session for more detail, file contents, or full output.",
            "args": {
                "sessionId": display_id,
                "message": "Please share the complete output or file contents."
            }
        })]
    };
    let mut message = if is_abnormal {
        format!(
            "Session {} ended abnormally ({}).\n\nLast known output:\n{}\n\nRecovery: this child session will not continue on its own. Use agent__messageToSession(...) to retry from the last stable step.",
            display_id, status, assistant_text
        )
    } else if normalized_status == "terminated" {
        format!(
            "Session {} was terminated.\n\nLast known output:\n{}\n\nIf you still need the work, restart it explicitly with agent__messageToSession(...).",
            display_id, assistant_text
        )
    } else {
        format!(
            "Session {} is terminal ({}).\n\nResult:\n{}\n\nIf you need more detail, use agent__messageToSession(\"{}\", message=\"Please share the complete output or file contents.\") to ask the child session for the full result.",
            display_id, status, assistant_text, display_id
        )
    };
    if let Some(enrichment) = enrichment {
        message = append_check_session_context_to_message(&message, enrichment);
    }
    let hint = SuccessHint::new(message.clone(), vec![]);
    let mut response_data = build_agent_session_tool_data(
        "agent__checkSession",
        session_id,
        &message,
        status,
        if is_abnormal {
            "error"
        } else if normalized_status == "terminated" {
            "terminated"
        } else {
            "success"
        },
        turn_count,
        next_actions,
    );
    response_data.insert("result".to_string(), Value::String(assistant_text));
    response_data.insert("abnormalTermination".to_string(), Value::Bool(is_abnormal));
    response_data.insert("recoverable".to_string(), Value::Bool(is_recoverable));
    if is_recoverable {
        response_data.insert(
            "recoveryStrategy".to_string(),
            Value::String("agent__messageToSession".to_string()),
        );
        response_data.insert(
            "recoveryMessage".to_string(),
            Value::String(default_session_recovery_message(status)),
        );
    }
    // Always include hasMoreDetail hint so UI can show a "view more" action
    response_data.insert("hasMoreDetail".to_string(), Value::Bool(true));
    if let Some(enrichment) = enrichment {
        apply_check_session_enrichment(&mut response_data, enrichment);
    }

    hint.to_mcp_result_with_data(Some(Value::Object(response_data)))
}

pub(super) async fn build_terminal_check_session_result(
    storage_session_id: &crate::utils::session_id::StorageSessionId,
    status: &str,
    turn_count: usize,
    enrichment: &CheckSessionEnrichment,
) -> Result<MCPResult, String> {
    // Message fetch requires the opaque storage key, never display_session_id().
    let messages_value =
        fetch_session_messages_for_result(storage_session_id, CHECK_SESSION_RESULT_MESSAGE_LIMIT)
            .await?;

    Ok(build_terminal_check_session_result_from_messages(
        storage_session_id.as_str(),
        status,
        turn_count,
        &messages_value,
        Some(enrichment),
    ))
}

pub(super) async fn build_paused_check_session_result(
    storage_session_id: &crate::utils::session_id::StorageSessionId,
    turn_count: usize,
    enrichment: &CheckSessionEnrichment,
) -> Result<MCPResult, String> {
    // Message fetch requires the opaque storage key, never display_session_id().
    let messages_value =
        fetch_session_messages_for_result(storage_session_id, CHECK_SESSION_RESULT_MESSAGE_LIMIT)
            .await?;

    Ok(build_paused_check_session_result_from_messages(
        storage_session_id.as_str(),
        turn_count,
        &messages_value,
        Some(enrichment),
    ))
}
