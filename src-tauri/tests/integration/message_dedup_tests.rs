use crate::common;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::test::MockRuntime;
use tauri_mcp_agent_lib::agent::state::AgentSession;
use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::repositories::{
    MessageRepository, SessionMetadata, SessionRepository, SessionStatus, SqliteMessageRepository,
    SqliteSessionRepository,
};
use tauri_mcp_agent_lib::services::MessageService;
use tokio::sync::RwLock;

fn build_session_metadata(session_id: &str) -> SessionMetadata {
    let now = chrono::Utc::now().timestamp_millis();
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Message dedup test session".to_string()),
        status: SessionStatus::Idle,
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
    }
}

fn build_user_message(session_id: &str, id: &str, text: &str) -> Message {
    Message {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: "user".to_string(),
        content: vec![MCPContent::Text {
            text: text.to_string(),
            is_error: None,
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

fn build_agent_session(session_id: &str) -> AgentSession {
    use std::sync::atomic::AtomicBool;
    use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
    use tauri_mcp_agent_lib::agent::state::CompactionRuntimeState;
    use tauri_mcp_agent_lib::agent::state::PendingEventManager;
    use tokio_util::sync::CancellationToken;

    AgentSession {
        metadata: build_session_metadata(session_id),
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
        cache_initialized: Arc::new(AtomicBool::new(true)),
        last_synced_at: Arc::new(RwLock::new(None)),
        repeated_thinking_retry_count: Arc::new(RwLock::new(0)),
        repeated_text_loop_retry_count: Arc::new(RwLock::new(0)),
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
async fn test_content_dedup_same_text() {
    let db = common::setup_test_db_with_migrations().await;
    let message_repo = SqliteMessageRepository::new(db.clone());
    let session_repo = SqliteSessionRepository::new(db.clone());

    tauri_mcp_agent_lib::set_message_repository(SqliteMessageRepository::new(db.clone()));
    tauri_mcp_agent_lib::set_session_repository(session_repo.clone());

    let session_id = "test-session-dedup-1";
    session_repo
        .upsert_session(&build_session_metadata(session_id))
        .await
        .expect("session created");

    let active_sessions = Arc::new(RwLock::new(HashMap::new()));
    active_sessions
        .write()
        .await
        .insert(session_id.to_string(), build_agent_session(session_id));

    let mock_app = tauri::test::mock_app();
    let mock_handle = mock_app.handle();
    let app_handle: &tauri::AppHandle = unsafe {
        &*(mock_handle as *const tauri::AppHandle<MockRuntime> as *const tauri::AppHandle)
    };

    // 1. First message injection
    let msg1 = build_user_message(session_id, "msg-1", "Hello Antigravity");

    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![msg1.clone()],
        false,
    )
    .await
    .expect("inject 1 succeeds");

    // 2. Inject duplicate message
    let msg2 = build_user_message(session_id, "msg-2", "Hello Antigravity");
    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![msg2.clone()],
        false,
    )
    .await
    .expect("inject 2 succeeds");

    // Check memory cache: msg-1 should be popped and only msg-2 should remain
    let sessions = active_sessions.read().await;
    let session = sessions.get(session_id).expect("session exists");
    let cached_msgs = session.messages.read().await;

    assert_eq!(cached_msgs.len(), 1);
    assert_eq!(cached_msgs[0].id, "msg-2");

    // Check DB: Since Phase 3 (orphan DB cleanup) is cut, both msg-1 and msg-2 should remain in DB.
    let db_msgs = message_repo
        .get_messages_by_session(session_id, 10)
        .await
        .expect("db load");
    assert_eq!(db_msgs.len(), 2);
}

#[tokio::test]
async fn test_content_dedup_different_text() {
    let db = common::setup_test_db_with_migrations().await;
    let message_repo = SqliteMessageRepository::new(db.clone());
    let session_repo = SqliteSessionRepository::new(db.clone());

    tauri_mcp_agent_lib::set_message_repository(SqliteMessageRepository::new(db.clone()));
    tauri_mcp_agent_lib::set_session_repository(session_repo.clone());

    let session_id = "test-session-dedup-2";
    session_repo
        .upsert_session(&build_session_metadata(session_id))
        .await
        .expect("session created");

    let active_sessions = Arc::new(RwLock::new(HashMap::new()));
    active_sessions
        .write()
        .await
        .insert(session_id.to_string(), build_agent_session(session_id));

    let mock_app = tauri::test::mock_app();
    let mock_handle = mock_app.handle();
    let app_handle: &tauri::AppHandle = unsafe {
        &*(mock_handle as *const tauri::AppHandle<MockRuntime> as *const tauri::AppHandle)
    };

    let msg1 = build_user_message(session_id, "msg-1", "First message");
    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![msg1],
        false,
    )
    .await
    .expect("inject 1 succeeds");

    let msg2 = build_user_message(session_id, "msg-2", "Second message");
    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![msg2],
        false,
    )
    .await
    .expect("inject 2 succeeds");

    // Both should exist since their content is different
    let sessions = active_sessions.read().await;
    let session = sessions.get(session_id).expect("session exists");
    let cached_msgs = session.messages.read().await;

    assert_eq!(cached_msgs.len(), 2);
    assert_eq!(cached_msgs[0].id, "msg-1");
    assert_eq!(cached_msgs[1].id, "msg-2");

    let db_msgs = message_repo
        .get_messages_by_session(session_id, 10)
        .await
        .expect("db load");
    assert_eq!(db_msgs.len(), 2);
}

#[tokio::test]
async fn test_content_dedup_rich_content_different() {
    let db = common::setup_test_db_with_migrations().await;
    let _message_repo = SqliteMessageRepository::new(db.clone());
    let session_repo = SqliteSessionRepository::new(db.clone());

    tauri_mcp_agent_lib::set_message_repository(SqliteMessageRepository::new(db.clone()));
    tauri_mcp_agent_lib::set_session_repository(session_repo.clone());

    let session_id = "test-session-dedup-3";
    session_repo
        .upsert_session(&build_session_metadata(session_id))
        .await
        .expect("session created");

    let active_sessions = Arc::new(RwLock::new(HashMap::new()));
    active_sessions
        .write()
        .await
        .insert(session_id.to_string(), build_agent_session(session_id));

    let mock_app = tauri::test::mock_app();
    let mock_handle = mock_app.handle();
    let app_handle: &tauri::AppHandle = unsafe {
        &*(mock_handle as *const tauri::AppHandle<MockRuntime> as *const tauri::AppHandle)
    };

    // msg1 has attachments, msg2 does not
    let mut msg1 = build_user_message(session_id, "msg-1", "Same text");
    msg1.attachments = Some(serde_json::json!({"file": "a.txt"}));

    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![msg1],
        false,
    )
    .await
    .expect("inject 1 succeeds");

    let msg2 = build_user_message(session_id, "msg-2", "Same text");
    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![msg2],
        false,
    )
    .await
    .expect("inject 2 succeeds");

    // Deduplication should NOT occur because attachments differ
    let sessions = active_sessions.read().await;
    let session = sessions.get(session_id).expect("session exists");
    let cached_msgs = session.messages.read().await;

    assert_eq!(cached_msgs.len(), 2);
    assert_eq!(cached_msgs[0].id, "msg-1");
    assert_eq!(cached_msgs[1].id, "msg-2");
}
