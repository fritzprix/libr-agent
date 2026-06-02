use crate::common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use sea_orm::{ConnectOptions, Database, EntityTrait, Set};
use sea_orm_migration::MigratorTrait;
use tauri_mcp_agent_lib::agent::concurrency::{
    ConcurrencyGate, DEFAULT_MAX_ACTIVE_AGENTS, DEFAULT_MAX_ACTIVE_PROCESSES,
    DEFAULT_MAX_SUSPENDED_AGENTS, DEFAULT_MAX_SUSPENDED_PROCESSES,
};
use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
use tauri_mcp_agent_lib::agent::events::{AgentEvent, AgentEventDispatcher};
use tauri_mcp_agent_lib::agent::lifecycle::{
    init_session_with_messages, recover_sessions_with_dispatcher,
};
use tauri_mcp_agent_lib::agent::session_bus::SessionBus;
use tauri_mcp_agent_lib::agent::state::{AgentSession, MAX_CACHED_MESSAGES};
use tauri_mcp_agent_lib::agent::types::{ToolCall, ToolCallFunction};
use tauri_mcp_agent_lib::entity::session;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::repositories::compact_context_repository::CompactContextRepository;
use tauri_mcp_agent_lib::repositories::{
    CompactContextRecord, InMemorySessionRepository, MessageRepository as _, SessionMetadata,
    SessionRepository, SessionStatus, SqliteCompactContextRepository, SqliteMessageRepository,
};
use tauri_mcp_agent_lib::{
    get_compact_context_repository, get_message_repository, init_concurrency_gate,
    init_session_bus, set_compact_context_repository, set_message_repository,
};
use tauri_mcp_agent_lib::{migration::Migrator, utils::sqlite::format_sqlite_url};
use tokio::sync::{Mutex, OnceCell, RwLock};

#[derive(Default)]
struct RecordingDispatcher {
    events: StdMutex<Vec<AgentEvent>>,
}

impl RecordingDispatcher {
    fn agent_events(&self) -> Vec<AgentEvent> {
        self.events
            .lock()
            .expect("agent event lock poisoned")
            .clone()
    }
}

impl AgentEventDispatcher for RecordingDispatcher {
    fn emit_agent_event(&self, event: AgentEvent) -> Result<(), String> {
        self.events
            .lock()
            .expect("agent event lock poisoned")
            .push(event);
        Ok(())
    }

    fn emit_compact_state(
        &self,
        _event: tauri_mcp_agent_lib::agent::llm::types::CompactStateEvent,
    ) -> Result<(), String> {
        Ok(())
    }
}

struct TestRuntimeContext {
    _temp_dir: tempfile::TempDir,
    db: sea_orm::DatabaseConnection,
}

static TEST_RUNTIME: OnceCell<TestRuntimeContext> = OnceCell::const_new();
static TEST_GUARD: Mutex<()> = Mutex::const_new(());

async fn test_db() -> sea_orm::DatabaseConnection {
    TEST_RUNTIME
        .get()
        .expect("test runtime should be initialized")
        .db
        .clone()
}

async fn ensure_test_runtime_state() {
    let _ = TEST_RUNTIME
        .get_or_init(|| async {
            common::register_sqlite_vec();
            let temp_dir = tempfile::tempdir().expect("temp dir should be created");
            let db_path = temp_dir.path().join("session-recovery-tests.db");
            let url = format!("{}?mode=rwc", format_sqlite_url(&db_path.to_string_lossy()));
            let mut options = ConnectOptions::new(url);
            options
                .min_connections(1)
                .max_connections(1)
                .sqlx_logging(false);
            let db = Database::connect(options)
                .await
                .expect("recovery test database should connect");
            Migrator::up(&db, None)
                .await
                .expect("migrations should run");
            set_message_repository(SqliteMessageRepository::new(db.clone()));
            set_compact_context_repository(SqliteCompactContextRepository::new(db.clone()));
            TestRuntimeContext {
                _temp_dir: temp_dir,
                db,
            }
        })
        .await;

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
        name: Some("Recovery regression".to_string()),
        status,
        model: "gpt-5.4".to_string(),
        provider: "openai".to_string(),
        agent_config: Some(r#"{"name":"Recovery regression"}"#.to_string()),
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
        unsafe_mode: false,
        workspace_override: None,
    }
}

