use crate::common;

use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use tauri_mcp_agent_lib::mcp::builtin::knowledge::KnowledgeServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::mcp::types::{ContextVolatility, MCPContent};
use tauri_mcp_agent_lib::repositories::{KnowledgeV2Repository, SqliteKnowledgeV2Repository};

fn extract_text_content(result: &tauri_mcp_agent_lib::mcp::types::MCPResult) -> String {
    result
        .content
        .as_ref()
        .expect("text content expected")
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn knowledge_prune_blocks_partial_delete_when_any_id_is_missing() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteKnowledgeV2Repository::new(db.clone());
    let assistant_id = "assistant-prune-block";

    let existing_chunk_id = repo
        .record_chunk(
            assistant_id.to_string(),
            "Knowledge chunk that should survive validation failure.".to_string(),
            None,
            Some("test".to_string()),
            vec![0.42; 384],
        )
        .await
        .expect("record_chunk should succeed");
    let missing_chunk_id = existing_chunk_id + 999;

    let server = KnowledgeServer::new(assistant_id.to_string(), Arc::new(db))
        .await
        .expect("knowledge server should initialize");

    let result = server
        .call_tool(
            "pruneKnowledge",
            json!({
                "target_ids": [existing_chunk_id, existing_chunk_id, missing_chunk_id],
                "action": "delete"
            }),
            None,
        )
        .await
        .expect("prune_knowledge should return an MCP error result");

    assert_eq!(result.is_error, Some(true));
    let text = extract_text_content(&result);
    assert!(text.contains(&missing_chunk_id.to_string()));
    assert!(text.contains("searchKnowledge"));

    let structured = result
        .structured_content
        .expect("structured content expected on validation failure");
    assert_eq!(
        structured["requestedIds"],
        json!([existing_chunk_id, existing_chunk_id, missing_chunk_id])
    );
    assert_eq!(
        structured["normalizedIds"],
        json!([existing_chunk_id, missing_chunk_id])
    );
    assert_eq!(structured["validatedIds"], json!([existing_chunk_id]));
    assert_eq!(structured["missingIds"], json!([missing_chunk_id]));

    assert!(
        repo.get_chunk_detail(existing_chunk_id).await.is_ok(),
        "existing chunk should remain because validation must happen before deletion"
    );
}

#[tokio::test]
async fn knowledge_prune_success_reports_deleted_ids_in_text_and_json() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteKnowledgeV2Repository::new(db.clone());
    let assistant_id = "assistant-prune-success";

    let chunk_id = repo
        .record_chunk(
            assistant_id.to_string(),
            "Knowledge chunk that should be deleted.".to_string(),
            None,
            Some("test".to_string()),
            vec![0.11; 384],
        )
        .await
        .expect("record_chunk should succeed");

    let server = KnowledgeServer::new(assistant_id.to_string(), Arc::new(db))
        .await
        .expect("knowledge server should initialize");

    let result = server
        .call_tool(
            "pruneKnowledge",
            json!({
                "target_ids": [chunk_id],
                "action": "delete"
            }),
            None,
        )
        .await
        .expect("prune_knowledge should succeed");

    assert_eq!(result.is_error, Some(false));
    let text = extract_text_content(&result);
    assert!(text.contains(&chunk_id.to_string()));
    assert!(
        !text.contains("searchKnowledge"),
        "prune success should not nudge searchKnowledge (Rule 7 lean hints)"
    );
    assert!(
        !text.contains("Suggested Follow-ups"),
        "prune success should omit follow-up hints"
    );

    let structured = result
        .structured_content
        .expect("structured content expected on success");
    assert_eq!(structured["requestedIds"], json!([chunk_id]));
    assert_eq!(structured["normalizedIds"], json!([chunk_id]));
    assert_eq!(structured["deletedIds"], json!([chunk_id]));
    assert_eq!(structured["deletedCount"], 1);

    assert!(
        repo.get_chunk_detail(chunk_id).await.is_err(),
        "chunk should be deleted after successful prune_knowledge"
    );
}

