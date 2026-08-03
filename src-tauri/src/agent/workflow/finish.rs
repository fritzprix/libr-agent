//! Centralized workflow settlement gate.
//!
//! All natural-completion and workflow-error finalization paths converge here so
//! terminal assistant messages are durable before parents or the UI observe
//! `Idle` / `Error`.
//!
//! See `docs/architecture/workflow-settlement.md` for the finish-window TOCTOU
//! scenario and why the success path performs two pending-event scans.

use crate::agent::events::{AgentEvent, AgentEventDispatcher, WorkflowCompletionReason};
use crate::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType};
use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::{SessionRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

fn latest_assistant_message_from_cache(messages: &[Message]) -> Option<Message> {
    messages
        .iter()
        .rev()
        .find(|message| message.role == "assistant")
        .cloned()
}

fn user_facing_persist_session_missing_error() -> String {
    "The session could not be found while saving the final response.".to_string()
}

fn user_facing_persist_storage_error() -> String {
    "The final response could not be saved. The session has been marked as failed.".to_string()
}

fn user_facing_workflow_event_emit_error(event_label: &str) -> String {
    format!(
        "The session state was updated, but the UI could not be notified ({event_label}). \
         Refresh or check the session status manually."
    )
}

fn emit_settlement_agent_event(
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    event: AgentEvent,
    event_label: &str,
) -> Result<(), String> {
    dispatcher.emit_agent_event(event).map_err(|error| {
        log::error!(
            "Failed to emit {event_label} for session {session_id}: {error}. \
                 Session status was already updated; StatusChanged should have reached the UI."
        );
        user_facing_workflow_event_emit_error(event_label)
    })
}

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
///
/// This is invoked twice on the success settlement path: once inside
/// [`settle_before_terminal_transition`] and again immediately before writing
/// `Idle`. Between those two scans another task can enqueue into
/// `pending_events` (the finish-window race). The second scan closes that
/// window without introducing a new terminal state machine.
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

/// Persist the terminal assistant message synchronously before exposing `Idle`.
///
/// When `terminal_message` is provided it is written directly; otherwise the latest
/// assistant row from the in-memory cache is used.
pub async fn persist_terminal_assistant_sync(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    terminal_message: Option<&Message>,
) -> Result<(), String> {
    let message_to_persist = if let Some(message) = terminal_message {
        Some(message.clone())
    } else {
        let sessions = active_sessions.read().await;
        let session = sessions.get(session_id).ok_or_else(|| {
            log::error!(
                "Terminal persist cache lookup failed: session {} is not in active_sessions",
                session_id
            );
            user_facing_persist_session_missing_error()
        })?;
        let messages = session.messages.read().await;
        latest_assistant_message_from_cache(&messages)
    };

    let Some(message) = message_to_persist else {
        return Ok(());
    };

    let repo = crate::state::get_message_repository();
    repo.insert(&message).await.map_err(|error| {
        log::error!(
            "Failed to persist terminal assistant message: session={}, msg_id={}, error={}",
            session_id,
            message.id,
            error
        );
        user_facing_persist_storage_error()
    })
}

/// Persist terminal messages and run the first pending-work scan.
///
/// Returns `Ok(true)` when pending events restarted the workflow. The success
/// path still performs a second pending scan in
/// [`settle_session_and_go_idle_with_dispatcher`] to close the finish-window
/// race that can occur after this function returns.
pub async fn settle_before_terminal_transition(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: &str,
    terminal_message: Option<&Message>,
) -> Result<bool, String> {
    persist_terminal_assistant_sync(active_sessions, session_id, terminal_message).await?;

    continue_workflow_if_pending_events(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        session_id,
    )
    .await
}

async fn mark_error_after_persist_failure(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    user_facing_error: &str,
) -> Result<(), String> {
    log::error!(
        "Terminal persist failed for session {}. Transitioning to Error. user_message={}",
        session_id,
        user_facing_error
    );
    crate::agent::lifecycle::update_session_status_with_dispatcher(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        SessionStatus::Error,
    )
    .await?;

    let workflow_error =
        AgentRuntimeError::new(AgentRuntimeErrorType::AiServiceError, user_facing_error)
            .with_code("TERMINAL_PERSIST_FAILED");

    if let Err(emit_error) = dispatcher.emit_agent_event(AgentEvent::WorkflowError {
        session_id: session_id.to_string(),
        error: workflow_error,
    }) {
        log::error!(
            "Session {} is Error after terminal persist failure, but WorkflowError failed to emit: {}. \
             StatusChanged should still have reached the UI.",
            session_id,
            emit_error
        );
    }

    Ok(())
}

async fn handle_terminal_persist_failure(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    persist_error: String,
) -> Result<(), String> {
    if let Err(mark_error) = mark_error_after_persist_failure(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        &persist_error,
    )
    .await
    {
        log::error!(
            "Persist failed for session {} and marking Error also failed: {}",
            session_id,
            mark_error
        );
    }
    Err(persist_error)
}

async fn transition_to_terminal_status_and_emit(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    status: SessionStatus,
    event: AgentEvent,
    event_label: &str,
) -> Result<(), String> {
    crate::agent::lifecycle::update_session_status_with_dispatcher(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        status,
    )
    .await?;

    emit_settlement_agent_event(dispatcher, session_id, event, event_label)
}

/// Finish a workflow only after terminal messages are durable and no pending work remains.
///
/// Returns `Ok(true)` when pending events restarted the workflow (caller must not treat
/// the session as terminal). Returns `Ok(false)` when the session settled to `Idle`.
#[allow(clippy::too_many_arguments)]
pub async fn settle_session_and_go_idle_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    terminal_message: Option<&Message>,
    completion_reason: WorkflowCompletionReason,
) -> Result<bool, String> {
    let workflow_restarted = match settle_before_terminal_transition(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        session_id,
        terminal_message,
    )
    .await
    {
        Ok(restarted) => restarted,
        Err(error) => {
            return handle_terminal_persist_failure(
                session_repo,
                active_sessions,
                dispatcher,
                session_id,
                error,
            )
            .await
            .map(|()| false);
        }
    };

    if workflow_restarted {
        return Ok(true);
    }

    // Finish-window guard: a producer can enqueue into `pending_events` after
    // `settle_before_terminal_transition` returns `Ok(false)` but before we write
    // `Idle`. Without this second scan, parents polling `checkSession(wait=true)`
    // could observe an empty terminal result while another LLM turn is required.
    if continue_workflow_if_pending_events(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        session_id,
    )
    .await?
    {
        return Ok(true);
    }

    transition_to_terminal_status_and_emit(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        SessionStatus::Idle,
        AgentEvent::WorkflowCompleted {
            session_id: session_id.to_string(),
            reason: completion_reason,
        },
        "WorkflowCompleted",
    )
    .await?;

    Ok(false)
}

