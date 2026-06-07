use serde_json::json;
use tauri_mcp_agent_lib::mcp::builtin::agent::handlers::{
    build_paused_check_session_result_from_messages,
    build_terminal_check_session_result_from_messages,
};
use tauri_mcp_agent_lib::mcp::types::MCPContent;

fn extract_text(result: &tauri_mcp_agent_lib::mcp::types::MCPResult) -> String {
    result
        .content
        .as_ref()
        .expect("text content expected")
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn terminal_check_session_result_includes_final_answer_without_waiting() {
    let result = build_terminal_check_session_result_from_messages(
        "session-terminal-123",
        "idle",
        7,
        &[json!({
            "role": "assistant",
            "content": [
                {
                    "type": "text",
                    "text": "All subtasks completed successfully."
                }
            ]
        })],
    );

    let text = extract_text(&result);
    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content expected");
    let next_actions = structured
        .get("nextActions")
        .and_then(|value| value.as_array())
        .expect("nextActions array expected");
    let follow_up_action = next_actions
        .first()
        .expect("messageToSession follow-up expected");

    assert!(text.contains("Session session-terminal-123 is terminal (idle)."));
    assert!(text.contains("All subtasks completed successfully."));
    assert!(text.contains("If you need more detail, use messageToSession"));
    assert_eq!(
        structured
            .get("responseStatus")
            .and_then(|value| value.as_str()),
        Some("success")
    );
    assert_eq!(
        structured.get("status").and_then(|value| value.as_str()),
        Some("idle")
    );
    assert_eq!(
        structured.get("turnCount").and_then(|value| value.as_u64()),
        Some(7)
    );
    assert_eq!(
        structured.get("result").and_then(|value| value.as_str()),
        Some("All subtasks completed successfully.")
    );
    assert_eq!(
        follow_up_action
            .get("toolName")
            .and_then(|value| value.as_str()),
        Some("messageToSession")
    );
    assert_eq!(
        follow_up_action
            .get("reason")
            .and_then(|value| value.as_str()),
        Some("Request the child session for more detail, file contents, or full output.")
    );
    assert_eq!(
        follow_up_action
            .get("args")
            .and_then(|value| value.get("sessionId"))
            .and_then(|value| value.as_str()),
        Some("session-terminal-123")
    );
    assert_eq!(
        follow_up_action
            .get("args")
            .and_then(|value| value.get("message"))
            .and_then(|value| value.as_str()),
        Some("Please share the complete output or file contents.")
    );
}

#[test]
fn terminal_check_session_result_uses_last_text_block_in_assistant_content() {
    let result = build_terminal_check_session_result_from_messages(
        "session-terminal-multi-text",
        "idle",
        2,
        &[json!({
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Working on it..."},
                {"type": "thinking", "thinking": "internal reasoning"},
                {"type": "text", "text": "All subtasks completed successfully."}
            ]
        })],
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content expected");

    assert_eq!(
        structured.get("result").and_then(|value| value.as_str()),
        Some("All subtasks completed successfully.")
    );
}

#[test]
fn terminal_check_session_result_falls_back_to_tool_text_when_assistant_has_no_text() {
    let result = build_terminal_check_session_result_from_messages(
        "session-terminal-456",
        "idle",
        3,
        &[
            json!({
                "role": "tool",
                "content": [
                    {
                        "type": "text",
                        "text": "Structured tool output summary"
                    }
                ]
            }),
            json!({
                "role": "assistant",
                "content": []
            }),
        ],
    );

    let text = extract_text(&result);
    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content expected");

    assert!(text.contains("[Tool Response Fallback]"));
    assert!(text.contains("Structured tool output summary"));
    assert_eq!(
        structured.get("result").and_then(|value| value.as_str()),
        Some("[Tool Response Fallback]\nStructured tool output summary")
    );
}

#[test]
fn terminal_error_check_session_result_marks_session_as_recoverable() {
    let result = build_terminal_check_session_result_from_messages(
        "session-error-123",
        "error",
        4,
        &[json!({
            "role": "assistant",
            "content": [
                {
                    "type": "text",
                    "text": "Tool execution failed while reading the workspace."
                }
            ]
        })],
    );

    let text = extract_text(&result);
    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content expected");

    assert!(text.contains("ended abnormally (error)"));
    assert!(text.contains("Use messageToSession"));
    assert_eq!(
        structured
            .get("responseStatus")
            .and_then(|value| value.as_str()),
        Some("error")
    );
    assert_eq!(
        structured
            .get("recoverable")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        structured
            .get("recoveryStrategy")
            .and_then(|value| value.as_str()),
        Some("messageToSession")
    );
}

#[test]
fn paused_check_session_result_includes_recovery_guidance() {
    let result = build_paused_check_session_result_from_messages(
        "session-paused-123",
        2,
        &[json!({
            "role": "assistant",
            "content": [
                {
                    "type": "text",
                    "text": "Halfway through the delegated task."
                }
            ]
        })],
    );

    let text = extract_text(&result);
    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content expected");

    assert!(text.contains("is paused and will not make progress on its own"));
    assert!(text.contains("messageToSession"));
    assert_eq!(
        structured.get("status").and_then(|value| value.as_str()),
        Some("paused")
    );
    assert_eq!(
        structured
            .get("responseStatus")
            .and_then(|value| value.as_str()),
        Some("paused")
    );
    assert_eq!(
        structured
            .get("recoverable")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
}
