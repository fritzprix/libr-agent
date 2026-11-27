use super::messages::{
    user_facing_persist_session_missing_error, user_facing_persist_storage_error,
    user_facing_workflow_event_emit_error,
};
use super::persist::{
    latest_assistant_message_from_cache, mark_error_after_persist_failure,
    transition_to_terminal_status_and_emit_with_hook_sink, TerminalTransition,
};
use super::stop_hook::{evaluate_stop_hook_outcome, settle_stub_event, StopHookTransition};
use super::*;
use crate::agent::context::registry::ContextRegistry;
use crate::agent::events::AgentEventDispatcher;
use crate::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType};
use crate::agent::state::{CompactionRuntimeState, PendingEvent, PendingEventManager};
use crate::repositories::{
    DbError, InMemorySessionRepository, SessionListCursor, SessionListPage, SessionMetadata,
    SessionRepository, SessionStatus,
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

    fn emit_completion_cancel(
        &self,
        _request: crate::agent::llm::types::CompletionCancelRequest,
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
        status_reason: None,
        forked_from: None,
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

    fn emit_completion_cancel(
        &self,
        _request: crate::agent::llm::types::CompletionCancelRequest,
    ) -> Result<(), String> {
        Ok(())
    }
}

/// `SessionRepository` double that always fails status writes (#3071
/// G2.1) while delegating everything else to an in-memory store.
struct StatusWriteFailingRepository {
    inner: InMemorySessionRepository,
}

impl StatusWriteFailingRepository {
    fn new() -> Self {
        Self {
            inner: InMemorySessionRepository::new(),
        }
    }
}

#[async_trait::async_trait]
impl SessionRepository for StatusWriteFailingRepository {
    async fn upsert_session(&self, session: &SessionMetadata) -> Result<(), DbError> {
        self.inner.upsert_session(session).await
    }

    async fn get_session(&self, session_id: &str) -> Result<Option<SessionMetadata>, DbError> {
        self.inner.get_session(session_id).await
    }

    async fn update_status(
        &self,
        _session_id: &str,
        _status: SessionStatus,
        _reason: Option<&str>,
    ) -> Result<(), DbError> {
        Err(DbError::TransactionFailed(
            "test double: status write always fails".to_string(),
        ))
    }

    async fn update_session_config(
        &self,
        session_id: &str,
        model: Option<String>,
        provider: Option<String>,
    ) -> Result<(), DbError> {
        self.inner
            .update_session_config(session_id, model, provider)
            .await
    }

    async fn update_assistant_id(
        &self,
        session_id: &str,
        assistant_id: Option<String>,
    ) -> Result<(), DbError> {
        self.inner
            .update_assistant_id(session_id, assistant_id)
            .await
    }

    async fn update_name(&self, session_id: &str, name: String) -> Result<(), DbError> {
        self.inner.update_name(session_id, name).await
    }

    async fn get_all_sessions(&self) -> Result<Vec<SessionMetadata>, DbError> {
        self.inner.get_all_sessions().await
    }

    async fn get_all_session_ids(&self) -> Result<Vec<String>, DbError> {
        self.inner.get_all_session_ids().await
    }

    async fn list_sessions(
        &self,
        cursor: Option<SessionListCursor>,
        limit: u64,
        search: Option<&str>,
        bookmarked_only: bool,
        status: Option<&str>,
    ) -> Result<SessionListPage, DbError> {
        self.inner
            .list_sessions(cursor, limit, search, bookmarked_only, status)
            .await
    }

    async fn list_attention_sessions(&self) -> Result<Vec<SessionMetadata>, DbError> {
        self.inner.list_attention_sessions().await
    }

    async fn get_child_session_ids(&self, parent_session_id: &str) -> Result<Vec<String>, DbError> {
        self.inner.get_child_session_ids(parent_session_id).await
    }

    async fn get_child_sessions(
        &self,
        parent_session_id: &str,
    ) -> Result<Vec<SessionMetadata>, DbError> {
        self.inner.get_child_sessions(parent_session_id).await
    }

    async fn get_sessions_by_org_id(&self, org_id: &str) -> Result<Vec<SessionMetadata>, DbError> {
        self.inner.get_sessions_by_org_id(org_id).await
    }

    async fn delete_session(&self, session_id: &str) -> Result<(), DbError> {
        self.inner.delete_session(session_id).await
    }

    async fn orphan_and_delete_session(&self, session_id: &str) -> Result<(), DbError> {
        self.inner.orphan_and_delete_session(session_id).await
    }

    async fn toggle_bookmark(&self, session_id: &str, bookmarked: bool) -> Result<(), DbError> {
        self.inner.toggle_bookmark(session_id, bookmarked).await
    }

    async fn update_execution_mode(
        &self,
        session_id: &str,
        mode: crate::execution_mode::ExecutionMode,
    ) -> Result<(), DbError> {
        self.inner.update_execution_mode(session_id, mode).await
    }

    async fn update_workspace_override(
        &self,
        session_id: &str,
        override_path: Option<String>,
    ) -> Result<(), DbError> {
        self.inner
            .update_workspace_override(session_id, override_path)
            .await
    }

    async fn update_org_identity(
        &self,
        session_id: &str,
        org_id: Option<String>,
        org_name: Option<String>,
        org_root_session_id: Option<String>,
    ) -> Result<(), DbError> {
        self.inner
            .update_org_identity(session_id, org_id, org_name, org_root_session_id)
            .await
    }

    async fn update_last_viewed_at(
        &self,
        session_id: &str,
        last_viewed_at: i64,
    ) -> Result<(), DbError> {
        self.inner
            .update_last_viewed_at(session_id, last_viewed_at)
            .await
    }

    async fn update_attention(
        &self,
        session_id: &str,
        last_attention_at: i64,
        reason: crate::repositories::session_repository::SessionAttentionReason,
    ) -> Result<(), DbError> {
        self.inner
            .update_attention(session_id, last_attention_at, reason)
            .await
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
            workspace_isolation: crate::models::workspace_isolation::WorkspaceIsolationMode::Host,
            docker_config: None,
            docker_container_name: None,
            docker_host_workspace_path: None,
            status_reason: None,
            forked_from: None,
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
        reasoning_budget_retry_count: Arc::new(RwLock::new(0)),
        malformed_tool_markup_retry_count: Arc::new(RwLock::new(0)),
        invalid_tool_args_retry_count: Arc::new(RwLock::new(0)),
        pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
        pending_approvals: Arc::new(RwLock::new(HashMap::new())),
        pending_user_prompt: crate::agent::state::new_user_prompt_slot(),
        context_registry: Arc::new(ContextRegistry::new()),
        compact_context: Arc::new(RwLock::new(None)),
        compaction: CompactionRuntimeState::new(),
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

#[test]
fn settle_stub_event_picks_stop_for_top_level_session() {
    assert_eq!(
        settle_stub_event(None),
        crate::services::hooks::HookEvent::Stop
    );
}

#[test]
fn settle_stub_event_picks_subagent_stop_when_parent_set() {
    assert_eq!(
        settle_stub_event(Some("parent-session")),
        crate::services::hooks::HookEvent::SubagentStop
    );
}

#[test]
fn evaluate_stop_hook_outcome_continues_up_to_cap_then_settles() {
    use crate::services::hooks::{
        clear_hook_context, take_turn_hook_context, HookDecision, HookDispatchResult, HookEvent,
        MAX_STOP_HOOK_CONTINUATIONS,
    };

    let sid = "finish-stop-hook-cap-test";
    clear_hook_context(sid);

    for i in 1..=MAX_STOP_HOOK_CONTINUATIONS {
        let cont = evaluate_stop_hook_outcome(
            sid,
            HookEvent::Stop,
            HookDispatchResult {
                additional_context: vec![],
                decision: Some(HookDecision::Block),
                reason: Some(format!("keep going #{i}")),
                updated_input: None,
                stop_reason: None,
                stops_processing: false,
                display_content: None,
            },
        );
        assert_eq!(
            cont,
            StopHookTransition::Continue,
            "continuation #{i} must be allowed"
        );
        let ctx = take_turn_hook_context(sid).expect("feedback queued");
        assert!(ctx.contains(&format!("keep going #{i}")));
    }

    // Next block exceeds MAX_STOP_HOOK_CONTINUATIONS -> must settle to Idle.
    let over_cap = evaluate_stop_hook_outcome(
        sid,
        HookEvent::Stop,
        HookDispatchResult {
            additional_context: vec![],
            decision: Some(HookDecision::Block),
            reason: Some("one too many".to_string()),
            updated_input: None,
            stop_reason: None,
            stops_processing: false,
            display_content: None,
        },
    );
    assert_eq!(
        over_cap,
        StopHookTransition::Settle,
        "must not continue past MAX_STOP_HOOK_CONTINUATIONS"
    );
    assert!(take_turn_hook_context(sid).is_none());
    clear_hook_context(sid);
}
#[test]
fn stop_hook_continue_false_settles_with_user_visible_reason_even_with_block() {
    use crate::services::hooks::{
        clear_hook_context, take_turn_hook_context, HookDecision, HookDispatchResult, HookEvent,
    };

    let sid = "finish-stop-hook-stop-test";
    clear_hook_context(sid);
    let transition = evaluate_stop_hook_outcome(
        sid,
        HookEvent::Stop,
        HookDispatchResult {
            additional_context: vec![],
            decision: Some(HookDecision::Block),
            reason: Some("keep going".to_string()),
            updated_input: None,
            stop_reason: Some("budget exhausted".to_string()),
            stops_processing: true,
            display_content: None,
        },
    );
    assert_eq!(
        transition,
        StopHookTransition::StoppedByHook("budget exhausted".to_string()),
        "continue:false must settle the turn and surface stopReason"
    );
    assert!(
        take_turn_hook_context(sid).is_none(),
        "a stopped turn must not queue continuation feedback"
    );
    assert_eq!(
        crate::services::hooks::stop_hook_continuation_count(sid),
        0,
        "stopping must not consume the continuation budget"
    );
    clear_hook_context(sid);
}

#[test]
fn stop_failure_error_type_labels_are_stable() {
    // #3007: `StopFailure` matchers compare against these labels, so
    // renames are a breaking change for plugin configs.
    assert_eq!(
        error_type_label(&AgentRuntimeErrorType::RateLimitError),
        "rate_limit"
    );
    assert_eq!(
        error_type_label(&AgentRuntimeErrorType::AuthenticationError),
        "authentication"
    );
    assert_eq!(
        error_type_label(&AgentRuntimeErrorType::ContextLimitError),
        "context_limit"
    );
    assert_eq!(error_type_label(&AgentRuntimeErrorType::McpError), "mcp");
    assert_eq!(
        error_type_label(&AgentRuntimeErrorType::EmptySelectionError),
        "empty_selection"
    );
}

#[tokio::test]
async fn finalize_session_error_emits_workflow_error_through_shared_helper() {
    // #3071 G1.1/G1.4: Error settlement must go through the shared helper
    // (status Error + WorkflowError). Removing the helper call breaks this.
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
        "sess-finalize-error",
        SessionStatus::Busy,
    )
    .await;

    let dispatcher = RecordingDispatcher::default();
    let error = AgentRuntimeError::new(AgentRuntimeErrorType::RateLimitError, "rate limited");
    let mut stop_failure_recorded = None;
    finalize_session_error_with_hook_sink(
        &session_repo,
        &active_sessions,
        &dispatcher,
        "sess-finalize-error",
        error,
        |_sessions, sid, error_type| {
            stop_failure_recorded = Some((sid, error_type));
        },
    )
    .await
    .expect("finalize");

    assert_eq!(
        session_repo
            .get_session("sess-finalize-error")
            .await
            .expect("load")
            .expect("exists")
            .status,
        SessionStatus::Error
    );
    let events = dispatcher.agent_events();
    assert!(
        events.iter().any(|e| matches!(
            e,
            AgentEvent::WorkflowError { session_id, error }
                if session_id == "sess-finalize-error"
                    && matches!(
                        error.error_type,
                        AgentRuntimeErrorType::RateLimitError
                    )
        )),
        "WorkflowError must be emitted before StopFailure is spawned: {events:?}"
    );
    assert_eq!(
        stop_failure_recorded,
        Some(("sess-finalize-error".to_string(), "rate_limit")),
        "StopFailure must be dispatched from the shared Error settlement path"
    );
}

#[tokio::test]
async fn settle_to_idle_does_not_leave_session_in_error() {
    // #3071 G1.4: normal Idle settlement must not use the Error/StopFailure path.
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
        "sess-idle-no-stop-failure",
        SessionStatus::Busy,
    )
    .await;

    let dispatcher = RecordingDispatcher::default();
    let mut stop_failure_emitted = false;
    transition_to_terminal_status_and_emit_with_hook_sink(
        &session_repo,
        &active_sessions,
        &dispatcher,
        "sess-idle-no-stop-failure",
        TerminalTransition {
            status: SessionStatus::Idle,
            event: AgentEvent::WorkflowCompleted {
                session_id: "sess-idle-no-stop-failure".to_string(),
                reason: WorkflowCompletionReason::Natural,
                stop_reason: None,
            },
            event_label: "WorkflowCompleted",
        },
        |_sessions, _sid, _error_type| {
            stop_failure_emitted = true;
        },
    )
    .await
    .expect("idle settle");

    assert_eq!(
        session_repo
            .get_session("sess-idle-no-stop-failure")
            .await
            .expect("load")
            .expect("exists")
            .status,
        SessionStatus::Idle
    );
    assert!(
        !dispatcher
            .agent_events()
            .iter()
            .any(|e| matches!(e, AgentEvent::WorkflowError { .. })),
        "Idle settlement must not emit WorkflowError"
    );
    assert!(
        !stop_failure_emitted,
        "Idle settlement must not dispatch StopFailure"
    );
}

#[tokio::test]
async fn finalize_session_error_emits_workflow_error_when_status_write_fails() {
    // #3071 G2.1 / G3.1: the WorkflowError emission and StopFailure dispatch
    // must not depend on the persisted status write succeeding, observed
    // through an injected sink with no test branch in production dispatch.
    crate::init_session_bus(crate::agent::session_bus::SessionBus::new());
    crate::state::init_concurrency_gate(crate::agent::concurrency::ConcurrencyGate::new(
        crate::agent::concurrency::DEFAULT_MAX_ACTIVE_AGENTS,
        crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_AGENTS,
        crate::agent::concurrency::DEFAULT_MAX_ACTIVE_PROCESSES,
        crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_PROCESSES,
    ));

    let session_repo = Arc::new(StatusWriteFailingRepository::new()) as Arc<dyn SessionRepository>;
    let active_sessions = Arc::new(RwLock::new(HashMap::new()));
    seed_in_memory_session(
        &session_repo,
        &active_sessions,
        "sess-finalize-error-write-fail",
        SessionStatus::Busy,
    )
    .await;

    let dispatcher = RecordingDispatcher::default();
    let error = AgentRuntimeError::new(AgentRuntimeErrorType::NetworkError, "network down");
    let mut stop_failure_recorded = None;
    finalize_session_error_with_hook_sink(
        &session_repo,
        &active_sessions,
        &dispatcher,
        "sess-finalize-error-write-fail",
        error,
        |_sessions, sid, error_type| {
            stop_failure_recorded = Some((sid, error_type));
        },
    )
    .await
    .expect("Error settlement must complete despite a failed status write");

    let events = dispatcher.agent_events();
    assert!(
        events.iter().any(|e| matches!(
            e,
            AgentEvent::WorkflowError { session_id, .. }
                if session_id == "sess-finalize-error-write-fail"
        )),
        "WorkflowError must be emitted even when the Error status write fails: {events:?}"
    );
    assert_eq!(
        stop_failure_recorded,
        Some(("sess-finalize-error-write-fail".to_string(), "network")),
        "StopFailure must be dispatched even when the Error status write fails"
    );
}