/// Finish a workflow only after terminal messages are durable and no pending work remains.
///
/// Returns `Ok(true)` when pending events restarted the workflow (caller must not treat
/// the session as terminal). Returns `Ok(false)` when the session settled to `Idle`.
pub async fn settle_session_and_go_idle(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: &str,
    terminal_message: Option<&Message>,
    completion_reason: WorkflowCompletionReason,
) -> Result<bool, String> {
    let dispatcher = crate::agent::tauri_events::TauriEventDispatcher::new(app_handle.clone());
    settle_session_and_go_idle_with_dispatcher(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        &dispatcher,
        session_id,
        terminal_message,
        completion_reason,
    )
    .await
}

/// Finalize a failed workflow only after terminal messages are durable.
///
/// Unlike the success settlement path, this does not re-check `pending_events`.
/// Doing so would create an async recursion cycle through `handle_llm_error_with_outcome`.
pub async fn settle_session_and_finalize_error_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    terminal_message: Option<&Message>,
    error: AgentRuntimeError,
) -> Result<(), String> {
    if let Err(persist_error) =
        persist_terminal_assistant_sync(active_sessions, session_id, terminal_message).await
    {
        return handle_terminal_persist_failure(
            session_repo,
            active_sessions,
            dispatcher,
            session_id,
            persist_error,
        )
        .await;
    }

    transition_to_terminal_status_and_emit(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        SessionStatus::Error,
        AgentEvent::WorkflowError {
            session_id: session_id.to_string(),
            error,
        },
        "WorkflowError",
    )
    .await
}

