use sea_orm::{DatabaseConnection, EntityTrait, Set};

use crate::entity::prelude::Session as SessionEntity;
use crate::entity::session;
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;

use super::persist;
use super::sqlite::SqliteMessageRepository;
use super::types::MessageRepository;


async fn setup_test_db() -> SqliteMessageRepository {
    crate::lifecycle::database::register_sqlite_vec();
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory database");

    // Run migrations
    use migration::{Migrator, MigratorTrait};
    Migrator::up(&db, None)
        .await
        .expect("Failed to run migrations");

    SqliteMessageRepository::new(db)
}

async fn create_test_session(db: &DatabaseConnection, session_id: &str) {
    let now = chrono::Utc::now().timestamp_millis();
    let session = session::ActiveModel {
        id: Set(session_id.to_string()),
        name: Set(Some("Test Session".to_string())),
        status: Set("idle".to_string()),
        model: Set("gpt-4".to_string()),
        provider: Set("openai".to_string()),
        execution_mode: Set("normal".to_string()),
        is_bookmarked: Set(false),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    SessionEntity::insert(session)
        .exec(db)
        .await
        .expect("Failed to create session");
}

fn create_dummy_message(id: &str, session_id: &str) -> Message {
    Message {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: "user".to_string(),
        content: vec![MCPContent::Text {
            text: "Hello".to_string(),
        }],
        tool_calls: None,
        tool_call_id: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        created_at: 1000,
        updated_at: 1000,
        source: None,
        error: None,
        metadata: None,
        usage: None,
        prompt_tokens: None,
    }
}

#[test]
fn encode_decode_preserves_structured_content_envelope() {
    let structured = serde_json::json!({
        "sessionId": "a1b2c3d4e5",
        "status": "started",
        "responseStatus": "pending",
    });
    let mut message = create_dummy_message("tool-structured", "session1");
    message.role = "tool".to_string();
    message.metadata = Some(serde_json::json!({
        "structuredContent": structured,
    }));

    let encoded = persist::encode_persisted_error(&message);
    assert_eq!(
        encoded
            .as_ref()
            .and_then(|value| value.get("structuredContent")),
        Some(&structured)
    );

    let (error, metadata) = persist::decode_persisted_tool_error("[]", encoded);
    assert!(error.is_none());
    assert_eq!(
        metadata
            .as_ref()
            .and_then(|value| value.get("structuredContent")),
        Some(&structured)
    );
}

#[test]
fn encode_decode_preserves_tool_error_and_structured_content() {
    let structured = serde_json::json!({ "path": "a.txt", "action": "created" });
    let mut message = create_dummy_message("tool-both", "session1");
    message.metadata = Some(serde_json::json!({
        "toolError": true,
        "structuredContent": structured,
    }));

    let encoded = persist::encode_persisted_error(&message);
    let (error, metadata) = persist::decode_persisted_tool_error("[]", encoded);

    assert!(error.is_none());
    assert_eq!(
        metadata
            .as_ref()
            .and_then(|value| value.get("toolError"))
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        metadata
            .as_ref()
            .and_then(|value| value.get("structuredContent")),
        Some(&structured)
    );
}

#[tokio::test]
async fn test_insert_and_get_messages() {
    let repo = setup_test_db().await;
    create_test_session(&repo.db, "session1").await;
    let message = create_dummy_message("msg1", "session1");

    repo.insert(&message).await.expect("Failed to insert");

    let messages = repo
        .get_messages_by_session("session1", 10)
        .await
        .expect("Failed to get messages");

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].id, "msg1");
}

#[tokio::test]
async fn test_reload_derives_tool_error_from_error_column_marker() {
    let repo = setup_test_db().await;
    create_test_session(&repo.db, "session1").await;

    let mut message = create_dummy_message("tool-err", "session1");
    message.role = "tool".to_string();
    message.tool_call_id = Some("call-1".to_string());
    message.content = vec![MCPContent::Text {
        text: "tool failed".to_string(),
    }];
    message.metadata = Some(serde_json::json!({ "toolError": true }));

    repo.insert(&message).await.expect("Failed to insert");

    let loaded = repo
        .get_by_id("tool-err")
        .await
        .expect("Failed to get message")
        .expect("message should exist");

    assert_eq!(
        loaded
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("toolError"))
            .and_then(|value| value.as_bool()),
        Some(true),
        "toolError must be reconstructed from persisted error-column marker"
    );
    assert!(
        loaded.error.is_none(),
        "toolError marker must not surface as Message.error"
    );
}

#[tokio::test]
async fn test_reload_preserves_structured_content_in_error_column_envelope() {
    let repo = setup_test_db().await;
    create_test_session(&repo.db, "session1").await;

    let structured = serde_json::json!({
        "sessionId": "a1b2c3d4e5",
        "status": "started",
        "responseStatus": "pending",
        "toolName": "startSession",
    });

    let mut message = create_dummy_message("tool-structured", "session1");
    message.role = "tool".to_string();
    message.tool_call_id = Some("call-spawn".to_string());
    message.content = vec![MCPContent::Text {
        text: "Session started successfully".to_string(),
    }];
    message.metadata = Some(serde_json::json!({
        "structuredContent": structured,
    }));

    repo.insert(&message).await.expect("Failed to insert");

    let loaded = repo
        .get_by_id("tool-structured")
        .await
        .expect("Failed to get message")
        .expect("message should exist");

    assert_eq!(
        loaded
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("structuredContent")),
        Some(&structured),
        "structuredContent must survive DB reload for structured tool cards"
    );
    assert!(
        loaded.error.is_none(),
        "structuredContent envelope must not surface as Message.error"
    );
}

