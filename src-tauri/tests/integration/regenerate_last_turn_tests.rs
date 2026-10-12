use crate::common;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::SystemTime;

use tauri::test::MockRuntime;
use tauri_mcp_agent_lib::agent::concurrency::{
    ConcurrencyGate, DEFAULT_MAX_ACTIVE_AGENTS, DEFAULT_MAX_ACTIVE_PROCESSES,
    DEFAULT_MAX_SUSPENDED_AGENTS, DEFAULT_MAX_SUSPENDED_PROCESSES,
};
use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
use tauri_mcp_agent_lib::agent::session_bus::SessionBus;
use tauri_mcp_agent_lib::agent::state::{
    AgentSession, CompactionPhase, CompactionRuntimeState, PendingEventManager,
};
use tauri_mcp_agent_lib::agent::workflow::regenerate_last_turn;
use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::repositories::{
    InMemorySessionRepository, SessionMetadata, SessionRepository, SessionStatus,
};
use tauri_mcp_agent_lib::{init_concurrency_gate, init_session_bus};
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

static TEST_GUARD: Mutex<()> = Mutex::const_new(());

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
        name: Some("Regenerate".to_string()),
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

fn build_message(session_id: &str, id: &str, role: &str, created_at: i64) -> Message {
    Message {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: role.to_string(),
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

fn build_agent_session(metadata: SessionMetadata, messages: Vec<Message>) -> AgentSession {
    AgentSession {
        metadata,
        is_running: true,
        active_permit: None,
        status_transition: Arc::new(RwLock::new(None)),
        transition_lock: Arc::new(tokio::sync::Mutex::new(())),
        cancellation_token: CancellationToken::new(),
        cancel_pending: Arc::new(AtomicBool::new(false)),
        pending_execution: None,
        messages: Arc::new(RwLock::new(messages)),
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

fn mock_app_handle(mock_app: &tauri::App<MockRuntime>) -> &tauri::AppHandle {
    let mock_handle = mock_app.handle();
    unsafe { &*(mock_handle as *const tauri::AppHandle<MockRuntime> as *const tauri::AppHandle) }
}

#[tokio::test]
async fn regenerate_rejects_when_session_is_busy() {
    let _guard = TEST_GUARD.lock().await;
    init_runtime_primitives();

    let session_id = "regen-busy";
    let metadata = build_session_metadata(session_id, SessionStatus::Busy);
    let repo = Arc::new(InMemorySessionRepository::new());
    repo.upsert_session(&metadata)
        .await
        .expect("session upsert");
    let session_repo: Arc<dyn SessionRepository> = repo;

    let messages = vec![
        build_message(session_id, "u1", "user", 1),
        build_message(session_id, "a1", "assistant", 2),
    ];
    let active_sessions = Arc::new(RwLock::new(HashMap::from([(
        session_id.to_string(),
        build_agent_session(metadata, messages),
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

    let err = regenerate_last_turn(
        &session_repo,
        &active_sessions,
        &proxy_manager,
        app_handle,
        session_id.to_string(),
    )
    .await
    .expect_err("busy sessions must reject regenerate");

    assert!(err.contains("must be Idle"), "unexpected error: {err}");
}
