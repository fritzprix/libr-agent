use crate::common;

use std::collections::HashMap;
use std::sync::Arc;
use tauri::test::MockRuntime;
use tauri_mcp_agent_lib::agent::state::{AgentSession, PendingEvent, PendingEventManager};
use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::repositories::{
    MessageRepository, PendingQueueRepository, SessionMetadata, SessionRepository, SessionStatus,
    SqliteMessageRepository, SqlitePendingQueueRepository, SqliteSessionRepository,
};
use tokio::sync::RwLock;

fn build_session_metadata(session_id: &str) -> SessionMetadata {
    let now = chrono::Utc::now().timestamp_millis();
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Pending queue".to_string()),
        status: SessionStatus::Idle,
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
    build_user_message_with_text(session_id, id, created_at, &format!("text-{id}"))
}

fn build_user_message_with_text(
    session_id: &str,
    id: &str,
    created_at: i64,
    text: &str,
) -> Message {
    Message {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: "user".to_string(),
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
        created_at,
        updated_at: created_at,
        source: None,
        error: None,
        metadata: None,
    }
}

fn build_agent_session(session_id: &str) -> AgentSession {
    use std::sync::atomic::AtomicBool;
    use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
    use tauri_mcp_agent_lib::agent::state::CompactionRuntimeState;
    use tokio_util::sync::CancellationToken;

    AgentSession {
        metadata: build_session_metadata(session_id),
        is_running: false,
        active_permit: None,
        status_transition: Arc::new(tokio::sync::RwLock::new(None)),
        transition_lock: Arc::new(tokio::sync::Mutex::new(())),
        cancellation_token: CancellationToken::new(),
        cancel_pending: Arc::new(AtomicBool::new(false)),
        pending_execution: None,
        messages: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        cache_initialized: Arc::new(AtomicBool::new(true)),
        last_synced_at: Arc::new(RwLock::new(None)),
        repeated_thinking_retry_count: Arc::new(RwLock::new(0)),
        repeated_text_loop_retry_count: Arc::new(RwLock::new(0)),
        bad_tool_args_retry_count: Arc::new(RwLock::new(0)),
        bad_tool_args_incident_count: Arc::new(RwLock::new(0)),
        reasoning_budget_retry_count: Arc::new(RwLock::new(0)),
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
async fn claim_all_pending_messages_promotes_every_queued_prompt_in_one_turn() {
    let db = common::setup_test_db_with_migrations().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    let message_repo = SqliteMessageRepository::new(db.clone());
    let queue_repo = SqlitePendingQueueRepository::new(db.clone());

    tauri_mcp_agent_lib::set_message_repository(SqliteMessageRepository::new(db.clone()));
    tauri_mcp_agent_lib::set_pending_queue_repository(SqlitePendingQueueRepository::new(
        db.clone(),
    ));
    tauri_mcp_agent_lib::set_session_repository(session_repo.clone());

    let session_id = format!("claim-all-pending-{}", uuid::Uuid::new_v4());
    session_repo
        .upsert_session(&build_session_metadata(&session_id))
        .await
        .expect("session should be created");

    let active_sessions = Arc::new(RwLock::new(HashMap::new()));
    active_sessions
        .write()
        .await
        .insert(session_id.clone(), build_agent_session(&session_id));

    let mock_app = tauri::test::mock_app();
    let mock_handle = mock_app.handle();
    let app_handle: &tauri::AppHandle = unsafe {
        &*(mock_handle as *const tauri::AppHandle<MockRuntime> as *const tauri::AppHandle)
    };

    let msg_a = build_user_message_with_text(&session_id, "msg-a", 1_000, "first");
    let msg_b = build_user_message_with_text(&session_id, "msg-b", 2_000, "second");
    let msg_c = build_user_message_with_text(&session_id, "msg-c", 3_000, "third");

    for msg in [&msg_a, &msg_b, &msg_c] {
        tauri_mcp_agent_lib::agent::pending_queue::enqueue_pending_user_message(
            &active_sessions,
            app_handle,
            &session_id,
            msg,
        )
        .await
        .expect("enqueue should succeed");
    }

    let pending_before = tauri_mcp_agent_lib::agent::pending_queue::list_pending_messages(
        &active_sessions,
        &session_id,
    )
    .await
    .expect("list pending before claim");
    assert_eq!(pending_before.len(), 3);

    let claimed = tauri_mcp_agent_lib::agent::pending_queue::claim_all_pending_messages(
        &active_sessions,
        app_handle,
        &session_id,
    )
    .await
    .expect("claim all should succeed");

    assert_eq!(
        claimed.len(),
        1,
        "multiple pending messages should merge into 1 single user message"
    );
    assert_eq!(claimed[0].id, "msg-a");

    let merged_text = match &claimed[0].content[0] {
        tauri_mcp_agent_lib::mcp::types::MCPContent::Text { text, .. } => text.as_str(),
        _ => panic!("Expected text content"),
    };
    assert_eq!(merged_text, "first\n\n---\n\nsecond\n\n---\n\nthird");

    let pending_after = tauri_mcp_agent_lib::agent::pending_queue::list_pending_messages(
        &active_sessions,
        &session_id,
    )
    .await
    .expect("list pending after claim");
    assert!(pending_after.is_empty());

    let cached_ids: Vec<String> = active_sessions
        .read()
        .await
        .get(&session_id)
        .expect("session should exist")
        .messages
        .read()
        .await
        .iter()
        .map(|msg| msg.id.clone())
        .collect();
    assert_eq!(cached_ids, vec!["msg-a"]);

    let index_after = queue_repo
        .list_by_session(&session_id)
        .await
        .expect("index list after claim");
    assert!(index_after.is_empty());

    let msg_a_rows = message_repo
        .get_by_ids(vec!["msg-a".to_string()])
        .await
        .expect("merged message msg-a should exist in DB");
    assert_eq!(msg_a_rows.len(), 1);

    for msg_id in ["msg-b", "msg-c"] {
        let rows = message_repo
            .get_by_ids(vec![msg_id.to_string()])
            .await
            .expect("query DB for deleted message");
        assert!(
            rows.is_empty(),
            "absorbed message {msg_id} should be deleted from DB"
        );
    }
}

#[tokio::test]
async fn claim_all_pending_messages_caps_batch_and_leaves_fifo_remainder() {
    let db = common::setup_test_db_with_migrations().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    let message_repo = SqliteMessageRepository::new(db.clone());
    let queue_repo = SqlitePendingQueueRepository::new(db.clone());

    tauri_mcp_agent_lib::set_message_repository(SqliteMessageRepository::new(db.clone()));
    tauri_mcp_agent_lib::set_pending_queue_repository(SqlitePendingQueueRepository::new(
        db.clone(),
    ));
    tauri_mcp_agent_lib::set_session_repository(session_repo.clone());

    let session_id = format!("claim-cap-pending-{}", uuid::Uuid::new_v4());
    session_repo
        .upsert_session(&build_session_metadata(&session_id))
        .await
        .expect("session should be created");

    let active_sessions = Arc::new(RwLock::new(HashMap::new()));
    active_sessions
        .write()
        .await
        .insert(session_id.clone(), build_agent_session(&session_id));

    let mock_app = tauri::test::mock_app();
    let mock_handle = mock_app.handle();
    let app_handle: &tauri::AppHandle = unsafe {
        &*(mock_handle as *const tauri::AppHandle<MockRuntime> as *const tauri::AppHandle)
    };

    let batch_cap = tauri_mcp_agent_lib::agent::pending_queue::MAX_PENDING_CLAIM_BATCH;
    let total = batch_cap + 2;
    for i in 0..total {
        let msg = build_user_message_with_text(
            &session_id,
            &format!("cap-msg-{i}"),
            1_000 + i as i64,
            &format!("text-{i}"),
        );
        tauri_mcp_agent_lib::agent::pending_queue::enqueue_pending_user_message(
            &active_sessions,
            app_handle,
            &session_id,
            &msg,
        )
        .await
        .expect("enqueue should succeed");
    }

    let claimed = tauri_mcp_agent_lib::agent::pending_queue::claim_all_pending_messages(
        &active_sessions,
        app_handle,
        &session_id,
    )
    .await
    .expect("claim all should succeed");

    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].id, "cap-msg-0");

    let pending_after = tauri_mcp_agent_lib::agent::pending_queue::list_pending_messages(
        &active_sessions,
        &session_id,
    )
    .await
    .expect("list pending after capped claim");
    assert_eq!(pending_after.len(), 2);
    assert_eq!(pending_after[0].id, format!("cap-msg-{batch_cap}"));
    assert_eq!(pending_after[1].id, format!("cap-msg-{}", batch_cap + 1));

    let index_after = queue_repo
        .list_by_session(&session_id)
        .await
        .expect("index after capped claim");
    assert_eq!(index_after.len(), 2);

    // Merged keeper preserved in DB; absorbed messages deleted; remainder preserved.
    let keeper = message_repo
        .get_by_ids(vec!["cap-msg-0".to_string()])
        .await
        .expect("keeper lookup");
    assert_eq!(keeper.len(), 1);
    let absorbed = message_repo
        .get_by_ids(vec!["cap-msg-1".to_string()])
        .await
        .expect("absorbed lookup");
    assert!(
        absorbed.is_empty(),
        "absorbed message cap-msg-1 should be deleted from DB"
    );
    let remainder = message_repo
        .get_by_ids(vec![format!("cap-msg-{batch_cap}")])
        .await
        .expect("remainder lookup");
    assert_eq!(
        remainder.len(),
        1,
        "unclaimed remainder message should remain in DB"
    );
}

