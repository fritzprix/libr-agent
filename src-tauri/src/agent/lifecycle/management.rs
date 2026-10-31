use crate::agent::concurrency::ActiveAgentPermit;
use crate::agent::context::registry::ContextRegistry;
use crate::agent::events::{AgentEvent, AgentEventDispatcher};
use crate::agent::state::{AgentSession, SessionStatusTransition};
use crate::agent::tauri_events::TauriEventDispatcher;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::{SessionMetadata, SessionStatus};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

/// Resume an existing session by loading it into active sessions
#[allow(dead_code)]
pub async fn resume_session(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    _context_registry: Arc<ContextRegistry>,
    session_id: &str,
) -> Result<SessionMetadata, String> {
    // Get session metadata from database using injected repository
    let session = session_repo
        .get_session(session_id)
        .await
        .map_err(|e| format!("Failed to get session: {}", e))?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    if let Some(workspace_override) = session.workspace_override.as_deref() {
        let path = PathBuf::from(workspace_override);
        if path.is_dir() {
            if let Ok(session_manager) = crate::session::get_session_manager() {
                if let Err(e) = session_manager
                    .register_session_override(session_id, path)
                    .await
                {
                    log::warn!(
                        "Failed to pre-register workspace override for resumed session {}: {}",
                        session_id,
                        e
                    );
                }
            }
        } else {
            // Keep the preference — transient path unavailability must not wipe it.
            log::warn!(
                "Persisted workspace override '{}' for session {} is unavailable or not a directory; \
                 keeping the preference and falling back to the default workspace until it returns.",
                workspace_override,
                session_id
            );
        }
    }

    if session.workspace_isolation
        == crate::models::workspace_isolation::WorkspaceIsolationMode::Docker
    {
        if session.status == SessionStatus::Provisioning {
            crate::services::docker_provisioning::ensure_provisioning_task_for_session(
                &crate::services::docker_provisioning::DockerProvisioningDeps {
                    session_repo: Arc::clone(session_repo),
                    active_sessions: Arc::clone(active_sessions),
                    proxy_manager: Arc::clone(proxy_manager),
                    app_handle: app_handle.clone(),
                },
                &session,
            )
            .await?;
        } else if let Err(error) =
            crate::services::WorkspaceRuntimeManager::ensure_runtime_cached(&session).await
        {
            // Store this failure: an older reason may name a different cause.
            let reason =
                crate::services::docker_provisioning::failure_reason(&session, &error.to_string());
            let dispatcher = TauriEventDispatcher::new(app_handle.clone());
            if let Err(record_error) = record_session_error_with_dispatcher(
                session_repo,
                active_sessions,
                &dispatcher,
                session_id,
                &reason,
            )
            .await
            {
                log::warn!(
                    "Failed to record Docker runtime failure for session {}: {}",
                    session_id,
                    record_error
                );
            }
            return Err(reason);
        }
    }

    // Deserialize agent config (live assistant settings + session lineage)
    // SSOT: MCP activation is session → assistant → mcpServerIds.
    let (tool_ids, mcp_server_ids) = crate::agent::resolve_session_mcp_bindings(&session).await?;

    // Load compact context if exists (SP17)
    let compact_context_record =
        crate::agent::lifecycle::load_compact_context_record(session_id, "session resume").await?;

    // Activate session in memory first so open/resume can return without waiting on MCP.
    let mut active = active_sessions.write().await;
    if let Some(existing_session) = active.get_mut(session_id) {
        log::info!(
            "Session {} already active in memory, updating metadata only",
            session_id
        );
        existing_session.metadata = session.clone();
        // Invalidate cached stable prompt — metadata (assistant binding, name) may have
        // changed between sessions, so it must be rebuilt on the next LLM call.
        *existing_session.cached_stable_prompt.write().await = None;
        // Update compact context if it was loaded
        crate::agent::lifecycle::set_compact_context_if_loaded(
            existing_session,
            compact_context_record,
        )
        .await;
        // Transient states (pending_execution, messages, cancellation_token, etc.) are preserved
    } else {
        log::info!(
            "Session {} not in memory, initializing new active session state",
            session_id
        );
        // Resume historically initializes an empty registry (caller arg unused).
        active.insert(
            session_id.to_string(),
            AgentSession::new(
                session.clone(),
                Arc::new(ContextRegistry::new()),
                compact_context_record,
            ),
        );
    }
    drop(active);

    // MCP proxy creation (incl. HTTP startup) must not block session UI.
    // Workflow soft-waits via ensure_proxy_ready; discovery surfaces via runtime-state events.
    crate::agent::lifecycle::spawn_create_proxy_for_session(
        Arc::clone(proxy_manager),
        app_handle.clone(),
        session_id.to_string(),
        tool_ids,
        mcp_server_ids,
    )
    .await;

    log::info!("Resumed agent session: {}", session_id);
    Ok(session)
}

