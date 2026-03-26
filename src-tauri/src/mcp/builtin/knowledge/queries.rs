use super::{
    embed,
    helpers::{DEFAULT_LIMIT, MAX_LIMIT},
    KnowledgeServer,
};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::repositories::KnowledgeV2Repository;
use serde_json::{json, Value};

/// Search knowledge using hybrid approach
pub async fn search_knowledge(
    _server: &KnowledgeServer,
    args: Value,
    assistant_id: &str,
) -> Result<MCPResult, String> {
    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => return Ok(missing_param_error("query", ToolGroup::Knowledge)),
    };

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(DEFAULT_LIMIT)
        .clamp(1, MAX_LIMIT);
    let mode = match args
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("hybrid")
    {
        "keyword" => "keyword",
        "semantic" => "semantic",
        "hybrid" => "hybrid",
        other => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Unsupported search mode '{other}'. Expected one of: keyword, semantic, hybrid."
                ),
                ToolGroup::Knowledge,
            )
            .to_mcp_result());
        }
    };

    let query_embedding = if matches!(mode, "semantic" | "hybrid") {
        match embed::generate_embedding(query) {
            Ok(embedding) => Some(embedding),
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::InternalError,
                    format!("Failed to generate query embedding: {}", e),
                    ToolGroup::Knowledge,
                )
                .to_mcp_result());
            }
        }
    } else {
        None
    };

    let repo = crate::state::get_knowledge_v2_repository();
    let text_query = if matches!(mode, "keyword" | "hybrid") {
        Some(query)
    } else {
        None
    };

    match repo
        .search_hybrid(assistant_id, text_query, query_embedding, limit)
        .await
    {
        Ok(results) => {
            let mut output_text =
                format!("Found {} relevant knowledge entries:\n\n", results.len());
            for (model, score) in &results {
                output_text.push_str(&format!(
                    "### Chunk ID: {} (Score: {:.4})\n{}\n\n",
                    model.id, score, model.content
                ));
            }

            if results.is_empty() {
                output_text = "No relevant knowledge found for your query.".to_string();
            }

            Ok(
                SuccessHint::new(output_text, vec![]).to_mcp_result_with_data(Some(json!({
                    "mode": mode,
                    "results": results.iter().map(|(m, d)| json!({
                        "id": m.id,
                        "content": m.content,
                        "score": d
                    })).collect::<Vec<_>>()
                }))),
            )
        }
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Failed to search knowledge: {}", e),
            ToolGroup::Knowledge,
        )
        .to_mcp_result()),
    }
}

/// Explore graph context
pub async fn explore_context(
    _server: &KnowledgeServer,
    args: Value,
    assistant_id: &str,
) -> Result<MCPResult, String> {
    let entity_name = match args.get("entity_name").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => return Ok(missing_param_error("entity_name", ToolGroup::Knowledge)),
    };

    let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(1) as u32;

    let repo = crate::state::get_knowledge_v2_repository();
    match repo
        .get_graph_context(assistant_id, entity_name, depth)
        .await
    {
        Ok(context) => {
            if context.get("error").is_some() {
                return Ok(SuccessHint::new(
                    format!("Entity '{}' not found in graph.", entity_name),
                    vec!["Use record_knowledge to add information about this entity.".to_string()],
                )
                .to_mcp_result());
            }

            let summary = format!("Graph context for '{}' (Depth: {}):", entity_name, depth);
            Ok(SuccessHint::new(summary, vec![]).to_mcp_result_with_data(Some(context)))
        }
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Failed to explore graph: {}", e),
            ToolGroup::Knowledge,
        )
        .to_mcp_result()),
    }
}
