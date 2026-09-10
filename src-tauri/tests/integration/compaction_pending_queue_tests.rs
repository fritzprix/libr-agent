use crate::common;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::SystemTime;

use tauri::test::MockRuntime;
use tauri_mcp_agent_lib::agent::compact_recovery::handle_compact_error_state;
use tauri_mcp_agent_lib::agent::concurrency::{
    ConcurrencyGate, DEFAULT_MAX_ACTIVE_AGENTS, DEFAULT_MAX_ACTIVE_PROCESSES,
    DEFAULT_MAX_SUSPENDED_AGENTS, DEFAULT_MAX_SUSPENDED_PROCESSES,
};
use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
use tauri_mcp_agent_lib::agent::events::AgentEventDispatcher;
use tauri_mcp_agent_lib::agent::llm::types::{
    AgentRuntimeError, AgentRuntimeErrorType, CompactStateEvent,
};
use tauri_mcp_agent_lib::agent::session_bus::SessionBus;
use tauri_mcp_agent_lib::agent::state::{
    AgentSession, CompactionKind, CompactionPhase, CompactionResumeAction, CompactionRuntimeState,
    InFlightCompaction, PendingEvent, PendingEventManager,
};
use tauri_mcp_agent_lib::agent::workflow::{
    compact_pending_continue_kind, CompactPendingContinueKind,
};
use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::repositories::{
    InMemorySessionRepository, MessageRepository, PendingQueueRepository, SessionMetadata,
    SessionRepository, SessionStatus, SqliteMessageRepository, SqlitePendingQueueRepository,
    SqliteSessionRepository,
};
use tauri_mcp_agent_lib::{init_concurrency_gate, init_session_bus};
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

static TEST_GUARD: Mutex<()> = Mutex::const_new(());

#[derive(Default)]
struct NoopDispatcher;

impl AgentEventDispatcher for NoopDispatcher {
    fn emit_agent_event(
        &self,
        _event: tauri_mcp_agent_lib::agent::events::AgentEvent,
    ) -> Result<(), String> {
        Ok(())
    }

    fn emit_compact_state(&self, _event: CompactStateEvent) -> Result<(), String> {
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
        name: Some("Compact pending".to_string()),
        status,
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
    }
}

fn build_user_message(session_id: &str, id: &str, created_at: i64) -> Message {
    Message {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: "user".to_string(),
        content: vec![MCPContent::Text {
            text: format!("text-{id}"),
        }],
        tool_calls: None,
        tool_call_id: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: None,
        prompt_tokens: None,
        created_at,
        updated_at: created_at,
        source: None,
        error: None,
        metadata: None,
    }
}

fn build_agent_session(
    metadata: SessionMetadata,
    compaction_phase: CompactionPhase,
) -> AgentSession {
    let last_tail = match &compaction_phase {
        CompactionPhase::InFlight(in_flight) => in_flight.current_tail_id.clone(),
        CompactionPhase::Idle => None,
    };
    AgentSession {
        metadata,
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
        reasoning_budget_retry_count: Arc::new(RwLock::new(0)),
        pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
        pending_approvals: Arc::new(RwLock::new(HashMap::new())),
        context_registry: Arc::new(ContextRegistry::new()),
        compact_context: Arc::new(RwLock::new(None)),
        compaction: CompactionRuntimeState::with_test_state(compaction_phase, last_tail),
        expected_response_id: Arc::new(RwLock::new(None)),
        cached_stable_prompt: Arc::new(RwLock::new(None)),
        last_completion_request: Arc::new(RwLock::new(None)),
        last_submitted_input_message_id: Arc::new(RwLock::new(None)),
        last_session_context_snapshot: Arc::new(RwLock::new(None)),
        session_context_turns_since_force_fresh: Arc::new(RwLock::new(0)),
        tool_loop_resample_attempts: Arc::new(RwLock::new(HashMap::new())),
        tool_poll_trackers: Arc::new(RwLock::new(HashMap::new())),
    }
}

fn mock_app_handle(mock_app: &tauri::App<MockRuntime>) -> &tauri::AppHandle {
    let mock_handle = mock_app.handle();
    unsafe { &*(mock_handle as *const tauri::AppHandle<MockRuntime> as *const tauri::AppHandle) }
}

#[test]
fn preflight_resume_always_continues_even_without_pending() {
    assert_eq!(
        compact_pending_continue_kind(
            &CompactionResumeAction::ResumeCompletion,
            SessionStatus::Busy,
            false,
        ),
        CompactPendingContinueKind::ResumeCompletion
    );
}