/// Update model/provider (and optionally assistant binding) for an existing session
#[allow(clippy::too_many_arguments)]
pub async fn update_session_config(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    _app_handle: &AppHandle,
    session_id: &str,
    model: Option<String>,
    provider: Option<String>,
    assistant_id: Option<String>,
    recursive: Option<bool>,
) -> Result<(), String> {
    let mut session_ids = vec![session_id.to_string()];

    if recursive.unwrap_or(false) {
        let descendant_ids =
            crate::services::SessionCleanupService::collect_descendant_ids(session_id).await?;
        session_ids.extend(descendant_ids);
    }

    for id in &session_ids {
        session_repo
            .update_session_config(id, model.clone(), provider.clone())
            .await
            .map_err(|e| format!("Failed to update session config for {}: {}", id, e))?;

        if id == session_id {
            if let Some(ref assistant_id) = assistant_id {
                session_repo
                    .update_assistant_id(id, Some(assistant_id.clone()))
                    .await
                    .map_err(|e| {
                        format!("Failed to update session assistant_id for {}: {}", id, e)
                    })?;
            }
        }
    }

    let mut active = active_sessions.write().await;
    let now = chrono::Utc::now().timestamp_millis();
    for id in &session_ids {
        if let Some(session) = active.get_mut(id) {
            if let Some(ref m) = model {
                session.metadata.model = m.clone();
            }
            if let Some(ref p) = provider {
                session.metadata.provider = p.clone();
            }
            if id == session_id {
                if let Some(ref id) = assistant_id {
                    session.metadata.assistant_id = Some(id.clone());
                }
            }
            session.metadata.updated_at = now;
            *session.cached_stable_prompt.write().await = None;
        }
    }
    drop(active);

    // Emit resource update event to trigger frontend session list refresh
    crate::agent::tauri_events::emit_resource_updated(
        "session",
        "update",
        Some(session_id.to_string()),
    );

    log::info!(
        "Updated agent config for session: {} (recursive: {:?})",
        session_id,
        recursive
    );

    Ok(())
}

/// Update session status in database and emit event
pub async fn update_session_status(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    status: SessionStatus,
) -> Result<(), String> {
    let dispatcher = TauriEventDispatcher::new(app_handle.clone());
    update_session_status_with_dispatcher(
        session_repo,
        active_sessions,
        &dispatcher,
        session_id,
        status,
    )
    .await
}

/// Like [`update_session_status`], and stores why the session has the new status.
pub async fn update_session_status_with_reason(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    status: SessionStatus,
    reason: &str,
) -> Result<(), String> {
    let dispatcher = TauriEventDispatcher::new(app_handle.clone());
    apply_session_status(
        session_repo,
        active_sessions,
        &dispatcher,
        session_id,
        status,
        Some(reason),
        TransitionGuard::Always,
    )
    .await
    .map(|_| ())
}

/// Sets `Error` with `reason` for a session that may not be loaded in memory (for example when
/// loading it failed). Loaded sessions take the normal status transition. A `Busy` session is
/// not changed: its running workflow owns the status and the active agent permit.
pub async fn record_session_error_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    reason: &str,
) -> Result<(), String> {
    {
        // Keep the lock until the write: a concurrent resume must not load the session between
        // the check and the write.
        let active = active_sessions.write().await;
        match active.get(session_id) {
            Some(session) if session.metadata.status == SessionStatus::Busy => {
                log::info!(
                    "Session {} is running; not setting Error for: {}",
                    session_id,
                    reason
                );
                return Ok(());
            }
            Some(_) => {}
            None => {
                session_repo
                    .update_status(session_id, SessionStatus::Error, Some(reason))
                    .await
                    .map_err(|error| format!("Failed to update session status: {}", error))?;
                drop(active);
                return emit_unloaded_session_error(dispatcher, session_id, reason);
            }
        }
    }

    apply_session_status(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        SessionStatus::Error,
        Some(reason),
        TransitionGuard::UnlessBusy,
    )
    .await
    .map(|_| ())
}

