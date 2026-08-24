//! Windows-safe coverage for LLM request layout / session-context injection.
//! (Standalone binary — does not pull AppHandle/WebView into the link.)

use tauri_mcp_agent_lib::agent::llm::{
    build_request_layout, build_request_layout_with_inject_state,
    is_custom_openai_compatible_provider, provider_uses_synthetic_session_context,
    resolve_session_context_inject, select_last_submitted_input_message_id,
    should_force_fresh_session_context, synthetic_session_context_insert_index,
    SessionContextInjectState, SESSION_CONTEXT_HEARTBEAT_TURNS,
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

fn make_assistant_message(id: &str) -> Message {
    Message {
        id: id.to_string(),
        session_id: "session-1".to_string(),
        role: "assistant".to_string(),
        content: vec![MCPContent::Text {
            text: "ok".to_string(),
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
        source: None,
        error: None,
        metadata: None,
    }
}

fn make_tool_message(id: &str, tool_call_id: &str) -> Message {
    Message {
        id: id.to_string(),
        session_id: "session-1".to_string(),
        role: "tool".to_string(),
        content: vec![MCPContent::Text {
            text: "tool-result".to_string(),
        }],
        tool_calls: None,
        tool_call_id: Some(tool_call_id.to_string()),
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
        source: None,
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
    assert!(layout.messages[0].is_request_layout_scaffolding_message());
    assert!(layout.messages[0]
        .id
        .starts_with("custom-openai-session-context-"));
    assert_eq!(layout.messages[1].id, "real-user");
    assert!(text_at(&layout.messages[0], 0).contains("not a user request"));
}

#[test]
fn synthetic_session_context_is_placed_before_latest_external_user_on_first_turn() {
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
    assert!(layout.messages[0].is_request_layout_scaffolding_message());
    assert_eq!(
        layout.messages.last().map(|m| m.id.as_str()),
        Some("real-user")
    );
    assert!(text_at(&layout.messages[0], 0).contains("<session-context>"));
}

#[test]
fn synthetic_session_context_is_placed_before_previous_assistant() {
    let messages = vec![
        make_user_message("user-1", Some(MessageSource::Ui)),
        make_assistant_message("assistant-1"),
        make_user_message("user-2", Some(MessageSource::Ui)),
    ];
    assert_eq!(synthetic_session_context_insert_index(&messages), 1);

    let layout = build_request_layout(
        "openai",
        "session-1",
        Some("system".to_string()),
        Some("background context".to_string()),
        messages,
    );

    assert_eq!(layout.messages.len(), 4);
    assert!(layout.messages[1].is_request_layout_scaffolding_message());
    assert_eq!(layout.messages[2].id, "assistant-1");
    assert_eq!(layout.messages[3].id, "user-2");
    assert_eq!(
        layout.messages.last().map(|m| m.role.as_str()),
        Some("user")
    );
    assert!(!layout
        .messages
        .last()
        .unwrap()
        .is_request_layout_scaffolding_message());
}

#[test]
fn synthetic_session_context_is_placed_before_assistant_in_tool_loop() {
    let messages = vec![
        make_user_message("user-1", Some(MessageSource::Ui)),
        make_assistant_message("assistant-1"),
        make_tool_message("tool-1", "call-1"),
        make_tool_message("tool-2", "call-2"),
    ];
    assert_eq!(synthetic_session_context_insert_index(&messages), 1);

    let layout = build_request_layout(
        "openai",
        "session-1",
        Some("system".to_string()),
        Some("background context".to_string()),
        messages,
    );

    assert!(layout.messages[1].is_request_layout_scaffolding_message());
    assert_eq!(layout.messages[2].id, "assistant-1");
    assert_eq!(
        layout.messages.last().map(|m| m.role.as_str()),
        Some("tool")
    );
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

    // First turn + overlay: SC before external user; overlay remains last.
    assert_eq!(layout.messages.len(), 3);
    assert!(layout.messages[0].is_request_layout_scaffolding_message());
    assert_eq!(layout.messages[1].id, "real-user");
    assert!(layout.messages[2].is_compaction_overlay_message());
    assert_eq!(
        select_last_submitted_input_message_id(&layout.messages).as_deref(),
        Some("real-user")
    );
}

#[test]
fn synthetic_session_context_before_assistant_keeps_compaction_overlay_last() {
    let messages = vec![
        make_user_message("user-1", Some(MessageSource::Ui)),
        make_assistant_message("assistant-1"),
        make_user_message("user-2", Some(MessageSource::Ui)),
        make_user_message(
            "compaction-instruction-1",
            Some(MessageSource::CompactionInstruction),
        ),
    ];
    assert_eq!(synthetic_session_context_insert_index(&messages), 1);

    let layout = build_request_layout(
        "openai",
        "session-1",
        Some("system".to_string()),
        Some("sc".to_string()),
        messages,
    );

    assert!(layout.messages[1].is_request_layout_scaffolding_message());
    assert_eq!(layout.messages[2].id, "assistant-1");
    assert!(layout
        .messages
        .last()
        .unwrap()
        .is_compaction_overlay_message());
}

#[test]
fn lagged_snapshot_is_used_when_previous_assistant_exists() {
    let messages = vec![
        make_user_message("user-1", Some(MessageSource::Ui)),
        make_assistant_message("assistant-1"),
        make_user_message("user-2", Some(MessageSource::Ui)),
    ];
    let state = SessionContextInjectState {
        current: Some("current-sc".to_string()),
        previous: Some("previous-sc".to_string()),
        turns_since_force_fresh: 1,
    };
    let outcome = resolve_session_context_inject(&messages, &state);
    assert!(!outcome.force_fresh);
    assert_eq!(outcome.inject_text.as_deref(), Some("previous-sc"));
    assert_eq!(outcome.next_previous.as_deref(), Some("current-sc"));

    let (layout, _) = build_request_layout_with_inject_state(
        "openai",
        "session-1",
        Some("system".to_string()),
        state,
        messages,
    );
    assert!(text_at(&layout.messages[1], 0).contains("previous-sc"));
    assert!(!text_at(&layout.messages[1], 0).contains("current-sc"));
}

#[test]
fn force_fresh_on_compaction_overlay_and_heartbeat() {
    let with_assistant = vec![
        make_user_message("user-1", Some(MessageSource::Ui)),
        make_assistant_message("assistant-1"),
    ];
    assert!(!should_force_fresh_session_context(
        &with_assistant,
        Some("prev"),
        1
    ));
    assert!(should_force_fresh_session_context(
        &with_assistant,
        Some("prev"),
        SESSION_CONTEXT_HEARTBEAT_TURNS
    ));

    let with_overlay = vec![
        make_user_message("user-1", Some(MessageSource::Ui)),
        make_assistant_message("assistant-1"),
        make_user_message(
            "compaction-instruction-1",
            Some(MessageSource::CompactionInstruction),
        ),
    ];
    assert!(should_force_fresh_session_context(
        &with_overlay,
        Some("prev"),
        1
    ));
}

#[test]
fn anthropic_includes_disclaimer_without_xml_wrapper() {
    let layout = build_request_layout(
        "anthropic",
        "session-1",
        Some("system".to_string()),
        Some("background context".to_string()),
        vec![make_user_message("real-user", Some(MessageSource::Ui))],
    );

    let text = text_at(&layout.messages[0], 0);
    assert!(!text.contains("<session-context>"));
    assert!(text.contains("not a user request"));
    assert!(text.contains("background context"));
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
    let text = text_at(&layout.messages[0], 0);
    assert!(text.contains("<session-context>"));
    assert!(text.contains("not a user request"));
    assert!(text.contains("volatile context"));
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
    assert!(system.contains("<session-context>"));
    assert!(system.contains("not a user request"));
    assert!(system.contains("volatile context"));
    assert_eq!(layout.messages.len(), 0);
}
