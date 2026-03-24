use std::fs;

use tauri_mcp_agent_lib::agent::tools::{
    spill_oversized_tool_result_messages, TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES,
};
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::session::get_session_manager;

fn make_tool_message(session_id: &str, tool_call_id: &str, text: &str) -> Message {
    Message {
        id: format!("message-{tool_call_id}"),
        session_id: session_id.to_string(),
        role: "tool".to_string(),
        content: vec![MCPContent::Text {
            text: text.to_string(),
            is_error: None,
        }],
        tool_calls: None,
        tool_call_id: Some(tool_call_id.to_string()),
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: None,
        created_at: 0,
        updated_at: 0,
        source: Some("tool".to_string()),
        error: None,
        metadata: None,
    }
}

#[tokio::test]
async fn tool_result_spillover_writes_large_tool_output_to_workspace_file() {
    let session_id = format!("spillover-test-{}", uuid::Uuid::new_v4());
    let original_text =
        "large tool output ".repeat((TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES / 18) + 200);

    let processed = spill_oversized_tool_result_messages(
        &session_id,
        vec![make_tool_message(
            &session_id,
            "tool_call_large",
            &original_text,
        )],
    )
    .await
    .expect("spillover should succeed");

    let message = &processed[0];
    let MCPContent::Text { text, .. } = &message.content[0] else {
        panic!("expected text content");
    };

    assert!(text.contains("Tool output was too large to inline"));
    assert!(text.contains(".libragent/tool-results/"));
    assert!(text.contains("readFile(\""));

    let start = text.find('`').expect("path opening backtick") + 1;
    let end = text[start..]
        .find('`')
        .map(|offset| start + offset)
        .expect("path closing backtick");
    let relative_path = &text[start..end];

    let session_manager = get_session_manager().expect("session manager");
    let workspace_dir = session_manager.get_session_workspace_dir_by_id(&session_id);
    let spilled_file = workspace_dir.join(relative_path.replace('/', "\\"));

    let spilled_text = fs::read_to_string(&spilled_file).expect("spilled file should exist");
    assert_eq!(spilled_text, original_text);

    let _ = fs::remove_dir_all(workspace_dir);
}

#[tokio::test]
async fn tool_result_spillover_leaves_small_tool_output_inline() {
    let session_id = format!("spillover-test-{}", uuid::Uuid::new_v4());
    let original_text = "small tool output";

    let processed = spill_oversized_tool_result_messages(
        &session_id,
        vec![make_tool_message(
            &session_id,
            "tool_call_small",
            original_text,
        )],
    )
    .await
    .expect("small tool output should remain inline");

    let message = &processed[0];
    let MCPContent::Text { text, .. } = &message.content[0] else {
        panic!("expected text content");
    };

    assert_eq!(text, original_text);
}
