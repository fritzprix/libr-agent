use serde_json::{json, Value};

use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, SuccessHint, ToolGroup};
use crate::mcp::builtin::session_api::formatting::{
    latest_assistant_message_text, latest_tool_message_text,
};
use crate::mcp::builtin::session_api::utils::{
    build_agent_session_tool_data, build_agent_tool_data,
};
use crate::mcp::types::{MCPContent, MCPResult};
use crate::repositories::message_repository::MessageRepository;

fn read_optional_string(args: &Value, key: &str) -> Result<Option<String>, String> {
    match args.get(key) {
        Some(value) if value.is_null() => Ok(None),
        Some(value) => value
            .as_str()
            .map(|text| Some(text.to_string()))
            .ok_or_else(|| format!("Argument '{}' must be a string", key)),
        None => Ok(None),
    }
}

fn extract_result_text(result: &MCPResult) -> Option<String> {
    result
        .content
        .as_ref()?
        .iter()
        .find_map(|content| match content {
            MCPContent::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
}

fn normalize_agent_config_result(
    mut result: MCPResult,
    tool_name: &str,
    next_actions: Vec<Value>,
) -> MCPResult {
    if result.is_error == Some(true) {
        return result;
    }

    let message =
        extract_result_text(&result).unwrap_or_else(|| format!("{} completed.", tool_name));
    let existing = result.structured_content.take();
    let resource_id = existing
        .as_ref()
        .and_then(|value| value.as_object())
        .and_then(|object| object.get("id"))
        .and_then(|value| value.as_str());

    let mut data = build_agent_tool_data(
        tool_name,
        "agentConfig",
        resource_id,
        &message,
        "success",
        next_actions,
    );

    match existing {
        Some(Value::Object(object)) => {
            for (key, value) in object {
                data.insert(key, value);
            }
        }
        Some(value) => {
            data.insert("data".to_string(), value);
        }
        None => {}
    }

    result.structured_content = Some(Value::Object(data));
    result
}

fn default_session_recovery_message(status: &str) -> String {
    match status.to_ascii_lowercase().as_str() {
        "paused" => "Resume after interruption. Continue the delegated task from the last completed step, reassess any interrupted tool work, and then proceed to the final answer.".to_string(),
        "error" | "failed" => "Retry after failure. Inspect the previous error, preserve any completed work, fix the immediate cause if needed, and continue the delegated task from the safest next step.".to_string(),
        "terminated" => "Restart the delegated task from the latest conversation state and continue from the most sensible next step.".to_string(),
        _ => "Continue the delegated task from the latest stable point and report the final answer.".to_string(),
    }
}

fn caller_session_not_found_result(session_id: &str) -> MCPResult {
    guided_error(
        ErrorCategory::ResourceNotFound,
        format!("Caller session '{}' not found", session_id),
        ToolGroup::Agent,
    )
    .with_guidance(vec![
        "Resume the parent/root session and retry the operation".to_string(),
        "Use list(type=\"sessions\") to inspect delegated sessions if needed".to_string(),
        "The caller session may have been terminated or expired".to_string(),
    ])
    .to_mcp_result()
}

fn missing_explicit_org_result() -> MCPResult {
    guided_error(
        ErrorCategory::InvalidInput,
        "No explicit org is associated with the current session. Call createOrg first.".to_string(),
        ToolGroup::Agent,
    )
    .with_guidance(vec![
        "Use createOrg(name=\"...\") from the root session first".to_string(),
        "Or pass orgId explicitly when querying a known explicit org".to_string(),
    ])
    .to_mcp_result()
}

fn invalid_explicit_org_result(org_id: &str) -> MCPResult {
    guided_error(
        ErrorCategory::InvalidState,
        format!("Explicit org '{}' is missing a root session", org_id),
        ToolGroup::Agent,
    )
    .with_guidance(vec![
        "Use createOrg(name=\"...\") again from the root session if the org lineage was reset"
            .to_string(),
        "Use list(type=\"sessions\") to inspect the current delegated lineage".to_string(),
    ])
    .to_mcp_result()
}

fn recovery_action_for_session(session_id: &str, status: &str, reason: &str) -> Value {
    json!({
        "toolName": "messageToSession",
        "reason": reason,
        "args": {
            "sessionId": session_id,
            "message": default_session_recovery_message(status),
        }
    })
}

fn latest_session_output(messages_value: &[Value]) -> String {
    let (_, mut assistant_text) = latest_assistant_message_text(messages_value, None)
        .unwrap_or(("none".to_string(), "No final answer yet.".to_string()));

    if assistant_text == "[assistant message has no text content]" {
        if let Some(tool_text) = latest_tool_message_text(messages_value) {
            assistant_text = format!("[Tool Response Fallback]\n{}", tool_text);
        }
    }

    assistant_text
}

pub fn build_paused_check_session_result_from_messages(
    session_id: &str,
    turn_count: usize,
    messages_value: &[Value],
) -> MCPResult {
    let latest_output = latest_session_output(messages_value);
    let recovery_reason =
        "Wake the paused child session so it can continue from the last stable step.";
    let message = format!(
        "Session {} is paused and will not make progress on its own.\n\nLast known output:\n{}\n\nRecovery: send a follow-up message with messageToSession(...) to restart the child workflow.",
        session_id, latest_output
    );
    let next_actions = vec![
        recovery_action_for_session(session_id, "paused", recovery_reason),
        json!({
            "toolName": "checkSession",
            "reason": "Check again after sending a recovery message.",
            "args": {
                "sessionId": session_id,
                "wait": true
            }
        }),
    ];
    let hint = SuccessHint::new(message.clone(), vec![]);
    let mut response_data = build_agent_session_tool_data(
        "checkSession",
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
        Value::String("messageToSession".to_string()),
    );
    response_data.insert(
        "recoveryMessage".to_string(),
        Value::String(default_session_recovery_message("paused")),
    );
    response_data.insert("abnormalTermination".to_string(), Value::Bool(false));
    response_data.insert("result".to_string(), Value::String(latest_output));

    hint.to_mcp_result_with_data(Some(Value::Object(response_data)))
}

pub fn build_terminal_check_session_result_from_messages(
    session_id: &str,
    status: &str,
    turn_count: usize,
    messages_value: &[Value],
) -> MCPResult {
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
                "toolName": "checkSession",
                "reason": "Check again after sending a recovery message.",
                "args": {
                    "sessionId": session_id,
                    "wait": true
                }
            }),
        ]
    } else {
        vec![]
    };
    let message = if is_abnormal {
        format!(
            "Session {} ended abnormally ({}).\n\nLast known output:\n{}\n\nRecovery: this child session will not continue on its own. Use messageToSession(...) to retry from the last stable step.",
            session_id, status, assistant_text
        )
    } else if normalized_status == "terminated" {
        format!(
            "Session {} was terminated.\n\nLast known output:\n{}\n\nIf you still need the work, restart it explicitly with messageToSession(...).",
            session_id, assistant_text
        )
    } else {
        format!(
            "Session {} is terminal ({}).\n\nResult:\n{}",
            session_id, status, assistant_text
        )
    };
    let hint = SuccessHint::new(message.clone(), vec![]);
    let mut response_data = build_agent_session_tool_data(
        "checkSession",
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
            Value::String("messageToSession".to_string()),
        );
        response_data.insert(
            "recoveryMessage".to_string(),
            Value::String(default_session_recovery_message(status)),
        );
    }

    hint.to_mcp_result_with_data(Some(Value::Object(response_data)))
}

