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
        workspace_isolation:
            tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
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
    tauri_mcp_agent_lib::set_pending_queue_repository(
        tauri_mcp_agent_lib::repositories::SqlitePendingQueueRepository::new(db.clone()),
    );
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
        true,
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
        true,
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
    tauri_mcp_agent_lib::set_pending_queue_repository(
        tauri_mcp_agent_lib::repositories::SqlitePendingQueueRepository::new(db.clone()),
    );
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
        true,
    )
    .await
    .expect("inject 1 succeeds");

    let msg2 = build_user_message(session_id, "msg-2", "Second message");
    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![msg2],
        true,
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
    tauri_mcp_agent_lib::set_pending_queue_repository(
        tauri_mcp_agent_lib::repositories::SqlitePendingQueueRepository::new(db.clone()),
    );
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
        true,
    )
    .await
    .expect("inject 1 succeeds");

    let msg2 = build_user_message(session_id, "msg-2", "Same text");
    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![msg2],
        true,
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

fn build_tool_message(session_id: &str, id: &str, tool_call_id: &str, text: &str) -> Message {
    Message {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: "tool".to_string(),
        content: vec![MCPContent::Text {
            text: text.to_string(),
            is_error: None,
        }],
        tool_calls: None,
        tool_call_id: Some(tool_call_id.to_string()),
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
async fn test_tool_message_deduplication() {
    let db = common::setup_test_db_with_migrations().await;
    let _message_repo = SqliteMessageRepository::new(db.clone());
    let session_repo = SqliteSessionRepository::new(db.clone());

    tauri_mcp_agent_lib::set_message_repository(SqliteMessageRepository::new(db.clone()));
    tauri_mcp_agent_lib::set_pending_queue_repository(
        tauri_mcp_agent_lib::repositories::SqlitePendingQueueRepository::new(db.clone()),
    );
    tauri_mcp_agent_lib::set_session_repository(session_repo.clone());

    let session_id = "test-session-tool-dedup";
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

    // 1. Inject first tool message (call_1, output: "Success")
    let tool_msg1 = build_tool_message(session_id, "tool-1", "call_1", "Success");
    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![tool_msg1.clone()],
        true,
    )
    .await
    .expect("inject tool_msg1 succeeds");

    // 2. Inject duplicate tool message (same tool_call_id, same content) -> should be deduplicated (ignored)
    let tool_msg2 = build_tool_message(session_id, "tool-2", "call_1", "Success");
    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![tool_msg2.clone()],
        true,
    )
    .await
    .expect("inject tool_msg2 succeeds");

    {
        let sessions = active_sessions.read().await;
        let session = sessions.get(session_id).expect("session exists");
        let cached_msgs = session.messages.read().await;
        // Should only contain the first message because the second one is a duplicate
        assert_eq!(cached_msgs.len(), 1);
        assert_eq!(cached_msgs[0].id, "tool-1");
    }

    // 3. Different tool_call_id with same content must be kept (each call needs its own result)
    let tool_msg3 = build_tool_message(session_id, "tool-3", "call_2", "Success");
    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![tool_msg3.clone()],
        true,
    )
    .await
    .expect("inject tool_msg3 succeeds");

    {
        let sessions = active_sessions.read().await;
        let session = sessions.get(session_id).expect("session exists");
        let cached_msgs = session.messages.read().await;
        assert_eq!(cached_msgs.len(), 2);
        assert_eq!(cached_msgs[0].id, "tool-1");
        assert_eq!(cached_msgs[1].id, "tool-3");
    }

    // 4. Inject tool message with different content -> should NOT be deduplicated
    let tool_msg4 = build_tool_message(session_id, "tool-4", "call_3", "Different Success");
    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![tool_msg4.clone()],
        true,
    )
    .await
    .expect("inject tool_msg4 succeeds");

    {
        let sessions = active_sessions.read().await;
        let session = sessions.get(session_id).expect("session exists");
        let cached_msgs = session.messages.read().await;
        assert_eq!(cached_msgs.len(), 3);
        assert_eq!(cached_msgs[0].id, "tool-1");
        assert_eq!(cached_msgs[1].id, "tool-3");
        assert_eq!(cached_msgs[2].id, "tool-4");
    }
}

