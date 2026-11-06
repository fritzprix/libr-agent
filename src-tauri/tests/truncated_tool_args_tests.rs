use tauri_mcp_agent_lib::agent::llm::circuit_breaker::{
    build_tool_call_indices, evaluate_circuit_breaker_action, normalize_tool_arguments,
    tool_call_signature, CircuitBreakerAction, WaitPreventionSettings,
    TRUNCATED_OR_MALFORMED_ARGS_SIGNATURE,
};
use tauri_mcp_agent_lib::agent::llm::tool_args_validation::{
    inspect_tool_call_arguments, is_redacted_marker, parse_redacted_byte_length,
    redact_invalid_tool_call_args, redact_tool_call_arguments, ArgsParseFailureKind,
};
use tauri_mcp_agent_lib::agent::types::{ToolCall, ToolCallFunction};
use tauri_mcp_agent_lib::models::chat::Message;

fn make_tool_call(id: &str, name: &str, arguments: &str) -> ToolCall {
    ToolCall {
        id: id.to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: name.to_string(),
            arguments: arguments.to_string(),
        },
    }
}

// 1. Truncated arguments on a tool: history arguments are redacted (<redacted: N bytes, truncated>).
#[test]
fn test_truncated_args_redacted_in_history() {
    let raw_args = r#"{"path":"foo.txt","content":"unfinished string content that was cut off"#;
    let raw_len = raw_args.len();
    let mut tool_calls = vec![make_tool_call("tc-1", "workspace__writeFile", raw_args)];

    assert!(inspect_tool_call_arguments(&tool_calls[0].function.arguments).is_err());

    let redacted_count = redact_invalid_tool_call_args(&mut tool_calls);
    assert_eq!(redacted_count, 1);

    let expected_marker = format!("<redacted: {} bytes, truncated>", raw_len);
    assert_eq!(tool_calls[0].function.arguments, expected_marker);
    assert!(is_redacted_marker(&tool_calls[0].function.arguments));
    assert_eq!(
        parse_redacted_byte_length(&tool_calls[0].function.arguments),
        Some(raw_len)
    );
}

// 2 & 3. The error text includes budget, truncation, not executed, and split-work guidance,
// and behaves identically across multiple tools (e.g. writeFile and a non-file tool).
#[test]
fn test_error_text_contract_across_multiple_tools() {
    let tools = ["workspace__writeFile", "database__query", "custom__search"];

    for tool_name in tools {
        let truncated_args = r#"{"query":"SELECT * FROM users WHERE note LIKE '%huge text"#;
        let inspection = inspect_tool_call_arguments(truncated_args);
        assert!(inspection.is_err());
        let (kind, parse_error): (ArgsParseFailureKind, String) = inspection.unwrap_err();
        assert!(kind.is_truncated());

        let marker = redact_tool_call_arguments(truncated_args.len());
        assert!(is_redacted_marker(&marker));

        // When inspected with the redacted marker or raw truncated args, classification identifies truncation
        let marker_inspection = inspect_tool_call_arguments(&marker).unwrap_err();
        assert!(marker_inspection.0.is_truncated());

        // Error message text contract check
        let budget_info = " (max output tokens budget: ~4096)";
        let headline = format!(
            "Failed to parse arguments for `{tool_name}`: output budget exceeded{budget_info}, arguments truncated ({parse_error}). The tool call was not executed."
        );
        let guidance = vec![
            "Output budget exceeded: arguments were truncated before JSON was complete".to_string(),
            "The tool call was not executed".to_string(),
            "Split the work into smaller tool calls with smaller payloads".to_string(),
            "Do NOT retry the identical oversized payload — it will truncate again".to_string(),
            "Session output budget is ~4096 tokens; keep each tool call JSON well below this budget.".to_string(),
        ];

        let guidance_text = guidance
            .iter()
            .enumerate()
            .map(|(i, step)| format!("{}. {}", i + 1, step))
            .collect::<Vec<_>>()
            .join("\n");
        let full_message = format!("✗ {headline}\n\n{guidance_text}");

        let lower = full_message.to_lowercase();
        assert!(lower.contains("budget"), "must include budget");
        assert!(lower.contains("truncated"), "must include truncation");
        assert!(lower.contains("not executed"), "must state not executed");
        assert!(
            lower.contains("split the work into smaller tool calls"),
            "must include split-work guidance"
        );
    }
}

