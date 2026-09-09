//! ATIF-v1.7 and Markdown session-export mapping (Harbor-aligned).
//!
//! Standalone binary — does not pull `tauri::test::mock_app`.

use serde_json::json;
use tauri_mcp_agent_lib::agent::types::{ToolCall, ToolCallFunction};
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::{Message, MessageSource};
use tauri_mcp_agent_lib::session_export::{
    build_atif_trajectory, export_file_name, filter_session_analysis_messages,
    render_session_export, AtifAgentMetadata, SessionExportFormat,
};

fn metadata() -> AtifAgentMetadata {
    AtifAgentMetadata {
        name: "LibrAgent".to_string(),
        version: "0.8.33".to_string(),
        model_name: Some("openai/Qwen3.6-35B".to_string()),
        session_id: Some("sess-1".to_string()),
    }
}

fn blank_message(role: &str) -> Message {
    Message {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: "sess-1".to_string(),
        role: role.to_string(),
        content: vec![],
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

fn text_message(role: &str, text: &str) -> Message {
    let mut message = blank_message(role);
    message.content = vec![MCPContent::Text {
        text: text.to_string(),
    }];
    message
}

#[test]
fn build_atif_trajectory_maps_assistant_tools_and_observations() {
    let user = text_message("user", "Extract the ELF");
    let mut assistant = blank_message("assistant");
    assistant.thinking = Some("Plan the extractor".to_string());
    assistant.content = vec![
        MCPContent::Thinking {
            thinking: "Plan the extractor".to_string(),
            thinking_time: None,
        },
        MCPContent::Text {
            text: "I will run a shell command.".to_string(),
        },
    ];
    assistant.tool_calls = Some(vec![ToolCall {
        id: "call_1".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "shell__execute".to_string(),
            arguments: r#"{"command":"ls /app"}"#.to_string(),
        },
    }]);
    assistant.usage = Some(json!({
        "promptTokens": 100,
        "completionTokens": 20,
        "cachedPromptTokens": 40,
    }));

    let mut tool = text_message("tool", "a.out\nextract.js");
    tool.tool_call_id = Some("call_1".to_string());

    let mut final_assistant = text_message("assistant", "Done.");
    final_assistant.usage = Some(json!({
        "promptTokens": 50,
        "completionTokens": 5,
    }));

    let trajectory = build_atif_trajectory(&[user, assistant, tool, final_assistant], metadata());

    assert_eq!(trajectory.schema_version, "ATIF-v1.7");
    assert_eq!(trajectory.agent.name, "LibrAgent");
    assert_eq!(
        trajectory.agent.model_name.as_deref(),
        Some("openai/Qwen3.6-35B")
    );
    assert_eq!(trajectory.session_id.as_deref(), Some("sess-1"));
    assert_eq!(trajectory.steps.len(), 3);
    assert_eq!(trajectory.steps[0].source, "user");
    assert_eq!(trajectory.steps[1].source, "agent");
    assert_eq!(
        trajectory.steps[1].reasoning_content.as_deref(),
        Some("Plan the extractor")
    );
    let tool_calls = trajectory.steps[1].tool_calls.as_ref().expect("tool calls");
    assert_eq!(tool_calls[0].function_name, "shell__execute");
    assert_eq!(tool_calls[0].arguments, json!({"command": "ls /app"}));
    let observation = trajectory.steps[1]
        .observation
        .as_ref()
        .expect("observation");
    assert_eq!(
        observation.results[0].source_call_id.as_deref(),
        Some("call_1")
    );
    assert!(observation.results[0].content.contains("extract.js"));
    assert_eq!(
        trajectory.steps[1]
            .metrics
            .as_ref()
            .and_then(|metrics| metrics.prompt_tokens),
        Some(100)
    );
    let final_metrics = trajectory.final_metrics.expect("final metrics");
    assert_eq!(final_metrics.total_prompt_tokens, Some(150));
    assert_eq!(final_metrics.total_completion_tokens, Some(25));
    assert_eq!(final_metrics.total_cached_tokens, Some(40));
    assert_eq!(final_metrics.total_steps, 3);
}

#[test]
fn build_atif_trajectory_buffers_tool_result_before_assistant() {
    let mut tool = text_message("tool", "tool output first");
    tool.tool_call_id = Some("call_early".to_string());

    let mut assistant = text_message("assistant", "ran tool");
    assistant.tool_calls = Some(vec![ToolCall {
        id: "call_early".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "shell__execute".to_string(),
            arguments: "{}".to_string(),
        },
    }]);

    let trajectory = build_atif_trajectory(&[tool, assistant], metadata());
    assert_eq!(trajectory.steps.len(), 1);
    let observation = trajectory.steps[0]
        .observation
        .as_ref()
        .expect("buffered observation");
    assert_eq!(
        observation.results[0].source_call_id.as_deref(),
        Some("call_early")
    );
    assert!(observation.results[0].content.contains("tool output first"));
}