fn build_assistant_message_with_thinking(
    session_id: &str,
    id: &str,
    thinking: &str,
    text: &str,
) -> Message {
    Message {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: "assistant".to_string(),
        content: vec![MCPContent::Text {
            text: text.to_string(),
            is_error: None,
        }],
        tool_calls: None,
        tool_call_id: None,
        is_streaming: Some(false),
        thinking: Some(thinking.to_string()),
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

fn build_assistant_message_with_tool_calls(
    session_id: &str,
    id: &str,
    tool_calls: Vec<tauri_mcp_agent_lib::agent::types::ToolCall>,
) -> Message {
    Message {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: "assistant".to_string(),
        content: Vec::new(),
        tool_calls: Some(tool_calls),
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
async fn test_assistant_message_deduplication() {
    let db = common::setup_test_db_with_migrations().await;
    let _message_repo = SqliteMessageRepository::new(db.clone());
    let session_repo = SqliteSessionRepository::new(db.clone());

    tauri_mcp_agent_lib::set_message_repository(SqliteMessageRepository::new(db.clone()));
    tauri_mcp_agent_lib::set_pending_queue_repository(
        tauri_mcp_agent_lib::repositories::SqlitePendingQueueRepository::new(db.clone()),
    );
    tauri_mcp_agent_lib::set_session_repository(session_repo.clone());

    let session_id = "test-session-assistant-dedup";
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

    use tauri_mcp_agent_lib::agent::types::{ToolCall, ToolCallFunction};

    // 1. Test thinking-based deduplication
    let msg1 = build_assistant_message_with_thinking(
        session_id,
        "ast-1",
        "I should run list_dir",
        "running list_dir",
    );
    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![msg1],
        true,
    )
    .await
    .expect("inject ast-1 succeeds");

    // Same thinking + same text -> should be deduplicated
    let msg2 = build_assistant_message_with_thinking(
        session_id,
        "ast-2",
        "I should run list_dir",
        "running list_dir",
    );
    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![msg2],
        true,
    )
    .await
    .expect("inject ast-2 succeeds");

    {
        let sessions = active_sessions.read().await;
        let session = sessions.get(session_id).expect("session exists");
        let cached_msgs = session.messages.read().await;
        assert_eq!(cached_msgs.len(), 1);
        assert_eq!(cached_msgs[0].id, "ast-1");
    }

    // Different thinking -> should NOT be deduplicated
    let msg3 = build_assistant_message_with_thinking(
        session_id,
        "ast-3",
        "Actually I should run grep",
        "running list_dir",
    );
    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![msg3],
        true,
    )
    .await
    .expect("inject ast-3 succeeds");

    {
        let sessions = active_sessions.read().await;
        let session = sessions.get(session_id).expect("session exists");
        let cached_msgs = session.messages.read().await;
        assert_eq!(cached_msgs.len(), 2);
        assert_eq!(cached_msgs[1].id, "ast-3");
    }

    // 2. Test tool_calls-based deduplication
    let tc1 = ToolCall {
        id: "call_1".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "list_dir".to_string(),
            arguments: r#"{"path": "/home"}"#.to_string(),
        },
    };
    let msg4 = build_assistant_message_with_tool_calls(session_id, "ast-4", vec![tc1.clone()]);
    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![msg4],
        true,
    )
    .await
    .expect("inject ast-4 succeeds");

    // Same tool_call name + arguments -> should be deduplicated
    let tc2 = ToolCall {
        id: "call_2".to_string(), // different ID, but same function details
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "list_dir".to_string(),
            arguments: r#"{"path": "/home"}"#.to_string(),
        },
    };
    let msg5 = build_assistant_message_with_tool_calls(session_id, "ast-5", vec![tc2]);
    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![msg5],
        true,
    )
    .await
    .expect("inject ast-5 succeeds");

    {
        let sessions = active_sessions.read().await;
        let session = sessions.get(session_id).expect("session exists");
        let cached_msgs = session.messages.read().await;
        // Total should be 3: ast-1, ast-3, ast-4 (ast-5 was deduped)
        assert_eq!(cached_msgs.len(), 3);
        assert_eq!(cached_msgs[2].id, "ast-4");
    }

    // Different tool_call arguments -> should NOT be deduplicated
    let tc3 = ToolCall {
        id: "call_3".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "list_dir".to_string(),
            arguments: r#"{"path": "/var"}"#.to_string(),
        },
    };
    let msg6 = build_assistant_message_with_tool_calls(session_id, "ast-6", vec![tc3]);
    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![msg6],
        true,
    )
    .await
    .expect("inject ast-6 succeeds");

    {
        let sessions = active_sessions.read().await;
        let session = sessions.get(session_id).expect("session exists");
        let cached_msgs = session.messages.read().await;
        assert_eq!(cached_msgs.len(), 4);
        assert_eq!(cached_msgs[3].id, "ast-6");
    }
}

