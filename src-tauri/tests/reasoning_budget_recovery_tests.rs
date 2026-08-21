use tauri_mcp_agent_lib::agent::llm::{
    evaluate_non_productive_completion_action, evaluate_streaming_issue_action,
    NonProductiveCompletionAction, StreamingIssueAction, REASONING_BUDGET_MAX_RETRIES,
    REPEATED_THINKING_MAX_RETRIES,
};

#[test]
fn reasoning_budget_retries_once_then_fails() {
    assert_eq!(
        evaluate_streaming_issue_action(
            Some("response-1"),
            "response-1",
            0,
            REASONING_BUDGET_MAX_RETRIES,
        ),
        StreamingIssueAction::CancelAndRetry {
            next_retry_count: 1
        }
    );
    assert_eq!(
        evaluate_streaming_issue_action(
            Some("response-1"),
            "response-1",
            REASONING_BUDGET_MAX_RETRIES,
            REASONING_BUDGET_MAX_RETRIES,
        ),
        StreamingIssueAction::CancelAndFail
    );
}

#[test]
fn reasoning_budget_retry_counter_is_independent_from_thinking_loop() {
    assert_ne!(REASONING_BUDGET_MAX_RETRIES, REPEATED_THINKING_MAX_RETRIES);
    assert_eq!(
        evaluate_non_productive_completion_action(0, REASONING_BUDGET_MAX_RETRIES),
        NonProductiveCompletionAction::Retry {
            next_retry_count: 1
        }
    );
}
