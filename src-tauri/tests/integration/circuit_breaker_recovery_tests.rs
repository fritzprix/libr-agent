use tauri_mcp_agent_lib::agent::llm::circuit_breaker::{
    build_tool_call_indices, evaluate_circuit_breaker_action, CircuitBreakerAction,
};
use tauri_mcp_agent_lib::agent::types::{ToolCall, ToolCallFunction};
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;

fn test_message(
    id: &str,
    role: &str,
    tool_calls: Option<Vec<ToolCall>>,
    tool_call_id: Option<&str>,
    metadata: Option<serde_json::Value>,
    text: &str,
    is_error: Option<bool>,
) -> Message {
    Message {
        id: id.to_string(),
        session_id: "session-test".to_string(),
        role: role.to_string(),
        content: vec![MCPContent::Text {
            text: text.to_string(),
            is_error,
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

fn test_tool_call(id: &str, name: &str, arguments: &str) -> ToolCall {
    ToolCall {
        id: id.to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: name.to_string(),
            arguments: arguments.to_string(),
        },
    }
}

fn evaluate(
    messages: &[Message],
    tool_call: &ToolCall,
    threshold: usize,
) -> Option<CircuitBreakerAction> {
    let call_signature_by_id = build_tool_call_indices(messages);
    evaluate_circuit_breaker_action(messages, tool_call, &call_signature_by_id, threshold)
}

#[test]
fn natural_recovery_prefers_repeated_identical_error_before_hard_break() {
    let repeated_args = r#"{"path":"src/main.ts"}"#;
    let repeated_error = "Error: file not found";
    let current_call = test_tool_call("tc-3", "workspace__readFile", repeated_args);
    let messages = vec![
        test_message(
            "assistant-1",
            "assistant",
            Some(vec![test_tool_call(
                "tc-1",
                "workspace__readFile",
                repeated_args,
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-1",
            "tool",
            None,
            Some("tc-1"),
            Some(serde_json::json!({ "toolError": true })),
            repeated_error,
            Some(true),
        ),
        test_message(
            "assistant-2",
            "assistant",
            Some(vec![test_tool_call(
                "tc-2",
                "workspace__readFile",
                repeated_args,
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-2",
            "tool",
            None,
            Some("tc-2"),
            Some(serde_json::json!({ "toolError": true })),
            repeated_error,
            Some(true),
        ),
    ];

    assert_eq!(
        evaluate(&messages, &current_call, 3),
        Some(CircuitBreakerAction::NaturalRecoveryError {
            count: 3,
            tool_name: "workspace__readFile".to_string(),
            args: repeated_args.to_string(),
        })
    );
}

#[test]
fn hard_break_requires_exceeding_identical_error_threshold() {
    let repeated_args = r#"{"path":"src/main.ts"}"#;
    let repeated_error = "Error: file not found";
    let current_call = test_tool_call("tc-4", "workspace__readFile", repeated_args);
    let messages = vec![
        test_message(
            "assistant-1",
            "assistant",
            Some(vec![test_tool_call(
                "tc-1",
                "workspace__readFile",
                repeated_args,
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-1",
            "tool",
            None,
            Some("tc-1"),
            Some(serde_json::json!({ "toolError": true })),
            repeated_error,
            Some(true),
        ),
        test_message(
            "assistant-2",
            "assistant",
            Some(vec![test_tool_call(
                "tc-2",
                "workspace__readFile",
                repeated_args,
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-2",
            "tool",
            None,
            Some("tc-2"),
            Some(serde_json::json!({ "toolError": true })),
            repeated_error,
            Some(true),
        ),
        test_message(
            "assistant-3",
            "assistant",
            Some(vec![test_tool_call(
                "tc-3",
                "workspace__readFile",
                repeated_args,
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-3",
            "tool",
            None,
            Some("tc-3"),
            Some(serde_json::json!({ "toolError": true })),
            repeated_error,
            Some(true),
        ),
    ];

    assert_eq!(
        evaluate(&messages, &current_call, 3),
        Some(CircuitBreakerAction::HardBreak {
            count: 4,
            tool_name: "workspace__readFile".to_string(),
            args: repeated_args.to_string(),
        })
    );
}

#[test]
fn different_args_failures_do_not_skip_straight_to_hard_break() {
    let current_call = test_tool_call("tc-3", "planning__clearScratchpad", r#"{"id":193}"#);
    let messages = vec![
        test_message(
            "assistant-1",
            "assistant",
            Some(vec![test_tool_call(
                "tc-1",
                "planning__clearScratchpad",
                r#"{"id":191}"#,
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-1",
            "tool",
            None,
            Some("tc-1"),
            Some(serde_json::json!({ "toolError": true })),
            "Error: todo 191 missing",
            Some(true),
        ),
        test_message(
            "assistant-2",
            "assistant",
            Some(vec![test_tool_call(
                "tc-2",
                "planning__clearScratchpad",
                r#"{"id":192}"#,
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-2",
            "tool",
            None,
            Some("tc-2"),
            Some(serde_json::json!({ "toolError": true })),
            "Error: todo 192 missing",
            Some(true),
        ),
    ];

    assert_eq!(evaluate(&messages, &current_call, 3), None);
}

#[test]
fn natural_recovery_prefers_repeated_identical_success_before_hard_break() {
    let repeated_args = r#"{"path":"src/main.ts"}"#;
    let repeated_success = "src/main.ts contents";
    let current_call = test_tool_call("tc-3", "workspace__readFile", repeated_args);
    let messages = vec![
        test_message(
            "assistant-1",
            "assistant",
            Some(vec![test_tool_call(
                "tc-1",
                "workspace__readFile",
                repeated_args,
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-1",
            "tool",
            None,
            Some("tc-1"),
            None,
            repeated_success,
            Some(false),
        ),
        test_message(
            "assistant-2",
            "assistant",
            Some(vec![test_tool_call(
                "tc-2",
                "workspace__readFile",
                repeated_args,
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-2",
            "tool",
            None,
            Some("tc-2"),
            None,
            repeated_success,
            Some(false),
        ),
    ];

    assert_eq!(
        evaluate(&messages, &current_call, 3),
        Some(CircuitBreakerAction::NaturalRecoverySuccess {
            count: 3,
            tool_name: "workspace__readFile".to_string(),
            args: repeated_args.to_string(),
        })
    );
}

#[test]
fn same_call_signature_counts_even_when_success_text_differs() {
    // Same tool + args is a loop even if the successful payload text changes
    // (e.g. polling). Counter resets only when a different tool/args appears.
    let repeated_args = r#"{"path":"src/main.ts"}"#;
    let current_call = test_tool_call("tc-3", "workspace__readFile", repeated_args);
    let messages = vec![
        test_message(
            "assistant-1",
            "assistant",
            Some(vec![test_tool_call(
                "tc-1",
                "workspace__readFile",
                repeated_args,
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-1",
            "tool",
            None,
            Some("tc-1"),
            None,
            "src/main.ts contents v1",
            Some(false),
        ),
        test_message(
            "assistant-2",
            "assistant",
            Some(vec![test_tool_call(
                "tc-2",
                "workspace__readFile",
                repeated_args,
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-2",
            "tool",
            None,
            Some("tc-2"),
            None,
            "src/main.ts contents v2",
            Some(false),
        ),
    ];

    assert_eq!(
        evaluate(&messages, &current_call, 3),
        Some(CircuitBreakerAction::NaturalRecoverySuccess {
            count: 3,
            tool_name: "workspace__readFile".to_string(),
            args: repeated_args.to_string(),
        })
    );
}

#[test]
fn loop_prevention_blocks_do_not_reset_repeat_counter() {
    // After natural recovery short-circuits the 3rd identical call, further
    // identical calls must keep failing — the loop-prevention tool result must
    // not look like a “new outcome” that clears the streak.
    let repeated_success = "Teamwork artifact directory is ready";
    let loop_prevention = "Loop prevention: 'agent__prepareTeamworkWorkspace' was called 3 times with identical parameters and the same successful result.\n\nThis call was blocked.";
    let current_call = test_tool_call("tc-4", "agent__prepareTeamworkWorkspace", "{}");
    let messages = vec![
        test_message(
            "assistant-1",
            "assistant",
            Some(vec![test_tool_call(
                "tc-1",
                "agent__prepareTeamworkWorkspace",
                "{}",
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-1",
            "tool",
            None,
            Some("tc-1"),
            None,
            repeated_success,
            Some(false),
        ),
        test_message(
            "assistant-2",
            "assistant",
            Some(vec![test_tool_call(
                "tc-2",
                "agent__prepareTeamworkWorkspace",
                "{}",
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-2",
            "tool",
            None,
            Some("tc-2"),
            None,
            repeated_success,
            Some(false),
        ),
        test_message(
            "assistant-3",
            "assistant",
            Some(vec![test_tool_call(
                "tc-3",
                "agent__prepareTeamworkWorkspace",
                "{}",
            )]),
            None,
            None,
            "",
            None,
        ),
        test_message(
            "tool-3-loop-prevention",
            "tool",
            None,
            Some("tc-3"),
            Some(serde_json::json!({
                "toolError": true,
                "structuredContent": { "loopPrevention": true },
                "loopPrevention": true
            })),
            loop_prevention,
            Some(true),
        ),
    ];

    assert_eq!(
        evaluate(&messages, &current_call, 3),
        Some(CircuitBreakerAction::HardBreak {
            count: 4,
            tool_name: "agent__prepareTeamworkWorkspace".to_string(),
            args: "{}".to_string(),
        })
    );

    // A different tool/args after the streak resets counting.
    let different_call = test_tool_call(
        "tc-5",
        "workspace__writeFile",
        r#"{"path":"@teamwork/agents.md","content":"x"}"#,
    );
    assert_eq!(evaluate(&messages, &different_call, 3), None);
}