async fn insert_sqlite_session(db: &sea_orm::DatabaseConnection, session_id: &str) {
    let now = chrono::Utc::now().timestamp_millis();
    let session = session::ActiveModel {
        id: Set(session_id.to_string()),
        name: Set(Some("Recovery regression".to_string())),
        status: Set("idle".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
        is_bookmarked: Set(false),
        yolo_mode: Set(false),
        unsafe_mode: Set(false),
        ..Default::default()
    };
    session::Entity::insert(session)
        .exec(db)
        .await
        .expect("sqlite session insert should succeed");
}

fn build_user_message(session_id: &str, index: usize) -> Message {
    let timestamp = 1_000 + index as i64;
    Message {
        id: format!("msg-{index:04}"),
        session_id: session_id.to_string(),
        role: "user".to_string(),
        content: vec![MCPContent::Text {
            text: format!("message {index}"),
            is_error: None,
        }],
        tool_calls: None,
        tool_call_id: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        usage: None,
        prompt_tokens: None,
        attachments: None,
        tool_use: None,
        created_at: timestamp,
        updated_at: timestamp,
        source: None,
        error: None,
        metadata: None,
    }
}

fn build_assistant_tool_call_message(
    session_id: &str,
    message_id: &str,
    tool_call_id: &str,
) -> Message {
    let mut message = build_user_message(session_id, MAX_CACHED_MESSAGES + 10);
    message.id = message_id.to_string();
    message.role = "assistant".to_string();
    message.content = vec![MCPContent::Text {
        text: "assistant requested a tool".to_string(),
        is_error: None,
    }];
    message.tool_calls = Some(vec![ToolCall {
        id: tool_call_id.to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "workspace__read".to_string(),
            arguments: "{}".to_string(),
        },
    }]);
    message.tool_call_id = None;
    message
}

#[tokio::test]
async fn recover_sessions_marks_crashed_busy_sessions_paused_without_auto_resuming() {
    let _guard = TEST_GUARD.lock().await;
    ensure_test_runtime_state().await;

    let session_id = format!("recovery-{}", uuid::Uuid::new_v4());
    let repo = Arc::new(InMemorySessionRepository::new());
    let session_repo: Arc<dyn SessionRepository> = repo.clone();
    let active_sessions: Arc<RwLock<HashMap<String, AgentSession>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let dispatcher = RecordingDispatcher::default();

    repo.upsert_session(&build_session_metadata(&session_id, SessionStatus::Busy))
        .await
        .expect("session upsert should succeed");

    recover_sessions_with_dispatcher(
        &session_repo,
        &active_sessions,
        &dispatcher,
        Arc::new(ContextRegistry::new()),
    )
    .await
    .expect("session recovery should succeed");

    let persisted = repo
        .get_session(&session_id)
        .await
        .expect("session lookup should succeed")
        .expect("session should still exist");
    assert_eq!(persisted.status, SessionStatus::Paused);

    let active = active_sessions.read().await;
    let recovered = active
        .get(&session_id)
        .expect("recovered session should be loaded in memory");
    assert_eq!(recovered.metadata.status, SessionStatus::Paused);
    assert!(!recovered.is_running);

    assert!(
        dispatcher.agent_events().iter().any(|event| matches!(
            event,
            AgentEvent::StatusChanged {
                session_id: changed_session_id,
                status: SessionStatus::Paused,
            } if changed_session_id == &session_id
        )),
        "recovery should emit a paused status change for the recovered session"
    );
}

