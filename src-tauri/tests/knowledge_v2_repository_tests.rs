use sea_orm::*;
use sea_orm_migration::MigratorTrait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use std::sync::Once;
use tauri_mcp_agent_lib::entity::knowledge_chunk_entity;
use tauri_mcp_agent_lib::migration::Migrator;
use tauri_mcp_agent_lib::repositories::{KnowledgeV2Repository, SqliteKnowledgeV2Repository};

fn register_sqlite_vec() {
    static REGISTER_SQLITE_VEC: Once = Once::new();

    REGISTER_SQLITE_VEC.call_once(|| unsafe {
        libsqlite3_sys::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    });
}

async fn setup_db() -> DatabaseConnection {
    register_sqlite_vec();

    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("Invalid database URL")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("Failed to create test pool");

    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);
    Migrator::up(&db, None)
        .await
        .expect("Migrations should run");

    db
}

#[tokio::test]
async fn knowledge_v2_repository_supports_keyword_and_semantic_search() {
    let db = setup_db().await;
    let repo = SqliteKnowledgeV2Repository::new(db);

    let chunk_id = repo
        .record_chunk(
            "assistant-1".to_string(),
            "Rust powers the LibrAgent knowledge server".to_string(),
            Some(r#"["rust","knowledge"]"#.to_string()),
            Some("unit-test".to_string()),
            vec![0.25; 384],
        )
        .await
        .expect("record_chunk should succeed");

    let keyword_results = repo
        .search_hybrid("assistant-1", Some("Rust"), None, 5)
        .await
        .expect("keyword search should succeed");
    assert_eq!(keyword_results.len(), 1);
    assert_eq!(keyword_results[0].0.id, chunk_id);

    let semantic_results = repo
        .search_hybrid("assistant-1", None, Some(vec![0.25; 384]), 5)
        .await
        .expect("semantic search should succeed");
    assert!(!semantic_results.is_empty());
    assert_eq!(semantic_results[0].0.id, chunk_id);

    let hybrid_results = repo
        .search_hybrid("assistant-1", Some("knowledge"), Some(vec![0.25; 384]), 5)
        .await
        .expect("hybrid search should succeed");
    assert!(!hybrid_results.is_empty());
}

#[tokio::test]
async fn knowledge_v2_graph_context_stays_scoped_to_assistant() {
    let db = setup_db().await;
    let repo = SqliteKnowledgeV2Repository::new(db.clone());

    let chunk_id = repo
        .record_chunk(
            "assistant-graph".to_string(),
            "LibrAgent uses sqlite-vec and fastembed".to_string(),
            Some(r#"["libragent","sqlite-vec"]"#.to_string()),
            None,
            vec![0.5; 384],
        )
        .await
        .expect("record_chunk should succeed");

    let libragent_id = repo
        .upsert_entity(
            "assistant-graph".to_string(),
            "LibrAgent".to_string(),
            Some("Project".to_string()),
            Some("Desktop AI agent platform".to_string()),
        )
        .await
        .expect("upsert_entity should succeed");
    let sqlite_vec_id = repo
        .upsert_entity(
            "assistant-graph".to_string(),
            "sqlite-vec".to_string(),
            Some("Technology".to_string()),
            None,
        )
        .await
        .expect("upsert_entity should succeed");

    repo.create_relationship(
        "assistant-graph".to_string(),
        libragent_id,
        sqlite_vec_id,
        "USES".to_string(),
    )
    .await
    .expect("create_relationship should succeed");
    repo.link_chunk_to_entity(chunk_id, libragent_id)
        .await
        .expect("link_chunk_to_entity should succeed");

    let graph = repo
        .get_graph_context("assistant-graph", "LibrAgent", 2)
        .await
        .expect("get_graph_context should succeed");

    assert_eq!(graph["root_entity"], "LibrAgent");
    assert_eq!(graph["nodes"].as_array().map(Vec::len), Some(2));
    assert_eq!(graph["edges"].as_array().map(Vec::len), Some(1));

    let links = knowledge_chunk_entity::Entity::find()
        .all(&db)
        .await
        .unwrap();
    assert_eq!(links.len(), 1);

    let missing = repo
        .get_graph_context("other-assistant", "LibrAgent", 1)
        .await
        .expect("scoped lookup should succeed");
    assert_eq!(missing["error"], "Entity not found");
}
