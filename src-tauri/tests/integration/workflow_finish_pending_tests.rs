use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::SystemTime;

use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
use tauri_mcp_agent_lib::agent::state::{
    AgentSession, CompactionRuntimeState, PendingEvent, PendingEventManager,
};
use tauri_mcp_agent_lib::agent::workflow::session_has_pending_events;
use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::repositories::{SessionMetadata, SessionStatus};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

fn build_session(session_id: &str) -> AgentSession {
    let now = chrono::Utc::now().timestamp_millis();
    AgentSession {
        metadata: SessionMetadata {
            id: session_id.to_string(),
            name: None,
            status: SessionStatus::Busy,
            model: "gpt-5.4".to_string(),
            provider: "openai".to_string(),
            assistant_id: None,
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
            execution_mode: ExecutionMode::Normal,
            workspace_override: None,
            workspace_isolation:
                tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode::Host,
            docker_config: None,
            docker_container_name: None,
            docker_host_workspace_path: None,
        },
        is_running: true,
        active_permit: None,
        status_transition: Arc::new(RwLock::new(None)),
        transition_lock: Arc::new(tokio::sync::Mutex::new(())),
        cancellation_token: CancellationToken::new(),
        cancel_pending: Arc::new(AtomicBool::new(false)),
        pending_execution: None,
        messages: Arc::new(RwLock::new(Vec::new())),
        cache_initialized: Arc::new(AtomicBool::new(true)),
        last_synced_at: Arc::new(RwLock::new(Some(SystemTime::now()))),
        repeated_thinking_retry_count: Arc::new(RwLock::new(0)),
        repeated_text_loop_retry_count: Arc::new(RwLock::new(0)),
        bad_tool_args_retry_count: Arc::new(RwLock::new(0)),
        bad_tool_args_incident_count: Arc::new(RwLock::new(0)),
        pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
        pending_approvals: Arc::new(RwLock::new(HashMap::new())),
        context_registry: Arc::new(ContextRegistry::new()),
        compact_context: Arc::new(RwLock::new(None)),
        compaction: CompactionRuntimeState::new(),
        expected_response_id: Arc::new(RwLock::new(None)),
        cached_stable_prompt: Arc::new(RwLock::new(None)),
        last_completion_request: Arc::new(RwLock::new(None)),
        last_submitted_input_message_id: Arc::new(RwLock::new(None)),
        tool_loop_resample_attempts: Arc::new(RwLock::new(HashMap::new())),
        tool_poll_trackers: Arc::new(RwLock::new(HashMap::new())),
    }
}

#[tokio::test]
async fn finish_window_detects_messages_queued_during_busy_to_idle_race() {
    let session = build_session("sess-finish-race");
    session
        .pending_events
        .write()
        .await
        .add(PendingEvent::Message("queued-during-finish".to_string()));

    let sessions = Arc::new(RwLock::new(HashMap::from([(
        "sess-finish-race".to_string(),
        session,
    )])));

    assert!(
        session_has_pending_events(&sessions, "sess-finish-race").await,
        "finish-window re-check must observe messages injected while status was still Busy"
    );
}