#[tokio::test]
async fn pending_queue_repository_orders_fifo_and_supports_selective_remove() {
    let db = common::setup_test_db_with_migrations().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    let message_repo = SqliteMessageRepository::new(db.clone());
    let queue_repo = SqlitePendingQueueRepository::new(db);
    let session_id = format!("pending-queue-{}", uuid::Uuid::new_v4());

    session_repo
        .upsert_session(&build_session_metadata(&session_id))
        .await
        .expect("session should be created");

    let msg_a = build_user_message(&session_id, "msg-a", 1_000);
    let msg_b = build_user_message(&session_id, "msg-b", 2_000);
    let msg_c = build_user_message(&session_id, "msg-c", 3_000);

    for msg in [&msg_a, &msg_b, &msg_c] {
        message_repo
            .insert(msg)
            .await
            .expect("message should persist");
        queue_repo
            .enqueue(&session_id, &msg.id, msg.created_at)
            .await
            .expect("queue entry should persist");
    }

    let listed = queue_repo
        .list_by_session(&session_id)
        .await
        .expect("list should succeed");
    assert_eq!(
        listed
            .iter()
            .map(|entry| entry.message_id.as_str())
            .collect::<Vec<_>>(),
        vec!["msg-a", "msg-b", "msg-c"]
    );
    assert!(listed[0].queue_seq < listed[1].queue_seq);
    assert!(listed[1].queue_seq < listed[2].queue_seq);

    // Same created_at must still keep enqueue order via queue_seq.
    let msg_d = build_user_message(&session_id, "zzz-later-id", 1_000);
    let msg_e = build_user_message(&session_id, "aaa-earlier-id", 1_000);
    for msg in [&msg_d, &msg_e] {
        message_repo
            .insert(msg)
            .await
            .expect("message should persist");
        queue_repo
            .enqueue(&session_id, &msg.id, msg.created_at)
            .await
            .expect("queue entry should persist");
    }
    let with_same_ts = queue_repo
        .list_by_session(&session_id)
        .await
        .expect("list should succeed");
    let same_ts_tail: Vec<&str> = with_same_ts
        .iter()
        .rev()
        .take(2)
        .rev()
        .map(|e| e.message_id.as_str())
        .collect();
    assert_eq!(same_ts_tail, vec!["zzz-later-id", "aaa-earlier-id"]);

    queue_repo
        .remove("msg-b")
        .await
        .expect("selective cancel should succeed");

    let after_remove = queue_repo
        .list_by_session(&session_id)
        .await
        .expect("list after remove should succeed");
    assert!(!after_remove.iter().any(|entry| entry.message_id == "msg-b"));

    // FK cascade: deleting a message drops its pending_queue row.
    message_repo
        .delete_by_id("msg-a")
        .await
        .expect("message delete should succeed");
    let after_fk = queue_repo
        .list_by_session(&session_id)
        .await
        .expect("list after FK cascade");
    assert!(
        !after_fk.iter().any(|entry| entry.message_id == "msg-a"),
        "pending_queue row should cascade-delete with message"
    );

    let removed_ids = queue_repo
        .remove_all_for_session(&session_id)
        .await
        .expect("discard all should succeed");
    assert!(!removed_ids.is_empty());

    let empty = queue_repo
        .list_by_session(&session_id)
        .await
        .expect("list after discard should succeed");
    assert!(empty.is_empty());
}