#[test]
fn manual_nothing_without_pending_is_noop() {
    assert_eq!(
        compact_pending_continue_kind(&CompactionResumeAction::Nothing, SessionStatus::Idle, false,),
        CompactPendingContinueKind::None
    );
}

#[test]
fn manual_busy_with_pending_leaves_to_running_turn() {
    assert_eq!(
        compact_pending_continue_kind(&CompactionResumeAction::Nothing, SessionStatus::Busy, true,),
        CompactPendingContinueKind::None
    );
}

#[test]
fn manual_queued_with_pending_leaves_to_queued_turn() {
    assert_eq!(
        compact_pending_continue_kind(
            &CompactionResumeAction::Nothing,
            SessionStatus::Queued,
            true,
        ),
        CompactPendingContinueKind::None
    );
}

#[test]
fn manual_idle_with_pending_starts_from_queue() {
    assert_eq!(
        compact_pending_continue_kind(&CompactionResumeAction::Nothing, SessionStatus::Idle, true,),
        CompactPendingContinueKind::StartFromPending
    );
}

#[test]
fn manual_paused_with_pending_starts_from_queue() {
    assert_eq!(
        compact_pending_continue_kind(
            &CompactionResumeAction::Nothing,
            SessionStatus::Paused,
            true,
        ),
        CompactPendingContinueKind::StartFromPending
    );
}

#[test]
fn manual_error_with_pending_leaves_waiters_durable() {
    assert_eq!(
        compact_pending_continue_kind(&CompactionResumeAction::Nothing, SessionStatus::Error, true,),
        CompactPendingContinueKind::None
    );
}

#[test]
fn manual_provisioning_with_pending_leaves_waiters_durable() {
    assert_eq!(
        compact_pending_continue_kind(
            &CompactionResumeAction::Nothing,
            SessionStatus::Provisioning,
            true,
        ),
        CompactPendingContinueKind::None
    );
}

#[tokio::test]
async fn resume_workflow_rejects_while_compaction_in_flight() {
    let _guard = TEST_GUARD.lock().await;
    init_runtime_primitives();

    let session_id = "resume-during-compact";
    let metadata = build_session_metadata(session_id, SessionStatus::Paused);
    let repo = Arc::new(InMemorySessionRepository::new());
    repo.upsert_session(&metadata)
        .await
        .expect("session upsert");
    let session_repo: Arc<dyn SessionRepository> = repo;

    let active_sessions = Arc::new(RwLock::new(HashMap::from([(
        session_id.to_string(),
        build_agent_session(
            metadata,
            CompactionPhase::InFlight(InFlightCompaction {
                kind: CompactionKind::Manual,
                current_tail_id: Some("tail-manual".to_string()),
                started_at_ms: 1_000,
            }),
        ),
    )])));

    let mock_app = tauri::test::mock_app();
    let app_handle = mock_app_handle(&mock_app);
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let session_workspace_manager = Arc::new(
        tauri_mcp_agent_lib::session::SessionManager::new_with_base_dir(
            temp_dir.path().join("session-root"),
        )
        .expect("session manager"),
    );
    let db = common::setup_test_db_with_migrations().await;
    let proxy_manager = Arc::new(tauri_mcp_agent_lib::mcp::MCPServiceProxyManager::new(
        Arc::new(db),
        session_workspace_manager,
    ));

    let err = tauri_mcp_agent_lib::agent::workflow::resume_workflow(
        &session_repo,
        &active_sessions,
        &proxy_manager,
        app_handle,
        session_id.to_string(),
    )
    .await
    .expect_err("resume must fail while compaction is in flight");
    assert!(
        err.contains("compaction is in flight"),
        "unexpected error: {err}"
    );

    let active = active_sessions.read().await;
    let session = active.get(session_id).expect("session");
    assert!(
        session.compaction.snapshot().await.is_in_flight(),
        "resume must not clear in-flight compaction"
    );
    assert_eq!(session.metadata.status, SessionStatus::Paused);
}