#[tokio::test]
async fn test_reload_preserves_structured_content_with_tool_error() {
    let repo = setup_test_db().await;
    create_test_session(&repo.db, "session1").await;

    let structured = serde_json::json!({
        "path": "a.txt",
        "action": "created",
    });

    let mut message = create_dummy_message("tool-both", "session1");
    message.role = "tool".to_string();
    message.tool_call_id = Some("call-both".to_string());
    message.content = vec![MCPContent::Text {
        text: "write failed".to_string(),
    }];
    message.metadata = Some(serde_json::json!({
        "toolError": true,
        "structuredContent": structured,
    }));

    repo.insert(&message).await.expect("Failed to insert");

    let loaded = repo
        .get_by_id("tool-both")
        .await
        .expect("Failed to get message")
        .expect("message should exist");

    assert_eq!(
        loaded
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("toolError"))
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        loaded
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("structuredContent")),
        Some(&structured)
    );
    assert!(loaded.error.is_none());
}

#[tokio::test]
async fn test_reload_derives_tool_error_from_legacy_content_json_is_error() {
    let repo = setup_test_db().await;
    create_test_session(&repo.db, "session1").await;

    // Simulate a pre-cleanup DB row that still has content[].isError.
    use sea_orm::{ActiveModelTrait, Set};
    let now = chrono::Utc::now().timestamp_millis();
    crate::entity::message::ActiveModel {
        id: Set("legacy-tool-err".to_string()),
        session_id: Set("session1".to_string()),
        role: Set("tool".to_string()),
        content: Set(r#"[{"type":"text","text":"legacy fail","isError":true}]"#.to_string()),
        tool_calls: Set(None),
        tool_call_id: Set(Some("call-legacy".to_string())),
        is_streaming: Set(Some(0)),
        thinking: Set(None),
        thinking_signature: Set(None),
        assistant_id: Set(None),
        attachments: Set(None),
        tool_use: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        source: Set(Some("tool".to_string())),
        error: Set(None),
        usage: Set(None),
        prompt_tokens: Set(None),
    }
    .insert(&repo.db)
    .await
    .expect("Failed to insert legacy row");

    let loaded = repo
        .get_by_id("legacy-tool-err")
        .await
        .expect("Failed to get message")
        .expect("message should exist");

    assert_eq!(
        loaded
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("toolError"))
            .and_then(|value| value.as_bool()),
        Some(true),
        "toolError must be reconstructed from legacy content JSON isError"
    );
    // Typed content must not retain the removed field (serde ignores unknown keys).
    assert!(matches!(
        loaded.content.first(),
        Some(MCPContent::Text { text }) if text == "legacy fail"
    ));
}

#[tokio::test]
async fn test_insert_many() {
    let repo = setup_test_db().await;
    create_test_session(&repo.db, "session1").await;
    let messages = vec![
        create_dummy_message("msg1", "session1"),
        create_dummy_message("msg2", "session1"),
    ];

    repo.insert_many(messages)
        .await
        .expect("Failed to insert many");

    let messages = repo
        .get_messages_by_session("session1", 10)
        .await
        .expect("Failed to get messages");

    assert_eq!(messages.len(), 2);
}

#[tokio::test]
async fn test_get_recent_messages() {
    let repo = setup_test_db().await;
    create_test_session(&repo.db, "session1").await;
    create_test_session(&repo.db, "session2").await;

    let mut msg1 = create_dummy_message("msg1", "session1");
    msg1.created_at = 1000;
    let mut msg2 = create_dummy_message("msg2", "session2");
    msg2.created_at = 2000;

    repo.insert(&msg1).await.expect("Failed to insert");
    repo.insert(&msg2).await.expect("Failed to insert");

    let recent = repo
        .get_recent_messages(10)
        .await
        .expect("Failed to get recent");
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].id, "msg2"); // msg2 is newer
}

#[tokio::test]
async fn test_get_page_uses_persisted_row_order_when_created_at_is_inverted() {
    let repo = setup_test_db().await;
    create_test_session(&repo.db, "session1").await;

    let mut assistant = create_dummy_message("assistant-owner", "session1");
    assistant.created_at = 2000;
    assistant.updated_at = 2000;
    repo.insert(&assistant)
        .await
        .expect("Failed to insert assistant");

    let mut tool_a = create_dummy_message("tool-result-a", "session1");
    tool_a.created_at = 1000;
    tool_a.updated_at = 1000;
    repo.insert(&tool_a).await.expect("Failed to insert tool A");

    let mut tool_b = create_dummy_message("tool-result-b", "session1");
    tool_b.created_at = 1001;
    tool_b.updated_at = 1001;
    repo.insert(&tool_b).await.expect("Failed to insert tool B");

    let page = repo
        .get_page("session1", 1, 10)
        .await
        .expect("Failed to get page");
    let ids: Vec<String> = page.items.into_iter().map(|message| message.id).collect();
    assert_eq!(
        ids,
        vec![
            "assistant-owner".to_string(),
            "tool-result-a".to_string(),
            "tool-result-b".to_string()
        ]
    );
}
