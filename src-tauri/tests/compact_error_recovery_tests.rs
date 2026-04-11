use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use tauri_mcp_agent_lib::agent::concurrency::{
    ConcurrencyGate, DEFAULT_MAX_ACTIVE_AGENTS, DEFAULT_MAX_ACTIVE_PROCESSES,
    DEFAULT_MAX_SUSPENDED_AGENTS, DEFAULT_MAX_SUSPENDED_PROCESSES,
};
use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
use tauri_mcp_agent_lib::agent::events::{AgentEvent, AgentEventDispatcher};
use tauri_mcp_agent_lib::agent::llm::types::{
    AgentRuntimeError, AgentRuntimeErrorType, CompactStateEvent, CompactStatePhase,
};
use tauri_mcp_agent_lib::agent::session_bus::SessionBus;
use tauri_mcp_agent_lib::agent::session_manager::handle_compact_error_with_dispatcher;
use tauri_mcp_agent_lib::agent::state::{AgentSession, PendingEventManager};
use tauri_mcp_agent_lib::repositories::{
    InMemorySessionRepository, SessionMetadata, SessionRepository, SessionStatus,
};
use tauri_mcp_agent_lib::{init_concurrency_gate, init_session_bus};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct RecordingDispatcher {
    compact_states: Mutex<Vec<CompactStateEvent>>,
    agent_events: Mutex<Vec<AgentEvent>>,
}

impl RecordingDispatcher {
    fn compact_states(&self) -> Vec<CompactStateEvent> {
        self.compact_states
            .lock()
            .expect("compact state lock poisoned")
            .clone()
    }

    fn agent_events(&self) -> Vec<AgentEvent> {
        self.agent_events
            .lock()
            .expect("agent event lock poisoned")
            .clone()
    }
}

impl AgentEventDispatcher for RecordingDispatcher {
    fn emit_agent_event(&self, event: AgentEvent) -> Result<(), String> {
        self.agent_events
            .lock()
            .expect("agent event lock poisoned")
            .push(event);
        Ok(())
    }

    fn emit_compact_state(&self, event: CompactStateEvent) -> Result<(), String> {
        self.compact_states
            .lock()
            .expect("compact state lock poisoned")
            .push(event);
        Ok(())
    }
}

fn init_runtime_primitives() {
    init_session_bus(SessionBus::new());
    init_concurrency_gate(ConcurrencyGate::new(
        DEFAULT_MAX_ACTIVE_AGENTS,
        DEFAULT_MAX_SUSPENDED_AGENTS,
        DEFAULT_MAX_ACTIVE_PROCESSES,
        DEFAULT_MAX_SUSPENDED_PROCESSES,
    ));
}

fn build_session_metadata(session_id: &str, status: SessionStatus) -> SessionMetadata {
    let now = chrono::Utc::now().timestamp_millis();
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Compact recovery".to_string()),
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
        workspace_override: None,
    }
}

fn build_agent_session(
    metadata: SessionMetadata,
    awaiting_compact_completion: bool,
) -> AgentSession {
    AgentSession {
        metadata,
        is_running: true,
        active_permit: None,
        status_transition: Arc::new(RwLock::new(None)),
        transition_lock: Arc::new(tokio::sync::Mutex::new(())),
        cancellation_token: CancellationToken::new(),
        yolo_mode: Arc::new(AtomicBool::new(false)),
        cancel_pending: Arc::new(AtomicBool::new(false)),
        pending_execution: None,
        messages: Arc::new(RwLock::new(Vec::new())),
        cache_initialized: Arc::new(AtomicBool::new(true)),
        last_synced_at: Arc::new(RwLock::new(Some(SystemTime::now()))),
        thinking_only_count: Arc::new(RwLock::new(0)),
        pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
        pending_approvals: Arc::new(RwLock::new(HashMap::new())),
        context_registry: Arc::new(ContextRegistry::new()),
        compact_context: Arc::new(RwLock::new(None)),
        compact_in_flight: Arc::new(AtomicBool::new(true)),
        last_compacted_tail_id: Arc::new(RwLock::new(Some("tail-before-error".to_string()))),
        awaiting_compact_completion: Arc::new(AtomicBool::new(awaiting_compact_completion)),
        finalize_workflow_after_compact: Arc::new(AtomicBool::new(false)),
        deferred_workflow_step: Arc::new(RwLock::new(None)),
        compact_started_at_ms: Arc::new(RwLock::new(None)),
        expected_response_id: Arc::new(RwLock::new(None)),
        cached_stable_prompt: Arc::new(RwLock::new(None)),
        last_completion_request: Arc::new(RwLock::new(None)),
    }
}

