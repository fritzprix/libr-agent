//! Windows-safe coverage for outcome-aware per-tool circuit-breaker streaks.
//!
//! Canonical home for these cases. Do not duplicate them under
//! `tests/integration/circuit_breaker_recovery_tests.rs` (that binary is
//! `#![cfg(not(windows))]` and only covers recovery/escalation paths).

#[allow(dead_code)]
#[path = "common/circuit_breaker_fixtures.rs"]
mod circuit_breaker_fixtures;

use circuit_breaker_fixtures::{test_message, test_tool_call};
use tauri_mcp_agent_lib::agent::llm::circuit_breaker::{
    build_tool_call_indices, evaluate_circuit_breaker_action,
    sanitize_circuit_breaker_log_tool_name, CircuitBreakerAction,
};
use tauri_mcp_agent_lib::agent::llm::natural_recovery::{
    build_loop_prevention_guidance, LoopPreventionKind, LoopPreventionShortCircuit,
};
use tauri_mcp_agent_lib::agent::types::ToolCall;
use tauri_mcp_agent_lib::models::chat::Message;

fn evaluate(
    messages: &[Message],
    tool_call: &ToolCall,
    threshold: usize,
) -> Option<CircuitBreakerAction> {
    let call_signature_by_id = build_tool_call_indices(messages);
    evaluate_circuit_breaker_action(messages, tool_call, &call_signature_by_id, threshold, 1)
}

#[test]
fn different_success_outcomes_do_not_accumulate_toward_threshold() {
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
        None,
        "changing outcomes must not Soft-block the next identical call"
    );
}

