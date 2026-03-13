use serde_json::json;
use tauri_mcp_agent_lib::agent::llm::context_selector::*;
use tauri_mcp_agent_lib::agent::llm::token_utils::*;
use tauri_mcp_agent_lib::agent::types::{ToolCall as AgentToolCall, ToolCallFunction};
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;

fn make_message(id: &str, role: &str, text: &str) -> Message {
    Message {
        id: id.to_string(),
        session_id: "test-session".to_string(),
        role: role.to_string(),
        content: vec![MCPContent::Text {
            text: text.to_string(),
            is_error: None,
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
        created_at: 0,
        updated_at: 0,
        source: None,
        error: None,
        metadata: None,
    }
}

fn make_message_simple(role: &str, text: &str) -> Message {
    make_message(&format!("msg-{}", text.len()), role, text)
}

#[test]
fn test_find_compaction_split_index() {
    let mut msgs = vec![];
    for i in 0..20 {
        msgs.push(make_message(&format!("msg{}", i), "user", "Short message"));
    }
    // Function always returns messages.len() — compact everything
    let idx = find_compaction_split_index(&msgs);
    assert_eq!(idx, 20);
}

#[test]
fn test_find_compaction_split_index_with_calibration() {
    let mut msgs = vec![];
    for i in 0..10 {
        let mut msg = make_message(&format!("msg{}", i), "assistant", "Test content");
        if i == 5 {
            msg.usage = Some(json!({ "totalTokens": 20000 }));
        }
        msgs.push(msg);
    }
    // Function always returns messages.len() — compact everything
    let idx = find_compaction_split_index(&msgs);
    assert_eq!(idx, 10);
}

#[test]
fn test_remove_incomplete_tool_chains() {
    let mut msg1 = make_message("m1", "assistant", "I will call tools");
    msg1.tool_calls = Some(vec![
        AgentToolCall {
            id: "call_A".to_string(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: "toolA".to_string(),
                arguments: "{}".to_string(),
            },
        },
        AgentToolCall {
            id: "call_B".to_string(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: "toolB".to_string(),
                arguments: "{}".to_string(),
            },
        },
    ]);

    let mut msg2 = make_message("m2", "tool", "result A");
    msg2.tool_call_id = Some("call_A".to_string());

    let cleaned = remove_incomplete_tool_chains(vec![msg1, msg2]);
    assert_eq!(cleaned.len(), 2);

    let cleaned_assistant = &cleaned[0];
    assert_eq!(cleaned_assistant.tool_calls.as_ref().unwrap().len(), 1);
    assert_eq!(
        cleaned_assistant.tool_calls.as_ref().unwrap()[0].id,
        "call_A"
    );
}

#[test]
fn test_batch_tool_calls_in_messages() {
    let mut msg = make_message("m1", "assistant", "Many tools");
    let mut tool_calls = vec![];
    for i in 0..5 {
        tool_calls.push(AgentToolCall {
            id: format!("call_{}", i),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: "tool".to_string(),
                arguments: "{}".to_string(),
            },
        });
    }
    msg.tool_calls = Some(tool_calls);

    let msgs = vec![msg];
    let batched = batch_tool_calls_in_messages(&msgs, 3);

    assert_eq!(batched.len(), 2);
    assert_eq!(batched[0].id, "m1_batch_0");
    assert_eq!(batched[1].id, "m1_batch_1");
}

#[test]
fn test_select_messages_within_context() {
    let mut msgs = vec![];
    for i in 0..5 {
        let role = if i % 2 == 0 { "user" } else { "assistant" };
        msgs.push(make_message(&format!("msg{}", i), role, "Message content"));
    }

    let selected = select_messages_within_context(&msgs, "openai", Some(5000), None, None);
    assert_eq!(selected.len(), 5);
    assert_eq!(selected[0].id, "msg0");
    assert_eq!(selected[4].id, "msg4");
}

#[test]
fn test_select_messages_regression_large_message() {
    let mut msgs = vec![];
    msgs.push(make_message(
        "big_msg",
        "user",
        &"Very long content ".repeat(100),
    ));

    let selected = select_messages_within_context(&msgs, "gemini", Some(10), None, None);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].id, "big_msg");
}

#[test]
fn test_estimate_text_tokens() {
    let text = "Hello, world! This is a test.";
    let tokens = estimate_text_tokens(text);
    assert!(tokens > 0);
}

#[test]
fn test_calculate_compact_threshold() {
    assert_eq!(calculate_compact_threshold(10000), 9000);
}

#[test]
fn test_estimate_tokens_bpe_basic_text() {
    let msg = make_message_simple("user", "Hello assistant");
    let tokens = estimate_tokens_bpe(&msg);
    assert!(tokens > 1);
}

#[test]
fn test_estimate_tokens_bpe_with_tools() {
    let mut msg = make_message_simple("assistant", "I will use a tool");
    msg.tool_calls = Some(vec![AgentToolCall {
        id: "call_123".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "get_weather".to_string(),
            arguments: "{\"city\": \"Seoul\"}".to_string(),
        },
    }]);

    let tokens = estimate_tokens_bpe(&msg);
    assert!(tokens > 5);
}

#[test]
fn test_grounded_total_tokens_no_grounding() {
    let messages = vec![
        make_message_simple("user", "Hello"),
        make_message_simple("assistant", "Hi there"),
        make_message_simple("user", "What's up"),
    ];

    let tokens = calculate_grounded_total_tokens(&messages, 10, 5);
    assert!(tokens > 15);
}

#[test]
fn test_grounded_total_tokens_with_grounding() {
    let msg1 = make_message_simple("user", "Hello");
    let mut msg2 = make_message_simple("assistant", "Hi there");
    msg2.usage = Some(json!({ "totalTokens": 100 }));

    let msg3 = make_message_simple("user", "A very long new message");
    let messages = vec![msg1, msg2, msg3.clone()];

    let tokens = calculate_grounded_total_tokens(&messages, 50, 50);
    let expected = 100 + estimate_tokens_bpe(&msg3);
    assert_eq!(tokens, expected);
}

#[test]
fn test_grounded_total_tokens_ignores_grounding_after_compaction() {
    // After compaction, Step A rebuilds as [compact-summary, ...tail].
    // compact-summary appears BEFORE the grounded assistant message,
    // so calculate_grounded_total_tokens must fall back to full BPE.
    let mut summary = make_message_simple("system", "Summary...");
    summary.id = "compact-summary-123".to_string();

    let mut grounded = make_message_simple("assistant", "Hi there");
    grounded.usage = Some(json!({ "totalTokens": 100 }));

    let tail = make_message_simple("user", "Hello");

    // compact-summary (idx 0) is BEFORE grounded assistant (idx 1) → BPE fallback
    let messages = vec![summary.clone(), grounded.clone(), tail.clone()];

    let tokens = calculate_grounded_total_tokens(&messages, 10, 5);
    let expected = estimate_tokens_bpe(&summary)
        + estimate_tokens_bpe(&grounded)
        + estimate_tokens_bpe(&tail)
        + 10
        + 5;

    assert_eq!(tokens, expected);
}
