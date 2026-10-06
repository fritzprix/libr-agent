use serde_json::{json, Value};

pub fn build_agent_tool_data(
    tool_name: &str,
    resource_type: &str,
    resource_id: Option<&str>,
    message: &str,
    response_status: &str,
    next_actions: Vec<Value>,
) -> serde_json::Map<String, Value> {
    let mut data = serde_json::Map::new();
    data.insert("toolName".to_string(), Value::String(tool_name.to_string()));
    data.insert(
        "resourceType".to_string(),
        Value::String(resource_type.to_string()),
    );
    data.insert("message".to_string(), Value::String(message.to_string()));
    data.insert(
        "responseStatus".to_string(),
        Value::String(response_status.to_string()),
    );

    if let Some(resource_id) = resource_id {
        data.insert(
            "resourceId".to_string(),
            Value::String(resource_id.to_string()),
        );
    }

    if !next_actions.is_empty() {
        data.insert("nextActions".to_string(), Value::Array(next_actions));
    }

    data
}

/// Insert `sessionId` (and `storageSessionId` alias — same value) for tool payloads.
pub fn insert_agent_session_id_fields(
    data: &mut serde_json::Map<String, Value>,
    storage_session_id: &str,
) {
    let display_id = crate::utils::session_id::display_session_id(storage_session_id);
    data.insert("sessionId".to_string(), Value::String(display_id));
    data.insert(
        "storageSessionId".to_string(),
        Value::String(storage_session_id.to_string()),
    );
}

pub fn build_agent_session_tool_data(
    tool_name: &str,
    session_id: &str,
    message: &str,
    session_status: &str,
    response_status: &str,
    turn_count: usize,
    next_actions: Vec<Value>,
) -> serde_json::Map<String, Value> {
    let display_id = crate::utils::session_id::display_session_id(session_id);
    let mut data = build_agent_tool_data(
        tool_name,
        "session",
        Some(&display_id),
        message,
        response_status,
        next_actions,
    );
    insert_agent_session_id_fields(&mut data, session_id);
    data.insert(
        "status".to_string(),
        Value::String(session_status.to_string()),
    );
    data.insert("turnCount".to_string(), json!(turn_count));
    data
}

pub fn check_session_next_actions(session_id: &str) -> Vec<Value> {
    let display_id = crate::utils::session_id::display_session_id(session_id);
    vec![
        json!({
            "toolName": "agent__checkSession",
            "reason": "Poll the session again for the latest status and turn count.",
            "args": {
                "sessionId": display_id,
                "wait": false
            }
        }),
        json!({
            "toolName": "agent__checkSession",
            "reason": "Block again later when you want to wait for a terminal result.",
            "args": {
                "sessionId": display_id,
                "wait": true
            }
        }),
    ]
}

pub fn read_required_string(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
        .ok_or_else(|| format!("Missing required parameter: {key}"))
}
