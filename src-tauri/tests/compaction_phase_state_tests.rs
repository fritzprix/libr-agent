use tauri_mcp_agent_lib::agent::events::WorkflowCompletionReason;
use tauri_mcp_agent_lib::agent::state::{
    CompactionBeginOutcome, CompactionKind, CompactionPhase, CompactionResumeAction,
    CompactionReuseOutcome, CompactionRuntimeState, DeferredWorkflowStep,
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
async fn deferred_post_response_action_overrides_preflight_resume_on_success() {
    let compaction = CompactionRuntimeState::new();

    let begin = compaction
        .try_begin(
            CompactionKind::Preflight,
            Some("tail-preflight".to_string()),
            5678,
        )
        .await;
    assert!(matches!(begin, CompactionBeginOutcome::Started));

    let deferred_step = DeferredWorkflowStep::FinalizeWorkflow {
        reason: WorkflowCompletionReason::Natural,
    };
    let attached = compaction
        .attach_deferred_workflow_step(deferred_step.clone())
        .await;
    assert_eq!(attached, CompactionReuseOutcome::Promoted);

    let action = compaction.complete_success().await;
    match action {
        CompactionResumeAction::RunDeferred(DeferredWorkflowStep::FinalizeWorkflow { reason }) => {
            assert_eq!(reason, WorkflowCompletionReason::Natural);
        }
        other => panic!("expected deferred workflow action, got {:?}", other),
    }

    assert!(matches!(
        compaction.snapshot().await.phase,
        CompactionPhase::Idle
    ));
}
