use crate::common;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
    SqlitePlaybookRepository, SqliteSessionRepository, SqliteSettingsRepository,
};
use tauri_mcp_agent_lib::server::wake::{verify_wake_token, wake_token_for};
use tauri_mcp_agent_lib::utils::session_id::StorageSessionId;
use tauri_mcp_agent_lib::{
    init_concurrency_gate, init_session_bus, set_settings_repository, try_get_session_bus,
};
use tokio::sync::{Mutex as AsyncMutex, RwLock};
use tokio_util::sync::CancellationToken;

static TEST_GUARD: AsyncMutex<()> = AsyncMutex::const_new(());

/// Seeded in every harness DB; sessions reference it so lazy proxy creation
/// can resolve an `AgentConfig`.
const ASSISTANT_ID: &str = "settlement-test-assistant";

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

    fn emit_completion_cancel(
        &self,
        _request: tauri_mcp_agent_lib::agent::llm::types::CompletionCancelRequest,
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

    fn emit_completion_cancel(
        &self,
        _request: tauri_mcp_agent_lib::agent::llm::types::CompletionCancelRequest,
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

        // Proxy creation (`MCPServiceProxy::builder(...).build()`) resolves the
        // session's assistant config, so every harness session carries this
        // seeded assistant.
        common::seed_test_assistant(
            &db,
            ASSISTANT_ID,
            "Settlement Test Assistant",
            json!({"systemPrompt": "settlement test"}),
        )
        .await;

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
        // Builtin servers read their repositories at tool-call time (e.g. the
        // `playbook__listPlaybooks` idle-session probe below).
        tauri_mcp_agent_lib::set_playbook_repository(SqlitePlaybookRepository::new(db.clone()));

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
        assistant_id: Some(ASSISTANT_ID.to_string()),
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
        status_reason: None,
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
        cancel_pending: Arc::new(AtomicBool::new(false)),
        pending_execution: None,
        messages: Arc::new(RwLock::new(Vec::new())),
        cache_initialized: Arc::new(AtomicBool::new(true)),
        last_synced_at: Arc::new(RwLock::new(None)),
        repeated_thinking_retry_count: Arc::new(RwLock::new(0)),
        repeated_text_loop_retry_count: Arc::new(RwLock::new(0)),
        reasoning_budget_retry_count: Arc::new(RwLock::new(0)),
        malformed_tool_markup_retry_count: Arc::new(RwLock::new(0)),
        pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
        pending_approvals: Arc::new(RwLock::new(HashMap::new())),
        pending_user_prompt: tauri_mcp_agent_lib::agent::state::new_user_prompt_slot(),
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
                ..
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
                ..
            } if session_id == &harness.session_id
        )
    }));
    assert!(!dispatcher
        .agent_events()
        .iter()
        .any(|event| { matches!(event, AgentEvent::WorkflowError { .. }) }));
}

// ── Idle-preservation on the real completion path (issue #2633 G1.1/G1.2) ──
//
// Normal completion must go Idle, not terminate: background processes, wake
// tokens, fence state, and the session proxy all survive the finish path.
// These run through `settle_session_and_go_idle_with_dispatcher` — the exact
// function `finish.rs` uses — so the whole caller→transition chain is pinned,
// replacing the old source-text check. The `integration_tests` binary is
// `#![cfg(not(windows))]`, so the Unix process probes below are always live.

fn marker_pid_file(session_id: &str) -> PathBuf {
    std::env::temp_dir().join(format!("issue2633-idle-{}.pid", session_id))
}

fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Poll for death rather than checking liveness inline: `kill -0` still
/// succeeds on a killed-but-not-yet-reaped zombie (issue #2652 precedent).
async fn wait_for_pid_death(pid: u32) -> bool {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if !pid_alive(pid) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

/// Spawns a real background process through the production
/// `workspace__spawnProcess` tool path (lazy builtin-proxy creation) and
/// returns its OS pid once the child has reported it.
async fn spawn_real_process(harness: &SettlementHarness) -> u32 {
    let pid_file = marker_pid_file(&harness.session_id);
    let _ = std::fs::remove_file(&pid_file);

    harness
        .proxy_manager
        .call_tool(
            &harness.session_id,
            "workspace__spawnProcess",
            json!({
                "command": format!("echo $$ > '{}' && sleep 60", pid_file.display()),
                "name": "idle-probe",
            }),
        )
        .await
        .expect("workspace__spawnProcess must succeed on a live session");

    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok(content) = std::fs::read_to_string(&pid_file) {
            if let Ok(pid) = content.trim().parse::<u32>() {
                let _ = std::fs::remove_file(&pid_file);
                assert!(
                    pid_alive(pid),
                    "the spawned process ({pid}) should be running before settlement"
                );
                return pid;
            }
        }
        assert!(
            Instant::now() < deadline,
            "the spawned process never reported its pid"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Issue #2633 G1.1: the real normal-completion path must not behave like
/// terminate. A real background process, the session's wake token, the
/// teardown fence state, and the proxy itself all survive the finish-path
/// Idle transition; only explicit terminate force-stops them.
#[tokio::test]
async fn normal_completion_path_preserves_process_wake_fence_and_proxy() {
    let harness = SettlementHarness::new("sess-idle-preserve-real-path", SessionStatus::Busy).await;
    let pid = spawn_real_process(&harness).await;
    let token = wake_token_for(&harness.session_id);
    let terminal = build_assistant_message(
        &harness.session_id,
        "asst-idle-preserve",
        "Workflow finished; background work continues.",
    );

    let restarted = settle_session_and_go_idle_with_dispatcher(
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

    assert!(!restarted);
    assert_eq!(harness.session_status().await, SessionStatus::Idle);
    assert!(
        pid_alive(pid),
        "normal idle completion must not kill background process {pid}"
    );
    assert!(
        verify_wake_token(&harness.session_id, &token),
        "normal idle completion must not revoke the wake token"
    );
    assert!(
        !harness
            .proxy_manager
            .is_teardown_fenced(&harness.session_id)
            .await,
        "normal idle completion must not fence the session"
    );
    assert!(
        harness
            .proxy_manager
            .get_proxy(&harness.session_id)
            .await
            .is_some(),
        "normal idle completion must keep the proxy for delegate result delivery"
    );

    // Late work from an idle (not terminated) session is still accepted.
    harness
        .proxy_manager
        .call_tool(&harness.session_id, "playbook__listPlaybooks", json!({}))
        .await
        .expect("an idle session must still accept tool calls");

    // Cleanup for the test itself, through the explicit terminate primitive.
    harness
        .proxy_manager
        .terminate_session_resources(&harness.session_id)
        .await;
    assert!(
        wait_for_pid_death(pid).await,
        "explicit terminate must kill background process {pid}"
    );
}

/// Issue #2633 G1.2: a child that finishes after the parent already went Idle
/// must still deliver its result. The status-change notification fired by the
/// real completion path wakes a checkSession-style waiter that subscribed
/// before reading status, and the child's terminal message stays retrievable.
#[tokio::test]
async fn child_completing_after_parent_idle_delivers_result_to_waiting_parent() {
    let parent_id = "sess-idle-parent-wait-child";
    let child_id = "sess-child-completes-late";
    let harness = SettlementHarness::new(parent_id, SessionStatus::Busy).await;

    // Delegate child in the same DB, linked to the parent.
    let mut child_metadata = build_session_metadata(child_id, SessionStatus::Busy);
    child_metadata.parent_session_id = Some(parent_id.to_string());
    harness
        .session_repo
        .upsert_session(&child_metadata)
        .await
        .expect("child session created");
    harness.active_sessions.write().await.insert(
        child_id.to_string(),
        build_agent_session(child_id, SessionStatus::Busy),
    );

    // 1. The parent completes normally first through the real finish path.
    let parent_terminal = build_assistant_message(
        parent_id,
        "asst-parent-idle-first",
        "Parent finished; waiting on child result.",
    );
    let parent_restarted = settle_session_and_go_idle_with_dispatcher(
        &harness.session_repo,
        &harness.active_sessions,
        &harness.proxy_manager,
        harness.app_handle(),
        &harness.dispatcher,
        parent_id,
        Some(&parent_terminal),
        WorkflowCompletionReason::Natural,
    )
    .await
    .expect("parent settlement succeeds");
    assert!(!parent_restarted);
    assert_eq!(harness.session_status().await, SessionStatus::Idle);

    // 2. A parent-side waiter mirrors the agent__checkSession wait: subscribe
    //    to the child notifier BEFORE reading status so no wake can slip
    //    between the status read and the wait.
    let bus = try_get_session_bus().expect("session bus initialized by harness");
    let child_notifier = bus.get_or_create(child_id);
    let child_wake = child_notifier.notified();

    let child_before = harness
        .session_repo
        .get_session(child_id)
        .await
        .expect("child load")
        .expect("child exists");
    assert_eq!(child_before.status, SessionStatus::Busy);

    // 3. The child finishes through the same real completion path.
    let child_terminal = build_assistant_message(
        child_id,
        "asst-child-terminal",
        "CHILD_RESULT: all subtasks done.",
    );
    let child_restarted = settle_session_and_go_idle_with_dispatcher(
        &harness.session_repo,
        &harness.active_sessions,
        &harness.proxy_manager,
        harness.app_handle(),
        &harness.dispatcher,
        child_id,
        Some(&child_terminal),
        WorkflowCompletionReason::Natural,
    )
    .await
    .expect("child settlement succeeds");
    assert!(!child_restarted);

    // 4. The idle transition's status-change notification wakes the waiter...
    tokio::time::timeout(Duration::from_secs(5), child_wake)
        .await
        .expect("child idle settlement must wake the waiting parent's status subscriber");

    // ...the poll observes the terminal status the wait loop treats as done...
    let child_after = harness
        .session_repo
        .get_session(child_id)
        .await
        .expect("child load")
        .expect("child exists");
    assert_eq!(child_after.status, SessionStatus::Idle);

    // ...and the child's result itself is retrievable for the parent.
    let messages_for_check_session = fetch_session_messages_for_result(
        &StorageSessionId::from_resolved(child_id.to_string()),
        50,
    )
    .await
    .expect("parent-side checkSession message fetch");
    assert_eq!(
        latest_session_output(&messages_for_check_session),
        "CHILD_RESULT: all subtasks done."
    );
}
