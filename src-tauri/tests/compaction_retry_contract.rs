use tauri_mcp_agent_lib::agent::compact::{
    build_awaiting_compaction_state, build_compaction_retry, classify_compaction_failure,
    CompactionFailureDecision, CompactionRequest, ContextUsageSummary, PendingCompactionRequest,
    MAX_COMPACTION_RETRIES,
};
use tauri_mcp_agent_lib::models::chat::Message;

fn make_pending(request_id: &str, retry_count: u8) -> PendingCompactionRequest {
    PendingCompactionRequest {
        request: CompactionRequest {
            request_id: request_id.to_string(),
            session_id: "session-1".to_string(),
            messages: Vec::<Message>::new(),
            model: "gpt-5.4".to_string(),
            provider: "openai".to_string(),
        },
        from_id: "from".to_string(),
        to_id: "to".to_string(),
        retry_count,
        context_usage: ContextUsageSummary {
            total_tokens: 4096,
            context_window: 8192,
            model_max_context: Some(16384),
        },
        compacted_range: None,
    }
}

#[test]
fn compaction_retry_increments_attempt_and_rotates_request_id() {
    let pending = make_pending("request-1", 0);

    let retried = build_compaction_retry(&pending).expect("retry should be available");

    assert_eq!(retried.retry_count, 1);
    assert_ne!(retried.request.request_id, pending.request.request_id);
    assert_eq!(retried.request.session_id, pending.request.session_id);
    assert_eq!(retried.from_id, pending.from_id);
    assert_eq!(retried.to_id, pending.to_id);
}

#[test]
fn compaction_retry_stops_after_max_retries() {
    let pending = make_pending("request-max", MAX_COMPACTION_RETRIES);

    assert!(build_compaction_retry(&pending).is_none());
}

#[test]
fn awaiting_state_preserves_context_usage_across_retries() {
    let pending = make_pending("request-2", 2);

    let state = build_awaiting_compaction_state("session-1", &pending);

    assert_eq!(state.status, "awaiting");
    assert_eq!(state.session_id, "session-1");
    assert_eq!(state.context_usage.expect("usage").total_tokens, 4096);
}

#[test]
fn compaction_failure_classification_retries_before_limit() {
    let pending = make_pending("request-3", 1);

    let decision = classify_compaction_failure(&pending);

    assert_eq!(
        decision,
        CompactionFailureDecision::Retry {
            attempts: 2,
            request_id: "request-3".to_string(),
        }
    );
}

#[test]
fn compaction_failure_classification_exhausts_after_third_retry() {
    let pending = make_pending("request-4", MAX_COMPACTION_RETRIES);

    let decision = classify_compaction_failure(&pending);

    assert_eq!(
        decision,
        CompactionFailureDecision::Exhausted {
            attempts: MAX_COMPACTION_RETRIES + 1,
        }
    );
}