fn emit_unloaded_session_error(
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    reason: &str,
) -> Result<(), String> {
    wake_status_waiters(session_id);
    dispatcher.emit_agent_event(AgentEvent::StatusChanged {
        session_id: session_id.to_string(),
        status: SessionStatus::Error,
        status_reason: Some(reason.to_string()),
    })
}

pub async fn update_session_status_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    status: SessionStatus,
) -> Result<(), String> {
    apply_session_status(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        status,
        None,
        TransitionGuard::Always,
    )
    .await
    .map(|_| ())
}

/// Moves a `Queued` session to `Busy` for the task that holds its active agent permit. Returns
/// `Ok(false)` and drops that permit when the session is no longer `Queued` (for example a failed
/// resume set `Error` meanwhile); its status and reason stay, and the work must not start.
pub async fn promote_queued_session_to_busy(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
) -> Result<bool, String> {
    let dispatcher = TauriEventDispatcher::new(app_handle.clone());
    promote_queued_session_to_busy_with_dispatcher(
        session_repo,
        active_sessions,
        &dispatcher,
        session_id,
    )
    .await
}

pub async fn promote_queued_session_to_busy_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
) -> Result<bool, String> {
    apply_session_status(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        SessionStatus::Busy,
        None,
        TransitionGuard::OnlyFromQueued,
    )
    .await
}

/// Condition that [`apply_session_status`] checks under the session's transition lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransitionGuard {
    Always,
    /// A `Busy` session keeps its status and permit.
    UnlessBusy,
    /// Only a `Queued` session changes. Otherwise the permit of a session that is not `Busy`
    /// is given back.
    OnlyFromQueued,
}

/// Returns `Ok(false)` without a change when `guard` does not allow it for the status that the
/// session has once the transition lock is held.
async fn apply_session_status(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    status: SessionStatus,
    reason: Option<&str>,
    guard: TransitionGuard,
) -> Result<bool, String> {
    // Acquire concurrency permits OUTSIDE `transition_lock`. Waiting on a full
    // gate while holding the lock deadlocks cancel/clear for the same session
    // (and blocks every other status transition for it).
    let mut pre_acquired_permit: Option<ActiveAgentPermit> = None;
    {
        let active = active_sessions.read().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        let needs_permit = session.metadata.status != SessionStatus::Busy
            && status == SessionStatus::Busy
            && session.active_permit.is_none();
        if needs_permit {
            drop(active);
            let gate = crate::state::get_concurrency_gate();
            pre_acquired_permit = Some(gate.acquire_active_agent().await?);
        }
    }

    let (initial_status, transition_lock, transition_handle) = {
        let active = active_sessions.read().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        (
            session.metadata.status.clone(),
            Arc::clone(&session.transition_lock),
            Arc::clone(&session.status_transition),
        )
    };
    let _transition_guard = transition_lock.lock().await;

    let prev_status = {
        let active = active_sessions.read().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        if session.metadata.status != initial_status {
            session.metadata.status.clone()
        } else {
            initial_status
        }
    };
    let allowed = match guard {
        TransitionGuard::Always => true,
        TransitionGuard::UnlessBusy => prev_status != SessionStatus::Busy,
        TransitionGuard::OnlyFromQueued => prev_status == SessionStatus::Queued,
    };
    if !allowed {
        if guard == TransitionGuard::OnlyFromQueued && prev_status != SessionStatus::Busy {
            // Take the permit while the transition lock is held: the status cannot go back to
            // `Queued` meanwhile, and a start task stores a permit only on a `Queued` session.
            let mut active = active_sessions.write().await;
            if let Some(session) = active.get_mut(session_id) {
                session.active_permit.take();
            }
        }
        *transition_handle.write().await = None;
        log::info!(
            "Session {} is {:?}; not changing its status to {:?}",
            session_id,
            prev_status,
            status
        );
        return Ok(false);
    }
    let is_prev_busy = prev_status == SessionStatus::Busy;
    let is_next_busy = status == SessionStatus::Busy;
    let mut acquired_permit: Option<ActiveAgentPermit> = pre_acquired_permit.take();
    let mut released_permit: Option<ActiveAgentPermit> = None;

    *transition_handle.write().await = Some(SessionStatusTransition::ToStatus(status.clone()));

    if !matches!((is_prev_busy, is_next_busy), (false, false) | (true, true)) {
        match (is_prev_busy, is_next_busy) {
            (false, true) => {
                let active = active_sessions.read().await;
                let has_permit = active
                    .get(session_id)
                    .and_then(|s| s.active_permit.as_ref())
                    .is_some();
                if has_permit {
                    if let Some(permit) = acquired_permit.take() {
                        drop(permit);
                    }
                } else if acquired_permit.is_none() {
                    *transition_handle.write().await = None;
                    return Err(format!(
                        "Cannot transition session {} to Busy without an active agent permit",
                        session_id
                    ));
                }
            }
            (true, false) => {
                if let Some(permit) = acquired_permit.take() {
                    drop(permit);
                }
                let mut active = active_sessions.write().await;
                let session = active
                    .get_mut(session_id)
                    .ok_or_else(|| format!("Session not found: {}", session_id))?;
                released_permit = session.active_permit.take();
            }
            _ => {}
        }
    } else if let Some(permit) = acquired_permit.take() {
        drop(permit);
    }

    let persist_result = session_repo
        .update_status(session_id, status.clone(), reason)
        .await;

    if let Err(error) = persist_result {
        let mut active = active_sessions.write().await;
        if let Some(session) = active.get_mut(session_id) {
            if let Some(permit) = acquired_permit.take() {
                drop(permit);
            }
            if let Some(permit) = released_permit.take() {
                session.active_permit = Some(permit);
            }
        }
        *transition_handle.write().await = None;
        return Err(format!("Failed to update session status: {}", error));
    }

    // Update in-memory state
    let mut active = active_sessions.write().await;
    if let Some(session) = active.get_mut(session_id) {
        if let Some(permit) = acquired_permit.take() {
            session.active_permit = Some(permit);
        }
        session.metadata.status = status.clone();
        session.metadata.status_reason = reason.map(str::to_string);
        // SP3: Update is_running based on status.
        // Busy means the workflow is active/running.
        session.is_running = status == SessionStatus::Busy;
    }
    *transition_handle.write().await = None;
    drop(active); // Release write lock before waking waiters.

    // SP1: Wake any tasks sleeping on this session's status change (e.g. awaitAgent).
    wake_status_waiters(session_id);

    // Emit status changed event
    let event = AgentEvent::StatusChanged {
        session_id: session_id.to_string(),
        status,
        status_reason: reason.map(str::to_string),
    };
    dispatcher.emit_agent_event(event)?;

    Ok(true)
}

