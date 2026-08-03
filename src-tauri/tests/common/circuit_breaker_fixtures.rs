//! Shared Message/ToolCall builders for circuit-breaker tests.
//!
//! Keep this module free of Tauri/DB deps so Windows-safe standalone test
//! binaries can `#[path]`-include it without pulling the WebView stack.

use tauri_mcp_agent_lib::agent::types::{ToolCall, ToolCallFunction};
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;

pub fn test_message(
    id: &str,
    role: &str,
    tool_calls: Option<Vec<ToolCall>>,
    tool_call_id: Option<&str>,
    metadata: Option<serde_json::Value>,
    text: &str,
    is_error: Option<bool>,
) -> Message {
    let metadata = match (metadata, is_error) {
        (Some(mut value), Some(true)) => {
            if let Some(obj) = value.as_object_mut() {
                obj.insert("toolError".to_string(), serde_json::Value::Bool(true));
            }
            Some(value)
        }
        (None, Some(true)) => Some(serde_json::json!({ "toolError": true })),
        (other, _) => other,
    };

    Message {
        id: id.to_string(),
        session_id: "session-test".to_string(),
        role: role.to_string(),
        content: vec![MCPContent::Text {
            text: text.to_string(),
        }],
        tool_calls,
        tool_call_id: tool_call_id.map(str::to_string),
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        created_at: 0,
        updated_at: 0,
        source: None,
        error: None,
        metadata,
        usage: None,
        prompt_tokens: None,
    }
}

pub fn test_tool_call(id: &str, name: &str, arguments: &str) -> ToolCall {
    ToolCall {
        id: id.to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: name.to_string(),
            arguments: arguments.to_string(),
        },
    }
}

pub fn mixed_batch(ids: [&str; 3], args_suffix: &str) -> Vec<ToolCall> {
    vec![
        test_tool_call(
            ids[0],
            "workspace__readFile",
            &format!(r#"{{"path":"a{args_suffix}.ts"}}"#),
        ),
        test_tool_call(
            ids[1],
            "workspace__listDirectory",
            &format!(r#"{{"path":"b{args_suffix}"}}"#),
        ),
        test_tool_call(
            ids[2],
            "workspace__grepFiles",
            &format!(r#"{{"query":"c{args_suffix}"}}"#),
        ),
    ]
}
