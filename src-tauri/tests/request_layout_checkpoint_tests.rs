//! Windows-safe coverage for LLM request layout / session-context injection.
//! (Standalone binary — does not pull AppHandle/WebView into the link.)

use tauri_mcp_agent_lib::agent::llm::{
    build_request_layout, is_custom_openai_compatible_provider,
    provider_uses_synthetic_session_context, select_last_submitted_input_message_id,
    SESSION_CONTEXT_TOOL_NAME,
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

fn text_at(message: &Message, index: usize) -> &str {
    match &message.content[index] {
        MCPContent::Text { text, .. } => text.as_str(),
        other => panic!("expected text content, got {:?}", other),
    }
}

fn assert_session_context_tool_pair(messages: &[Message], real_user_id: &str) {
    assert!(messages.len() >= 3);
    let assistant = &messages[0];
    let tool_result = &messages[1];
    assert!(assistant.is_request_layout_scaffolding_message());
    assert!(tool_result.is_request_layout_scaffolding_message());
    assert_eq!(assistant.role, "assistant");
    assert_eq!(tool_result.role, "tool");
    let tool_calls = assistant.tool_calls.as_ref().expect("tool_calls");
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].function.name, SESSION_CONTEXT_TOOL_NAME);
    assert_eq!(
        tool_result.tool_call_id.as_deref(),
        Some(tool_calls[0].id.as_str())
    );
    assert!(text_at(tool_result, 0).contains("background context"));
    assert_eq!(messages.last().map(|m| m.id.as_str()), Some(real_user_id));
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

    assert_eq!(layout.messages.len(), 3);
    assert!(layout.messages[0]
        .id
        .starts_with("custom-openai-session-context-"));
    assert_session_context_tool_pair(&layout.messages, "real-user");
}

#[test]
fn synthetic_session_context_is_placed_before_latest_external_user() {
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
    assert_session_context_tool_pair(&layout.messages, "real-user");
}

#[test]
fn synthetic_session_context_stays_before_compaction_overlay() {
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

    assert_eq!(layout.messages.len(), 4);
    assert!(layout.messages[0].is_request_layout_scaffolding_message());
    assert!(layout.messages[1].is_request_layout_scaffolding_message());
    assert_eq!(layout.messages[2].id, "real-user");
    assert!(layout.messages[3].is_compaction_overlay_message());
    assert_eq!(
        select_last_submitted_input_message_id(&layout.messages).as_deref(),
        Some("real-user")
    );
}

#[test]
fn anthropic_uses_tool_pair_without_user_disclaimer() {
    let layout = build_request_layout(
        "anthropic",
        "session-1",
        Some("system".to_string()),
        Some("background context".to_string()),
        vec![make_user_message("real-user", Some(MessageSource::Ui))],
    );

    assert_session_context_tool_pair(&layout.messages, "real-user");
    let result_text = text_at(&layout.messages[1], 0);
    assert!(!result_text.contains("NOT a user instruction"));
    assert!(!result_text.contains("<session-context>"));
}

#[test]
fn non_synthetic_provider_appends_context_to_system_prompt() {
    let layout = build_request_layout(
        "together",
        "session-1",
        Some("system prompt".to_string()),
        Some("volatile context".to_string()),
        vec![make_user_message("real-user", Some(MessageSource::Ui))],
    );

    let system = layout.system_prompt.expect("system prompt");
    assert!(system.starts_with("system prompt\n\n"));
    assert!(system.contains("## Runtime Session Context (telemetry)"));
    assert!(system.contains("volatile context"));
    assert_eq!(layout.messages.len(), 1);
    assert_eq!(layout.messages[0].content.len(), 1);
    assert_eq!(text_at(&layout.messages[0], 0), "hello");
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

    let system = layout.system_prompt.expect("system prompt");
    assert!(system.starts_with("system prompt\n\n"));
    assert!(system.contains("## Runtime Session Context (telemetry)"));
    assert!(system.contains("volatile context"));
    assert_eq!(layout.messages.len(), 0);
}