#[tokio::test]
async fn knowledge_record_success_omits_hints_without_entities() {
    let db = common::setup_test_db_with_migrations().await;
    let assistant_id = "assistant-record-lean-hints";

    let server = KnowledgeServer::new(assistant_id.to_string(), Arc::new(db))
        .await
        .expect("knowledge server should initialize");

    let result = server
        .call_tool(
            "recordKnowledge",
            json!({
                "content": "Plain note about deployment window without graph payload.",
                "source": "ops-note",
                "tags": ["ops"],
                "auto_extract": false
            }),
            None,
        )
        .await
        .expect("recordKnowledge should succeed");

    assert_eq!(result.is_error, Some(false));
    let text = extract_text_content(&result);
    assert!(text.contains("Knowledge recorded successfully"));
    assert!(
        !text.contains("searchKnowledge"),
        "steady-path record must not nudge searchKnowledge"
    );
    assert!(
        !text.contains("exploreContext"),
        "record without entities must not nudge exploreContext"
    );
    assert!(
        !text.contains("Suggested Follow-ups"),
        "record without entities must omit follow-up hints"
    );

    let structured = result
        .structured_content
        .expect("structured content expected on record success");
    assert_eq!(structured["success"], true);
    assert!(structured["id"].is_number());
    assert_eq!(structured["extracted_entities"], json!([]));
}

#[tokio::test]
async fn knowledge_record_success_hints_explore_when_entities_present() {
    let db = common::setup_test_db_with_migrations().await;
    let assistant_id = "assistant-record-entity-hint";

    let server = KnowledgeServer::new(assistant_id.to_string(), Arc::new(db))
        .await
        .expect("knowledge server should initialize");

    let result = server
        .call_tool(
            "recordKnowledge",
            json!({
                "content": "Aurora-7 uses Redis for session cache.",
                "auto_extract": false,
                "entities": [
                    {"name": "Aurora-7", "entity_type": "Service"},
                    {"name": "Redis", "entity_type": "Technology"}
                ],
                "relationships": [
                    {
                        "source": "Aurora-7",
                        "target": "Redis",
                        "relation_type": "USES"
                    }
                ]
            }),
            None,
        )
        .await
        .expect("recordKnowledge with entities should succeed");

    assert_eq!(result.is_error, Some(false));
    let text = extract_text_content(&result);
    assert!(text.contains("exploreContext"));
    assert!(
        !text.contains("searchKnowledge"),
        "entity record success should not nudge searchKnowledge"
    );

    let structured = result
        .structured_content
        .expect("structured content expected on record success");
    let entities = structured["extracted_entities"]
        .as_array()
        .expect("extracted_entities array");
    assert!(!entities.is_empty());
}

#[tokio::test]
async fn knowledge_service_context_is_static_and_stable() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteKnowledgeV2Repository::new(db.clone());
    let assistant_id = "assistant-service-context";

    let server = KnowledgeServer::new(assistant_id.to_string(), Arc::new(db.clone()))
        .await
        .expect("knowledge server should initialize");

    let before = server.get_service_context(None).await;
    assert_eq!(before.volatility, ContextVolatility::Stable);
    assert!(before.context_prompt.contains("# Knowledge Base"));
    assert!(!before.context_prompt.contains("Assistant ID"));
    assert!(!before.context_prompt.contains("Stored Chunks"));

    repo.record_chunk(
        assistant_id.to_string(),
        "Chunk that must not change service context text.".to_string(),
        Some(r#"["alpha"]"#.to_string()),
        Some("unit-test".to_string()),
        vec![0.01; 384],
    )
    .await
    .expect("record_chunk should succeed");

    let after = server.get_service_context(None).await;
    assert_eq!(before.context_prompt, after.context_prompt);
    assert_eq!(after.volatility, ContextVolatility::Stable);
}

