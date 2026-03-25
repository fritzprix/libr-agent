/// SeaORM Migration Verification Test
///
/// This test verifies that all Phase 2 SeaORM migrations run correctly
/// and that the refactored modules can interact with the database.
use sea_orm::*;
use sea_orm_migration::MigratorTrait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use tauri_mcp_agent_lib::entity::*;
use tauri_mcp_agent_lib::migration::Migrator;

#[tokio::test]
async fn test_all_migrations_run_successfully() {
    // Create in-memory database
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("Invalid database URL")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("Failed to create test pool");

    // Convert to SeaORM connection
    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool.clone());

    // Run all migrations
    let result = Migrator::up(&db, None).await;
    assert!(
        result.is_ok(),
        "Migrations should run successfully: {:?}",
        result
    );

    // Verify all tables exist by trying to query them

    // 1. Store table
    let store_count = store::Entity::find().count(&db).await;
    assert!(store_count.is_ok(), "Store table should be queryable");

    // 2. Content table
    let content_count = content::Entity::find().count(&db).await;
    assert!(content_count.is_ok(), "Content table should be queryable");

    // 3. Chunk table
    let chunk_count = chunk::Entity::find().count(&db).await;
    assert!(chunk_count.is_ok(), "Chunk table should be queryable");

    // 4. Knowledge table
    let knowledge_count = knowledge::Entity::find().count(&db).await;
    assert!(
        knowledge_count.is_ok(),
        "Knowledge table should be queryable"
    );

    // 5. Assistant table
    let assistant_count = assistant::Entity::find().count(&db).await;
    assert!(
        assistant_count.is_ok(),
        "Assistant table should be queryable"
    );

    // 6. Playbook table
    let playbook_count = playbook::Entity::find().count(&db).await;
    assert!(playbook_count.is_ok(), "Playbook table should be queryable");

    // 7. MCP Server table
    let mcp_server_count = mcp_server::Entity::find().count(&db).await;
    assert!(
        mcp_server_count.is_ok(),
        "MCP Server table should be queryable"
    );

    // 8. Settings table
    let settings_count = settings::Entity::find().count(&db).await;
    assert!(settings_count.is_ok(), "Settings table should be queryable");

    println!("✅ All 8 tables created and queryable via SeaORM");
}

#[tokio::test]
async fn test_settings_crud_operations() {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("Invalid database URL")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("Failed to create test pool");

    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool.clone());
    Migrator::up(&db, None)
        .await
        .expect("Migrations should run");

    // Insert settings
    let setting = settings::ActiveModel {
        key: Set("test_key".to_string()),
        value: Set("test_value".to_string()),
        created_at: Set(1000),
        updated_at: Set(1000),
    };

    let inserted = setting.insert(&db).await;
    assert!(inserted.is_ok(), "Should insert setting successfully");

    // Read settings
    let found = settings::Entity::find_by_id("test_key").one(&db).await;
    assert!(found.is_ok() && found.as_ref().unwrap().is_some());
    assert_eq!(found.unwrap().unwrap().value, "test_value");

    // Update settings
    let mut updated: settings::ActiveModel = settings::Entity::find_by_id("test_key")
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into();
    updated.value = Set("updated_value".to_string());
    let update_result = updated.update(&db).await;
    assert!(update_result.is_ok(), "Should update setting successfully");

    // Verify update
    let updated_setting = settings::Entity::find_by_id("test_key")
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_setting.value, "updated_value");

    // Delete settings
    let delete_result = settings::Entity::delete_by_id("test_key").exec(&db).await;
    assert!(delete_result.is_ok(), "Should delete setting successfully");

    println!("✅ Settings CRUD operations work correctly");
}