/// Regression: Session API + Docker queues the initial prompt while status is
/// Provisioning. After provisioning, drain promotes that prompt via
/// `start_workflow` into the active transcript. The durable `pending_queue`
/// index row must be cleared (message body kept). Otherwise
/// `terminate` → `discard_all_pending_messages` deletes the first user message,
/// so Harbor/API sessions lose the first bubble while host/GUI sessions keep it.
#[tokio::test]
async fn promoted_pending_prompt_survives_terminate_discard_after_index_clear() {
    let db = common::setup_test_db_with_migrations().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    let message_repo = SqliteMessageRepository::new(db.clone());
    let queue_repo = SqlitePendingQueueRepository::new(db);
    let session_id = format!("promote-pending-{}", uuid::Uuid::new_v4());
    let message_id = "api-first-user";

    session_repo
        .upsert_session(&build_session_metadata(&session_id))
        .await
        .expect("session should be created");

    let message = build_user_message(&session_id, message_id, 1_000);
    message_repo
        .insert(&message)
        .await
        .expect("queued user message should persist");
    queue_repo
        .enqueue(&session_id, message_id, 1_000)
        .await
        .expect("pending index should enqueue");

    // Simulate docker drain_and_start: clear index only, keep message body.
    queue_repo
        .remove(message_id)
        .await
        .expect("promoted prompt must leave pending_queue");

    let index_after_promote = queue_repo
        .list_by_session(&session_id)
        .await
        .expect("list after promote");
    assert!(
        index_after_promote.is_empty(),
        "promoted prompt must not remain indexed as waiting"
    );

    // Simulate terminate discard: remove_all + delete indexed message ids.
    let discarded_ids = queue_repo
        .remove_all_for_session(&session_id)
        .await
        .expect("discard index should succeed");
    for id in &discarded_ids {
        message_repo
            .delete_by_id(id)
            .await
            .expect("discard deletes only still-indexed waiting prompts");
    }

    let surviving = message_repo
        .get_by_ids(vec![message_id.to_string()])
        .await
        .expect("lookup after terminate discard");
    assert_eq!(
        surviving.len(),
        1,
        "first API user message must survive terminate after docker promote"
    );
    assert_eq!(surviving[0].id, message_id);
}

