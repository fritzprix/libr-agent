use serde_json::json;
use tauri_mcp_agent_lib::mcp::builtin::scratchpad::handlers;
use tauri_mcp_agent_lib::mcp::builtin::scratchpad::ScratchpadServer;
use tauri_mcp_agent_lib::mcp::types::MCPContent;

fn extract_text(result: &tauri_mcp_agent_lib::mcp::types::MCPResult) -> String {
    let content = result
        .content
        .as_ref()
        .expect("expected MCPResult.content")
        .first()
        .expect("expected at least one MCPContent item");

    match content {
        MCPContent::Text { text, .. } => text.clone(),
        other => panic!("expected MCPContent::Text, got: {other:?}"),
    }
}

#[tokio::test]
async fn think_returns_short_ack_without_echoing_thought_in_text() {
    let thought = "Need to inspect workspace__readFile before editing.";
    let next_action = "Call workspace__readFile on src/main.rs";

    let result = handlers::think(json!({
        "thought": thought,
        "nextAction": next_action,
    }))
    .await
    .expect("think should succeed");

    let text = extract_text(&result);
    assert_eq!(result.is_error, Some(false));
    assert!(
        text.contains("Thought noted"),
        "text should be a short ACK, got: {text}"
    );
    assert!(
        !text.contains(thought),
        "LLM-facing text must not echo thought (token bloat), got: {text}"
    );
    assert!(
        !text.contains(next_action),
        "LLM-facing text must not echo nextAction, got: {text}"
    );
    assert!(
        !text.contains("## Thinking Process"),
        "must not use the old verbose markdown template, got: {text}"
    );

    let data = result
        .structured_content
        .as_ref()
        .expect("structured_content should be present for UI/trace");
    assert_eq!(data.get("thought").and_then(|v| v.as_str()), Some(thought));
    assert_eq!(
        data.get("nextAction").and_then(|v| v.as_str()),
        Some(next_action)
    );
    assert!(
        data.get("id")
            .and_then(|v| v.as_str())
            .is_some_and(|id| !id.is_empty()),
        "structured payload should include a non-empty id"
    );
}

#[tokio::test]
async fn think_rejects_missing_or_blank_thought() {
    for args in [
        json!({}),
        json!({ "thought": "" }),
        json!({ "thought": "   " }),
        json!({ "thought": null }),
    ] {
        let result = handlers::think(args.clone())
            .await
            .expect("handler returns MCPResult errors as Ok");
        let text = extract_text(&result);
        assert_eq!(
            result.is_error,
            Some(true),
            "blank thought should be an error for args={args}"
        );
        assert!(
            text.contains("thought") || text.contains("Thought"),
            "error should mention thought for args={args}, got: {text}"
        );
    }
}

#[test]
fn think_schema_asks_for_concise_reasoning() {
    let tool = ScratchpadServer::tools_static()
        .into_iter()
        .find(|tool| tool.name == "think")
        .expect("think tool should exist");

    assert!(
        tool.description.contains("concise"),
        "description should discourage verbose dumps, got: {}",
        tool.description
    );
    assert!(
        !tool.description.to_lowercase().contains("chain of thought"),
        "schema should not invite long chain-of-thought dumps, got: {}",
        tool.description
    );
}
