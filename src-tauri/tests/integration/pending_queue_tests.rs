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

    queue_repo
        .remove("msg-b")
        .await
        .expect("selective cancel should succeed");

    let after_remove = queue_repo
        .list_by_session(&session_id)
        .await
        .expect("list after remove should succeed");
    assert_eq!(
        after_remove
            .iter()
            .map(|entry| entry.message_id.as_str())
            .collect::<Vec<_>>(),
        vec!["msg-a", "msg-c"]
    );

    let removed_ids = queue_repo
        .remove_all_for_session(&session_id)
        .await
        .expect("discard all should succeed");
    assert_eq!(removed_ids, vec!["msg-a".to_string(), "msg-c".to_string()]);

    let empty = queue_repo
        .list_by_session(&session_id)
        .await
        .expect("list after discard should succeed");
    assert!(empty.is_empty());
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
