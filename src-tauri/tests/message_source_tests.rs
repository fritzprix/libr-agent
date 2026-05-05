use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::{Message, MessageSource};

fn make_message(source: Option<MessageSource>) -> Message {
    Message {
        id: "msg-1".to_string(),
        session_id: "session-1".to_string(),
        role: "user".to_string(),
        content: vec![MCPContent::Text {
            text: "hello".to_string(),
            is_error: None,
        }],
        tool_calls: None,
        tool_call_id: None,
        is_streaming: None,
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: None,
        created_at: 1,
        updated_at: 1,
        source,
        error: None,
        metadata: None,
    }
}

#[test]
fn message_source_deserializes_known_values_to_typed_variants() {
    let message: Message = serde_json::from_value(serde_json::json!({
        "id": "msg-1",
        "sessionId": "session-1",
        "role": "user",
        "content": [{ "type": "text", "text": "hello" }],
        "createdAt": 1,
        "updatedAt": 1,
        "source": "channel"
    }))
    .expect("message should deserialize");

    assert_eq!(message.source, Some(MessageSource::Channel));
}

#[test]
fn message_source_deserializes_unknown_values_without_failing() {
    let message: Message = serde_json::from_value(serde_json::json!({
        "id": "msg-1",
        "sessionId": "session-1",
        "role": "user",
        "content": [{ "type": "text", "text": "hello" }],
        "createdAt": 1,
        "updatedAt": 1,
        "source": "future-source"
    }))
    .expect("message should deserialize");

    assert_eq!(
        message.source,
        Some(MessageSource::Unknown("future-source".to_string()))
    );
}

#[test]
fn message_source_serializes_to_wire_strings() {
    let message = make_message(Some(MessageSource::SwarmLegacy));

    let value = serde_json::to_value(&message).expect("message should serialize");
    assert_eq!(
        value.get("source"),
        Some(&serde_json::json!("swarm_legacy"))
    );
}
