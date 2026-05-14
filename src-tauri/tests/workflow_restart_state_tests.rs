use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
use tauri_mcp_agent_lib::agent::state::{
    AgentSession, CompactionRuntimeState, DeferredWorkflowStep, PendingEventManager,
};
use tauri_mcp_agent_lib::agent::workflow::reset_session_execution_state;
use tauri_mcp_agent_lib::repositories::{SessionMetadata, SessionStatus};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

fn build_session_metadata(session_id: &str, status: SessionStatus) -> SessionMetadata {
    let now = chrono::Utc::now().timestamp_millis();
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Workflow restart test".to_string()),
        status,
        model: "gpt-5.4".to_string(),
        provider: "openai".to_string(),
        agent_config: None,
        parent_session_id: None,
        lineage_id: None,
        depth: None,
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: now,
        updated_at: now,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        yolo_mode: false,
        unsafe_mode: false,
        workspace_override: None,
    }
}

fn build_agent_session(metadata: SessionMetadata) -> AgentSession {
    AgentSession {
        metadata,
        is_running: true,
        active_permit: None,
        status_transition: Arc::new(RwLock::new(None)),
        transition_lock: Arc::new(tokio::sync::Mutex::new(())),
        cancellation_token: CancellationToken::new(),
        yolo_mode: Arc::new(AtomicBool::new(false)),
        unsafe_mode: Arc::new(AtomicBool::new(false)),
        cancel_pending: Arc::new(AtomicBool::new(false)),
        pending_execution: None,
        messages: Arc::new(RwLock::new(Vec::new())),
        cache_initialized: Arc::new(AtomicBool::new(true)),
        last_synced_at: Arc::new(RwLock::new(Some(SystemTime::now()))),
        repeated_thinking_retry_count: Arc::new(RwLock::new(0)),
        pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
        pending_approvals: Arc::new(RwLock::new(HashMap::new())),
        context_registry: Arc::new(ContextRegistry::new()),
        compact_context: Arc::new(RwLock::new(None)),
        compaction: CompactionRuntimeState::new(),
        expected_response_id: Arc::new(RwLock::new(None)),
        cached_stable_prompt: Arc::new(RwLock::new(None)),
        last_completion_request: Arc::new(RwLock::new(None)),
    }
}

#[tokio::test]
async fn reset_session_execution_state_clears_cancel_poison_and_stale_compaction_flags() {
    let metadata = build_session_metadata("session-reset-1", SessionStatus::Paused);
    let mut session = build_agent_session(metadata);

    session.cancel_pending.store(true, Ordering::SeqCst);
    session.cancellation_token.cancel();
    session.compaction.in_flight.store(true, Ordering::SeqCst);
    session
        .compaction
        .awaiting_completion
        .store(true, Ordering::SeqCst);
    *session.compaction.deferred_workflow_step.write().await =
        Some(DeferredWorkflowStep::RequestCompletion);

    reset_session_execution_state(&mut session).await;

    assert!(!session.cancel_pending.load(Ordering::SeqCst));
    assert!(!session.cancellation_token.is_cancelled());
    assert!(!session.compaction.in_flight.load(Ordering::SeqCst));
    assert!(!session
        .compaction
        .awaiting_completion
        .load(Ordering::SeqCst));
    assert!(session
        .compaction
        .deferred_workflow_step
        .read()
        .await
        .is_none());
}

#[tokio::test]
async fn reset_session_execution_state_replaces_cancelled_token_with_fresh_instance() {
    let metadata = build_session_metadata("session-reset-2", SessionStatus::Paused);
    let mut session = build_agent_session(metadata);
    let poisoned_token = session.cancellation_token.clone();
    poisoned_token.cancel();

    reset_session_execution_state(&mut session).await;

    assert!(poisoned_token.is_cancelled());
    assert!(!session.cancellation_token.is_cancelled());
}