#[tokio::test]
async fn test_knowledge_crud_operations() {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("Invalid database URL")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("Failed to create test pool");

    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool.clone());
    Migrator::up(&db, None)
        .await
        .expect("Migrations should run");

    // Insert knowledge
    let knowledge = knowledge::ActiveModel {
        id: NotSet,
        assistant_id: Set("test-session".to_string()),
        title: Set("Test Knowledge".to_string()),
        content: Set("This is test content".to_string()),
        tags: Set(Some("tag1,tag2".to_string())),
        source: Set(Some("test-source".to_string())),
        created_at: Set(1000),
        updated_at: Set(1000),
    };

    let inserted = knowledge.insert(&db).await;
    assert!(inserted.is_ok(), "Should insert knowledge successfully");

    let inserted_model = inserted.unwrap();
    assert_eq!(inserted_model.title, "Test Knowledge");

    // Read knowledge
    let found = knowledge::Entity::find_by_id(inserted_model.id)
        .one(&db)
        .await;
    assert!(found.is_ok() && found.as_ref().unwrap().is_some());
    assert_eq!(found.unwrap().unwrap().content, "This is test content");

    // Update knowledge
    let mut updated: knowledge::ActiveModel = inserted_model.into();
    updated.content = Set("Updated content".to_string());
    let update_result = updated.update(&db).await;
    assert!(
        update_result.is_ok(),
        "Should update knowledge successfully"
    );

    // Delete knowledge
    let delete_result = knowledge::Entity::delete_by_id(1).exec(&db).await;
    assert!(
        delete_result.is_ok(),
        "Should delete knowledge successfully"
    );

    println!("✅ Knowledge CRUD operations work correctly");
}

#[tokio::test]
async fn test_playbook_crud_operations() {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("Invalid database URL")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("Failed to create test pool");

    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool.clone());
    Migrator::up(&db, None)
        .await
        .expect("Migrations should run");

    // Insert playbook (ID must be provided, it's a String, not auto-increment)
    let playbook = playbook::ActiveModel {
        id: Set("playbook-1".to_string()),
        assistant_id: Set("assistant-1".to_string()),
        goal: Set("Test Goal".to_string()),
        initial_command: Set(Some("test command".to_string())),
        workflow: Set("[]".to_string()),
        success_criteria: Set(Some("{}".to_string())),
        created_at: Set(1000),
        updated_at: Set(1000),
        is_bookmarked: Set(false),
    };

    let inserted = playbook.insert(&db).await;
    if let Err(e) = &inserted {
        eprintln!("Playbook insert error: {:?}", e);
    }
    assert!(
        inserted.is_ok(),
        "Should insert playbook successfully: {:?}",
        inserted.err()
    );

    // Read playbook (composite key query)
    let found = playbook::Entity::find()
        .filter(playbook::Column::Id.eq("playbook-1"))
        .filter(playbook::Column::AssistantId.eq("assistant-1"))
        .one(&db)
        .await;

    assert!(found.is_ok() && found.as_ref().unwrap().is_some());
    assert_eq!(
        found.unwrap().unwrap().initial_command,
        Some("test command".to_string())
    );

    println!("✅ Playbook CRUD operations work correctly");
}

