use super::constants::{
    MALFORMED_TOOL_MARKUP_MAX_RETRIES, MALFORMED_TOOL_MARKUP_NOTE, REASONING_BUDGET_MAX_RETRIES,
};
use super::counter::StreamingRecoveryCounter;
use super::handlers::evaluate_non_productive_completion_action;
use super::types::NonProductiveCompletionAction;

#[test]
fn malformed_tool_markup_retries_twice_then_gives_up() {
    assert_eq!(MALFORMED_TOOL_MARKUP_MAX_RETRIES, 2);
    for (count, expected) in [
        (
            0,
            NonProductiveCompletionAction::Retry {
                next_retry_count: 1,
            },
        ),
        (
            1,
            NonProductiveCompletionAction::Retry {
                next_retry_count: 2,
            },
        ),
        (2, NonProductiveCompletionAction::Fail),
    ] {
        assert_eq!(
            evaluate_non_productive_completion_action(
                count,
                StreamingRecoveryCounter::MalformedToolMarkup.max_retries()
            ),
            expected
        );
    }
}

#[test]
fn malformed_tool_markup_note_is_neutral() {
    let note = MALFORMED_TOOL_MARKUP_NOTE;
    assert!(note.contains("structured tool call"));
    assert!(note.contains("continue your answer"));
    assert!(!note.contains("__"), "must not name or steer to a tool");
    assert!(!note.contains("tool_choice"));
}

#[test]
fn reasoning_budget_retries_once() {
    assert_eq!(REASONING_BUDGET_MAX_RETRIES, 1);
    assert_eq!(
        evaluate_non_productive_completion_action(0, REASONING_BUDGET_MAX_RETRIES),
        NonProductiveCompletionAction::Retry {
            next_retry_count: 1
        }
    );
    assert_eq!(
        evaluate_non_productive_completion_action(1, REASONING_BUDGET_MAX_RETRIES),
        NonProductiveCompletionAction::Fail
    );
}

#[test]
fn completion_cancel_tells_the_frontend_whether_a_retry_follows() {
    // The frontend shows "Retrying…" only when `willRetry` is true (#2705).
    let cancel = crate::agent::llm::types::CompletionCancelRequest {
        session_id: "s1".to_string(),
        response_message_id: "r1".to_string(),
        reason: "reasoning-budget-exceeded".to_string(),
        will_retry: false,
    };

    let json = serde_json::to_value(&cancel).unwrap();

    assert_eq!(json["willRetry"], serde_json::json!(false));
    assert_eq!(json["reason"], "reasoning-budget-exceeded");
}

#[test]
fn streaming_issue_report_accepts_optional_max_output_tokens() {
    use crate::agent::llm::types::StreamingIssueReport;

    let with_source: StreamingIssueReport = serde_json::from_value(serde_json::json!({
        "sessionId": "s1",
        "responseMessageId": "r1",
        "issueKind": "REASONING_BUDGET_EXCEEDED",
        "observedTailChars": 10,
        "patternLength": 7372,
        "repetitionCount": 7400,
        "maxOutputTokens": 8192,
        "maxOutputTokensSource": "settings",
    }))
    .unwrap();
    assert_eq!(with_source.max_output_tokens, Some(8192));
    assert_eq!(
        with_source.max_output_tokens_source.as_deref(),
        Some("settings")
    );

    // Loop reports do not send the fields.
    let without: StreamingIssueReport = serde_json::from_value(serde_json::json!({
        "sessionId": "s1",
        "responseMessageId": "r1",
        "issueKind": "REPEATED_TEXT_LOOP",
        "observedTailChars": 10,
        "patternLength": 64,
        "repetitionCount": 3,
    }))
    .unwrap();
    assert_eq!(without.max_output_tokens, None);
    assert_eq!(without.max_output_tokens_source, None);
}
