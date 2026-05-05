use tauri_mcp_agent_lib::agent::llm::{
    evaluate_non_productive_completion_action, evaluate_streaming_issue_action,
    NonProductiveCompletionAction, StreamingIssueAction, REPEATED_THINKING_MAX_RETRIES,
};

#[test]
fn repeated_thinking_report_is_ignored_for_stale_response_ids() {
    let action = evaluate_streaming_issue_action(Some("response-new"), "response-old", 0);

    assert_eq!(action, StreamingIssueAction::Ignore);
}

#[test]
fn repeated_thinking_report_retries_while_budget_remains() {
    let action = evaluate_streaming_issue_action(Some("response-1"), "response-1", 1);

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
    );

    assert_eq!(action, StreamingIssueAction::CancelAndFail);
}

#[test]
fn thinking_only_completion_uses_same_retry_budget() {
    let action = evaluate_non_productive_completion_action(1);

    assert_eq!(
        action,
        NonProductiveCompletionAction::Retry {
            next_retry_count: 2
        }
    );
}

#[test]
fn thinking_only_completion_fails_when_retry_budget_is_exhausted() {
    let action = evaluate_non_productive_completion_action(REPEATED_THINKING_MAX_RETRIES);

    assert_eq!(action, NonProductiveCompletionAction::Fail);
}
