mod common;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8};
use std::sync::{Arc, OnceLock};

use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
use tauri_mcp_agent_lib::agent::lifecycle::init_session_with_messages;
use tauri_mcp_agent_lib::agent::state::{
    AgentSession, CacheInitializationState, CompactRepairState, CompactionRuntimeState,
    PendingEventManager,
};
use tauri_mcp_agent_lib::migration::Migrator;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::repositories::{
    CompactContextRecord, CompactContextRepository, MessageRepository, SessionMetadata,
    SessionRepository, SessionStatus, SqliteCompactContextRepository, SqliteMessageRepository,
    SqliteSessionRepository,
};
use tauri_mcp_agent_lib::{set_compact_context_repository, set_message_repository};
use tokio::sync::{Mutex, OnceCell, RwLock};
use tokio_util::sync::CancellationToken;

static TEST_DB_URL: OnceLock<String> = OnceLock::new();
static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
static TEST_DB_ROOT: OnceCell<DatabaseConnection> = OnceCell::const_new();

fn test_mutex() -> &'static Mutex<()> {
    TEST_MUTEX.get_or_init(|| Mutex::new(()))
}

fn test_db_url() -> &'static str {
    TEST_DB_URL.get_or_init(|| {
        let db_path = std::env::temp_dir().join(format!(
            "libragent_session_cache_recent_slice_regression_{}.db",
            uuid::Uuid::new_v4()
        ));
        format!("sqlite://{}?mode=rwc", db_path.display())
    })
}

async fn connect_test_db() -> DatabaseConnection {
    common::register_sqlite_vec();
    let mut options = ConnectOptions::new(test_db_url().to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Database::connect(options)
        .await
        .expect("Failed to connect test database")
}

async fn ensure_test_db_root() -> DatabaseConnection {
    TEST_DB_ROOT
        .get_or_init(|| async {
            let db = connect_test_db().await;
            Migrator::up(&db, None)
                .await
                .expect("Migrations should run");
            set_message_repository(SqliteMessageRepository::new(db.clone()));
            set_compact_context_repository(SqliteCompactContextRepository::new(db.clone()));
            db
        })
        .await
        .clone()
}

async fn test_db() -> DatabaseConnection {
    ensure_test_db_root().await;
    connect_test_db().await
}

fn build_session_metadata(session_id: &str) -> SessionMetadata {
    let now = chrono::Utc::now().timestamp_millis();
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Session cache recent slice regression".to_string()),
        status: SessionStatus::Idle,
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
        unsafe_mode: false,
        workspace_override: None,
    }
}

fn build_agent_session(metadata: SessionMetadata) -> AgentSession {
    AgentSession {
        metadata,
        is_running: false,
        active_permit: None,
        status_transition: Arc::new(RwLock::new(None)),
        transition_lock: Arc::new(tokio::sync::Mutex::new(())),
        cancellation_token: CancellationToken::new(),
        yolo_mode: Arc::new(AtomicBool::new(false)),
        unsafe_mode: Arc::new(AtomicBool::new(false)),
        cancel_pending: Arc::new(AtomicBool::new(false)),
        pending_execution: None,
        messages: Arc::new(RwLock::new(Vec::new())),
        cache_state: Arc::new(AtomicU8::new(
            tauri_mcp_agent_lib::agent::state::CacheInitializationState::Uninitialized as u8,
        )),
        last_synced_at: Arc::new(RwLock::new(None)),
        repeated_thinking_retry_count: Arc::new(RwLock::new(0)),
        pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
        pending_approvals: Arc::new(RwLock::new(HashMap::new())),
        context_registry: Arc::new(ContextRegistry::new()),
        compact_context: Arc::new(RwLock::new(None)),
        compact_repair_state: Arc::new(AtomicU8::new(
            tauri_mcp_agent_lib::agent::state::CompactRepairState::NotNeeded as u8,
        )),
        compaction: CompactionRuntimeState::new(),
        expected_response_id: Arc::new(RwLock::new(None)),
        cached_stable_prompt: Arc::new(RwLock::new(None)),
        last_completion_request: Arc::new(RwLock::new(None)),
    }
}

