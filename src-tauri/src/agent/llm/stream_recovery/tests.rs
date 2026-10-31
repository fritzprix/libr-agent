use super::constants::{
    BAD_TOOL_ARGS_MAX_INCIDENTS, BAD_TOOL_ARGS_MAX_RETRIES, MALFORMED_TOOL_MARKUP_MAX_RETRIES,
    MALFORMED_TOOL_MARKUP_NOTE, REASONING_BUDGET_MAX_RETRIES,
};
use super::counter::StreamingRecoveryCounter;
use super::handlers::evaluate_non_productive_completion_action;
use super::types::NonProductiveCompletionAction;

#[test]
fn bad_tool_args_retries_until_budget() {
    assert_eq!(
        evaluate_non_productive_completion_action(0, BAD_TOOL_ARGS_MAX_RETRIES),
        NonProductiveCompletionAction::Retry {
            next_retry_count: 1
        }
    );
    assert_eq!(
        evaluate_non_productive_completion_action(1, BAD_TOOL_ARGS_MAX_RETRIES),
        NonProductiveCompletionAction::Retry {
            next_retry_count: 2
        }
    );
    assert_eq!(
        evaluate_non_productive_completion_action(2, BAD_TOOL_ARGS_MAX_RETRIES),
        NonProductiveCompletionAction::Fail
    );
}

#[test]
fn bad_tool_args_incident_cap_is_positive() {
    assert!(BAD_TOOL_ARGS_MAX_INCIDENTS >= 1);
    assert!(BAD_TOOL_ARGS_MAX_RETRIES >= 1);
}

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
