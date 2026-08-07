use crate::common;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use tauri::test::MockRuntime;
use tauri_mcp_agent_lib::agent::concurrency::{
    ConcurrencyGate, DEFAULT_MAX_ACTIVE_AGENTS, DEFAULT_MAX_ACTIVE_PROCESSES,
    DEFAULT_MAX_SUSPENDED_AGENTS, DEFAULT_MAX_SUSPENDED_PROCESSES,
};
use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
use tauri_mcp_agent_lib::agent::events::{
    AgentEvent, AgentEventDispatcher, WorkflowCompletionReason,
};
use tauri_mcp_agent_lib::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType};
use tauri_mcp_agent_lib::agent::session_bus::SessionBus;
use tauri_mcp_agent_lib::agent::state::{
    AgentSession, CompactionRuntimeState, PendingEvent, PendingEventManager,
};
use tauri_mcp_agent_lib::agent::workflow::{
    continue_workflow_if_pending_events, persist_terminal_assistant_sync,
    session_has_pending_events, settle_session_and_finalize_error_with_dispatcher,
    settle_session_and_go_idle_with_dispatcher,
};
use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::mcp::builtin::agent::utils::{
    fetch_session_messages_for_result, latest_session_output,
};
use tauri_mcp_agent_lib::mcp::service_proxy_manager::MCPServiceProxyManager;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::repositories::{
    MessageRepository, SessionMetadata, SessionRepository, SessionStatus, SqliteMessageRepository,
    SqliteSessionRepository, SqliteSettingsRepository,
};
use tauri_mcp_agent_lib::utils::session_id::StorageSessionId;
use tauri_mcp_agent_lib::{init_concurrency_gate, init_session_bus, set_settings_repository};
use tokio::sync::{Mutex as AsyncMutex, RwLock};
use tokio_util::sync::CancellationToken;

static TEST_GUARD: AsyncMutex<()> = AsyncMutex::const_new(());

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
        _event: tauri_mcp_agent_lib::agent::llm::types::CompactStateEvent,
    ) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Default)]
struct WorkflowEventFailingDispatcher {
    fail_workflow_events: bool,
    events: Mutex<Vec<AgentEvent>>,
}

impl WorkflowEventFailingDispatcher {
    fn agent_events(&self) -> Vec<AgentEvent> {
        self.events.lock().expect("event lock").clone()
    }
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
        _event: tauri_mcp_agent_lib::agent::llm::types::CompactStateEvent,
    ) -> Result<(), String> {
        Ok(())
    }
}

struct SettlementHarness {
    mock_app: tauri::App<MockRuntime>,
    session_id: String,
    session_repo: Arc<dyn SessionRepository>,
    message_repo: SqliteMessageRepository,
    active_sessions: Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: Arc<MCPServiceProxyManager>,
    dispatcher: RecordingDispatcher,
}

impl SettlementHarness {
    fn app_handle(&self) -> &tauri::AppHandle {
        unsafe {
            &*(self.mock_app.handle() as *const tauri::AppHandle<MockRuntime>
                as *const tauri::AppHandle)
        }
    }

    async fn new(session_id: &str, status: SessionStatus) -> Self {
        let _guard = TEST_GUARD.lock().await;

        let db = common::setup_test_db_with_migrations().await;

        init_session_bus(SessionBus::new());
        init_concurrency_gate(ConcurrencyGate::new(
            DEFAULT_MAX_ACTIVE_AGENTS,
            DEFAULT_MAX_SUSPENDED_AGENTS,
            DEFAULT_MAX_ACTIVE_PROCESSES,
            DEFAULT_MAX_SUSPENDED_PROCESSES,
        ));
        set_settings_repository(SqliteSettingsRepository::new(db.clone()));

        let message_repo = SqliteMessageRepository::new(db.clone());
        let sqlite_session_repo = SqliteSessionRepository::new(db.clone());

        tauri_mcp_agent_lib::set_message_repository(SqliteMessageRepository::new(db.clone()));
        tauri_mcp_agent_lib::set_session_repository(sqlite_session_repo.clone());

        let session_repo = Arc::new(sqlite_session_repo.clone()) as Arc<dyn SessionRepository>;
        session_repo
            .upsert_session(&build_session_metadata(session_id, status.clone()))
            .await
            .expect("session created");

        let active_sessions = Arc::new(RwLock::new(HashMap::new()));
        let session = build_agent_session(session_id, status);
        active_sessions
            .write()
            .await
            .insert(session_id.to_string(), session);

        let temp_dir = tempfile::TempDir::new().expect("temp dir");
        let session_workspace_manager = Arc::new(
            tauri_mcp_agent_lib::session::SessionManager::new_with_base_dir(
                temp_dir.path().join("session-root"),
            )
            .expect("session manager"),
        );
        let proxy_manager = Arc::new(MCPServiceProxyManager::new(
            Arc::new(db),
            session_workspace_manager,
        ));

        Self {
            mock_app: tauri::test::mock_app(),
            session_id: session_id.to_string(),
            session_repo,
            message_repo,
            active_sessions,
            proxy_manager,
            dispatcher: RecordingDispatcher::default(),
        }
    }

