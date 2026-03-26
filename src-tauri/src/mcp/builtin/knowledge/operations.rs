use super::{embed, KnowledgeServer};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::repositories::KnowledgeV2Repository;
use serde_json::{json, Value};

/// Record new knowledge into the local vector DB and graph.
pub async fn record_knowledge(
    _server: &KnowledgeServer,
    args: Value,
    assistant_id: &str,
) -> Result<MCPResult, String> {
    let content = match args.get("content").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => return Ok(missing_param_error("content", ToolGroup::Knowledge)),
    };

    let tags = args.get("tags").and_then(|v| {
        if v.is_array() {
            Some(v.to_string())
        } else {
            None
        }
    });

    let auto_extract = args
        .get("auto_extract")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let source = args
        .get("source")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // 1. Generate embedding
    // In a real implementation, we might want to chunk large content first.
    // For now, we assume reasonable sized content or handled by fastembed.
    let embedding = match embed::generate_embedding(content) {
        Ok(e) => e,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::InternalError,
                format!("Failed to generate embedding: {}", e),
                ToolGroup::Knowledge,
            )
            .to_mcp_result());
        }
    };

    // 2. Save to DB via Repository
    let repo = crate::state::get_knowledge_v2_repository();
    match repo
        .record_chunk(
            assistant_id.to_string(),
            content.to_string(),
            tags.clone(),
            source.clone(),
            embedding,
        )
        .await
    {
        Ok(chunk_id) => {
            // Current v2 behavior only derives simple entity links from explicit tags.
            if auto_extract {
                if let Some(tags_json) = tags {
                    if let Ok(tags_arr) = serde_json::from_str::<Vec<String>>(&tags_json) {
                        for tag in tags_arr {
                            if let Ok(entity_id) = repo
                                .upsert_entity(
                                    assistant_id.to_string(),
                                    tag,
                                    Some("Tag".to_string()),
                                    None,
                                )
                                .await
                            {
                                let _ = repo.link_chunk_to_entity(chunk_id, entity_id).await;
                            }
                        }
                    }
                }
            }

            let mut next_steps = vec!["Use search_knowledge to query this information".to_string()];
            if auto_extract {
                next_steps.push(
                    "Simple tag/entity links were seeded from the provided tags when available."
                        .to_string(),
                );
            }

            let hint = SuccessHint::new(
                format!("Knowledge recorded successfully (ID: {})", chunk_id),
                next_steps,
            );
            Ok(hint.to_mcp_result_with_data(Some(json!({
                "success": true,
                "id": chunk_id,
                "source": source,
                "auto_extract": auto_extract,
            }))))
        }
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Failed to save knowledge: {}", e),
            ToolGroup::Knowledge,
        )
        .to_mcp_result()),
    }
}

/// Prune or manage existing knowledge.
pub async fn prune_knowledge(
    _server: &KnowledgeServer,
    args: Value,
    assistant_id: &str,
) -> Result<MCPResult, String> {
    let action = match args.get("action").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => return Ok(missing_param_error("action", ToolGroup::Knowledge)),
    };

    let target_ids = match args.get("target_ids").and_then(|v| v.as_array()) {
        Some(v) => v,
        None => return Ok(missing_param_error("target_ids", ToolGroup::Knowledge)),
    };

    let repo = crate::state::get_knowledge_v2_repository();
    let mut deleted_count = 0;

    match action {
        "delete" => {
            for id_val in target_ids {
                if let Some(id) = id_val.as_i64() {
                    if repo.delete_chunk(id as i32, assistant_id).await.is_ok() {
                        deleted_count += 1;
                    }
                }
            }
            Ok(SuccessHint::new(
                format!(
                    "Deleted {}/{} knowledge chunks.",
                    deleted_count,
                    target_ids.len()
                ),
                vec![],
            )
            .to_mcp_result_with_data(Some(json!({ "deleted": deleted_count }))))
        }
        _ => Err(format!("Action '{}' not supported yet", action)),
    }
}
