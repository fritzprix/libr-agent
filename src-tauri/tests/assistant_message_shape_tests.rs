use tauri_mcp_agent_lib::agent::llm::inspect_assistant_message_shape;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;

fn build_assistant_message(content: Vec<MCPContent>, thinking: Option<&str>) -> Message {
    Message {
        id: "assistant-thinking-only".to_string(),
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
        usage: None,
        prompt_tokens: None,
        created_at: 1,
        updated_at: 1,
        source: None,
        error: None,
        metadata: None,
    }
}

#[test]
fn thinking_only_in_content_array_is_detected_for_retry() {
    let shape = inspect_assistant_message_shape(&build_assistant_message(
        vec![MCPContent::Thinking {
            thinking: "Still reasoning about the task...".to_string(),
            thinking_time: None,
        }],
        Some("Still reasoning about the task..."),
    ));

    assert!(shape.has_thinking);
    assert!(!shape.has_renderable_content);
    assert!(!shape.has_tool_calls);
    assert!(shape.is_thinking_only_completion());
}

#[test]
fn whitespace_text_with_thinking_is_still_thinking_only() {
    let shape = inspect_assistant_message_shape(&build_assistant_message(
        vec![
            MCPContent::Thinking {
                thinking: "internal reasoning".to_string(),
                thinking_time: None,
            },
            MCPContent::Text {
                text: "   \n".to_string(),
            },
        ],
        Some("internal reasoning"),
    ));

    assert!(shape.is_thinking_only_completion());
}

#[test]
fn text_output_with_thinking_is_not_thinking_only() {
    let shape = inspect_assistant_message_shape(&build_assistant_message(
        vec![
            MCPContent::Thinking {
                thinking: "internal reasoning".to_string(),
                thinking_time: None,
            },
            MCPContent::Text {
                text: "Here is the answer.".to_string(),
            },
        ],
        Some("internal reasoning"),
    ));

    assert!(shape.has_renderable_content);
    assert!(!shape.is_thinking_only_completion());
}