/// Finalize a failed workflow only after terminal messages are durable.
///
/// Unlike the success settlement path, this does not re-check `pending_events`.
/// Doing so would create an async recursion cycle through `handle_llm_error_with_outcome`.
pub async fn settle_session_and_finalize_error(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    terminal_message: Option<&Message>,
    error: AgentRuntimeError,
) -> Result<(), String> {
    let dispatcher = crate::agent::tauri_events::TauriEventDispatcher::new(app_handle.clone());
    settle_session_and_finalize_error_with_dispatcher(
        session_repo,
        active_sessions,
        &dispatcher,
        session_id,
        terminal_message,
        error,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::context::registry::ContextRegistry;
    use crate::agent::events::AgentEventDispatcher;
    use crate::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType};
    use crate::agent::state::{CompactionRuntimeState, PendingEvent, PendingEventManager};
    use crate::repositories::{
        InMemorySessionRepository, SessionMetadata, SessionRepository, SessionStatus,
    };
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};
    use std::time::SystemTime;
    use tokio_util::sync::CancellationToken;

    #[derive(Default)]
    struct WorkflowEventFailingDispatcher {
        fail_workflow_events: bool,
        events: Mutex<Vec<AgentEvent>>,
    }

    impl AgentEventDispatcher for WorkflowEventFailingDispatcher {
        fn emit_agent_event(&self, event: AgentEvent) -> Result<(), String> {
            if self.fail_workflow_events
                && matches!(
                    event,
                    AgentEvent::WorkflowCompleted { .. } | AgentEvent::WorkflowError { .. }
                )
            {
                return Err("event emission failed".to_string());
            }
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

    async fn seed_in_memory_session(
        session_repo: &Arc<dyn SessionRepository>,
        active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
        session_id: &str,
        status: SessionStatus,
    ) {
        let metadata = SessionMetadata {
            id: session_id.to_string(),
            name: None,
            status: status.clone(),
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
            created_at: 1,
            updated_at: 1,
            last_viewed_at: None,
            last_message_at: None,
            last_attention_at: None,
            last_attention_reason: None,
            is_bookmarked: false,
            execution_mode: crate::execution_mode::ExecutionMode::Normal,
            workspace_override: None,
            workspace_isolation: crate::models::workspace_isolation::WorkspaceIsolationMode::Host,
            docker_config: None,
            docker_container_name: None,
            docker_host_workspace_path: None,
        };
        session_repo
            .upsert_session(&metadata)
            .await
            .expect("session upsert");
        active_sessions
            .write()
            .await
            .insert(session_id.to_string(), build_session(session_id));
    }

    #[derive(Default)]
    struct RecordingDispatcher {
        events: Mutex<Vec<AgentEvent>>,
    }

    impl RecordingDispatcher {
        fn agent_events(&self) -> Vec<AgentEvent> {
            self.events.lock().expect("event lock").clone()
        }
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

    #[test]
    fn user_facing_persist_errors_hide_internal_details() {
        assert_eq!(
            user_facing_persist_session_missing_error(),
            "The session could not be found while saving the final response."
        );
        assert!(!user_facing_persist_storage_error().contains("session="));
        assert!(!user_facing_workflow_event_emit_error("WorkflowCompleted").contains("emit"));
    }

    #[tokio::test]
    async fn mark_error_after_persist_failure_emits_workflow_error() {
        crate::init_session_bus(crate::agent::session_bus::SessionBus::new());
        crate::state::init_concurrency_gate(crate::agent::concurrency::ConcurrencyGate::new(
            crate::agent::concurrency::DEFAULT_MAX_ACTIVE_AGENTS,
            crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_AGENTS,
            crate::agent::concurrency::DEFAULT_MAX_ACTIVE_PROCESSES,
            crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_PROCESSES,
        ));

        let session_repo = Arc::new(InMemorySessionRepository::new()) as Arc<dyn SessionRepository>;
        let active_sessions = Arc::new(RwLock::new(HashMap::new()));
        seed_in_memory_session(
            &session_repo,
            &active_sessions,
            "sess-persist-mark-error",
            SessionStatus::Busy,
        )
        .await;

        let dispatcher = RecordingDispatcher::default();
        let user_message = user_facing_persist_storage_error();
        mark_error_after_persist_failure(
            &session_repo,
            &active_sessions,
            &dispatcher,
            "sess-persist-mark-error",
            &user_message,
        )
        .await
        .expect("mark error succeeds");

        assert_eq!(
            session_repo
                .get_session("sess-persist-mark-error")
                .await
                .expect("session load")
                .expect("session exists")
                .status,
            SessionStatus::Error
        );
        assert!(dispatcher.agent_events().iter().any(|event| {
            matches!(
                event,
                AgentEvent::WorkflowError { session_id, error }
                    if session_id == "sess-persist-mark-error"
                        && error.display_message == user_message
            )
        }));
    }

    #[tokio::test]
    async fn settle_session_and_finalize_error_with_dispatcher_propagates_emit_failure() {
        crate::init_session_bus(crate::agent::session_bus::SessionBus::new());
        crate::state::init_concurrency_gate(crate::agent::concurrency::ConcurrencyGate::new(
            crate::agent::concurrency::DEFAULT_MAX_ACTIVE_AGENTS,
            crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_AGENTS,
            crate::agent::concurrency::DEFAULT_MAX_ACTIVE_PROCESSES,
            crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_PROCESSES,
        ));

        let session_repo = Arc::new(InMemorySessionRepository::new()) as Arc<dyn SessionRepository>;
        let active_sessions = Arc::new(RwLock::new(HashMap::new()));
        seed_in_memory_session(
            &session_repo,
            &active_sessions,
            "sess-emit-failure",
            SessionStatus::Busy,
        )
        .await;

        let dispatcher = WorkflowEventFailingDispatcher {
            fail_workflow_events: true,
            ..Default::default()
        };
        let runtime_error = AgentRuntimeError::new(
            AgentRuntimeErrorType::AiServiceError,
            "Provider request failed",
        );

        let result = settle_session_and_finalize_error_with_dispatcher(
            &session_repo,
            &active_sessions,
            &dispatcher,
            "sess-emit-failure",
            None,
            runtime_error,
        )
        .await;

        assert_eq!(
            result,
            Err(user_facing_workflow_event_emit_error("WorkflowError"))
        );
        assert_eq!(
            session_repo
                .get_session("sess-emit-failure")
                .await
                .expect("session load")
                .expect("session exists")
                .status,
            SessionStatus::Error
        );
    }

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

    #[test]
    fn latest_assistant_message_from_cache_returns_newest_assistant() {
        use crate::mcp::types::MCPContent;
        use crate::models::chat::Message;

        fn assistant_message(id: &str, text: &str) -> Message {
            Message {
                id: id.to_string(),
                session_id: "sess".to_string(),
                role: "assistant".to_string(),
                content: vec![MCPContent::Text {
                    text: text.to_string(),
                }],
                tool_calls: None,
                tool_call_id: None,
                is_streaming: None,
                thinking: None,
                thinking_signature: None,
                assistant_id: None,
                attachments: None,
                tool_use: None,
                usage: None,
                prompt_tokens: None,
                created_at: 1,
                updated_at: 1,
                source: None,
                error: None,
                metadata: None,
            }
        }

        let messages = vec![
            assistant_message("asst-1", "first"),
            Message {
                id: "tool-1".to_string(),
                session_id: "sess".to_string(),
                role: "tool".to_string(),
                content: vec![MCPContent::Text {
                    text: "tool output".to_string(),
                }],
                tool_calls: None,
                tool_call_id: None,
                is_streaming: None,
                thinking: None,
                thinking_signature: None,
                assistant_id: None,
                attachments: None,
                tool_use: None,
                usage: None,
                prompt_tokens: None,
                created_at: 2,
                updated_at: 2,
                source: None,
                error: None,
                metadata: None,
            },
            assistant_message("asst-2", "final answer"),
        ];

        let latest = latest_assistant_message_from_cache(&messages).expect("assistant");
        assert_eq!(latest.id, "asst-2");
        let MCPContent::Text { text, .. } = &latest.content[0] else {
            panic!("expected text content");
        };
        assert_eq!(text, "final answer");
    }
}
