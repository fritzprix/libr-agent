use tauri_mcp_agent_lib::scheduled::runner::{
    build_scheduled_task_message_id, should_recreate_task_session_after_error,
};

#[test]
fn scheduled_runner_rotates_pinned_sessions_after_context_limit_failures() {
    assert!(should_recreate_task_session_after_error(
        "The newest non-compactable context is too large for the configured context window (projected 99108 > limit 98304). Reduce the newest message or attachment payload and retry.",
        true,
        false,
    ));
    assert!(should_recreate_task_session_after_error(
        "Conversation context still exceeds the configured limit even after reserving safety margin (projected 99108 > limit 98304). Wait for compaction or reduce recent input size.",
        true,
        false,
    ));
}

#[test]
fn scheduled_runner_does_not_rotate_fresh_or_unpinned_sessions() {
    let error = "The newest non-compactable context is too large for the configured context window";

    assert!(!should_recreate_task_session_after_error(
        error, false, false,
    ));
    assert!(!should_recreate_task_session_after_error(error, true, true));
    assert!(!should_recreate_task_session_after_error(
        "Provider timeout",
        true,
        false,
    ));
}

#[test]
fn scheduled_runner_message_id_is_stable_per_run_and_session() {
    let run_at = 1_234_567_890_i64;

    let first = build_scheduled_task_message_id("task-1", "session-A", run_at);
    let second = build_scheduled_task_message_id("task-1", "session-A", run_at);
    let rotated = build_scheduled_task_message_id("task-1", "session-B", run_at);

    assert_eq!(first, second);
    assert_ne!(first, rotated);
}
