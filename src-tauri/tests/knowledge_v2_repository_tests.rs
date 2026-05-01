pub mod common;

use sea_orm::{ConnectionTrait, EntityTrait, Statement};
use tauri_mcp_agent_lib::entity::knowledge_chunk_entity;
use tauri_mcp_agent_lib::entity::knowledge_chunk_v2;
use tauri_mcp_agent_lib::entity::knowledge_entity;
use tauri_mcp_agent_lib::entity::knowledge_relationship;
use tauri_mcp_agent_lib::repositories::{KnowledgeV2Repository, SqliteKnowledgeV2Repository};

#[tokio::test]
async fn knowledge_v2_repository_supports_keyword_and_semantic_search() {
    let db = common::setup_test_db_with_migrations().await;
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
async fn knowledge_v2_repository_sanitizes_special_character_queries() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteKnowledgeV2Repository::new(db);

    let chunk_id = repo
        .record_chunk(
            "assistant-special".to_string(),
            "R&D (alpha) uses foo & bar with sqlite-vec.".to_string(),
            Some(r#"["r&d","alpha","sqlite-vec"]"#.to_string()),
            Some("unit-test".to_string()),
            vec![0.4; 384],
        )
        .await
        .expect("record_chunk should succeed");

    let keyword_results = repo
        .search_hybrid("assistant-special", Some("R&D (alpha)"), None, 5)
        .await
        .expect("special-character keyword search should succeed");
    assert_eq!(keyword_results.len(), 1);
    assert_eq!(keyword_results[0].0.id, chunk_id);

    let ampersand_results = repo
        .search_hybrid("assistant-special", Some("foo & bar"), None, 5)
        .await
        .expect("ampersand keyword search should succeed");
    assert_eq!(ampersand_results.len(), 1);
    assert_eq!(ampersand_results[0].0.id, chunk_id);

    let punctuation_only_results = repo
        .search_hybrid("assistant-special", Some("("), None, 5)
        .await
        .expect("punctuation-only query should not fail");
    assert!(punctuation_only_results.is_empty());
}

#[tokio::test]
async fn knowledge_v2_graph_context_stays_scoped_to_assistant() {
    let db = common::setup_test_db_with_migrations().await;
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
    assert_eq!(graph["linked_chunks"].as_array().map(Vec::len), Some(1));
    assert_eq!(graph["linked_chunks"][0]["id"], chunk_id);

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

#[tokio::test]
async fn delete_chunk_global_prunes_orphan_graph_state() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteKnowledgeV2Repository::new(db.clone());

    let chunk_one_id = repo
        .record_chunk(
            "assistant-global".to_string(),
            "Chunk one".to_string(),
            Some(r#"["shared","orphan"]"#.to_string()),
            Some("test".to_string()),
            vec![0.3; 384],
        )
        .await
        .expect("record_chunk should succeed");
    let chunk_two_id = repo
        .record_chunk(
            "assistant-global".to_string(),
            "Chunk two".to_string(),
            Some(r#"["shared"]"#.to_string()),
            Some("test".to_string()),
            vec![0.31; 384],
        )
        .await
        .expect("record_chunk should succeed");

    let shared_entity_id = repo
        .upsert_entity(
            "assistant-global".to_string(),
            "Shared".to_string(),
            Some("Concept".to_string()),
            None,
        )
        .await
        .expect("upsert_entity should succeed");
    let orphan_entity_id = repo
        .upsert_entity(
            "assistant-global".to_string(),
            "Orphan".to_string(),
            Some("Concept".to_string()),
            None,
        )
        .await
        .expect("upsert_entity should succeed");

    repo.link_chunk_to_entity(chunk_one_id, shared_entity_id)
        .await
        .expect("link_chunk_to_entity should succeed");
    repo.link_chunk_to_entity(chunk_one_id, orphan_entity_id)
        .await
        .expect("link_chunk_to_entity should succeed");
    repo.link_chunk_to_entity(chunk_two_id, shared_entity_id)
        .await
        .expect("link_chunk_to_entity should succeed");
    repo.create_relationship(
        "assistant-global".to_string(),
        shared_entity_id,
        orphan_entity_id,
        "RELATES_TO".to_string(),
    )
    .await
    .expect("create_relationship should succeed");

    let summary = repo
        .delete_chunk_global(chunk_one_id)
        .await
        .expect("delete_chunk_global should succeed");

    assert_eq!(summary.orphan_entity_count, 1);
    assert_eq!(summary.orphan_relationship_count, 1);

    let remaining_chunks = knowledge_chunk_v2::Entity::find().all(&db).await.unwrap();
    assert_eq!(remaining_chunks.len(), 1);
    assert_eq!(remaining_chunks[0].id, chunk_two_id);

    let remaining_entities = knowledge_entity::Entity::find().all(&db).await.unwrap();
    assert_eq!(remaining_entities.len(), 1);
    assert_eq!(remaining_entities[0].id, shared_entity_id);

    let remaining_relationships = knowledge_relationship::Entity::find()
        .all(&db)
        .await
        .unwrap();
    assert!(remaining_relationships.is_empty());

    let remaining_links = knowledge_chunk_entity::Entity::find()
        .all(&db)
        .await
        .unwrap();
    assert_eq!(remaining_links.len(), 1);
    assert_eq!(remaining_links[0].chunk_id, chunk_two_id);
    assert_eq!(remaining_links[0].entity_id, shared_entity_id);
}

#[tokio::test]
async fn knowledge_v2_repository_lists_chunks_with_cursor_pagination() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteKnowledgeV2Repository::new(db.clone());

    let first_id = repo
        .record_chunk(
            "assistant-page".to_string(),
            "First chunk".to_string(),
            Some(r#"["page"]"#.to_string()),
            Some("test".to_string()),
            vec![0.2; 384],
        )
        .await
        .expect("record_chunk should succeed");
    let second_id = repo
        .record_chunk(
            "assistant-page".to_string(),
            "Second chunk".to_string(),
            Some(r#"["page"]"#.to_string()),
            Some("test".to_string()),
            vec![0.21; 384],
        )
        .await
        .expect("record_chunk should succeed");
    let third_id = repo
        .record_chunk(
            "assistant-page".to_string(),
            "Third chunk".to_string(),
            Some(r#"["page"]"#.to_string()),
            Some("test".to_string()),
            vec![0.22; 384],
        )
        .await
        .expect("record_chunk should succeed");

    let backend = db.get_database_backend();
    db.execute(Statement::from_sql_and_values(
        backend,
        "UPDATE knowledge_chunks_v2 SET created_at = ? WHERE id = ?",
        [1003_i64.into(), first_id.into()],
    ))
    .await
    .unwrap();
    db.execute(Statement::from_sql_and_values(
        backend,
        "UPDATE knowledge_chunks_v2 SET created_at = ? WHERE id = ?",
        [1002_i64.into(), second_id.into()],
    ))
    .await
    .unwrap();
    db.execute(Statement::from_sql_and_values(
        backend,
        "UPDATE knowledge_chunks_v2 SET created_at = ? WHERE id = ?",
        [1001_i64.into(), third_id.into()],
    ))
    .await
    .unwrap();

    let first_page = repo
        .list_chunks(Some("assistant-page"), None, None, 2)
        .await
        .expect("first page should load");
    assert_eq!(first_page.items.len(), 2);
    assert_eq!(
        first_page
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        vec![first_id, second_id]
    );
    let next_cursor = first_page.next_cursor.expect("should include next cursor");
    assert_eq!(next_cursor.id, second_id);
    assert_eq!(next_cursor.created_at, 1002);

    let second_page = repo
        .list_chunks(Some("assistant-page"), None, Some(next_cursor), 2)
        .await
        .expect("second page should load");
    assert_eq!(second_page.items.len(), 1);
    assert_eq!(second_page.items[0].id, third_id);
    assert!(second_page.next_cursor.is_none());
}