#[tokio::test]
async fn stale_pending_index_after_promote_lets_terminate_delete_first_user_message() {
    let db = common::setup_test_db_with_migrations().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    let message_repo = SqliteMessageRepository::new(db.clone());
    let queue_repo = SqlitePendingQueueRepository::new(db);
    let session_id = format!("stale-pending-{}", uuid::Uuid::new_v4());
    let message_id = "api-first-user-stale";

    session_repo
        .upsert_session(&build_session_metadata(&session_id))
        .await
        .expect("session should be created");

    let message = build_user_message(&session_id, message_id, 1_000);
    message_repo
        .insert(&message)
        .await
        .expect("queued user message should persist");
    queue_repo
        .enqueue(&session_id, message_id, 1_000)
        .await
        .expect("pending index should enqueue");

    // Bug shape: prompt was promoted to the active transcript but index remained.
    let discarded_ids = queue_repo
        .remove_all_for_session(&session_id)
        .await
        .expect("discard index should succeed");
    assert_eq!(discarded_ids, vec![message_id.to_string()]);
    for id in &discarded_ids {
        message_repo
            .delete_by_id(id)
            .await
            .expect("stale index causes delete of active first user message");
    }

    let surviving = message_repo
        .get_by_ids(vec![message_id.to_string()])
        .await
        .expect("lookup after buggy terminate discard");
    assert!(
        surviving.is_empty(),
        "documents pre-fix failure: stale pending_queue index deletes first user message"
    );
}