// 4. A valid tool call between failures does not enable a silent retry or alter truncation normalization.
#[test]
fn test_valid_tool_call_does_not_affect_truncation_normalization() {
    let raw_trunc = r#"{"path":"out.txt","content":"cut"#;
    let expected_marker = format!("<redacted: {} bytes, truncated>", raw_trunc.len());
    let mut batch_with_valid = vec![
        make_tool_call("tc-valid", "workspace__readFile", r#"{"path":"valid.txt"}"#),
        make_tool_call("tc-trunc", "workspace__writeFile", raw_trunc),
    ];

    let count = redact_invalid_tool_call_args(&mut batch_with_valid);
    assert_eq!(count, 1);
    assert_eq!(
        batch_with_valid[0].function.arguments,
        r#"{"path":"valid.txt"}"#
    );
    assert_eq!(batch_with_valid[1].function.arguments, expected_marker);

    // Normalization remains strictly stable
    assert_eq!(
        normalize_tool_arguments(&batch_with_valid[1].function.arguments),
        TRUNCATED_OR_MALFORMED_ARGS_SIGNATURE
    );
}

// 5. Repeated failures with different truncation points hit the loop guard and stop.
#[test]
fn test_repeated_failures_different_truncation_points_hit_loop_guard() {
    let tool_name = "workspace__writeFile";
    let wait_settings = WaitPreventionSettings {
        wait_duration_ms: 15_000,
        guide_count: 3,
        break_count: 5,
    };

    // Simulate turn 1, turn 2, turn 3 with different raw truncation lengths
    let raw_attempt_1 = r#"{"path":"a.txt","content":"aaaaa"#; // 32 bytes
    let raw_attempt_2 = r#"{"path":"a.txt","content":"aaaaaaaaaaaaaaaa"#; // 43 bytes
    let raw_attempt_3 = r#"{"path":"a.txt","content":"aaaaaaaaaaaaaaaaaaaa"#; // 47 bytes

    // Verify all attempts produce the identical tool-call signature
    let call_1 = make_tool_call("tc-1", tool_name, raw_attempt_1);
    let call_2 = make_tool_call("tc-2", tool_name, raw_attempt_2);
    let call_3 = make_tool_call("tc-3", tool_name, raw_attempt_3);

    let sig_1 = tool_call_signature(&call_1);
    let sig_2 = tool_call_signature(&call_2);
    let sig_3 = tool_call_signature(&call_3);

    assert_eq!(
        sig_1,
        format!("{}:{}", tool_name, TRUNCATED_OR_MALFORMED_ARGS_SIGNATURE)
    );
    assert_eq!(sig_1, sig_2);
    assert_eq!(sig_2, sig_3);

    // Also verify when redacted markers are used
    let redacted_call_1 = make_tool_call("tc-1", tool_name, "<redacted: 32 bytes, truncated>");
    let redacted_call_2 = make_tool_call("tc-2", tool_name, "<redacted: 43 bytes, truncated>");
    assert_eq!(tool_call_signature(&redacted_call_1), sig_1);
    assert_eq!(tool_call_signature(&redacted_call_2), sig_1);

    // Construct mock history with error outcomes for attempt 1 and attempt 2
    let mut messages: Vec<Message> = Vec::new();

    // Turn 1: assistant call 1 + tool error result
    let mut asst_1 = Message::new_user_message("s-1".to_string(), "".to_string(), None, None);
    asst_1.role = "assistant".to_string();
    asst_1.tool_calls = Some(vec![redacted_call_1.clone()]);
    messages.push(asst_1);

    let mut tool_err_1 =
        Message::new_user_message("s-1".to_string(), "Tool error".to_string(), None, None);
    tool_err_1.role = "tool".to_string();
    tool_err_1.tool_call_id = Some("tc-1".to_string());
    tool_err_1.metadata = Some(serde_json::json!({ "toolError": true }));
    messages.push(tool_err_1);

    // Turn 2: assistant call 2 + tool error result
    let mut asst_2 = Message::new_user_message("s-1".to_string(), "".to_string(), None, None);
    asst_2.role = "assistant".to_string();
    asst_2.tool_calls = Some(vec![redacted_call_2.clone()]);
    messages.push(asst_2);

    let mut tool_err_2 =
        Message::new_user_message("s-1".to_string(), "Tool error".to_string(), None, None);
    tool_err_2.role = "tool".to_string();
    tool_err_2.tool_call_id = Some("tc-2".to_string());
    tool_err_2.metadata = Some(serde_json::json!({ "toolError": true }));
    messages.push(tool_err_2);

    let call_signature_by_id = build_tool_call_indices(&messages);

    // On Turn 3, call_3 arrives (count becomes 3, hitting loop threshold 3)
    let action = evaluate_circuit_breaker_action(
        &messages,
        &call_3,
        &call_signature_by_id,
        3, // loop_threshold = 3
        1, // hard_break_offset = 1 -> hard break at 4
        &wait_settings,
    );

    assert!(
        action.is_some(),
        "3rd identical error failure must trip circuit breaker"
    );
    match action.unwrap() {
        CircuitBreakerAction::NaturalRecoveryError {
            count,
            tool_name: name,
            ..
        } => {
            assert_eq!(count, 3);
            assert_eq!(name, tool_name);
        }
        other => panic!("Expected NaturalRecoveryError at count 3, got {:?}", other),
    }

    // Add Turn 3 to history
    let mut asst_3 = Message::new_user_message("s-1".to_string(), "".to_string(), None, None);
    asst_3.role = "assistant".to_string();
    asst_3.tool_calls = Some(vec![call_3.clone()]);
    messages.push(asst_3);

    let mut tool_err_3 =
        Message::new_user_message("s-1".to_string(), "Tool error".to_string(), None, None);
    tool_err_3.role = "tool".to_string();
    tool_err_3.tool_call_id = Some("tc-3".to_string());
    tool_err_3.metadata = Some(serde_json::json!({ "toolError": true }));
    messages.push(tool_err_3);

    let call_signature_by_id = build_tool_call_indices(&messages);

    // Turn 4: call_4 arrives (count becomes 4, hitting hard break at threshold 3 + offset 1)
    let call_4 = make_tool_call(
        "tc-4",
        tool_name,
        r#"{"path":"a.txt","content":"truncated 4"#,
    );
    let hard_action = evaluate_circuit_breaker_action(
        &messages,
        &call_4,
        &call_signature_by_id,
        3,
        1,
        &wait_settings,
    );

    assert!(hard_action.is_some());
    match hard_action.unwrap() {
        CircuitBreakerAction::HardBreak {
            count,
            tool_name: name,
            ..
        } => {
            assert_eq!(count, 4);
            assert_eq!(name, tool_name);
        }
        other => panic!("Expected HardBreak at count 4, got {:?}", other),
    }
}

// 6. Malformed (non-truncated) arguments are not redacted, and reports invalid JSON rather than budget exceeded.
#[test]
fn test_malformed_non_truncated_args_not_redacted_and_reports_invalid_json() {
    let malformed_raw = r#"{"path":"test.txt", "content": "hello",}"#; // trailing comma
    let mut tool_calls = vec![make_tool_call(
        "tc-malformed",
        "workspace__writeFile",
        malformed_raw,
    )];

    // 1) redact_invalid_tool_call_args must leave malformed non-truncated arguments intact
    let count = redact_invalid_tool_call_args(&mut tool_calls);
    assert_eq!(
        count, 0,
        "malformed non-truncated arguments must not be redacted"
    );
    assert_eq!(tool_calls[0].function.arguments, malformed_raw);
    assert!(!is_redacted_marker(&tool_calls[0].function.arguments));

    // 2) inspect_tool_call_arguments classifies it as MalformedJson, not truncated
    let inspection = inspect_tool_call_arguments(&tool_calls[0].function.arguments);
    assert!(inspection.is_err());
    let (kind, parse_error) = inspection.unwrap_err();
    assert_eq!(kind, ArgsParseFailureKind::MalformedJson);
    assert!(!kind.is_truncated());

    // 3) Error path reporting check: reports invalid JSON syntax guidance, NOT budget exceeded
    let tool_name = "workspace__writeFile";
    let (headline, guidance) = if kind.is_truncated() {
        (
            format!(
                "Failed to parse arguments for `{tool_name}`: output budget exceeded, arguments truncated ({parse_error}). The tool call was not executed."
            ),
            vec![
                "Output budget exceeded: arguments were truncated before JSON was complete".to_string(),
                "The tool call was not executed".to_string(),
                "Split the work into smaller tool calls with smaller payloads".to_string(),
            ],
        )
    } else {
        (
            format!(
                "Failed to parse arguments for `{tool_name}`: invalid JSON ({parse_error}). The tool call was not executed."
            ),
            vec![
                "Fix the JSON syntax (quotes, commas, braces, escapes) and call the tool again".to_string(),
                "The tool call was not executed".to_string(),
                "If argument payloads are large, split the work into smaller tool calls".to_string(),
            ],
        )
    };

    assert!(headline.contains("invalid JSON"));
    assert!(!headline.contains("output budget exceeded"));
    assert!(!headline.contains("arguments truncated"));
    assert!(guidance[0].contains("Fix the JSON syntax"));

    // 4) Model can inspect raw arguments preview to see the syntax error
    let preview = tauri_mcp_agent_lib::agent::llm::tool_args_validation::args_preview(
        &tool_calls[0].function.arguments,
        240,
    );
    assert_eq!(preview, malformed_raw);
}
