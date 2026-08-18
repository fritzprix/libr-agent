//! Windows-safe streaming recovery coverage for reasoning-budget retries.
//!
//! The consolidated `integration_tests` binary is `#![cfg(not(windows))]` because
//! it links the full Tauri/WebView path. These cases stay runnable on Windows.

use tauri_mcp_agent_lib::agent::llm::{
    evaluate_non_productive_completion_action, evaluate_streaming_issue_action,
    NonProductiveCompletionAction, StreamingIssueAction, REASONING_BUDGET_MAX_RETRIES,
    REPEATED_TEXT_LOOP_MAX_RETRIES, REPEATED_THINKING_MAX_RETRIES,
};

#[test]
fn repeated_thinking_report_is_ignored_for_stale_response_ids() {
    let action = evaluate_streaming_issue_action(
        Some("response-new"),
        "response-old",
        0,
        REPEATED_THINKING_MAX_RETRIES,
    );

    assert_eq!(action, StreamingIssueAction::Ignore);
}

#[test]
fn repeated_thinking_report_retries_while_budget_remains() {
    let action = evaluate_streaming_issue_action(
        Some("response-1"),
        "response-1",
        1,
        REPEATED_THINKING_MAX_RETRIES,
    );

    assert_eq!(
        action,
        StreamingIssueAction::CancelAndRetry {
            next_retry_count: 2
        }
    );
}

#[test]
fn repeated_thinking_report_fails_when_retry_budget_is_exhausted() {
    let action = evaluate_streaming_issue_action(
        Some("response-1"),
        "response-1",
        REPEATED_THINKING_MAX_RETRIES,
        REPEATED_THINKING_MAX_RETRIES,
    );

    assert_eq!(action, StreamingIssueAction::CancelAndFail);
}

#[test]
fn repeated_text_loop_report_uses_the_same_action_semantics_with_text_budget() {
    let action = evaluate_streaming_issue_action(
        Some("response-1"),
        "response-1",
        1,
        REPEATED_TEXT_LOOP_MAX_RETRIES,
    );

    assert_eq!(
        action,
        StreamingIssueAction::CancelAndRetry {
            next_retry_count: 2
        }
    );
}

#[test]
fn thinking_only_completion_uses_same_retry_budget() {
    let action = evaluate_non_productive_completion_action(1, REPEATED_THINKING_MAX_RETRIES);

    assert_eq!(
        action,
        NonProductiveCompletionAction::Retry {
            next_retry_count: 2
        }
    );
}

#[test]
fn thinking_only_completion_fails_when_retry_budget_is_exhausted() {
    let action = evaluate_non_productive_completion_action(
        REPEATED_THINKING_MAX_RETRIES,
        REPEATED_THINKING_MAX_RETRIES,
    );

    assert_eq!(action, NonProductiveCompletionAction::Fail);
}

#[test]
fn reasoning_budget_report_retries_once() {
    let action = evaluate_streaming_issue_action(
        Some("response-1"),
        "response-1",
        0,
        REASONING_BUDGET_MAX_RETRIES,
    );

    assert_eq!(
        action,
        StreamingIssueAction::CancelAndRetry {
            next_retry_count: 1
        }
    );
}

#[test]
fn reasoning_budget_report_fails_after_the_single_retry() {
    let action = evaluate_streaming_issue_action(
        Some("response-1"),
        "response-1",
        REASONING_BUDGET_MAX_RETRIES,
        REASONING_BUDGET_MAX_RETRIES,
    );

    assert_eq!(action, StreamingIssueAction::CancelAndFail);
}

#[test]
fn reasoning_budget_retry_limit_is_independent_from_thinking_loop() {
    assert_ne!(REASONING_BUDGET_MAX_RETRIES, REPEATED_THINKING_MAX_RETRIES);

    let thinking_at_budget_limit = evaluate_streaming_issue_action(
        Some("response-1"),
        "response-1",
        REASONING_BUDGET_MAX_RETRIES,
        REPEATED_THINKING_MAX_RETRIES,
    );
    let budget_at_budget_limit = evaluate_streaming_issue_action(
        Some("response-1"),
        "response-1",
        REASONING_BUDGET_MAX_RETRIES,
        REASONING_BUDGET_MAX_RETRIES,
    );

    assert_eq!(
        thinking_at_budget_limit,
        StreamingIssueAction::CancelAndRetry {
            next_retry_count: 2
        }
    );
    assert_eq!(budget_at_budget_limit, StreamingIssueAction::CancelAndFail);
}
