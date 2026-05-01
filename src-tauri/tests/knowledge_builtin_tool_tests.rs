pub mod common;

use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use tauri_mcp_agent_lib::mcp::builtin::knowledge::KnowledgeServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
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
            "prune_knowledge",
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
    assert!(text.contains("search_knowledge"));

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
            "prune_knowledge",
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
    assert!(text.contains("search_knowledge"));

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

#[test]
fn knowledge_not_found_guidance_uses_real_tool_names() {
    let result = guided_error(
        ErrorCategory::ResourceNotFound,
        "Knowledge chunk 123 not found",
        ToolGroup::Knowledge,
    )
    .to_mcp_result();

    let text = extract_text_content(&result);
    assert!(text.contains("search_knowledge"));
    assert!(text.contains("explore_context"));
    assert!(!text.contains("searchKnowledge"));
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