#[tokio::test]
async fn strip_pending_queue_protects_active_stack_and_purges_stale_index() {
    let db = common::setup_test_db_with_migrations().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    let message_repo = SqliteMessageRepository::new(db.clone());
    let queue_repo = SqlitePendingQueueRepository::new(db.clone());

    tauri_mcp_agent_lib::set_message_repository(SqliteMessageRepository::new(db.clone()));
    tauri_mcp_agent_lib::set_pending_queue_repository(SqlitePendingQueueRepository::new(
        db.clone(),
    ));

    let session_id = format!("protect-stack-{}", uuid::Uuid::new_v4());
    session_repo
        .upsert_session(&build_session_metadata(&session_id))
        .await
        .expect("session should be created");

    let promoted =
        build_user_message_with_text(&session_id, "msg-promoted", 1_000, "already active");
    let waiting = build_user_message_with_text(&session_id, "msg-waiting", 2_000, "still queued");

    message_repo
        .insert(&promoted)
        .await
        .expect("promoted message should persist");
    message_repo
        .insert(&waiting)
        .await
        .expect("waiting message should persist");
    queue_repo
        .enqueue(&session_id, &promoted.id, promoted.created_at)
        .await
        .expect("stale linger index for promoted");
    queue_repo
        .enqueue(&session_id, &waiting.id, waiting.created_at)
        .await
        .expect("true waiter index");

    let mut slice = message_repo
        .get_recent_slice(&session_id, 40)
        .await
        .expect("recent slice should load");

    let mut protect_ids = std::collections::HashSet::new();
    protect_ids.insert(promoted.id.clone());

    let waiting_ids =
        tauri_mcp_agent_lib::agent::pending_queue::strip_pending_queue_messages_with_protect(
            &session_id,
            &mut slice.items,
            &protect_ids,
        )
        .await
        .expect("strip with protect should succeed");

    assert_eq!(waiting_ids, vec!["msg-waiting".to_string()]);
    let filtered_ids: Vec<&str> = slice.items.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        filtered_ids,
        vec!["msg-promoted"],
        "active-stack / promoted message must remain on transcript"
    );

    let queue_after = queue_repo
        .list_by_session(&session_id)
        .await
        .expect("pending queue after stale purge");
    assert_eq!(
        queue_after
            .iter()
            .map(|e| e.message_id.as_str())
            .collect::<Vec<_>>(),
        vec!["msg-waiting"],
        "stale linger row for promoted message must be purged"
    );
}

#[tokio::test]
async fn strip_pending_queue_keeps_answered_promoted_prompt_and_purges_index() {
    let db = common::setup_test_db_with_migrations().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    let message_repo = SqliteMessageRepository::new(db.clone());
    let queue_repo = SqlitePendingQueueRepository::new(db.clone());

    tauri_mcp_agent_lib::set_message_repository(SqliteMessageRepository::new(db.clone()));
    tauri_mcp_agent_lib::set_pending_queue_repository(SqlitePendingQueueRepository::new(
        db.clone(),
    ));

    let session_id = format!("answered-stale-{}", uuid::Uuid::new_v4());
    session_repo
        .upsert_session(&build_session_metadata(&session_id))
        .await
        .expect("session should be created");

    let user = build_user_message_with_text(&session_id, "msg-user", 1_000, "answered");
    let mut assistant = build_user_message(&session_id, "msg-assistant", 2_000);
    assistant.role = "assistant".to_string();
    assistant.content = vec![MCPContent::Text {
        text: "done".to_string(),
    }];

    message_repo
        .insert(&user)
        .await
        .expect("user message should persist");
    message_repo
        .insert(&assistant)
        .await
        .expect("assistant message should persist");
    queue_repo
        .enqueue(&session_id, &user.id, user.created_at)
        .await
        .expect("stale linger index should persist");

    let mut slice = message_repo
        .get_recent_slice(&session_id, 40)
        .await
        .expect("recent slice should load");

    let waiting_ids = tauri_mcp_agent_lib::agent::pending_queue::strip_pending_queue_messages(
        &session_id,
        &mut slice.items,
    )
    .await
    .expect("strip should succeed");

    assert!(
        waiting_ids.is_empty(),
        "answered prompt must not become a waiter: {waiting_ids:?}"
    );
    let filtered_ids: Vec<&str> = slice.items.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(filtered_ids, vec!["msg-user", "msg-assistant"]);

    let queue_after = queue_repo
        .list_by_session(&session_id)
        .await
        .expect("pending queue after answered stale purge");
    assert!(
        queue_after.is_empty(),
        "answered linger row must be purged: {queue_after:?}"
    );
}