async fn build_terminal_check_session_result(
    session_id: &str,
    status: &str,
    turn_count: usize,
) -> Result<MCPResult, String> {
    let repo = crate::state::get_message_repository();
    let messages = repo
        .get_messages_by_session(session_id, 5)
        .await
        .map_err(|e| format!("Failed to fetch session messages: {}", e))?;

    let messages_value: Vec<Value> = messages
        .into_iter()
        .map(|m| serde_json::to_value(m).unwrap_or_default())
        .collect();

    Ok(build_terminal_check_session_result_from_messages(
        session_id,
        status,
        turn_count,
        &messages_value,
    ))
}

async fn build_paused_check_session_result(
    session_id: &str,
    turn_count: usize,
) -> Result<MCPResult, String> {
    let repo = crate::state::get_message_repository();
    let messages = repo
        .get_messages_by_session(session_id, 5)
        .await
        .map_err(|e| format!("Failed to fetch session messages: {}", e))?;

    let messages_value: Vec<Value> = messages
        .into_iter()
        .map(|m| serde_json::to_value(m).unwrap_or_default())
        .collect();

    Ok(build_paused_check_session_result_from_messages(
        session_id,
        turn_count,
        &messages_value,
    ))
}
mod check_session;
mod configs;
mod orgs;
mod sessions;

pub use check_session::check_session;
pub use configs::{create_agent, list_agents_or_sessions, update_agent};
pub use orgs::{create_org, get_org};
pub use sessions::{
    compact_session_context, message_to_session, spawn_org_agent, start_session, stop_session,
};