#[tokio::test]
async fn init_session_with_messages_loads_recent_window_and_preserves_compact_anchor() {
    let _guard = TEST_GUARD.lock().await;
    ensure_test_runtime_state().await;

    let db = test_db().await;
    let session_id = format!("resume-window-{}", uuid::Uuid::new_v4());
    let repo = Arc::new(InMemorySessionRepository::new());
    let session_repo: Arc<dyn SessionRepository> = repo.clone();
    let active_sessions: Arc<RwLock<HashMap<String, AgentSession>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let dispatcher = RecordingDispatcher::default();
    let message_repo = get_message_repository();
    let compact_repo = get_compact_context_repository();

    insert_sqlite_session(&db, &session_id).await;

    for index in 0..(MAX_CACHED_MESSAGES + 5) {
        message_repo
            .insert(&build_user_message(&session_id, index))
            .await
            .expect("message insert should succeed");
    }

    let latest_message_id = format!("msg-{:04}", MAX_CACHED_MESSAGES + 4);
    compact_repo
        .upsert(&CompactContextRecord {
            id: format!("cc-{session_id}"),
            session_id: session_id.clone(),
            to_id: latest_message_id.clone(),
            condensed_count: Some(777),
            summary: "Resume summary".to_string(),
            created_at: chrono::Utc::now().timestamp_millis(),
        })
        .await
        .expect("compact context insert should succeed");

    repo.upsert_session(&build_session_metadata(&session_id, SessionStatus::Busy))
        .await
        .expect("session upsert should succeed");

    recover_sessions_with_dispatcher(
        &session_repo,
        &active_sessions,
        &dispatcher,
        Arc::new(ContextRegistry::new()),
    )
    .await
    .expect("session recovery should succeed");

    init_session_with_messages(&active_sessions, &session_id)
        .await
        .expect("cache init should succeed");

    let active = active_sessions.read().await;
    let recovered = active
        .get(&session_id)
        .expect("recovered session should be loaded in memory");
    let messages = recovered.messages.read().await.clone();
    let compact_context = recovered.compact_context.read().await.clone();

    assert_eq!(messages.len(), MAX_CACHED_MESSAGES);
    assert_eq!(
        messages.first().map(|message| message.id.as_str()),
        Some("msg-0005")
    );
    assert_eq!(
        messages.last().map(|message| message.id.as_str()),
        Some(latest_message_id.as_str())
    );
    assert!(
        messages
            .iter()
            .any(|message| message.id == latest_message_id),
        "cache init should load the latest causal window so compact anchors stay present"
    );
    assert_eq!(
        compact_context.as_ref().map(|record| record.to_id.as_str()),
        Some(latest_message_id.as_str())
    );
}

#[tokio::test]
async fn recover_sessions_closes_recent_orphaned_tool_calls_outside_oldest_page() {
    let _guard = TEST_GUARD.lock().await;
    ensure_test_runtime_state().await;

    let db = test_db().await;
    let session_id = format!("recovery-tools-{}", uuid::Uuid::new_v4());
    let repo = Arc::new(InMemorySessionRepository::new());
    let session_repo: Arc<dyn SessionRepository> = repo.clone();
    let active_sessions: Arc<RwLock<HashMap<String, AgentSession>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let dispatcher = RecordingDispatcher::default();
    let message_repo = get_message_repository();

    insert_sqlite_session(&db, &session_id).await;

    for index in 0..MAX_CACHED_MESSAGES {
        message_repo
            .insert(&build_user_message(&session_id, index))
            .await
            .expect("message insert should succeed");
    }

    let tool_call_id = "recent-orphan-call";
    message_repo
        .insert(&build_assistant_tool_call_message(
            &session_id,
            "assistant-recent-tool",
            tool_call_id,
        ))
        .await
        .expect("assistant tool-call insert should succeed");

    repo.upsert_session(&build_session_metadata(&session_id, SessionStatus::Busy))
        .await
        .expect("session upsert should succeed");

    recover_sessions_with_dispatcher(
        &session_repo,
        &active_sessions,
        &dispatcher,
        Arc::new(ContextRegistry::new()),
    )
    .await
    .expect("session recovery should succeed");

    let recent_slice = message_repo
        .get_recent_slice(&session_id, 4)
        .await
        .expect("recent slice should load");

    let orphan_tombstone = recent_slice
        .items
        .iter()
        .find(|message| message.tool_call_id.as_deref() == Some(tool_call_id))
        .expect("recovery should inject a tombstone for the recent orphaned tool call");
    assert_eq!(orphan_tombstone.role, "tool");
    assert!(
        orphan_tombstone
            .content
            .iter()
            .any(|content| matches!(content, MCPContent::Text { text, .. } if text.contains("did not complete"))),
        "tombstone text should explain the crash recovery"
    );
}