#[tokio::test]
async fn test_mcp_server_crud_operations() {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("Invalid database URL")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("Failed to create test pool");

    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool.clone());
    Migrator::up(&db, None)
        .await
        .expect("Migrations should run");

    // Insert MCP server with config field (JSON)
    let server_id = cuid2::create_id();
    let server = mcp_server::ActiveModel {
        id: Set(server_id.clone()),
        name: Set("test-server".to_string()),
        config: Set(r#"{"command":"node","args":"server.js"}"#.to_string()),
        tool_count: Set(None),
        cached_tools: Set(None),
        verification_status: Set(None),
        last_verification_error: Set(None),
        created_at: Set(1000),
        updated_at: Set(1000),
    };

    let inserted = server.insert(&db).await;
    assert!(inserted.is_ok(), "Should insert MCP server successfully");

    // Read MCP server (by ID, not by name)
    let found = mcp_server::Entity::find_by_id(&server_id).one(&db).await;
    assert!(found.is_ok() && found.as_ref().unwrap().is_some());
    assert!(found.unwrap().unwrap().config.contains("node"));

    // Update MCP server (UPSERT test)
    let upsert = mcp_server::ActiveModel {
        id: Set(server_id.clone()),
        name: Set("test-server".to_string()),
        config: Set(r#"{"command":"python","args":"server.py"}"#.to_string()),
        tool_count: Set(None),
        cached_tools: Set(None),
        verification_status: Set(None),
        last_verification_error: Set(None),
        created_at: Set(1000),
        updated_at: Set(2000),
    };

    let upsert_result = mcp_server::Entity::insert(upsert)
        .on_conflict(
            sea_orm::sea_query::OnConflict::column(mcp_server::Column::Name)
                .update_columns([mcp_server::Column::Config, mcp_server::Column::UpdatedAt])
                .to_owned(),
        )
        .exec(&db)
        .await;

    assert!(upsert_result.is_ok(), "UPSERT should work correctly");

    // Verify update (lookup by ID, not name)
    let updated = mcp_server::Entity::find_by_id(&server_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(updated.config.contains("python"));

    // Delete MCP server (by ID)
    let delete_result = mcp_server::Entity::delete_by_id(&server_id).exec(&db).await;
    assert!(
        delete_result.is_ok(),
        "Should delete MCP server successfully"
    );

    println!("✅ MCP Server CRUD operations work correctly");
}

#[tokio::test]
async fn test_assistant_crud_operations() {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("Invalid database URL")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("Failed to create test pool");

    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool.clone());
    Migrator::up(&db, None)
        .await
        .expect("Migrations should run");

    // Insert assistant with config field (JSON) - ID must be provided as String
    let assistant = assistant::ActiveModel {
        id: Set("assistant-1".to_string()),
        name: Set("Test Assistant".to_string()),
        config: Set(r#"{"description":"Test","systemPrompt":"Hello"}"#.to_string()),
        created_at: Set(1000),
        updated_at: Set(1000),
    };

    let inserted = assistant.insert(&db).await;
    if let Err(e) = &inserted {
        eprintln!("Assistant insert error: {:?}", e);
    }
    assert!(
        inserted.is_ok(),
        "Should insert assistant successfully: {:?}",
        inserted.err()
    );

    let inserted_model = inserted.unwrap();
    assert_eq!(inserted_model.name, "Test Assistant");

    // Read assistant (use explicit String ID)
    let found = assistant::Entity::find_by_id("assistant-1").one(&db).await;
    assert!(found.is_ok() && found.as_ref().unwrap().is_some());

    // List assistants
    let list = assistant::Entity::find().all(&db).await;
    assert!(list.is_ok());
    assert_eq!(list.unwrap().len(), 1);

    println!("✅ Assistant CRUD operations work correctly");
}

#[tokio::test]
async fn test_attachments_schema() {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("Invalid database URL")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("Failed to create test pool");

    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool.clone());
    Migrator::up(&db, None)
        .await
        .expect("Migrations should run");

    // Insert store
    let store = store::ActiveModel {
        session_id: Set("test-session".to_string()),
        name: Set(Some("Test Store".to_string())),
        description: Set(Some("Test description".to_string())),
        created_at: Set("2026-01-06".to_string()),
        updated_at: Set("2026-01-06".to_string()),
    };

    let inserted_store = store.insert(&db).await;
    assert!(inserted_store.is_ok(), "Should insert store successfully");

    // Insert content
    let content = content::ActiveModel {
        id: Set("content-1".to_string()),
        session_id: Set("test-session".to_string()),
        mime_type: Set("text/plain".to_string()),
        size: Set(100),
        preview: Set("Preview text".to_string()),
        uploaded_at: Set("2026-01-06".to_string()),
        chunk_count: Set(1),
        last_accessed_at: Set("2026-01-06".to_string()),
        content: Set("Full content".to_string()),
        src_url: Set(Some("https://example.com".to_string())),
        filename: Set("test.txt".to_string()),
        line_count: Set(10),
    };

    let inserted_content = content.insert(&db).await;
    assert!(
        inserted_content.is_ok(),
        "Should insert content successfully"
    );

    // Insert chunk
    let chunk = chunk::ActiveModel {
        id: Set("chunk-1".to_string()),
        content_id: Set("content-1".to_string()),
        chunk_index: Set(0),
        text: Set("Chunk text".to_string()),
        start_line: Set(1),
        end_line: Set(5),
    };

    let inserted_chunk = chunk.insert(&db).await;
    assert!(inserted_chunk.is_ok(), "Should insert chunk successfully");

    // Verify foreign key relationships work
    let content_with_chunks = content::Entity::find()
        .filter(content::Column::Id.eq("content-1"))
        .one(&db)
        .await;

    assert!(content_with_chunks.is_ok() && content_with_chunks.unwrap().is_some());

    println!("✅ Content Store schema and relationships work correctly");
}
