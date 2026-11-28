use crate::agent::concurrency::{
    acquire_queued_permit, session_cancel_token, ConcurrencyGate, QueuedPermitError,
};
use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::SessionStatus;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

/// Outcome of the per-turn active-permit guard (#3086 G1.1).
enum PermitGuard {
    /// The session holds (or just re-acquired) its active permit: request the LLM.
    Proceed,
    /// The session was cancelled while the permit was (re)acquired: skip the turn.
    Cancelled,
    /// The gate itself failed; surface the error instead of answering uncapped.
    Gate(String),
}

/// A `checkSession(wait=true)` that ends abnormally (child cancelled, wait
/// timed out) delivers its result without blocking on a full gate, so the Busy
/// caller is momentarily left without its active permit (#3086 G1.1). Before
/// the loop issues its next LLM request it must hold a permit again — this
/// guard re-acquires one, cancellably, at the single chokepoint every LLM
/// request passes through. Only a cancelled session continues without a
/// permit, and then the turn is skipped entirely.
///
/// Sessions that are not `Busy` (or gone) are left to the existing status
/// checks inside `request_llm_completion`: their permits are owned by the
/// lifecycle transitions, not by this guard.
async fn ensure_active_permit_for_llm(
    gate: &ConcurrencyGate,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> PermitGuard {
    if session_cancel_token(active_sessions, session_id)
        .await
        .is_none()
    {
        return PermitGuard::Proceed;
    }
    {
        let active = active_sessions.read().await;
        match active.get(session_id) {
            Some(session)
                if session.active_permit.is_some()
                    || session.metadata.status != SessionStatus::Busy =>
            {
                return PermitGuard::Proceed
            }
            Some(_) => {}
            None => return PermitGuard::Proceed,
        }
    }
    match acquire_queued_permit(gate, active_sessions, session_id).await {
        Ok(permit) => {
            let mut active = active_sessions.write().await;
            match active.get_mut(session_id) {
                Some(session)
                    if session.metadata.status == SessionStatus::Busy
                        && session.active_permit.is_none() =>
                {
                    session.active_permit = Some(permit);
                    drop(active);
                    log::info!(
                        "Re-acquired the active concurrency permit for session {} before its next LLM turn (#3086).",
                        session_id
                    );
                    PermitGuard::Proceed
                }
                _ => {
                    // Status changed or a permit appeared while we were queued:
                    // the freshly acquired permit drops here and returns to the gate.
                    drop(active);
                    PermitGuard::Proceed
                }
            }
        }
        Err(QueuedPermitError::Cancelled) => {
            log::info!(
                "Session {} was cancelled while re-acquiring its concurrency permit; skipping its next LLM turn (#3086).",
                session_id
            );
            PermitGuard::Cancelled
        }
        Err(QueuedPermitError::MissingSession) => PermitGuard::Proceed,
        Err(QueuedPermitError::Gate(error)) => PermitGuard::Gate(error),
    }
}

pub async fn request_llm_completion_with_recovery(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    match ensure_active_permit_for_llm(
        crate::state::get_concurrency_gate(),
        active_sessions,
        &session_id,
    )
    .await
    {
        PermitGuard::Proceed => {}
        PermitGuard::Cancelled => return Ok(()),
        PermitGuard::Gate(error) => return Err(error),
    }
    match super::request::request_llm_completion(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        session_id.clone(),
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(error) => {
            let outcome = crate::agent::llm::handle_llm_error_with_outcome(
                session_repo,
                active_sessions,
                app_handle,
                session_id,
                error.clone(),
            )
            .await?;
            crate::agent::llm::completion_result_from_error_handling_outcome(outcome, error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::context::registry::ContextRegistry;
    use crate::agent::state::{
        new_user_prompt_slot, AgentSession, CompactionPhase, CompactionRuntimeState,
        PendingEventManager,
    };
    use crate::agent::ExecutionMode;
    use crate::models::chat::Message;
    use crate::models::workspace_isolation::WorkspaceIsolationMode;
    use crate::repositories::SessionMetadata;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio_util::sync::CancellationToken;

    fn metadata(session_id: &str, status: SessionStatus) -> SessionMetadata {
        let now = chrono::Utc::now().timestamp_millis();
        SessionMetadata {
            id: session_id.to_string(),
            name: None,
            status,
            model: "test-model".to_string(),
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
            workspace_isolation: WorkspaceIsolationMode::Host,
            docker_config: None,
            docker_container_name: None,
            docker_host_workspace_path: None,
            status_reason: None,
            forked_from: None,
        }
    }

    fn agent_session(session_id: &str, status: SessionStatus) -> AgentSession {
        AgentSession {
            metadata: metadata(session_id, status),
            is_running: true,
            active_permit: None,
            status_transition: Arc::new(RwLock::new(None)),
            transition_lock: Arc::new(tokio::sync::Mutex::new(())),
            cancellation_token: CancellationToken::new(),
            cancel_pending: Arc::new(AtomicBool::new(false)),
            pending_execution: None,
            messages: Arc::new(RwLock::new(Vec::<Message>::new())),
            cache_initialized: Arc::new(AtomicBool::new(true)),
            last_synced_at: Arc::new(RwLock::new(None)),
            repeated_thinking_retry_count: Arc::new(RwLock::new(0)),
            repeated_text_loop_retry_count: Arc::new(RwLock::new(0)),
            reasoning_budget_retry_count: Arc::new(RwLock::new(0)),
            malformed_tool_markup_retry_count: Arc::new(RwLock::new(0)),
            invalid_tool_args_retry_count: Arc::new(RwLock::new(0)),
            pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
            pending_approvals: Arc::new(RwLock::new(HashMap::new())),
            pending_user_prompt: new_user_prompt_slot(),
            context_registry: Arc::new(ContextRegistry::new()),
            compact_context: Arc::new(RwLock::new(None)),
            compaction: CompactionRuntimeState::with_test_state(CompactionPhase::Idle, None),
            expected_response_id: Arc::new(RwLock::new(None)),
            cached_stable_prompt: Arc::new(RwLock::new(None)),
            last_completion_request: Arc::new(RwLock::new(None)),
            last_submitted_input_message_id: Arc::new(RwLock::new(None)),
            last_session_context_snapshot: Arc::new(RwLock::new(None)),
            session_context_turns_since_force_fresh: Arc::new(RwLock::new(0)),
            tool_loop_resample_attempts: Arc::new(RwLock::new(HashMap::new())),
            tool_poll_trackers: Arc::new(RwLock::new(HashMap::new())),
            max_input_context_override: None,
        }
    }

    async fn holds_permit(
        sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
        session_id: &str,
    ) -> bool {
        sessions
            .read()
            .await
            .get(session_id)
            .is_some_and(|session| session.active_permit.is_some())
    }

    /// G1.1: a Busy session whose permit was lost (the child-cancelled /
    /// gate-full wait path) re-acquires an active permit before its next LLM
    /// turn.
    #[tokio::test]
    async fn guard_reacquires_lost_permit_for_busy_session() {
        let gate = ConcurrencyGate::new(1, 4, 10, 20);
        let sessions = Arc::new(RwLock::new(HashMap::from([(
            "caller".to_string(),
            agent_session("caller", SessionStatus::Busy),
        )])));

        assert!(matches!(
            ensure_active_permit_for_llm(&gate, &sessions, "caller").await,
            PermitGuard::Proceed
        ));
        assert!(holds_permit(&sessions, "caller").await);
    }

    /// G1.1 boundary: with the gate full the guard queues for the slot
    /// instead of proceeding uncapped; releasing the holder lets it through.
    #[tokio::test]
    async fn guard_queues_until_a_slot_frees_instead_of_proceeding_uncapped() {
        let gate = ConcurrencyGate::new(1, 4, 10, 20);
        let holder = gate.acquire_active_agent().await.unwrap();
        let sessions = Arc::new(RwLock::new(HashMap::from([(
            "caller".to_string(),
            agent_session("caller", SessionStatus::Busy),
        )])));

        // Deterministic: the guard cannot hold a permit while the only slot is
        // taken, so a queued guard must leave the session permitless.
        let gate = Arc::new(gate);
        let gate_for_guard = Arc::clone(&gate);
        let sessions_for_guard = Arc::clone(&sessions);
        let guard = tokio::spawn(async move {
            let guard =
                ensure_active_permit_for_llm(&gate_for_guard, &sessions_for_guard, "caller");
            guard.await
        });
        tokio::task::yield_now().await;
        assert!(
            !holds_permit(&sessions, "caller").await,
            "a queued guard must not hand out a permit while the gate is full"
        );

        drop(holder);
        assert!(matches!(guard.await.unwrap(), PermitGuard::Proceed));
        assert!(holds_permit(&sessions, "caller").await);
    }

    /// G1.1: only a cancelled session continues without a permit — and then
    /// the guard skips its LLM turn.
    #[tokio::test]
    async fn guard_skips_llm_turn_for_cancelled_session_without_permit() {
        let gate = ConcurrencyGate::new(1, 4, 10, 20);
        let _holder = gate.acquire_active_agent().await.unwrap();
        let mut session = agent_session("caller", SessionStatus::Busy);
        session.cancellation_token.cancel();
        let sessions = Arc::new(RwLock::new(HashMap::from([(
            "caller".to_string(),
            session,
        )])));

        assert!(matches!(
            ensure_active_permit_for_llm(&gate, &sessions, "caller").await,
            PermitGuard::Cancelled
        ));
        assert!(!holds_permit(&sessions, "caller").await);
    }

    /// A session that is not `Busy` is left to the existing status checks: the
    /// guard proceeds without touching permits.
    #[tokio::test]
    async fn guard_leaves_non_busy_sessions_to_existing_checks() {
        let gate = ConcurrencyGate::new(1, 4, 10, 20);
        let _holder = gate.acquire_active_agent().await.unwrap();
        let sessions = Arc::new(RwLock::new(HashMap::from([(
            "caller".to_string(),
            agent_session("caller", SessionStatus::Paused),
        )])));

        assert!(matches!(
            ensure_active_permit_for_llm(&gate, &sessions, "caller").await,
            PermitGuard::Proceed
        ));
        assert!(!holds_permit(&sessions, "caller").await);
    }

    /// While the permit is still held the guard is a no-op fast path.
    #[tokio::test]
    async fn guard_is_noop_while_permit_held() {
        let gate = ConcurrencyGate::new(1, 4, 10, 20);
        let permit = gate.acquire_active_agent().await.unwrap();
        let mut session = agent_session("caller", SessionStatus::Busy);
        session.active_permit = Some(permit);
        let sessions = Arc::new(RwLock::new(HashMap::from([(
            "caller".to_string(),
            session,
        )])));

        assert!(matches!(
            ensure_active_permit_for_llm(&gate, &sessions, "caller").await,
            PermitGuard::Proceed
        ));
        assert!(
            holds_permit(&sessions, "caller").await,
            "the guard must not drop a held permit"
        );
    }
}
