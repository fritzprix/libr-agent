/// Integration tests for SqliteCompactContextRepository (SP17 compact context persistence).
///
/// Runs via `cargo test --tests` in CI. Requires a parent session row due to the
/// FK constraint on compact_contexts.session_id.
use sea_orm::{Database, DatabaseConnection, EntityTrait, Set};
use sea_orm_migration::MigratorTrait;
use tauri_mcp_agent_lib::entity::session;
use tauri_mcp_agent_lib::migration::Migrator;
use tauri_mcp_agent_lib::repositories::{
    CompactContextRecord, CompactContextRepository, SqliteCompactContextRepository,
};

async fn setup_db() -> (DatabaseConnection, SqliteCompactContextRepository) {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory DB");
    Migrator::up(&db, None).await.expect("Migrations failed");
    let repo = SqliteCompactContextRepository::new(db.clone());
    (db, repo)
}

async fn insert_parent_session(db: &DatabaseConnection, session_id: &str) {
    let model = session::ActiveModel {
        id: Set(session_id.to_string()),
        status: Set("Idle".to_string()),
        model: Set("gpt-4".to_string()),
        provider: Set("openai".to_string()),
        created_at: Set(123456789),
        updated_at: Set(123456789),
        is_bookmarked: Set(false),
        yolo_mode: Set(false),
        ..Default::default()
    };
    session::Entity::insert(model)
        .exec(db)
        .await
        .expect("Failed to insert parent session");
}

#[tokio::test]
async fn test_compact_context_crud() {
    let (db, repo) = setup_db().await;
    insert_parent_session(&db, "session-1").await;

    let record = CompactContextRecord {
        id: "cc-1".to_string(),
        session_id: "session-1".to_string(),
        from_id: "msg-1".to_string(),
        to_id: "msg-10".to_string(),
        summary: "Test summary".to_string(),
        created_at: 123456789,
    };

    repo.upsert(&record).await.unwrap();

    let retrieved = repo.get_by_session_id("session-1").await.unwrap().unwrap();
    assert_eq!(retrieved.summary, "Test summary");
    assert_eq!(retrieved.from_id, "msg-1");

    // ON CONFLICT upsert — update summary
    let mut updated = record.clone();
    updated.summary = "Updated summary".to_string();
    repo.upsert(&updated).await.unwrap();

    let retrieved = repo.get_by_session_id("session-1").await.unwrap().unwrap();
    assert_eq!(retrieved.summary, "Updated summary");

    repo.delete_by_session_id("session-1").await.unwrap();
    assert!(repo.get_by_session_id("session-1").await.unwrap().is_none());
}

#[tokio::test]
async fn test_compact_context_not_found() {
    let (_db, repo) = setup_db().await;
    assert!(repo
        .get_by_session_id("nonexistent")
        .await
        .unwrap()
        .is_none());
}
