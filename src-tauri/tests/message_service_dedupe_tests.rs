use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::services::MessageService;

fn make_message(id: &str, role: &str, text: &str) -> Message {
    Message {
        id: id.to_string(),
        session_id: "test-session".to_string(),
        role: role.to_string(),
        content: vec![MCPContent::Text {
            text: text.to_string(),
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
        created_at: 0,
        updated_at: 0,
        source: None,
        error: None,
        metadata: None,
    }
}

#[test]
fn filter_duplicate_injected_messages_skips_existing_and_incoming_duplicates() {
    let existing_messages = vec![make_message("scheduled-msg-1", "user", "Existing")];
    let incoming_messages = vec![
        make_message("scheduled-msg-1", "user", "Duplicate existing"),
        make_message("scheduled-msg-2", "user", "Fresh"),
        make_message("scheduled-msg-2", "user", "Duplicate incoming"),
    ];

    let accepted =
        MessageService::filter_duplicate_injected_messages(&existing_messages, &incoming_messages);

    assert_eq!(accepted.len(), 1);
    assert_eq!(accepted[0].id, "scheduled-msg-2");
    assert_eq!(accepted[0].role, "user");
}
