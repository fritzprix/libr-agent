use crate::commands::agent_commands::ToolCancellation;
use crate::mcp::types::MCPContent;
use crate::models::chat::{Message, MessageSource};

/// Convert MCP response result to agent MCPContent
pub fn convert_mcp_response_content(
    result: Option<crate::mcp::types::MCPResponseResult>,
) -> Option<Vec<crate::mcp::types::MCPContent>> {
    match result {
        Some(crate::mcp::types::MCPResponseResult::ToolCall(tool_result)) => tool_result.content,
        _ => None,
    }
}

pub fn build_tool_message_metadata(
    tool_error: bool,
    structured_content: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    let mut metadata = serde_json::Map::new();

    if tool_error {
        metadata.insert("toolError".to_string(), serde_json::Value::Bool(true));
    }

    if let Some(value) = structured_content {
        metadata.insert("structuredContent".to_string(), value);
    }

    if metadata.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(metadata))
    }
}

pub fn add_cancellation_metadata(
    structured_content: Option<serde_json::Value>,
    cancellation: Option<&ToolCancellation>,
) -> Option<serde_json::Value> {
    let Some(cancellation) = cancellation else {
        return structured_content;
    };
    let cancellation_value = serde_json::json!({
        "status": "cancelled",
        "cancelledBy": cancellation.cancelled_by_label(),
    });

    Some(match structured_content {
        Some(serde_json::Value::Object(mut object)) => {
            object.insert("cancellation".to_string(), cancellation_value);
            serde_json::Value::Object(object)
        }
        Some(value) => serde_json::json!({
            "toolResult": value,
            "cancellation": cancellation_value,
        }),
        None => serde_json::json!({
            "cancellation": cancellation_value,
        }),
    })
}

pub fn append_cancellation_note(
    content: &mut Vec<MCPContent>,
    cancellation: Option<&ToolCancellation>,
) {
    let Some(cancellation) = cancellation else {
        return;
    };

    let note = cancellation.display_message();
    let already_present = content
        .iter()
        .any(|item| matches!(item, MCPContent::Text { text } if text.contains(note)));
    if !already_present {
        content.push(MCPContent::Text {
            text: format!("Note: {note}"),
        });
    }
}

pub fn append_cancellation_note_to_error(
    error_message: &str,
    cancellation: Option<&ToolCancellation>,
) -> String {
    let Some(cancellation) = cancellation else {
        return error_message.to_string();
    };

    let note = cancellation.display_message();
    if error_message.contains(note)
        || error_message
            .to_ascii_lowercase()
            .contains("cancelled by user")
    {
        return error_message.to_string();
    }

    format!("{error_message}\n\nNote: {note}")
}

/// Create a tool result message from successful tool execution
pub fn create_tool_result_message(
    session_id: &str,
    tool_call_id: &str,
    content: String,
    structured_content: Option<serde_json::Value>,
) -> Message {
    let now = chrono::Utc::now().timestamp_millis();
    let content_array = vec![MCPContent::Text { text: content }];

    Message {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: "tool".to_string(),
        tool_call_id: Some(tool_call_id.to_string()),
        content: content_array,
        tool_calls: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        usage: None,
        prompt_tokens: None,
        attachments: None,
        tool_use: None,
        created_at: now,
        updated_at: now,
        source: Some(MessageSource::Tool),
        error: None,
        metadata: structured_content.map(|value| {
            serde_json::json!({
                "structuredContent": value,
            })
        }),
    }
}

/// Create an error tool result message from failed tool execution
pub fn create_error_tool_result(
    session_id: &str,
    tool_call_id: &str,
    error_message: &str,
    structured_content: Option<serde_json::Value>,
) -> Message {
    let now = chrono::Utc::now().timestamp_millis();
    let content_array = vec![MCPContent::Text {
        text: format!("Error: {}", error_message),
    }];

    Message {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: "tool".to_string(),
        tool_call_id: Some(tool_call_id.to_string()),
        content: content_array,
        tool_calls: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        usage: None,
        prompt_tokens: None,
        attachments: None,
        tool_use: None,
        created_at: now,
        updated_at: now,
        source: Some(MessageSource::Tool),
        error: None,
        metadata: build_tool_message_metadata(true, structured_content),
    }
}

