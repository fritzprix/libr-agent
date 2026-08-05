use tauri_mcp_agent_lib::agent::llm::{
    build_request_layout, is_custom_openai_compatible_provider,
    provider_uses_synthetic_session_context, select_last_submitted_input_message_id,
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
fn custom_openai_compatible_provider_uses_synthetic_session_context() {
    assert!(is_custom_openai_compatible_provider("custom:local-vllm"));
    assert!(!is_custom_openai_compatible_provider("custom:"));
    assert!(!is_custom_openai_compatible_provider("openai"));
    assert!(provider_uses_synthetic_session_context("custom:local-vllm"));

    let layout = build_request_layout(
        "custom:local-vllm",
        "session-1",
        Some("system".to_string()),
        Some("background context".to_string()),
        vec![make_user_message("real-user", Some(MessageSource::Ui))],
    );

    assert_eq!(layout.messages.len(), 2);
    assert!(layout
        .messages
        .last()
        .expect("custom provider should inject synthetic session context")
        .is_request_layout_scaffolding_message());
    assert!(layout
        .messages
        .last()
        .expect("synthetic message")
        .id
        .starts_with("custom-openai-session-context-"));
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

#[test]
fn non_synthetic_provider_merges_context_into_last_user_message() {
    let layout = build_request_layout(
        "together",
        "session-1",
        Some("system prompt".to_string()),
        Some("volatile context".to_string()),
        vec![make_user_message("real-user", Some(MessageSource::Ui))],
    );

    // System prompt should remain unchanged
    assert_eq!(layout.system_prompt.as_deref(), Some("system prompt"));

    // The user message's content should now contain the volatile context
    assert_eq!(layout.messages.len(), 1);
    let content = &layout.messages[0].content;
    assert!(content.len() > 1); // contains merged context + original text
    if let MCPContent::Text { text, .. } = &content[0] {
        assert!(text.contains("<session-context>"));
        assert!(text.contains("volatile context"));
    } else {
        panic!("First content item should be text containing the volatile context");
    }
}

#[test]
fn non_synthetic_provider_falls_back_to_system_prompt_when_no_user_message() {
    let layout = build_request_layout(
        "together",
        "session-1",
        Some("system prompt".to_string()),
        Some("volatile context".to_string()),
        vec![],
    );

    // Since there was no user message, volatile context is appended to system prompt
    assert_eq!(
        layout.system_prompt.as_deref(),
        Some("system prompt\n\nvolatile context")
    );
    assert_eq!(layout.messages.len(), 0);
}