#[tokio::test]
async fn strip_pending_queue_keeps_idle_incomplete_tip_via_protect() {
    // Idle session-start request with a stale pending_queue row must stay on the
    // message stack (never re-enter pending / LayeredPendingQueue only).
    let db = common::setup_test_db_with_migrations().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    let message_repo = SqliteMessageRepository::new(db.clone());
    let queue_repo = SqlitePendingQueueRepository::new(db.clone());

    tauri_mcp_agent_lib::set_message_repository(SqliteMessageRepository::new(db.clone()));
    tauri_mcp_agent_lib::set_pending_queue_repository(SqlitePendingQueueRepository::new(
        db.clone(),
    ));

    let session_id = format!("idle-tip-{}", uuid::Uuid::new_v4());
    session_repo
        .upsert_session(&build_session_metadata(&session_id))
        .await
        .expect("session should be created");

    let tip = build_user_message_with_text(&session_id, "msg-tip", 1_000, "session start");
    message_repo.insert(&tip).await.expect("tip should persist");
    queue_repo
        .enqueue(&session_id, &tip.id, tip.created_at)
        .await
        .expect("stale linger index");

    let mut slice = message_repo
        .get_recent_slice(&session_id, 40)
        .await
        .expect("recent slice should load");

    let mut protect_ids = std::collections::HashSet::new();
    if let Some(id) =
        tauri_mcp_agent_lib::agent::pending_queue::incomplete_turn_user_id(&slice.items)
    {
        protect_ids.insert(id.to_string());
    }

    let waiting_ids =
        tauri_mcp_agent_lib::agent::pending_queue::strip_pending_queue_messages_with_protect(
            &session_id,
            &mut slice.items,
            &protect_ids,
        )
        .await
        .expect("strip should succeed");

    assert!(waiting_ids.is_empty(), "idle tip must not become waiter");
    assert_eq!(
        slice
            .items
            .iter()
            .map(|m| m.id.as_str())
            .collect::<Vec<_>>(),
        vec!["msg-tip"]
    );
    assert!(
        queue_repo
            .list_by_session(&session_id)
            .await
            .expect("queue")
            .is_empty(),
        "stale tip index must be purged"
    );
}

#[test]
fn pending_event_manager_drains_one_message_in_fifo_order() {
    let mut manager = PendingEventManager::new();
    manager.add(PendingEvent::Message("first".to_string()));
    manager.add(PendingEvent::Message("second".to_string()));
    manager.add(PendingEvent::Message("third".to_string()));

    assert_eq!(manager.drain_one_message().as_deref(), Some("first"));
    assert_eq!(
        manager.message_ids(),
        vec!["second".to_string(), "third".to_string()]
    );

    assert!(manager.remove_message("third"));
    assert_eq!(manager.message_ids(), vec!["second".to_string()]);
    assert_eq!(manager.drain_one_message().as_deref(), Some("second"));
    assert!(manager.drain_one_message().is_none());
}

#[test]
fn pending_event_manager_restore_front_preserves_fifo_after_failed_claim() {
    let mut manager = PendingEventManager::new();
    manager.add(PendingEvent::Message("first".to_string()));
    manager.add(PendingEvent::Message("second".to_string()));

    let claimed = manager.drain_one_message().expect("front item");
    assert_eq!(claimed, "first");
    manager.restore_front_message(claimed);

    assert_eq!(
        manager.message_ids(),
        vec!["first".to_string(), "second".to_string()]
    );
    assert!(manager.contains_message("first"));
}

#[test]
fn pending_event_manager_drain_messages_drains_all_in_fifo_order() {
    let mut manager = PendingEventManager::new();
    manager.add(PendingEvent::Message("first".to_string()));
    manager.add(PendingEvent::Message("second".to_string()));
    manager.add(PendingEvent::Message("third".to_string()));

    assert_eq!(
        manager.drain_messages(),
        vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string()
        ]
    );
    assert!(manager.message_ids().is_empty());
}

#[test]
fn pending_event_manager_restore_front_pending_messages_preserves_batch_order() {
    let mut manager = PendingEventManager::new();
    manager.add(PendingEvent::Message("third".to_string()));

    manager.restore_front_pending_messages(&["first".to_string(), "second".to_string()]);

    assert_eq!(
        manager.message_ids(),
        vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string()
        ]
    );
}

#[test]
fn merge_user_message_contents_uses_shared_separator() {
    use tauri_mcp_agent_lib::agent::message_merge::{
        merge_user_message_contents, USER_MESSAGE_MERGE_SEPARATOR,
    };

    let session_id = "merge-sep";
    let a = build_user_message_with_text(session_id, "a", 1, "one");
    let b = build_user_message_with_text(session_id, "b", 2, "two");
    let merged = merge_user_message_contents(&[a, b]);
    match &merged[0] {
        MCPContent::Text { text, .. } => {
            assert_eq!(text, &format!("one{USER_MESSAGE_MERGE_SEPARATOR}two"));
        }
        _ => panic!("expected text"),
    }
}
