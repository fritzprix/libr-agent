use reqwest::Method;
use serde_json::{json, Value};

use crate::mcp::types::MCPResult;
use super::client::SessionApiClient;
use super::utils::{read_required_string, resolve_parent_session_id, success_result};
use super::types::MessageSummaryOptions;
use super::cache::{min_interval_notice, unchanged_messages_notice};
use super::formatting::{build_messages_summary, extract_session_status, latest_assistant_message_text};

pub async fn health_check(
    client: &SessionApiClient,
    _args: Value,
    _caller_session_id: Option<String>,
) -> Result<MCPResult, String> {
    let data = client
        .call_json(Method::GET, "/api/health", None, None)
        .await?;
    Ok(success_result(
        "Session API health check succeeded.".to_string(),
        data,
    ))
}

pub async fn create_session(
    client: &SessionApiClient,
    args: Value,
    caller_session_id: Option<String>,
) -> Result<MCPResult, String> {
    let assistant_id = read_required_string(&args, "assistantId")?;
    let request = read_required_string(&args, "request")?;

    let mut body = json!({
        "assistantId": assistant_id,
        "request": request,
    });

    if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
        body["name"] = Value::String(name.to_string());
    }

    if let Some(path) = args.get("workspacePath").and_then(|v| v.as_str()) {
        body["workspacePath"] = Value::String(path.to_string());
    }

    if let Some(max_depth) = args.get("maxDepth").and_then(|v| v.as_u64()) {
        body["maxDepth"] = Value::Number(max_depth.into());
    }

    if let Some(max_fanout) = args.get("maxFanout").and_then(|v| v.as_u64()) {
        body["maxFanout"] = Value::Number(max_fanout.into());
    }

    let explicit_parent = args.get("parentSessionId").and_then(|v| v.as_str());
    let effective_parent =
        resolve_parent_session_id(explicit_parent, caller_session_id.as_deref());

    if let Some(parent_session_id) = effective_parent {
        body["parentSessionId"] = Value::String(parent_session_id.to_string());
    }

    let data = client
        .call_json(Method::POST, "/api/sessions", Some(body), None)
        .await?;

    let session_id = data.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
    let status = data
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let parent = data
        .get("parentSessionId")
        .and_then(|v| v.as_str())
        .unwrap_or("none");
    let depth = data.get("depth").and_then(|v| v.as_u64()).unwrap_or(0);
    let lineage = data
        .get("lineageId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    Ok(success_result(
        format!(
            "Session created: {} (status: {}, parent: {}, depth: {}, lineage: {})",
            session_id, status, parent, depth, lineage
        ),
        data,
    ))
}

pub async fn create_child_session(
    client: &SessionApiClient,
    args: Value,
    caller_session_id: Option<String>,
) -> Result<MCPResult, String> {
    let assistant_id = read_required_string(&args, "assistantId")?;
    let request = read_required_string(&args, "request")?;

    let parent_session_id = resolve_parent_session_id(
        args.get("parentSessionId").and_then(|v| v.as_str()),
        caller_session_id.as_deref(),
    )
    .ok_or_else(|| {
        "Missing parent session context: provide parentSessionId or call from within a session"
            .to_string()
    })?;

    let mut body = json!({
        "parentSessionId": parent_session_id,
        "assistantId": assistant_id,
        "request": request,
    });

    if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
        body["name"] = Value::String(name.to_string());
    }

    if let Some(path) = args.get("workspacePath").and_then(|v| v.as_str()) {
        body["workspacePath"] = Value::String(path.to_string());
    }

    if let Some(max_depth) = args.get("maxDepth").and_then(|v| v.as_u64()) {
        body["maxDepth"] = Value::Number(max_depth.into());
    }

    if let Some(max_fanout) = args.get("maxFanout").and_then(|v| v.as_u64()) {
        body["maxFanout"] = Value::Number(max_fanout.into());
    }

    let data = client
        .call_json(Method::POST, "/api/sessions", Some(body), None)
        .await?;

    let child_id = data.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
    let depth = data.get("depth").and_then(|v| v.as_u64()).unwrap_or(0);
    let lineage = data
        .get("lineageId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    Ok(success_result(
        format!(
            "Child session created: {} (parent: {}, depth: {}, lineage: {})",
            child_id, parent_session_id, depth, lineage
        ),
        data,
    ))
}

pub async fn get_session(
    client: &SessionApiClient,
    args: Value,
    _caller_session_id: Option<String>,
) -> Result<MCPResult, String> {
    let session_id = read_required_string(&args, "sessionId")?;
    let data = client
        .call_json(
            Method::GET,
            &format!("/api/sessions/{}", session_id),
            None,
            None,
        )
        .await?;
    Ok(success_result(
        format!("Fetched session: {}", session_id),
        data,
    ))
}