#[tokio::test]
async fn user_message_during_manual_compaction_is_enqueued_without_clearing_inflight() {
    let _guard = TEST_GUARD.lock().await;
    let db = common::setup_test_db_with_migrations().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    let message_repo = SqliteMessageRepository::new(db.clone());
    let queue_repo = SqlitePendingQueueRepository::new(db.clone());
    tauri_mcp_agent_lib::set_message_repository(SqliteMessageRepository::new(db.clone()));
    tauri_mcp_agent_lib::set_pending_queue_repository(SqlitePendingQueueRepository::new(
        db.clone(),
    ));
    tauri_mcp_agent_lib::set_session_repository(session_repo.clone());

    let session_id = format!("compact-pending-enqueue-{}", uuid::Uuid::new_v4());
    let metadata = build_session_metadata(&session_id, SessionStatus::Idle);
    session_repo
        .upsert_session(&metadata)
        .await
        .expect("session create");

    let active_sessions = Arc::new(RwLock::new(HashMap::new()));
    active_sessions.write().await.insert(
        session_id.clone(),
        build_agent_session(
            metadata,
            CompactionPhase::InFlight(InFlightCompaction {
                kind: CompactionKind::Manual,
                current_tail_id: Some("tail-manual".to_string()),
                started_at_ms: 1_000,
            }),
        ),
    );

    let mock_app = tauri::test::mock_app();
    let app_handle = mock_app_handle(&mock_app);
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let session_workspace_manager = Arc::new(
        tauri_mcp_agent_lib::session::SessionManager::new_with_base_dir(
            temp_dir.path().join("session-root"),
        )
        .expect("session manager"),
    );
    let proxy_manager = Arc::new(tauri_mcp_agent_lib::mcp::MCPServiceProxyManager::new(
        Arc::new(db),
        session_workspace_manager,
    ));
    let session_repo: Arc<dyn SessionRepository> = Arc::new(session_repo);

    let user_message = build_user_message(&session_id, "queued-during-manual", 2_000);
    tauri_mcp_agent_lib::agent::workflow::start_workflow(
        &session_repo,
        &active_sessions,
        &proxy_manager,
        app_handle,
        session_id.clone(),
        user_message.clone(),
    )
    .await
    .expect("start_workflow should queue during manual compaction");

    let active = active_sessions.read().await;
    let session = active.get(&session_id).expect("session");
    let snapshot = session.compaction.snapshot().await;
    assert!(
        snapshot.is_in_flight(),
        "manual compaction must stay in flight after queued user message"
    );
    assert!(!snapshot.blocks_workflow());
    let pending_ids = session.pending_events.read().await.message_ids();
    assert_eq!(pending_ids, vec!["queued-during-manual".to_string()]);
    assert!(
        session.messages.read().await.is_empty(),
        "queued message must not enter the active cache mid-compaction"
    );
    drop(active);

    let stored = message_repo
        .get_by_id("queued-during-manual")
        .await
        .expect("db lookup")
        .expect("queued message persisted");
    assert_eq!(stored.id, user_message.id);

    let queue_ids = queue_repo
        .list_by_session(&session_id)
        .await
        .expect("queue list")
        .into_iter()
        .map(|entry| entry.message_id)
        .collect::<Vec<_>>();
    assert_eq!(queue_ids, vec!["queued-during-manual".to_string()]);
}

#[tokio::test]
async fn preflight_pending_is_claimed_on_resumed_turn() {
    let _guard = TEST_GUARD.lock().await;
    let db = common::setup_test_db_with_migrations().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    tauri_mcp_agent_lib::set_message_repository(SqliteMessageRepository::new(db.clone()));
    tauri_mcp_agent_lib::set_pending_queue_repository(SqlitePendingQueueRepository::new(
        db.clone(),
    ));
    tauri_mcp_agent_lib::set_session_repository(session_repo.clone());

    let session_id = format!("compact-pending-claim-{}", uuid::Uuid::new_v4());
    let metadata = build_session_metadata(&session_id, SessionStatus::Busy);
    session_repo
        .upsert_session(&metadata)
        .await
        .expect("session create");

    let active_sessions = Arc::new(RwLock::new(HashMap::new()));
    active_sessions.write().await.insert(
        session_id.clone(),
        build_agent_session(
            metadata,
            CompactionPhase::InFlight(InFlightCompaction {
                kind: CompactionKind::Preflight,
                current_tail_id: Some("tail-preflight".to_string()),
                started_at_ms: 1_000,
            }),
        ),
    );

    let mock_app = tauri::test::mock_app();
    let app_handle = mock_app_handle(&mock_app);

    let during_compact = build_user_message(&session_id, "msg-during-preflight", 2_000);
    tauri_mcp_agent_lib::agent::pending_queue::enqueue_pending_user_message(
        &active_sessions,
        app_handle,
        &session_id,
        &during_compact,
    )
    .await
    .expect("enqueue during preflight");

    // Compaction settle decides to resume; the resumed turn claims pending.
    let action = {
        let active = active_sessions.read().await;
        active
            .get(&session_id)
            .expect("session")
            .compaction
            .complete_success()
            .await
    };
    assert!(matches!(action, CompactionResumeAction::ResumeCompletion));
    assert_eq!(
        compact_pending_continue_kind(&action, SessionStatus::Busy, true),
        CompactPendingContinueKind::ResumeCompletion
    );

    let claimed = tauri_mcp_agent_lib::agent::pending_queue::claim_all_pending_messages(
        &active_sessions,
        app_handle,
        &session_id,
    )
    .await
    .expect("claim after resume");
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].id, "msg-during-preflight");

    let active = active_sessions.read().await;
    let session = active.get(&session_id).expect("session");
    assert_eq!(session.pending_events.read().await.count(), 0);
    assert!(session
        .messages
        .read()
        .await
        .iter()
        .any(|message| message.id == "msg-during-preflight"));
}