    async fn session_status(&self) -> SessionStatus {
        self.session_repo
            .get_session(&self.session_id)
            .await
            .expect("session load")
            .expect("session exists")
            .status
    }

    async fn in_memory_status(&self) -> SessionStatus {
        self.active_sessions
            .read()
            .await
            .get(&self.session_id)
            .expect("session in memory")
            .metadata
            .status
            .clone()
    }

    async fn push_assistant_to_cache(&self, message: Message) {
        self.active_sessions
            .read()
            .await
            .get(&self.session_id)
            .expect("session in memory")
            .messages
            .write()
            .await
            .push(message);
    }

    async fn queue_pending_message(&self, message_id: &str) {
        self.active_sessions
            .read()
            .await
            .get(&self.session_id)
            .expect("session in memory")
            .pending_events
            .write()
            .await
            .add(PendingEvent::Message(message_id.to_string()));
    }
}

fn build_session_metadata(session_id: &str, status: SessionStatus) -> SessionMetadata {
    let now = chrono::Utc::now().timestamp_millis();
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Settlement durability test".to_string()),
        status,
        model: "gpt-4".to_string(),
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

fn build_agent_session(session_id: &str, status: SessionStatus) -> AgentSession {
    AgentSession {
        metadata: build_session_metadata(session_id, status),
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
        last_synced_at: Arc::new(RwLock::new(None)),
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

fn build_assistant_message(session_id: &str, id: &str, text: &str) -> Message {
    Message {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: "assistant".to_string(),
        content: vec![MCPContent::Text {
            text: text.to_string(),
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
        created_at: chrono::Utc::now().timestamp_millis(),
        updated_at: chrono::Utc::now().timestamp_millis(),
        source: None,
        error: None,
        metadata: None,
    }
}

#[tokio::test]
async fn persist_terminal_assistant_sync_makes_cache_only_message_durable_before_idle() {
    let harness = SettlementHarness::new("sess-settlement-durability", SessionStatus::Busy).await;
    let terminal = build_assistant_message(
        &harness.session_id,
        "asst-terminal",
        "All subtasks completed.",
    );
    harness.push_assistant_to_cache(terminal.clone()).await;

    let db_before = harness
        .message_repo
        .get_messages_by_session(&harness.session_id, 10)
        .await
        .expect("db load before persist");
    assert!(db_before.is_empty());

    persist_terminal_assistant_sync(
        &harness.active_sessions,
        &harness.session_id,
        Some(&terminal),
    )
    .await
    .expect("persist succeeds");

    let messages_for_check_session = fetch_session_messages_for_result(
        &StorageSessionId::from_resolved(harness.session_id.clone()),
        50,
    )
    .await
    .expect("checkSession message fetch");
    assert_eq!(
        latest_session_output(&messages_for_check_session),
        "All subtasks completed."
    );
}

#[tokio::test]
async fn settle_session_and_go_idle_persists_terminal_message_and_sets_idle() {
    let harness = SettlementHarness::new("sess-settle-go-idle-success", SessionStatus::Busy).await;
    let terminal = build_assistant_message(
        &harness.session_id,
        "asst-idle-success",
        "Workflow finished cleanly.",
    );

    let settled = settle_session_and_go_idle_with_dispatcher(
        &harness.session_repo,
        &harness.active_sessions,
        &harness.proxy_manager,
        harness.app_handle(),
        &harness.dispatcher,
        &harness.session_id,
        Some(&terminal),
        WorkflowCompletionReason::Natural,
    )
    .await
    .expect("settlement succeeds");

    assert!(!settled);
    assert_eq!(harness.session_status().await, SessionStatus::Idle);
    assert_eq!(harness.in_memory_status().await, SessionStatus::Idle);
    assert!(harness.dispatcher.agent_events().iter().any(|event| {
        matches!(
            event,
            AgentEvent::WorkflowCompleted {
                session_id,
                reason: WorkflowCompletionReason::Natural,
            } if session_id == &harness.session_id
        )
    }));

    let messages_for_check_session = fetch_session_messages_for_result(
        &StorageSessionId::from_resolved(harness.session_id.clone()),
        50,
    )
    .await
    .expect("checkSession message fetch");
    assert_eq!(
        latest_session_output(&messages_for_check_session),
        "Workflow finished cleanly."
    );
}

#[tokio::test]
async fn settle_session_and_finalize_error_persists_terminal_message_and_sets_error() {
    let harness = SettlementHarness::new("sess-settle-finalize-error", SessionStatus::Busy).await;
    let terminal = build_assistant_message(
        &harness.session_id,
        "asst-error-final",
        "Provider request failed while finishing.",
    );
    let runtime_error = AgentRuntimeError::new(
        AgentRuntimeErrorType::AiServiceError,
        "Provider request failed",
    );

    settle_session_and_finalize_error_with_dispatcher(
        &harness.session_repo,
        &harness.active_sessions,
        &harness.dispatcher,
        &harness.session_id,
        Some(&terminal),
        runtime_error,
    )
    .await
    .expect("error settlement succeeds");

    assert_eq!(harness.session_status().await, SessionStatus::Error);
    assert_eq!(harness.in_memory_status().await, SessionStatus::Error);
    assert!(harness.dispatcher.agent_events().iter().any(|event| {
        matches!(
            event,
            AgentEvent::WorkflowError { session_id, .. } if session_id == &harness.session_id
        )
    }));
}

#[tokio::test]
async fn continue_workflow_if_pending_events_returns_false_when_queue_is_empty() {
    let harness = SettlementHarness::new("sess-continue-no-pending", SessionStatus::Busy).await;

    let restarted = continue_workflow_if_pending_events(
        &harness.session_repo,
        &harness.active_sessions,
        &harness.proxy_manager,
        harness.app_handle(),
        &harness.session_id,
    )
    .await
    .expect("pending check succeeds");

    assert!(!restarted);
    assert_eq!(harness.session_status().await, SessionStatus::Busy);
}

#[tokio::test]
async fn settlement_persists_terminal_message_before_pending_restart_decision() {
    let harness = SettlementHarness::new("sess-settle-pending-restart", SessionStatus::Busy).await;
    let terminal = build_assistant_message(
        &harness.session_id,
        "asst-pending-restart",
        "Partial answer before restart.",
    );
    harness.queue_pending_message("queued-user-msg").await;

    assert!(
        session_has_pending_events(&harness.active_sessions, &harness.session_id).await,
        "pending events must be visible before the settlement restart decision"
    );

    persist_terminal_assistant_sync(
        &harness.active_sessions,
        &harness.session_id,
        Some(&terminal),
    )
    .await
    .expect("terminal message must be durable before restart");

    assert_eq!(harness.session_status().await, SessionStatus::Busy);
    assert_eq!(harness.in_memory_status().await, SessionStatus::Busy);

    let db_messages = harness
        .message_repo
        .get_messages_by_session(&harness.session_id, 10)
        .await
        .expect("terminal message should be durable before restart");
    assert_eq!(db_messages.len(), 1);
    assert_eq!(db_messages[0].id, "asst-pending-restart");
}

#[tokio::test]
async fn finish_window_pending_detection_matches_second_idle_guard_input() {
    let harness = SettlementHarness::new("sess-finish-window", SessionStatus::Busy).await;

    assert!(
        !session_has_pending_events(&harness.active_sessions, &harness.session_id).await,
        "first pending scan should see an empty queue"
    );

    harness.queue_pending_message("queued-during-finish").await;

    assert!(
        session_has_pending_events(&harness.active_sessions, &harness.session_id).await,
        "second idle guard must observe messages injected after the first pending scan"
    );
}

#[tokio::test]
async fn persist_terminal_assistant_sync_fails_when_cache_session_missing() {
    let harness = SettlementHarness::new("sess-persist-cache-miss", SessionStatus::Busy).await;

    let error =
        persist_terminal_assistant_sync(&harness.active_sessions, "missing-session-id", None)
            .await
            .expect_err("cache lookup should fail when the session is absent");

    assert!(error.contains("could not be found while saving the final response"));
}

#[tokio::test]
async fn settle_session_and_go_idle_returns_error_when_terminal_cache_lookup_fails() {
    let harness = SettlementHarness::new("sess-settle-persist-failure", SessionStatus::Busy).await;

    let result = settle_session_and_go_idle_with_dispatcher(
        &harness.session_repo,
        &harness.active_sessions,
        &harness.proxy_manager,
        harness.app_handle(),
        &harness.dispatcher,
        "missing-session-id",
        None,
        WorkflowCompletionReason::Natural,
    )
    .await;

    assert!(result.is_err());
    assert!(result
        .expect_err("settlement should fail")
        .contains("could not be found while saving the final response"));
    assert_eq!(harness.session_status().await, SessionStatus::Busy);
}

#[tokio::test]
async fn settle_session_and_finalize_error_returns_error_when_terminal_cache_lookup_fails() {
    let harness =
        SettlementHarness::new("sess-finalize-persist-failure", SessionStatus::Busy).await;
    let runtime_error = AgentRuntimeError::new(
        AgentRuntimeErrorType::AiServiceError,
        "Provider request failed",
    );

    let result = settle_session_and_finalize_error_with_dispatcher(
        &harness.session_repo,
        &harness.active_sessions,
        &harness.dispatcher,
        "missing-session-id",
        None,
        runtime_error,
    )
    .await;

    assert!(result.is_err());
    assert!(result
        .expect_err("settlement should fail")
        .contains("could not be found while saving the final response"));
    assert_eq!(harness.session_status().await, SessionStatus::Busy);
}

#[tokio::test]
async fn settle_session_and_go_idle_returns_error_when_workflow_completed_emit_fails() {
    let harness =
        SettlementHarness::new("sess-settle-idle-emit-failure", SessionStatus::Busy).await;
    let terminal = build_assistant_message(
        &harness.session_id,
        "asst-idle-emit-failure",
        "Workflow finished but UI notify failed.",
    );
    let dispatcher = WorkflowEventFailingDispatcher {
        fail_workflow_events: true,
        ..Default::default()
    };

    let result = settle_session_and_go_idle_with_dispatcher(
        &harness.session_repo,
        &harness.active_sessions,
        &harness.proxy_manager,
        harness.app_handle(),
        &dispatcher,
        &harness.session_id,
        Some(&terminal),
        WorkflowCompletionReason::Natural,
    )
    .await;

    assert!(result.is_err());
    assert!(result
        .expect_err("settlement should fail when WorkflowCompleted emit fails")
        .contains("UI could not be notified"));
    assert_eq!(harness.session_status().await, SessionStatus::Idle);
    assert_eq!(harness.in_memory_status().await, SessionStatus::Idle);
    assert!(dispatcher.agent_events().iter().any(|event| {
        matches!(
            event,
            AgentEvent::StatusChanged {
                session_id,
                status: SessionStatus::Idle,
            } if session_id == &harness.session_id
        )
    }));
    assert!(!dispatcher
        .agent_events()
        .iter()
        .any(|event| { matches!(event, AgentEvent::WorkflowCompleted { .. }) }));
}

#[tokio::test]
async fn settle_session_and_finalize_error_returns_error_when_workflow_error_emit_fails() {
    let harness =
        SettlementHarness::new("sess-settle-error-emit-failure", SessionStatus::Busy).await;
    let runtime_error = AgentRuntimeError::new(
        AgentRuntimeErrorType::AiServiceError,
        "Provider request failed",
    );
    let dispatcher = WorkflowEventFailingDispatcher {
        fail_workflow_events: true,
        ..Default::default()
    };

    let result = settle_session_and_finalize_error_with_dispatcher(
        &harness.session_repo,
        &harness.active_sessions,
        &dispatcher,
        &harness.session_id,
        None,
        runtime_error,
    )
    .await;

    assert!(result.is_err());
    assert!(result
        .expect_err("settlement should fail when WorkflowError emit fails")
        .contains("UI could not be notified"));
    assert_eq!(harness.session_status().await, SessionStatus::Error);
    assert!(dispatcher.agent_events().iter().any(|event| {
        matches!(
            event,
            AgentEvent::StatusChanged {
                session_id,
                status: SessionStatus::Error,
            } if session_id == &harness.session_id
        )
    }));
    assert!(!dispatcher
        .agent_events()
        .iter()
        .any(|event| { matches!(event, AgentEvent::WorkflowError { .. }) }));
}