#[tokio::test]
async fn knowledge_search_structured_results_include_source_and_tags() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteKnowledgeV2Repository::new(db.clone());
    let assistant_id = "assistant-search-dual-channel";

    repo.record_chunk(
        assistant_id.to_string(),
        "Aurora runbook mentions rollback token RBK-TEST-001.".to_string(),
        Some(r#"["ops","aurora"]"#.to_string()),
        Some("runbook.md".to_string()),
        vec![0.33; 384],
    )
    .await
    .expect("record_chunk should succeed");

    let server = KnowledgeServer::new(assistant_id.to_string(), Arc::new(db))
        .await
        .expect("knowledge server should initialize");

    let result = server
        .call_tool(
            "searchKnowledge",
            json!({
                "query": "Aurora rollback token",
                "mode": "keyword",
                "limit": 5
            }),
            None,
        )
        .await
        .expect("searchKnowledge should succeed");

    assert_eq!(result.is_error, Some(false));
    let text = extract_text_content(&result);
    assert!(text.contains("Source: runbook.md"));
    assert!(text.contains("ops"));

    let structured = result
        .structured_content
        .expect("structured content expected on search success");
    let results = structured["results"]
        .as_array()
        .expect("results array expected");
    assert!(!results.is_empty());
    assert_eq!(results[0]["source"], "runbook.md");
    assert_eq!(results[0]["tags"], json!(["ops", "aurora"]));
    assert!(results[0]["id"].is_number());
    assert!(results[0]["content"].as_str().unwrap().contains("RBK-TEST-001"));
}

#[tokio::test]
async fn knowledge_explore_missing_entity_returns_resource_not_found() {
    let db = common::setup_test_db_with_migrations().await;
    let assistant_id = "assistant-explore-missing";

    let server = KnowledgeServer::new(assistant_id.to_string(), Arc::new(db))
        .await
        .expect("knowledge server should initialize");

    let result = server
        .call_tool(
            "exploreContext",
            json!({
                "entity_name": "DefinitelyMissingEntityXYZ",
                "depth": 1
            }),
            None,
        )
        .await
        .expect("exploreContext should return an MCP error result");

    assert_eq!(result.is_error, Some(true));
    let text = extract_text_content(&result);
    assert!(text.contains("DefinitelyMissingEntityXYZ"));
    assert!(text.contains("recordKnowledge") || text.contains("searchKnowledge"));

    let structured = result
        .structured_content
        .expect("structured empty-graph payload expected");
    assert_eq!(structured["found"], false);
    assert_eq!(structured["nodes"], json!([]));
    assert_eq!(structured["edges"], json!([]));
    assert_eq!(structured["chunks"], json!([]));
}

#[test]
fn knowledge_not_found_guidance_uses_real_tool_names() {
    let result = guided_error(
        ErrorCategory::ResourceNotFound,
        "Knowledge chunk 123 not found",
        ToolGroup::Knowledge,
    )
    .to_mcp_result();

    let text = extract_text_content(&result);
    // Guidance should use new camelCase tool names, not old snake_case names
    assert!(text.contains("exploreContext"));
    assert!(text.contains("searchKnowledge"));
    assert!(!text.contains("explore_context"));
    assert!(!text.contains("listKnowledge"));
}

#[tokio::test]
async fn knowledge_repository_atomic_delete_rejects_partial_deletes() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteKnowledgeV2Repository::new(db.clone());
    let assistant_id = "assistant-prune-atomic";

    let existing_chunk_id = repo
        .record_chunk(
            assistant_id.to_string(),
            "Knowledge chunk that should survive repository-level atomic delete failure."
                .to_string(),
            None,
            Some("test".to_string()),
            vec![0.24; 384],
        )
        .await
        .expect("record_chunk should succeed");
    let missing_chunk_id = existing_chunk_id + 50_000;

    let error = repo
        .delete_chunks_atomic(&[existing_chunk_id, missing_chunk_id], assistant_id)
        .await
        .expect_err("atomic delete should fail when any chunk is missing");

    assert!(
        matches!(
            error,
            tauri_mcp_agent_lib::repositories::DbError::NotFound(_)
        ),
        "expected NotFound error, got: {error:?}"
    );
    assert!(
        repo.get_chunk_detail(existing_chunk_id).await.is_ok(),
        "existing chunk should remain because delete_chunks_atomic must not partially delete"
    );
}