fn wake_status_waiters(session_id: &str) {
    match crate::state::try_get_session_bus() {
        Some(bus) => bus.notify_status_change(session_id),
        None => log::warn!(
            "SessionBus not initialized; status waiters for session {} are not woken",
            session_id
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::InMemorySessionRepository;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingDispatcher {
        events: Mutex<Vec<AgentEvent>>,
    }

    impl AgentEventDispatcher for RecordingDispatcher {
        fn emit_agent_event(&self, event: AgentEvent) -> Result<(), String> {
            self.events.lock().expect("event lock").push(event);
            Ok(())
        }

        fn emit_compact_state(
            &self,
            _event: crate::agent::llm::types::CompactStateEvent,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn a_later_failure_replaces_the_old_status_reason_of_an_unloaded_session() {
        let repo: Arc<dyn SessionRepository> = Arc::new(InMemorySessionRepository::new());
        let session = SessionMetadata {
            status: SessionStatus::Error,
            status_reason: Some("container libr-old was removed".to_string()),
            ..SessionMetadata::test_fixture("errored-session")
        };
        repo.upsert_session(&session).await.expect("upsert");
        let active_sessions = Arc::new(RwLock::new(HashMap::new()));
        let dispatcher = RecordingDispatcher::default();

        let reason = crate::services::docker_provisioning::failure_reason(
            &session,
            "Cannot connect to the Docker daemon",
        );
        record_session_error_with_dispatcher(
            &repo,
            &active_sessions,
            &dispatcher,
            "errored-session",
            &reason,
        )
        .await
        .expect("record failure");

        let stored = repo
            .get_session("errored-session")
            .await
            .expect("get")
            .expect("session");
        assert_eq!(stored.status, SessionStatus::Error);
        assert_eq!(stored.status_reason.as_deref(), Some(reason.as_str()));
        assert!(reason.contains("Cannot connect to the Docker daemon"));

        let events = dispatcher.events.lock().expect("event lock").clone();
        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::StatusChanged {
                session_id,
                status,
                status_reason,
            } => {
                assert_eq!(session_id, "errored-session");
                assert_eq!(*status, SessionStatus::Error);
                assert_eq!(status_reason.as_deref(), Some(reason.as_str()));
            }
            other => panic!("unexpected event: {:?}", other),
        }
    }

    async fn running_session(
        session_id: &str,
        gate: &crate::agent::concurrency::ConcurrencyGate,
    ) -> (
        Arc<dyn SessionRepository>,
        Arc<RwLock<HashMap<String, AgentSession>>>,
    ) {
        let repo: Arc<dyn SessionRepository> = Arc::new(InMemorySessionRepository::new());
        let session = SessionMetadata {
            status: SessionStatus::Busy,
            ..SessionMetadata::test_fixture(session_id)
        };
        repo.upsert_session(&session).await.expect("upsert");
        let mut loaded = AgentSession::new(session, Arc::new(ContextRegistry::new()), None);
        loaded.is_running = true;
        loaded.active_permit = Some(gate.acquire_active_agent().await.expect("permit"));
        let active_sessions = Arc::new(RwLock::new(HashMap::from([(
            session_id.to_string(),
            loaded,
        )])));
        (repo, active_sessions)
    }

    async fn assert_still_running(
        repo: &Arc<dyn SessionRepository>,
        active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
        dispatcher: &RecordingDispatcher,
        gate: &crate::agent::concurrency::ConcurrencyGate,
        session_id: &str,
    ) {
        let stored = repo
            .get_session(session_id)
            .await
            .expect("get")
            .expect("session");
        assert_eq!(stored.status, SessionStatus::Busy);
        assert_eq!(stored.status_reason, None);
        {
            let active = active_sessions.read().await;
            let loaded = active.get(session_id).expect("loaded session");
            assert_eq!(loaded.metadata.status, SessionStatus::Busy);
            assert!(loaded.is_running);
            assert!(loaded.active_permit.is_some());
            assert!(loaded.status_transition.read().await.is_none());
        }
        // The gate has one slot, so a second permit is only free if the session lost its own.
        let second_permit = tokio::time::timeout(
            std::time::Duration::from_millis(20),
            gate.acquire_active_agent(),
        )
        .await;
        assert!(second_permit.is_err(), "the session's permit was released");
        assert!(dispatcher.events.lock().expect("event lock").is_empty());
    }

    #[tokio::test]
    async fn a_failure_does_not_change_a_running_session() {
        let gate = crate::agent::concurrency::ConcurrencyGate::new(1, 1, 1, 1);
        let (repo, active_sessions) = running_session("running-session", &gate).await;
        let dispatcher = RecordingDispatcher::default();

        record_session_error_with_dispatcher(
            &repo,
            &active_sessions,
            &dispatcher,
            "running-session",
            "No such container: libr-gone",
        )
        .await
        .expect("record failure");

        assert_still_running(
            &repo,
            &active_sessions,
            &dispatcher,
            &gate,
            "running-session",
        )
        .await;
    }

    #[tokio::test]
    async fn unless_busy_leaves_a_session_that_is_busy_under_the_transition_lock() {
        let gate = crate::agent::concurrency::ConcurrencyGate::new(1, 1, 1, 1);
        let (repo, active_sessions) = running_session("busy-session", &gate).await;
        let dispatcher = RecordingDispatcher::default();

        apply_session_status(
            &repo,
            &active_sessions,
            &dispatcher,
            "busy-session",
            SessionStatus::Error,
            Some("No such container: libr-gone"),
            TransitionGuard::UnlessBusy,
        )
        .await
        .expect("apply status");

        assert_still_running(&repo, &active_sessions, &dispatcher, &gate, "busy-session").await;
    }

    async fn queued_session_holding_its_permit(
        session_id: &str,
        gate: &crate::agent::concurrency::ConcurrencyGate,
    ) -> (
        Arc<dyn SessionRepository>,
        Arc<RwLock<HashMap<String, AgentSession>>>,
    ) {
        let repo: Arc<dyn SessionRepository> = Arc::new(InMemorySessionRepository::new());
        let session = SessionMetadata {
            status: SessionStatus::Queued,
            ..SessionMetadata::test_fixture(session_id)
        };
        repo.upsert_session(&session).await.expect("upsert");
        let mut loaded = AgentSession::new(session, Arc::new(ContextRegistry::new()), None);
        // The start task has passed its `Queued` check and stored its permit.
        loaded.active_permit = Some(gate.acquire_active_agent().await.expect("permit"));
        let active_sessions = Arc::new(RwLock::new(HashMap::from([(
            session_id.to_string(),
            loaded,
        )])));
        (repo, active_sessions)
    }

    #[tokio::test]
    async fn an_error_written_after_the_queued_check_is_not_replaced_by_busy() {
        let gate = crate::agent::concurrency::ConcurrencyGate::new(1, 1, 1, 1);
        let (repo, active_sessions) =
            queued_session_holding_its_permit("queued-session", &gate).await;
        let reason = "Docker runtime unavailable: No such container: libr-gone";
        let transition_lock = {
            let active = active_sessions.read().await;
            Arc::clone(
                &active
                    .get("queued-session")
                    .expect("loaded session")
                    .transition_lock,
            )
        };
        let held = transition_lock.lock().await;

        let dispatcher = Arc::new(RecordingDispatcher::default());
        let promotion = tokio::spawn({
            let repo = Arc::clone(&repo);
            let active_sessions = Arc::clone(&active_sessions);
            let dispatcher = Arc::clone(&dispatcher);
            async move {
                promote_queued_session_to_busy_with_dispatcher(
                    &repo,
                    &active_sessions,
                    dispatcher.as_ref(),
                    "queued-session",
                )
                .await
            }
        });
        // The promotion clones the transition lock under the same read lock in which it reads
        // `Queued`, and keeps the clone while it waits: one more reference shows both.
        // Only a limit that stops a hang; the promotion normally waits after a few yields.
        const PROMOTION_WAIT_LIMIT: std::time::Duration = std::time::Duration::from_secs(5);
        let references_before = Arc::strong_count(&transition_lock);
        tokio::time::timeout(PROMOTION_WAIT_LIMIT, async {
            while Arc::strong_count(&transition_lock) == references_before {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the promotion reads the status and waits for the transition lock");
        assert!(!promotion.is_finished());

        // A failed resume writes Error before the Busy write.
        repo.update_status("queued-session", SessionStatus::Error, Some(reason))
            .await
            .expect("write error");
        {
            let mut active = active_sessions.write().await;
            let session = active.get_mut("queued-session").expect("loaded session");
            session.metadata.status = SessionStatus::Error;
            session.metadata.status_reason = Some(reason.to_string());
        }
        drop(held);

        let promoted = promotion.await.expect("join").expect("promote");

        assert!(!promoted, "the work must not start");
        let stored = repo
            .get_session("queued-session")
            .await
            .expect("get")
            .expect("session");
        assert_eq!(stored.status, SessionStatus::Error);
        assert_eq!(stored.status_reason.as_deref(), Some(reason));
        {
            let active = active_sessions.read().await;
            let loaded = active.get("queued-session").expect("loaded session");
            assert_eq!(loaded.metadata.status, SessionStatus::Error);
            assert_eq!(loaded.metadata.status_reason.as_deref(), Some(reason));
            assert!(!loaded.is_running);
            assert!(
                loaded.active_permit.is_none(),
                "the start task's permit is given back"
            );
            assert!(loaded.status_transition.read().await.is_none());
        }
        assert!(dispatcher.events.lock().expect("event lock").is_empty());
        let freed = tokio::time::timeout(
            std::time::Duration::from_millis(20),
            gate.acquire_active_agent(),
        )
        .await;
        assert!(freed.is_ok(), "the gate slot must be free again");
    }

    #[tokio::test]
    async fn a_queued_session_is_promoted_to_busy_with_its_permit() {
        let gate = crate::agent::concurrency::ConcurrencyGate::new(1, 1, 1, 1);
        let (repo, active_sessions) =
            queued_session_holding_its_permit("promoted-session", &gate).await;
        let dispatcher = RecordingDispatcher::default();

        let promoted = promote_queued_session_to_busy_with_dispatcher(
            &repo,
            &active_sessions,
            &dispatcher,
            "promoted-session",
        )
        .await
        .expect("promote");

        assert!(promoted);
        let stored = repo
            .get_session("promoted-session")
            .await
            .expect("get")
            .expect("session");
        assert_eq!(stored.status, SessionStatus::Busy);
        let active = active_sessions.read().await;
        let loaded = active.get("promoted-session").expect("loaded session");
        assert_eq!(loaded.metadata.status, SessionStatus::Busy);
        assert!(loaded.is_running);
        assert!(loaded.active_permit.is_some());
        let events = dispatcher.events.lock().expect("event lock").clone();
        assert!(matches!(
            events.as_slice(),
            [AgentEvent::StatusChanged {
                status: SessionStatus::Busy,
                ..
            }]
        ));
    }
}