#[tokio::test]
async fn race_enqueue_after_claim_leaves_remainder_for_next_turn() {
    let _guard = TEST_GUARD.lock().await;
    let db = common::setup_test_db_with_migrations().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    tauri_mcp_agent_lib::set_message_repository(SqliteMessageRepository::new(db.clone()));
    tauri_mcp_agent_lib::set_pending_queue_repository(SqlitePendingQueueRepository::new(
        db.clone(),
    ));
    tauri_mcp_agent_lib::set_session_repository(session_repo.clone());

    let session_id = format!("compact-pending-race-{}", uuid::Uuid::new_v4());
    let metadata = build_session_metadata(&session_id, SessionStatus::Busy);
    session_repo
        .upsert_session(&metadata)
        .await
        .expect("session create");

    let active_sessions = Arc::new(RwLock::new(HashMap::new()));
    active_sessions.write().await.insert(
        session_id.clone(),
        build_agent_session(metadata, CompactionPhase::Idle),
    );

    let mock_app = tauri::test::mock_app();
    let app_handle = mock_app_handle(&mock_app);

    let first = build_user_message(&session_id, "race-first", 1_000);
    tauri_mcp_agent_lib::agent::pending_queue::enqueue_pending_user_message(
        &active_sessions,
        app_handle,
        &session_id,
        &first,
    )
    .await
    .expect("enqueue first");

    let claimed = tauri_mcp_agent_lib::agent::pending_queue::claim_all_pending_messages(
        &active_sessions,
        app_handle,
        &session_id,
    )
    .await
    .expect("claim first batch");
    assert_eq!(claimed.len(), 1);

    // Finish-window / resume race: another prompt arrives after the drain.
    let second = build_user_message(&session_id, "race-second", 2_000);
    tauri_mcp_agent_lib::agent::pending_queue::enqueue_pending_user_message(
        &active_sessions,
        app_handle,
        &session_id,
        &second,
    )
    .await
    .expect("enqueue second after claim");

    let active = active_sessions.read().await;
    let session = active.get(&session_id).expect("session");
    assert_eq!(
        session.pending_events.read().await.message_ids(),
        vec!["race-second".to_string()]
    );
    assert!(
        tauri_mcp_agent_lib::agent::workflow::session_has_pending_events(
            &active_sessions,
            &session_id
        )
        .await
    );
}

#[tokio::test]
async fn blocking_compact_failure_preserves_pending_queue() {
    let _guard = TEST_GUARD.lock().await;
    init_runtime_primitives();

    let session_id = "compact-error-keeps-pending";
    let metadata = build_session_metadata(session_id, SessionStatus::Busy);
    let repo = Arc::new(InMemorySessionRepository::new());
    repo.upsert_session(&metadata)
        .await
        .expect("session upsert");
    let session_repo: Arc<dyn SessionRepository> = repo.clone();

    let session = build_agent_session(
        metadata,
        CompactionPhase::InFlight(InFlightCompaction {
            kind: CompactionKind::Preflight,
            current_tail_id: Some("tail-before-error".to_string()),
            started_at_ms: 1234,
        }),
    );
    session
        .pending_events
        .write()
        .await
        .add(PendingEvent::Message("survivor".to_string()));

    let active_sessions = Arc::new(RwLock::new(HashMap::from([(
        session_id.to_string(),
        session,
    )])));
    let dispatcher = NoopDispatcher;
    let error = AgentRuntimeError::new(AgentRuntimeErrorType::RateLimitError, "LLM rate limit hit");

    handle_compact_error_state(
        &session_repo,
        &active_sessions,
        &dispatcher,
        session_id.to_string(),
        error,
    )
    .await
    .expect("compact error handling");

    let active = active_sessions.read().await;
    let session = active.get(session_id).expect("session");
    assert_eq!(session.metadata.status, SessionStatus::Error);
    assert_eq!(
        session.pending_events.read().await.message_ids(),
        vec!["survivor".to_string()],
        "blocking compact failure must not discard pending waiters"
    );
    assert!(matches!(
        session.compaction.snapshot().await.phase,
        CompactionPhase::Idle
    ));
}
