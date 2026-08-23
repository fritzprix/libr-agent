//! Windows-safe coverage for LLM request layout / session-context injection.
//! (Standalone binary — does not pull AppHandle/WebView into the link.)

use tauri_mcp_agent_lib::agent::llm::{
    build_request_layout, is_custom_openai_compatible_provider,
    provider_uses_synthetic_session_context, select_last_submitted_input_message_id,
    should_inject_session_context_tool_pair, SESSION_CONTEXT_TOOL_NAME,
};
use tauri_mcp_agent_lib::agent::types::{ToolCall, ToolCallFunction};
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

fn make_assistant_with_tool_call(id: &str, tool_call_id: &str) -> Message {
    Message {
        id: id.to_string(),
        session_id: "session-1".to_string(),
        role: "assistant".to_string(),
        content: vec![],
        tool_calls: Some(vec![ToolCall {
            id: tool_call_id.to_string(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: "workspace__readFile".to_string(),
                arguments: "{}".to_string(),
            },
        }]),
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
        source: Some(MessageSource::Assistant),
        error: None,
        metadata: None,
    }
}

fn make_tool_result(id: &str, tool_call_id: &str) -> Message {
    Message {
        id: id.to_string(),
        session_id: "session-1".to_string(),
        role: "tool".to_string(),
        content: vec![MCPContent::Text {
            text: "file contents".to_string(),
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
        source: Some(MessageSource::Tool),
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

fn assert_session_context_tool_pair(messages: &[Message], pair_start: usize, real_user_id: &str) {
    assert!(messages.len() > pair_start + 1);
    let assistant = &messages[pair_start];
    let tool_result = &messages[pair_start + 1];
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
}

#[test]
fn fresh_user_turn_omits_session_context_tool_pair() {
    let messages = vec![make_user_message("real-user", Some(MessageSource::Ui))];
    assert!(!should_inject_session_context_tool_pair(&messages, 0));

    let layout = build_request_layout(
        "openai",
        "session-1",
        Some("system".to_string()),
        Some("background context".to_string()),
        messages,
    );

    assert_eq!(layout.system_prompt.as_deref(), Some("system"));
    assert_eq!(layout.messages.len(), 1);
    assert_eq!(layout.messages[0].id, "real-user");
    assert!(!layout.messages[0].is_request_layout_scaffolding_message());
}

#[test]
fn tool_loop_tail_injects_session_context_tool_pair() {
    let messages = vec![
        make_user_message("real-user", Some(MessageSource::Ui)),
        make_assistant_with_tool_call("asst-1", "call-1"),
        make_tool_result("tool-1", "call-1"),
    ];
    assert!(should_inject_session_context_tool_pair(
        &messages,
        messages.len()
    ));

    let layout = build_request_layout(
        "openai",
        "session-1",
        Some("system".to_string()),
        Some("background context".to_string()),
        messages,
    );

    assert_eq!(layout.messages.len(), 5);
    assert_eq!(layout.messages[0].id, "real-user");
    assert_eq!(layout.messages[1].id, "asst-1");
    assert_eq!(layout.messages[2].id, "tool-1");
    assert!(layout.messages[3].is_request_layout_scaffolding_message());
    assert!(layout.messages[4].is_request_layout_scaffolding_message());
    assert_eq!(
        layout.messages[3].tool_calls.as_ref().unwrap()[0]
            .function
            .name,
        SESSION_CONTEXT_TOOL_NAME
    );
    assert!(text_at(&layout.messages[4], 0).contains("background context"));
    assert_eq!(
        select_last_submitted_input_message_id(&layout.messages).as_deref(),
        Some("tool-1")
    );
}

#[test]
fn tool_turn_then_user_injects_pair_before_latest_external_user() {
    let messages = vec![
        make_user_message("user-1", Some(MessageSource::Ui)),
        make_assistant_with_tool_call("asst-1", "call-1"),
        make_tool_result("tool-1", "call-1"),
        make_user_message("real-user", Some(MessageSource::Ui)),
    ];
    assert!(should_inject_session_context_tool_pair(&messages, 3));

    let layout = build_request_layout(
        "custom:local-vllm",
        "session-1",
        Some("system".to_string()),
        Some("background context".to_string()),
        messages,
    );

    assert_eq!(layout.messages.len(), 6);
    assert!(layout.messages[3]
        .id
        .starts_with("custom-openai-session-context-"));
    assert_session_context_tool_pair(&layout.messages, 3, "real-user");
    assert_eq!(
        select_last_submitted_input_message_id(&layout.messages).as_deref(),
        Some("real-user")
    );
}

#[test]
fn text_only_assistant_turn_omits_session_context_tool_pair() {
    let mut assistant = make_assistant_with_tool_call("asst-1", "call-1");
    assistant.tool_calls = None;
    assistant.content = vec![MCPContent::Text {
        text: "done".to_string(),
    }];

    let messages = vec![
        make_user_message("user-1", Some(MessageSource::Ui)),
        assistant,
        make_user_message("real-user", Some(MessageSource::Ui)),
    ];
    assert!(!should_inject_session_context_tool_pair(&messages, 2));

    let layout = build_request_layout(
        "anthropic",
        "session-1",
        Some("system".to_string()),
        Some("background context".to_string()),
        messages,
    );

    assert_eq!(layout.messages.len(), 3);
    assert_eq!(layout.system_prompt.as_deref(), Some("system"));
    assert!(!layout
        .messages
        .iter()
        .any(Message::is_request_layout_scaffolding_message));
}

#[test]
fn synthetic_session_context_stays_before_compaction_overlay_after_tool_turn() {
    let layout = build_request_layout(
        "openai",
        "session-1",
        Some("system".to_string()),
        Some("background context".to_string()),
        vec![
            make_user_message("user-1", Some(MessageSource::Ui)),
            make_assistant_with_tool_call("asst-1", "call-1"),
            make_tool_result("tool-1", "call-1"),
            make_user_message("real-user", Some(MessageSource::Ui)),
            make_user_message(
                "compaction-instruction-1",
                Some(MessageSource::CompactionInstruction),
            ),
        ],
    );

    assert_eq!(layout.messages.len(), 7);
    assert!(layout.messages[3].is_request_layout_scaffolding_message());
    assert!(layout.messages[4].is_request_layout_scaffolding_message());
    assert_eq!(layout.messages[5].id, "real-user");
    assert!(layout.messages[6].is_compaction_overlay_message());
    assert_eq!(
        select_last_submitted_input_message_id(&layout.messages).as_deref(),
        Some("real-user")
    );
}

#[test]
fn anthropic_tool_pair_has_no_user_disclaimer_wrapper() {
    let layout = build_request_layout(
        "anthropic",
        "session-1",
        Some("system".to_string()),
        Some("background context".to_string()),
        vec![
            make_user_message("user-1", Some(MessageSource::Ui)),
            make_assistant_with_tool_call("asst-1", "call-1"),
            make_tool_result("tool-1", "call-1"),
            make_user_message("real-user", Some(MessageSource::Ui)),
        ],
    );

    assert_session_context_tool_pair(&layout.messages, 3, "real-user");
    let result_text = text_at(&layout.messages[4], 0);
    assert!(!result_text.contains("NOT a user instruction"));
    assert!(!result_text.contains("<session-context>"));
}

#[test]
fn non_synthetic_provider_omits_system_telemetry_and_skips_without_tool_turn() {
    let layout = build_request_layout(
        "together",
        "session-1",
        Some("system prompt".to_string()),
        Some("volatile context".to_string()),
        vec![make_user_message("real-user", Some(MessageSource::Ui))],
    );

    assert_eq!(layout.system_prompt.as_deref(), Some("system prompt"));
    assert!(!layout
        .system_prompt
        .as_deref()
        .unwrap_or("")
        .contains("Runtime Session Context"));
    assert_eq!(layout.messages.len(), 1);
}

#[test]
fn non_synthetic_provider_uses_tool_pair_after_tool_turn() {
    let layout = build_request_layout(
        "together",
        "session-1",
        Some("system prompt".to_string()),
        Some("volatile context".to_string()),
        vec![
            make_user_message("user-1", Some(MessageSource::Ui)),
            make_assistant_with_tool_call("asst-1", "call-1"),
            make_tool_result("tool-1", "call-1"),
        ],
    );

    assert_eq!(layout.system_prompt.as_deref(), Some("system prompt"));
    assert!(!layout
        .system_prompt
        .as_deref()
        .unwrap_or("")
        .contains("Runtime Session Context"));
    assert_eq!(layout.messages.len(), 5);
    assert!(layout.messages[3].is_request_layout_scaffolding_message());
    assert_eq!(
        layout.messages[3].tool_calls.as_ref().unwrap()[0]
            .function
            .name,
        SESSION_CONTEXT_TOOL_NAME
    );
    assert!(text_at(&layout.messages[4], 0).contains("volatile context"));
}