#[test]
fn identical_success_outcomes_still_trigger_natural_recovery() {
    let repeated_args = r#"{"processId":"abc","timeout":0}"#;
    let status = "Process abc: running";
    let current_call = test_tool_call("tc-3", "workspace__waitForProcess", repeated_args);
    let messages = vec![
        test_message(
            "assistant-1",
            "assistant",
            Some(vec![test_tool_call(
                "tc-1",
                "workspace__waitForProcess",
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
            status,
            Some(false),
        ),
        test_message(
            "assistant-2",
            "assistant",
            Some(vec![test_tool_call(
                "tc-2",
                "workspace__waitForProcess",
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
            status,
            Some(false),
        ),
    ];

    assert_eq!(
        evaluate(&messages, &current_call, 3),
        Some(CircuitBreakerAction::NaturalRecoverySuccess {
            count: 3,
            tool_name: "workspace__waitForProcess".to_string(),
            args: repeated_args.to_string(),
        })
    );
}

#[test]
fn polling_outcome_progress_resets_trailing_streak() {
    let repeated_args = r#"{"processId":"abc","timeout":0}"#;
    let current_call = test_tool_call("tc-4", "workspace__waitForProcess", repeated_args);
    let messages = vec![
        test_message(
            "assistant-1",
            "assistant",
            Some(vec![test_tool_call(
                "tc-1",
                "workspace__waitForProcess",
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
            "Process abc: running",
            Some(false),
        ),
        test_message(
            "assistant-2",
            "assistant",
            Some(vec![test_tool_call(
                "tc-2",
                "workspace__waitForProcess",
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
            "Process abc: running",
            Some(false),
        ),
        test_message(
            "assistant-3",
            "assistant",
            Some(vec![test_tool_call(
                "tc-3",
                "workspace__waitForProcess",
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
            None,
            "Process abc: completed",
            Some(false),
        ),
    ];

    assert_eq!(
        evaluate(&messages, &current_call, 3),
        None,
        "after outcome changes to completed, next identical call must not Soft-block"
    );
}

#[test]
fn structured_loop_fingerprint_drives_outcome_signature() {
    let repeated_args = r#"{"sessionId":"abc","wait":false}"#;
    let current_call = test_tool_call("tc-3", "agent__checkSession", repeated_args);
    let structured = |status: &str, turn: u64| {
        Some(serde_json::json!({
            "structuredContent": {
                "loopFingerprint": format!("{status}:{turn}"),
                "status": status,
                "turnCount": turn,
            }
        }))
    };

    let messages = vec![
        test_message(
            "assistant-1",
            "assistant",
            Some(vec![test_tool_call(
                "tc-1",
                "agent__checkSession",
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
            structured("busy", 1),
            "ignored text body",
            Some(false),
        ),
        test_message(
            "assistant-2",
            "assistant",
            Some(vec![test_tool_call(
                "tc-2",
                "agent__checkSession",
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
            structured("busy", 2),
            "ignored text body",
            Some(false),
        ),
    ];

    assert_eq!(
        evaluate(&messages, &current_call, 3),
        None,
        "changing structured loopFingerprint must reset the trailing streak"
    );

    let messages_identical = vec![
        test_message(
            "assistant-1",
            "assistant",
            Some(vec![test_tool_call(
                "tc-1",
                "agent__checkSession",
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
            structured("busy", 1),
            "ignored text body",
            Some(false),
        ),
        test_message(
            "assistant-2",
            "assistant",
            Some(vec![test_tool_call(
                "tc-2",
                "agent__checkSession",
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
            structured("busy", 1),
            "ignored text body",
            Some(false),
        ),
    ];

    assert!(
        matches!(
            evaluate(&messages_identical, &current_call, 3),
            Some(CircuitBreakerAction::NaturalRecoverySuccess { .. })
        ),
        "identical structured loopFingerprint must accumulate toward threshold"
    );
}

#[test]
fn success_track_guidance_recommends_blocking_wait() {
    let guidance = build_loop_prevention_guidance(&LoopPreventionShortCircuit {
        kind: LoopPreventionKind::RepeatedSuccessOutcome,
        tool_name: "workspace__waitForProcess".to_string(),
        args: "{}".to_string(),
        count: 3,
    });

    assert!(guidance.contains("blocked"));
    assert!(guidance.contains("blocking wait"));
    assert!(guidance.contains("wait=true"));
    assert!(!guidance.contains("sleep"));
}

#[test]
fn sanitize_circuit_breaker_log_tool_name_strips_controls() {
    let dirty = "planning__getCurrentState\n\u{0007}<script>";
    let clean = sanitize_circuit_breaker_log_tool_name(dirty);
    assert!(!clean.contains('\n'));
    assert!(!clean.contains('\u{0007}'));
    assert!(clean.contains("planning__getCurrentState"));
}

#[test]
fn soft_blocks_fill_offset_gap_until_hard_break() {
    // threshold=3, offset=3 → soft at 3/4/5, hard at 6. Counts in the gap must not execute.
    let repeated_args = r#"{"include_checked":true}"#;
    let status = "Planning state snapshot";
    let threshold = 3;
    let offset = 3;
    let loop_prevention = "Loop prevention: 'planning__getCurrentState' was called 3 times with identical parameters and the same successful result.\n\nThis call was blocked.";

    let eval = |msgs: &[Message], call: &ToolCall| {
        let call_signature_by_id = build_tool_call_indices(msgs);
        evaluate_circuit_breaker_action(msgs, call, &call_signature_by_id, threshold, offset)
    };

    let mut messages = vec![
        test_message(
            "assistant-1",
            "assistant",
            Some(vec![test_tool_call(
                "tc-1",
                "planning__getCurrentState",
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
            status,
            Some(false),
        ),
        test_message(
            "assistant-2",
            "assistant",
            Some(vec![test_tool_call(
                "tc-2",
                "planning__getCurrentState",
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
            status,
            Some(false),
        ),
    ];

    assert_eq!(
        eval(
            &messages,
            &test_tool_call("tc-3", "planning__getCurrentState", repeated_args)
        ),
        Some(CircuitBreakerAction::NaturalRecoverySuccess {
            count: 3,
            tool_name: "planning__getCurrentState".to_string(),
            args: repeated_args.to_string(),
        })
    );

    messages.push(test_message(
        "assistant-3",
        "assistant",
        Some(vec![test_tool_call(
            "tc-3",
            "planning__getCurrentState",
            repeated_args,
        )]),
        None,
        None,
        "",
        None,
    ));
    messages.push(test_message(
        "tool-3",
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
    ));

    assert_eq!(
        eval(
            &messages,
            &test_tool_call("tc-4", "planning__getCurrentState", repeated_args)
        ),
        Some(CircuitBreakerAction::NaturalRecoverySuccess {
            count: 4,
            tool_name: "planning__getCurrentState".to_string(),
            args: repeated_args.to_string(),
        }),
        "4th identical call in the offset gap must Soft-block"
    );

    messages.push(test_message(
        "assistant-4",
        "assistant",
        Some(vec![test_tool_call(
            "tc-4",
            "planning__getCurrentState",
            repeated_args,
        )]),
        None,
        None,
        "",
        None,
    ));
    messages.push(test_message(
        "tool-4",
        "tool",
        None,
        Some("tc-4"),
        Some(serde_json::json!({
            "toolError": true,
            "structuredContent": { "loopPrevention": true },
            "loopPrevention": true
        })),
        loop_prevention,
        Some(true),
    ));

    assert_eq!(
        eval(
            &messages,
            &test_tool_call("tc-5", "planning__getCurrentState", repeated_args)
        ),
        Some(CircuitBreakerAction::NaturalRecoverySuccess {
            count: 5,
            tool_name: "planning__getCurrentState".to_string(),
            args: repeated_args.to_string(),
        }),
        "5th identical call in the offset gap must Soft-block"
    );

    messages.push(test_message(
        "assistant-5",
        "assistant",
        Some(vec![test_tool_call(
            "tc-5",
            "planning__getCurrentState",
            repeated_args,
        )]),
        None,
        None,
        "",
        None,
    ));
    messages.push(test_message(
        "tool-5",
        "tool",
        None,
        Some("tc-5"),
        Some(serde_json::json!({
            "toolError": true,
            "structuredContent": { "loopPrevention": true },
            "loopPrevention": true
        })),
        loop_prevention,
        Some(true),
    ));

    assert_eq!(
        eval(
            &messages,
            &test_tool_call("tc-6", "planning__getCurrentState", repeated_args)
        ),
        Some(CircuitBreakerAction::HardBreak {
            count: 6,
            tool_name: "planning__getCurrentState".to_string(),
            args: repeated_args.to_string(),
        })
    );
}
