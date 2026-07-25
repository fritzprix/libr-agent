use crate::common;

use tauri_mcp_agent_lib::agent::state::{PendingEvent, PendingEventManager};
use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::repositories::{
    MessageRepository, PendingQueueRepository, SessionMetadata, SessionRepository, SessionStatus,
    SqliteMessageRepository, SqlitePendingQueueRepository, SqliteSessionRepository,
};

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
    Message {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: "user".to_string(),
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
        prompt_tokens: None,
        created_at,
        updated_at: created_at,
        source: None,
        error: None,
        metadata: None,
    }
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