#[tokio::test]
async fn preflight_compact_failure_transitions_workflow_to_error() {
    init_runtime_primitives();

    let session_id = "compact-error-awaiting";
    let metadata = build_session_metadata(session_id, SessionStatus::Busy);
    let repo = Arc::new(InMemorySessionRepository::new());
    repo.upsert_session(&metadata)
        .await
        .expect("session upsert should succeed");
    let session_repo: Arc<dyn SessionRepository> = repo.clone();

    let active_sessions = Arc::new(RwLock::new(HashMap::from([(
        session_id.to_string(),
        build_agent_session(metadata.clone(), true),
    )])));
    let dispatcher = RecordingDispatcher::default();
    let error = AgentRuntimeError::new(AgentRuntimeErrorType::RateLimitError, "LLM rate limit hit");

    handle_compact_error_with_dispatcher(
        &session_repo,
        &active_sessions,
        &dispatcher,
        session_id.to_string(),
        error.clone(),
    )
    .await
    .expect("compact error handling should succeed");

    let persisted = repo
        .get_session(session_id)
        .await
        .expect("session lookup should succeed")
        .expect("session should exist");
    assert_eq!(persisted.status, SessionStatus::Error);

    let active = active_sessions.read().await;
    let session = active.get(session_id).expect("active session should exist");
    assert_eq!(session.metadata.status, SessionStatus::Error);
    assert!(!session
        .compact_in_flight
        .load(std::sync::atomic::Ordering::SeqCst));
    assert!(!session
        .awaiting_compact_completion
        .load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(*session.last_compacted_tail_id.read().await, None);
    drop(active);

    let compact_states = dispatcher.compact_states();
    assert_eq!(compact_states.len(), 1);
    let compact_state = &compact_states[0];
    assert_eq!(compact_state.session_id, session_id);
    assert_eq!(
        compact_state.session_name.as_deref(),
        Some("Compact recovery")
    );
    assert!(!compact_state.compacting);
    assert!(matches!(&compact_state.phase, CompactStatePhase::Failed));
    assert_eq!(
        compact_state.error.as_deref(),
        Some(error.display_message.as_str())
    );

    let agent_events = dispatcher.agent_events();
    assert_eq!(agent_events.len(), 2);
    assert!(matches!(
        &agent_events[0],
        AgentEvent::StatusChanged {
            session_id: emitted_session_id,
            status: SessionStatus::Error,
        } if emitted_session_id == session_id
    ));
    assert!(matches!(
        &agent_events[1],
        AgentEvent::WorkflowError {
            session_id: emitted_session_id,
            error: emitted_error,
        } if emitted_session_id == session_id && emitted_error.display_message == error.display_message
    ));
}

#[tokio::test]
async fn background_compact_failure_clears_flags_without_failing_workflow() {
    init_runtime_primitives();

    let session_id = "compact-error-background";
    let metadata = build_session_metadata(session_id, SessionStatus::Busy);
    let repo = Arc::new(InMemorySessionRepository::new());
    repo.upsert_session(&metadata)
        .await
        .expect("session upsert should succeed");
    let session_repo: Arc<dyn SessionRepository> = repo.clone();

    let active_sessions = Arc::new(RwLock::new(HashMap::from([(
        session_id.to_string(),
        build_agent_session(metadata.clone(), false),
    )])));
    let dispatcher = RecordingDispatcher::default();

    handle_compact_error_with_dispatcher(
        &session_repo,
        &active_sessions,
        &dispatcher,
        session_id.to_string(),
        AgentRuntimeError::new(AgentRuntimeErrorType::AiServiceError, "summary call failed"),
    )
    .await
    .expect("compact error handling should succeed");

    let persisted = repo
        .get_session(session_id)
        .await
        .expect("session lookup should succeed")
        .expect("session should exist");
    assert_eq!(persisted.status, SessionStatus::Busy);

    let active = active_sessions.read().await;
    let session = active.get(session_id).expect("active session should exist");
    assert_eq!(session.metadata.status, SessionStatus::Busy);
    assert!(!session
        .compact_in_flight
        .load(std::sync::atomic::Ordering::SeqCst));
    assert!(!session
        .awaiting_compact_completion
        .load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(*session.last_compacted_tail_id.read().await, None);
    drop(active);

    assert_eq!(dispatcher.compact_states().len(), 1);
    assert!(dispatcher.agent_events().is_empty());
}
