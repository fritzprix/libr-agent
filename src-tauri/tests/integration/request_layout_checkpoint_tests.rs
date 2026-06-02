use tauri_mcp_agent_lib::agent::llm::{
    build_request_layout, select_last_submitted_input_message_id,
};
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::{Message, MessageSource};

fn make_user_message(id: &str, source: Option<MessageSource>) -> Message {
    Message {
        id: id.to_string(),
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
        prompt_tokens: None,
        created_at: 1,
        updated_at: 1,
        source,
        error: None,
        metadata: None,
    }
}

#[test]
fn checkpoint_selection_skips_synthetic_session_context_tail() {
    let layout = build_request_layout(
        "openai",
        "session-1",
        Some("system".to_string()),
        Some("background context".to_string()),
        vec![make_user_message("real-user", Some(MessageSource::Ui))],
    );

    assert_eq!(
        select_last_submitted_input_message_id(&layout.messages).as_deref(),
        Some("real-user")
    );
    assert!(layout
        .messages
        .last()
        .expect("layout should include synthetic session context")
        .is_request_layout_scaffolding_message());
}

#[test]
fn checkpoint_selection_skips_all_internal_synthetic_tail_messages() {
    let layout = build_request_layout(
        "openai",
        "session-1",
        Some("system".to_string()),
        Some("background context".to_string()),
        vec![
            make_user_message("real-user", Some(MessageSource::Ui)),
            make_user_message(
                "compaction-instruction-1",
                Some(MessageSource::CompactionInstruction),
            ),
        ],
    );

    assert_eq!(
        select_last_submitted_input_message_id(&layout.messages).as_deref(),
        Some("real-user")
    );
}