pub async fn wait_for_session_idle(
    client: &SessionApiClient,
    args: Value,
    _caller_session_id: Option<String>,
) -> Result<MCPResult, String> {
    let session_id = read_required_string(&args, "sessionId")?;

    let timeout_seconds = args
        .get("timeoutSeconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(180);
    let poll_interval_seconds = args
        .get("pollIntervalSeconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(3);
    let include_last_assistant_message = args
        .get("includeLastAssistantMessage")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let result_message_limit = args
        .get("resultMessageLimit")
        .and_then(|v| v.as_u64())
        .map(|v| v.clamp(1, 200))
        .unwrap_or(20);
    let assistant_message_max_chars = args
        .get("assistantMessageMaxChars")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(200000) as usize)
        .filter(|v| *v > 0);

    let (session_data, poll_count) = client
        .wait_until_session_terminal(
            &session_id,
            timeout_seconds,
            poll_interval_seconds,
        )
        .await?;

    let final_status = extract_session_status(&session_data);

    if !include_last_assistant_message {
        return Ok(success_result(
            format!(
                "Session {} reached terminal status '{}' after {} polls.",
                session_id, final_status, poll_count
            ),
            json!({
                "session": session_data,
                "status": final_status,
                "pollCount": poll_count,
                "messages": Value::Null
            }),
        ));
    }

    let messages_data = client
        .call_json(
            Method::GET,
            &format!("/api/sessions/{}/messages", session_id),
            None,
            Some(vec![(
                "limit".to_string(),
                result_message_limit.to_string(),
            )]),
        )
        .await?;

    let messages = messages_data
        .get("messages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let text = if let Some((message_id, assistant_text)) =
        latest_assistant_message_text(&messages, assistant_message_max_chars)
    {
        format!(
            "Session {} reached terminal status '{}' after {} polls.\n\nLatest assistant result [{}]:\n{}",
            session_id, final_status, poll_count, message_id, assistant_text
        )
    } else {
        format!(
            "Session {} reached terminal status '{}' after {} polls.\n\nNo assistant text message was found in the latest {} messages.",
            session_id, final_status, poll_count, result_message_limit
        )
    };

    Ok(success_result(
        text,
        json!({
            "session": session_data,
            "status": final_status,
            "pollCount": poll_count,
            "messages": messages_data
        }),
    ))
}

pub async fn get_messages(
    client: &SessionApiClient,
    args: Value,
    caller_session_id: Option<String>,
) -> Result<MCPResult, String> {
    let target_session_id = read_required_string(&args, "sessionId")?;

    let requested_limit = args.get("limit").and_then(|v| v.as_u64());
    let options = MessageSummaryOptions::from_args(&args);

    if options.skip_if_unchanged {
        if let Some(wait_notice) = min_interval_notice(
            caller_session_id.as_deref(),
            &target_session_id,
            requested_limit,
            options,
        )
        .await
        {
            return Ok(success_result(
                wait_notice,
                json!({
                    "sessionId": target_session_id,
                    "skipped": true,
                    "reason": "min_interval",
                    "minIntervalSeconds": options.min_interval_seconds,
                    "forcedRestSeconds": options.forced_rest_seconds,
                    "rapidCallThreshold": options.rapid_call_threshold
                }),
            ));
        }
    }

    let query = requested_limit.map(|v| vec![("limit".to_string(), v.to_string())]);

    let data = client
        .call_json(
            Method::GET,
            &format!("/api/sessions/{}/messages", target_session_id),
            None,
            query,
        )
        .await?;

    let messages = data
        .get("messages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if options.skip_if_unchanged {
        if let Some(unchanged_notice) = unchanged_messages_notice(
            &messages,
            caller_session_id.as_deref(),
            &target_session_id,
            requested_limit,
        )
        .await
        {
            return Ok(success_result(unchanged_notice, data));
        }
    }

    let summary_text =
        build_messages_summary(&messages, &target_session_id, options);

    Ok(success_result(summary_text, data))
}

pub async fn get_child_sessions(
    client: &SessionApiClient,
    args: Value,
    _caller_session_id: Option<String>,
) -> Result<MCPResult, String> {
    let parent_session_id = read_required_string(&args, "parentSessionId")?;

    let data = client
        .call_json(
            Method::GET,
            &format!("/api/sessions/{}/children", parent_session_id),
            None,
            None,
        )
        .await?;

    let count = data.get("count").and_then(|v| v.as_u64()).unwrap_or(0);

    Ok(success_result(
        format!(
            "Fetched {} child sessions for parent {}",
            count, parent_session_id
        ),
        data,
    ))
}

pub async fn send_message(
    client: &SessionApiClient,
    args: Value,
    _caller_session_id: Option<String>,
) -> Result<MCPResult, String> {
    let session_id = read_required_string(&args, "sessionId")?;
    let content = read_required_string(&args, "content")?;

    let data = client
        .call_json(
            Method::POST,
            &format!("/api/sessions/{}/messages", session_id),
            Some(json!({ "content": content })),
            None,
        )
        .await?;

    let message_id = data.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
    let status = data
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    Ok(success_result(
        format!("Message accepted: {} (status: {})", message_id, status),
        data,
    ))
}

pub async fn terminate_session(
    client: &SessionApiClient,
    args: Value,
    _caller_session_id: Option<String>,
) -> Result<MCPResult, String> {
    let session_id = read_required_string(&args, "sessionId")?;
    let data = client
        .call_json(
            Method::POST,
            &format!("/api/sessions/{}/terminate", session_id),
            None,
            None,
        )
        .await?;

    Ok(success_result(
        format!("Terminated session: {}", session_id),
        data,
    ))
}

pub async fn list_assistants(
    client: &SessionApiClient,
    _args: Value,
    _caller_session_id: Option<String>,
) -> Result<MCPResult, String> {
    let data = client
        .call_json(Method::GET, "/api/assistants", None, None)
        .await?;

    let assistant_count = data
        .get("assistants")
        .and_then(|v| v.as_array())
        .map(|arr| arr.len())
        .unwrap_or(0);

    Ok(success_result(
        format!("Fetched {} assistants", assistant_count),
        data,
    ))
}