#[tokio::test]
async fn test_batch_message_deduplication() {
    let db = common::setup_test_db_with_migrations().await;
    let _message_repo = SqliteMessageRepository::new(db.clone());
    let session_repo = SqliteSessionRepository::new(db.clone());

    tauri_mcp_agent_lib::set_message_repository(SqliteMessageRepository::new(db.clone()));
    tauri_mcp_agent_lib::set_pending_queue_repository(
        tauri_mcp_agent_lib::repositories::SqlitePendingQueueRepository::new(db.clone()),
    );
    tauri_mcp_agent_lib::set_session_repository(session_repo.clone());

    let session_id = "test-session-batch-dedup";
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

    // Inject multiple tool messages in a single batch.
    // Consecutive same tool_call_id+content is dropped; distinct call ids with same text are kept.
    let tool_msg1 = build_tool_message(session_id, "tool-1", "call_1", "Success");
    let tool_msg1_dup = build_tool_message(session_id, "tool-1-dup", "call_1", "Success");
    let tool_msg2 = build_tool_message(session_id, "tool-2", "call_2", "Success");
    let tool_msg3 = build_tool_message(session_id, "tool-3", "call_3", "Different Success");

    MessageService::inject_messages_to_session(
        &active_sessions,
        app_handle,
        session_id,
        vec![tool_msg1, tool_msg1_dup, tool_msg2, tool_msg3],
        true,
    )
    .await
    .expect("batch injection succeeds");

    {
        let sessions = active_sessions.read().await;
        let session = sessions.get(session_id).expect("session exists");
        let cached_msgs = session.messages.read().await;
        assert_eq!(cached_msgs.len(), 3);
        assert_eq!(cached_msgs[0].id, "tool-1");
        assert_eq!(cached_msgs[1].id, "tool-2");
        assert_eq!(cached_msgs[2].id, "tool-3");
    }
}

#[tokio::test]
async fn test_handle_llm_response_duplicate_prevention() {
    let db = common::setup_test_db_with_migrations().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    let session_repo_arc = Arc::new(session_repo.clone()) as Arc<dyn SessionRepository>;

    tauri_mcp_agent_lib::set_message_repository(SqliteMessageRepository::new(db.clone()));
    tauri_mcp_agent_lib::set_pending_queue_repository(
        tauri_mcp_agent_lib::repositories::SqlitePendingQueueRepository::new(db.clone()),
    );
    tauri_mcp_agent_lib::set_session_repository(session_repo.clone());

    let session_id = "test-session-llm-dedup";
    session_repo
        .upsert_session(&build_session_metadata(session_id))
        .await
        .expect("session created");

    let active_sessions = Arc::new(RwLock::new(HashMap::new()));
    let session = build_agent_session(session_id);
    active_sessions
        .write()
        .await
        .insert(session_id.to_string(), session);

    let mock_app = tauri::test::mock_app();
    let mock_handle = mock_app.handle();
    let app_handle: &tauri::AppHandle = unsafe {
        &*(mock_handle as *const tauri::AppHandle<MockRuntime> as *const tauri::AppHandle)
    };

    // 1. Manually push the first assistant message into the session cache
    let msg1 = build_assistant_message_with_thinking(
        session_id,
        "ast-1",
        "Thinking process...",
        "Hello, this is a response.",
    );
    {
        let sessions = active_sessions.read().await;
        let session = sessions.get(session_id).unwrap();
        session.messages.write().await.push(msg1.clone());
    }

    // 2. Prepare a duplicate assistant message (same thinking and text)
    let msg2 = build_assistant_message_with_thinking(
        session_id,
        "ast-2",
        "Thinking process...",
        "Hello, this is a response.",
    );

    // Create a dummy proxy manager (it won't be used because of the early return)
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let session_workspace_manager = Arc::new(
        tauri_mcp_agent_lib::session::SessionManager::new_with_base_dir(
            temp_dir.path().join("session-root"),
        )
        .expect("session manager"),
    );
    let proxy_manager = Arc::new(
        tauri_mcp_agent_lib::mcp::service_proxy_manager::MCPServiceProxyManager::new(
            Arc::new(db.clone()),
            session_workspace_manager,
        ),
    );

    // 3. Call handle_llm_response with the duplicate message
    let result = tauri_mcp_agent_lib::agent::llm::response::handle_llm_response(
        &session_repo_arc,
        &active_sessions,
        &proxy_manager,
        app_handle,
        session_id.to_string(),
        msg2,
    )
    .await;

    assert!(
        result.is_ok(),
        "handle_llm_response should succeed (early return)"
    );

    // 4. Verify that the cache still only contains 1 message (the duplicate was skipped)
    {
        let sessions = active_sessions.read().await;
        let session = sessions.get(session_id).unwrap();
        let cached_msgs = session.messages.read().await;
        assert_eq!(cached_msgs.len(), 1);
        assert_eq!(cached_msgs[0].id, "ast-1");
    }
}