fn build_message(session_id: &str, index: usize) -> Message {
    let created_at = 1_712_345_678_000_i64 + index as i64;
    Message {
        id: format!("msg-{index:04}"),
        session_id: session_id.to_string(),
        role: if index % 2 == 0 {
            "assistant".to_string()
        } else {
            "user".to_string()
        },
        content: vec![],
        tool_calls: None,
        tool_call_id: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: None,
        created_at,
        updated_at: created_at,
        source: None,
        error: None,
        metadata: None,
    }
}

#[tokio::test]
async fn init_session_cache_loads_recent_messages_so_compact_boundaries_survive_resume() {
    let _guard = test_mutex().lock().await;
    let db = test_db().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    let local_message_repo = SqliteMessageRepository::new(db.clone());
    let local_compact_repo = SqliteCompactContextRepository::new(db.clone());

    let session_id = format!("session-cache-recent-{}", uuid::Uuid::new_v4());
    let metadata = build_session_metadata(&session_id);
    session_repo
        .upsert_session(&metadata)
        .await
        .expect("session should be created");

    for index in 1..=1105 {
        local_message_repo
            .insert(&build_message(&session_id, index))
            .await
            .expect("message insert should succeed");
    }

    let compact_record = CompactContextRecord {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.clone(),
        from_id: "msg-0001".to_string(),
        to_id: "msg-1050".to_string(),
        summary: "Persisted summary".to_string(),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    local_compact_repo
        .upsert(&compact_record)
        .await
        .expect("compact context should be stored");

    let active_sessions = Arc::new(RwLock::new(HashMap::from([(
        session_id.clone(),
        build_agent_session(metadata),
    )])));

    init_session_with_messages(&active_sessions, &session_id)
        .await
        .expect("cache init should succeed");

    let sessions = active_sessions.read().await;
    let session = sessions
        .get(&session_id)
        .expect("active session should remain present");
    let loaded_messages = session.messages.read().await.clone();
    let loaded_ids: Vec<String> = loaded_messages
        .iter()
        .map(|message| message.id.clone())
        .collect();

    assert_eq!(loaded_messages.len(), 1000);
    assert_eq!(loaded_ids.first().map(String::as_str), Some("msg-0106"));
    assert_eq!(loaded_ids.last().map(String::as_str), Some("msg-1105"));
    assert!(loaded_ids.iter().any(|id| id == "msg-1050"));
    assert!(!loaded_ids.iter().any(|id| id == "msg-0001"));
    assert_eq!(
        session.cache_initialization_state(),
        CacheInitializationState::Ready
    );

    let loaded_compact_context = session.compact_context.read().await.clone();
    assert_eq!(
        loaded_compact_context
            .as_ref()
            .map(|record| record.to_id.as_str()),
        Some("msg-1050")
    );
    assert_eq!(
        session.compact_repair_state(),
        CompactRepairState::NotNeeded
    );
}

#[tokio::test]
async fn init_session_cache_arms_compact_repair_only_for_errored_long_sessions_missing_summary() {
    let _guard = test_mutex().lock().await;
    let db = test_db().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    let local_message_repo = SqliteMessageRepository::new(db.clone());

    let session_id = format!("session-cache-repair-{}", uuid::Uuid::new_v4());
    let mut metadata = build_session_metadata(&session_id);
    metadata.status = SessionStatus::Error;
    session_repo
        .upsert_session(&metadata)
        .await
        .expect("session should be created");

    for index in 1..=1105 {
        local_message_repo
            .insert(&build_message(&session_id, index))
            .await
            .expect("message insert should succeed");
    }

    let active_sessions = Arc::new(RwLock::new(HashMap::from([(
        session_id.clone(),
        build_agent_session(metadata),
    )])));

    init_session_with_messages(&active_sessions, &session_id)
        .await
        .expect("cache init should succeed");

    {
        let sessions = active_sessions.read().await;
        let session = sessions
            .get(&session_id)
            .expect("active session should remain present");
        assert_eq!(session.compact_repair_state(), CompactRepairState::Needed);
        assert!(session.compact_context.read().await.is_none());
    }
}
