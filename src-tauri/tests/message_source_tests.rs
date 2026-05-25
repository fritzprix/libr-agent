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

#[test]
fn external_request_message_semantics_match_source_policy() {
    assert!(make_message(None).is_external_request_message());
    assert!(make_message(Some(MessageSource::Api)).is_external_request_message());
    assert!(make_message(Some(MessageSource::SwarmLegacy)).is_external_request_message());
    assert!(make_message(Some(MessageSource::Channel)).is_external_request_message());
    assert!(make_message(Some(MessageSource::ScheduledTask)).is_external_request_message());
    assert!(make_message(Some(MessageSource::Ui)).is_external_request_message());

    assert!(!make_message(Some(MessageSource::Tool)).is_external_request_message());
    assert!(!make_message(Some(MessageSource::AgentTool)).is_external_request_message());
}

#[test]
fn unknown_source_still_uses_legacy_id_fallback_for_classification_helpers() {
    let mut compact_summary =
        make_message(Some(MessageSource::Unknown("future-source".to_string())));
    compact_summary.id = "compact-summary-legacy".to_string();
    assert!(compact_summary.is_compact_summary());

    let mut compaction_instruction =
        make_message(Some(MessageSource::Unknown("future-source".to_string())));
    compaction_instruction.id = "compaction-instruction-legacy".to_string();
    assert!(compaction_instruction.is_compaction_instruction());
    assert!(compaction_instruction.is_internal_synthetic_user_message());
    assert!(!compaction_instruction.is_external_request_message());
}