#[test]
fn build_atif_trajectory_empty_messages_still_valid() {
    let trajectory = build_atif_trajectory(
        &[],
        AtifAgentMetadata {
            name: "LibrAgent".to_string(),
            version: "0.8.33".to_string(),
            model_name: None,
            session_id: None,
        },
    );
    assert_eq!(trajectory.steps.len(), 1);
    assert_eq!(trajectory.steps[0].source, "agent");
    assert_eq!(
        trajectory.steps[0].message,
        "(no LibrAgent messages harvested for ATIF trajectory)"
    );
}

#[test]
fn export_file_name_sanitizes_path_chars_and_keeps_unicode() {
    assert_eq!(
        export_file_name(Some("분석/세션:1"), "sess-1", SessionExportFormat::Markdown),
        "분석_세션_1.md"
    );
    assert_eq!(
        export_file_name(Some("  "), "sess-1", SessionExportFormat::Atif),
        "sess-1_trajectory.json"
    );
    assert_eq!(
        export_file_name(None, "sess-1", SessionExportFormat::Atif),
        "sess-1_trajectory.json"
    );
}

#[test]
fn session_export_excludes_compaction_and_recovery_messages() {
    let user = text_message("user", "real user");
    let compact = Message::new_compact_summary_message("sess-1", "compacted".to_string(), 1);
    let mut recovery = text_message("user", "recovery overlay");
    recovery.source = Some(MessageSource::Recovery);
    let assistant = text_message("assistant", "real assistant");

    let filtered =
        filter_session_analysis_messages(vec![user.clone(), compact, recovery, assistant.clone()]);
    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0].id, user.id);
    assert_eq!(filtered[1].id, assistant.id);
}

#[test]
fn markdown_export_includes_thinking_and_skips_compaction() {
    let user = text_message("user", "hello");
    let compact =
        Message::new_compact_summary_message("sess-1", "should not appear".to_string(), 1);
    let mut assistant = text_message("assistant", "world");
    assistant.thinking = Some("plan".to_string());

    let bytes = render_session_export(
        vec![user, compact, assistant],
        metadata(),
        SessionExportFormat::Markdown,
    )
    .expect("markdown bytes");
    let markdown = String::from_utf8(bytes).expect("utf8");
    assert!(markdown.contains("## User"));
    assert!(markdown.contains("hello"));
    assert!(markdown.contains("world"));
    assert!(markdown.contains("plan"));
    assert!(!markdown.contains("should not appear"));
}

#[test]
fn atif_export_json_includes_agent_metadata() {
    let user = text_message("user", "hi");
    let bytes = render_session_export(vec![user], metadata(), SessionExportFormat::Atif)
        .expect("atif bytes");
    let payload: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(payload["schema_version"], "ATIF-v1.7");
    assert_eq!(payload["agent"]["name"], "LibrAgent");
    assert_eq!(payload["agent"]["version"], "0.8.33");
    assert_eq!(payload["agent"]["model_name"], "openai/Qwen3.6-35B");
    assert_eq!(payload["steps"][0]["source"], "user");
    assert_eq!(payload["steps"][0]["message"], "hi");
}

#[test]
fn session_export_preserves_multilingual_message_text() {
    let user = text_message("user", "한글 질문 with café");
    let assistant = text_message("assistant", "日本語の回答");

    let markdown = String::from_utf8(
        render_session_export(
            vec![user.clone(), assistant.clone()],
            metadata(),
            SessionExportFormat::Markdown,
        )
        .expect("markdown bytes"),
    )
    .expect("utf8");
    assert!(markdown.contains("한글 질문 with café"));
    assert!(markdown.contains("日本語の回答"));

    let payload: serde_json::Value = serde_json::from_slice(
        &render_session_export(vec![user, assistant], metadata(), SessionExportFormat::Atif)
            .expect("atif bytes"),
    )
    .expect("json");
    assert_eq!(payload["steps"][0]["message"], "한글 질문 with café");
    assert_eq!(payload["steps"][1]["message"], "日本語の回答");
}