/// Create a tool result message from strict MCP content.
///
/// `tool_error` is the SSOT for UI grouping (`metadata.toolError`).
pub fn create_tool_result_message_with_content(
    session_id: &str,
    tool_call_id: &str,
    content: Vec<MCPContent>,
    structured_content: Option<serde_json::Value>,
    tool_error: bool,
) -> Message {
    let now = chrono::Utc::now().timestamp_millis();

    Message {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: "tool".to_string(),
        tool_call_id: Some(tool_call_id.to_string()),
        content,
        tool_calls: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        usage: None,
        prompt_tokens: None,
        attachments: None,
        tool_use: None,
        created_at: now,
        updated_at: now,
        source: Some(MessageSource::Tool),
        error: None,
        metadata: build_tool_message_metadata(tool_error, structured_content),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::types::MCPContent;

    /// Regression: builtin tools that return is_error=true WITH mcp_content
    /// (e.g. guided_error from editFile) must surface that content to the
    /// agent — NOT collapse it to the bare "Unknown error" fallback.
    ///
    /// Root cause was that handle_tool_result only checked `result.is_error` and
    /// always called create_error_tool_result, discarding mcp_content entirely.
    #[test]
    fn test_error_with_mcp_content_preserves_guided_error_text() {
        let guided_text =
            "STALE ANCHOR on line 28 — retry with anchor: 'ab12cd'\n  → refresh anchors and retry NOW";
        let content = vec![MCPContent::Text {
            text: guided_text.to_string(),
        }];

        let msg = create_tool_result_message_with_content("sess1", "tc1", content, None, true);

        assert!(
            msg.content.iter().any(|c| matches!(c,
                MCPContent::Text { text } if text.contains("STALE ANCHOR")
            )),
            "guided_error text must be preserved in the tool message"
        );
        assert_eq!(
            msg.metadata
                .as_ref()
                .and_then(|m| m.get("toolError"))
                .and_then(|v| v.as_bool()),
            Some(true),
            "toolError metadata must be set from ToolExecutionResult.is_error"
        );
    }

    /// When is_error=true but mcp_content is None (e.g. JSON-RPC protocol error
    /// or arg-parse failure), the fallback path must still produce a message.
    /// "Unknown error" is acceptable here because there is literally no content.
    #[test]
    fn test_error_without_mcp_content_falls_back_to_error_string() {
        let msg = create_error_tool_result("sess1", "tc1", "Failed to parse args: EOF", None);
        assert!(
            msg.content.iter().any(|c| matches!(c,
                MCPContent::Text { text } if text.contains("Failed to parse args")
            )),
            "explicit error string must appear in the message"
        );

        let fallback = create_error_tool_result("sess1", "tc1", "Unknown error", None);
        assert!(
            fallback.content.iter().any(|c| matches!(c,
                MCPContent::Text { text } if text.contains("Unknown error")
            )),
            "Unknown error fallback must appear when no message is available"
        );
        assert_eq!(
            fallback
                .metadata
                .as_ref()
                .and_then(|m| m.get("toolError"))
                .and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn cancellation_metadata_preserves_tool_error_details() {
        let cancellation = ToolCancellation::user();
        let structured = add_cancellation_metadata(
            Some(serde_json::json!({
                "error": {
                    "message": "Command failed with exit code: -1"
                }
            })),
            Some(&cancellation),
        )
        .expect("cancellation metadata should be present");

        assert_eq!(
            structured["error"]["message"],
            "Command failed with exit code: -1"
        );
        assert_eq!(structured["cancellation"]["status"], "cancelled");
        assert_eq!(structured["cancellation"]["cancelledBy"], "user");
    }

    #[test]
    fn cancellation_note_is_added_without_replacing_tool_output() {
        let cancellation = ToolCancellation::user();
        let mut content = vec![MCPContent::Text {
            text: "Command failed with exit code: -1".to_string(),
        }];

        append_cancellation_note(&mut content, Some(&cancellation));

        assert_eq!(content.len(), 2);
        assert!(matches!(
            &content[0],
            MCPContent::Text { text } if text.contains("Command failed with exit code: -1")
        ));
        assert!(matches!(
            &content[1],
            MCPContent::Text { text } if text.contains("cancelled by the user")
        ));
    }
}
