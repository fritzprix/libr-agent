use serde_json::json;
use tauri_mcp_agent_lib::agent::llm::response::is_effectively_empty_llm_response;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;

fn assistant_message(
    content: Vec<MCPContent>,
    thinking: Option<&str>,
    usage: Option<serde_json::Value>,
) -> Message {
    Message {
        id: "msg-1".to_string(),
        session_id: "session-1".to_string(),
        role: "assistant".to_string(),
        content,
        tool_calls: None,
        tool_call_id: None,
        is_streaming: Some(false),
        thinking: thinking.map(str::to_string),
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage,
        created_at: 0,
        updated_at: 0,
        source: None,
        error: None,
        metadata: None,
    }
}

#[test]
fn whitespace_only_text_without_usage_is_effectively_empty() {
    let message = assistant_message(
        vec![MCPContent::Text {
            text: "   \n\t".to_string(),
            is_error: None,
        }],
        None,
        None,
    );

    assert!(is_effectively_empty_llm_response(&message));
}

#[test]
fn usage_only_response_is_not_effectively_empty() {
    let message = assistant_message(
        Vec::new(),
        None,
        Some(json!({
            "promptTokens": 100,
            "completionTokens": 12,
            "totalTokens": 112
        })),
    );

    assert!(!is_effectively_empty_llm_response(&message));
}

#[test]
fn thinking_only_response_is_not_effectively_empty() {
    let message = assistant_message(Vec::new(), Some("reasoning..."), None);

    assert!(!is_effectively_empty_llm_response(&message));
}
