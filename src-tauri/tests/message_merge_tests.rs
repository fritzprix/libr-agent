//! Windows-safe unit tests for shared user-message merge helpers.
//! (Not behind cfg(not(windows)) — no Tauri WebView link.)

use tauri_mcp_agent_lib::agent::message_merge::{
    merge_user_message_attachments, merge_user_message_contents, USER_MESSAGE_MERGE_SEPARATOR,
};
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;

fn text_message(id: &str, text: &str) -> Message {
    Message {
        id: id.to_string(),
        session_id: "s".to_string(),
        role: "user".to_string(),
        content: vec![MCPContent::Text {
            text: text.to_string(),
        }],
        tool_calls: None,
        tool_call_id: None,
        is_streaming: Some(false),
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

#[test]
fn merge_contents_joins_text_with_shared_separator() {
    let msgs = vec![
        text_message("a", "one"),
        text_message("b", "two"),
        text_message("c", "three"),
    ];
    let merged = merge_user_message_contents(&msgs);
    assert_eq!(merged.len(), 1);
    match &merged[0] {
        MCPContent::Text { text, .. } => {
            assert_eq!(
                text,
                &format!("one{USER_MESSAGE_MERGE_SEPARATOR}two{USER_MESSAGE_MERGE_SEPARATOR}three")
            );
        }
        _ => panic!("expected text"),
    }
}

#[test]
fn merge_attachments_concatenates_arrays() {
    let mut a = text_message("a", "one");
    a.attachments = Some(serde_json::json!([{"name": "a.txt"}]));
    let mut b = text_message("b", "two");
    b.attachments = Some(serde_json::json!([{"name": "b.txt"}]));
    let merged = merge_user_message_attachments(&[a, b]).expect("attachments");
    assert_eq!(
        merged,
        serde_json::json!([{"name": "a.txt"}, {"name": "b.txt"}])
    );
}
