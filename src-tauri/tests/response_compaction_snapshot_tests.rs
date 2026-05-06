use tauri_mcp_agent_lib::agent::llm::build_post_response_compaction_snapshot;
use tauri_mcp_agent_lib::models::chat::Message;

fn test_message(id: &str) -> Message {
    Message {
        id: id.to_string(),
        session_id: "session-1".to_string(),
        role: "assistant".to_string(),
        content: Vec::new(),
        tool_calls: None,
        tool_call_id: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: None,
        created_at: 0,
        updated_at: 0,
        source: None,
        error: None,
        metadata: None,
    }
}

#[test]
fn appends_pending_message_when_not_yet_cached() {
    let cached = vec![test_message("existing")];
    let pending = test_message("pending");

    let snapshot = build_post_response_compaction_snapshot(&cached, Some(&pending));

    let ids = snapshot
        .into_iter()
        .map(|message| message.id)
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["existing".to_string(), "pending".to_string()]);
}

#[test]
fn does_not_duplicate_pending_message_when_already_cached() {
    let cached = vec![test_message("existing"), test_message("pending")];
    let pending = test_message("pending");

    let snapshot = build_post_response_compaction_snapshot(&cached, Some(&pending));

    let ids = snapshot
        .into_iter()
        .map(|message| message.id)
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["existing".to_string(), "pending".to_string(),]);
}
