use tauri_mcp_agent_lib::agent::llm::types::CompactRequest;
use tauri_mcp_agent_lib::agent::state::{
    CompactionBeginOutcome, CompactionKind, CompactionPhase, CompactionRecoveryPhase,
    CompactionResumeAction, CompactionReuseOutcome, CompactionRuntimeState,
};

#[tokio::test]
async fn manual_in_flight_can_be_promoted_to_preflight_resume() {
    let compaction = CompactionRuntimeState::new();

    let begin = compaction
        .try_begin(
            CompactionKind::Manual,
            Some("tail-manual".to_string()),
            1234,
        )
        .await;
    assert!(matches!(begin, CompactionBeginOutcome::Started));

    let promoted = compaction.arm_resume_completion().await;
    assert_eq!(promoted, CompactionReuseOutcome::Promoted);

    let action = compaction.complete_success().await;
    assert!(matches!(action, CompactionResumeAction::ResumeCompletion));
    assert!(matches!(
        compaction.snapshot().await.phase,
        CompactionPhase::Idle
    ));
    assert_eq!(
        compaction.snapshot().await.last_compacted_tail_id,
        Some("tail-manual".to_string())
    );
}

#[tokio::test]
async fn clearing_runtime_state_resets_summary_retry_and_cached_request() {
    let compaction = CompactionRuntimeState::new();
    let begin = compaction
        .try_begin(
            CompactionKind::Preflight,
            Some("tail-preflight".to_string()),
            5678,
        )
        .await;
    assert!(matches!(begin, CompactionBeginOutcome::Started));

    compaction.increment_summary_retry_count().await;
    compaction
        .set_current_request(CompactRequest {
            session_id: "session-1".to_string(),
            session_name: "Session".to_string(),
            messages: Vec::new(),
            to_id: "m1".to_string(),
            compacted_delta_count: 2,
            parent_request: None,
            resume_completion_after_compact: true,
        })
        .await;
    compaction
        .set_recovery_progress(CompactionRecoveryPhase::DegradedTools, 2)
        .await;

    compaction.clear_runtime_state(false).await;

    assert_eq!(compaction.summary_retry_count().await, 0);
    assert!(compaction.current_request().await.is_none());
    assert!(matches!(
        compaction.snapshot().await.phase,
        CompactionPhase::Idle
    ));
    assert!(matches!(
        compaction.recovery_phase().await,
        CompactionRecoveryPhase::CacheAligned
    ));
    assert_eq!(compaction.retry_attempt().await, 0);
}

#[tokio::test]
async fn clearing_in_flight_state_preserves_recovery_progress() {
    let compaction = CompactionRuntimeState::new();
    compaction
        .set_recovery_progress(CompactionRecoveryPhase::OverflowRecovery, 1)
        .await;
    compaction.increment_summary_retry_count().await;

    compaction.clear_in_flight_state(false).await;

    assert_eq!(compaction.summary_retry_count().await, 0);
    assert!(matches!(
        compaction.recovery_phase().await,
        CompactionRecoveryPhase::OverflowRecovery
    ));
    assert_eq!(compaction.retry_attempt().await, 1);
}
