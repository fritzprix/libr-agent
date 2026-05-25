use tauri_mcp_agent_lib::agent::state::{
    CompactionBeginOutcome, CompactionKind, CompactionPhase, CompactionResumeAction,
    CompactionReuseOutcome, CompactionRuntimeState,
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
