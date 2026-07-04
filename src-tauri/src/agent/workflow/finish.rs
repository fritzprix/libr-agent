use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::SessionRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

pub async fn session_has_pending_events(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> bool {
    let active = active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        return session.pending_events.read().await.count() > 0;
    }

    false
}

/// Re-check `pending_events` immediately before transitioning to Idle.
///
/// Returns `true` when pending messages were found and a new LLM turn was
/// requested. Callers must skip the Idle transition in that case.
pub async fn continue_workflow_if_pending_events(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: &str,
) -> Result<bool, String> {
    if !session_has_pending_events(active_sessions, session_id).await {
        return Ok(false);
    }

    log::info!(
        "Pending messages detected for session {} during workflow finish. Continuing workflow.",
        session_id
    );

    crate::agent::llm::request_llm_completion_with_recovery(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        session_id.to_string(),
    )
    .await
    .map_err(|error| error.to_string())?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::context::registry::ContextRegistry;
    use crate::agent::state::{CompactionRuntimeState, PendingEvent, PendingEventManager};
    use crate::repositories::{SessionMetadata, SessionStatus};
    use std::sync::atomic::AtomicBool;
    use std::time::SystemTime;
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
                execution_mode: crate::execution_mode::ExecutionMode::Normal,
                workspace_override: None,
                workspace_isolation:
                    crate::models::workspace_isolation::WorkspaceIsolationMode::Host,
                docker_config: None,
                docker_container_name: None,
                docker_host_workspace_path: None,
            },
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
            repeated_text_loop_retry_count: Arc::new(RwLock::new(0)),
            pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
            pending_approvals: Arc::new(RwLock::new(HashMap::new())),
            context_registry: Arc::new(ContextRegistry::new()),
            compact_context: Arc::new(RwLock::new(None)),
            compaction: CompactionRuntimeState::new(),
            expected_response_id: Arc::new(RwLock::new(None)),
            cached_stable_prompt: Arc::new(RwLock::new(None)),
            last_completion_request: Arc::new(RwLock::new(None)),
            last_submitted_input_message_id: Arc::new(RwLock::new(None)),
        }
    }

    #[tokio::test]
    async fn session_has_pending_events_is_false_when_queue_empty() {
        let sessions = Arc::new(RwLock::new(HashMap::from([(
            "sess-1".to_string(),
            build_session("sess-1"),
        )])));

        assert!(!session_has_pending_events(&sessions, "sess-1").await);
    }

    #[tokio::test]
    async fn session_has_pending_events_is_true_when_queue_has_messages() {
        let session = build_session("sess-2");
        session
            .pending_events
            .write()
            .await
            .add(PendingEvent::Message("msg-1".to_string()));

        let sessions = Arc::new(RwLock::new(HashMap::from([(
            "sess-2".to_string(),
            session,
        )])));

        assert!(session_has_pending_events(&sessions, "sess-2").await);
    }
}
