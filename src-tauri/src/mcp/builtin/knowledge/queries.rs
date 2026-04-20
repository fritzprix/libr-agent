use super::{
    embed,
    helpers::{parse_db_tags, DEFAULT_LIMIT, MAX_LIMIT},
    KnowledgeServer,
};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::repositories::KnowledgeV2Repository;
use serde_json::{json, Value};
use std::collections::HashMap;

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

    let offset = args
        .get("offset")
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        .min(10_000);

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
        .search_hybrid(assistant_id, text_query, query_embedding, limit.saturating_add(offset).saturating_add(1))
        .await
    {
        Ok(all_results) => {
            if offset as usize >= all_results.len() && offset > 0 {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("Offset {} exceeds total results ({}).", offset, all_results.len()),
                    ToolGroup::Knowledge,
                )
                .with_guidance(vec!["Try calling again with offset: 0".to_string()])
                .to_mcp_result());
            }

            let has_more = all_results.len() as u64 > offset.saturating_add(limit);
            let paginated_results: Vec<_> = all_results.into_iter().skip(offset as usize).take(limit as usize).collect();

            let mut output_text = format!(
                "Found {} relevant knowledge entries (mode: {}):\n\n| ID | Score | Source | Tags | Content |\n|---|---|---|---|---|\n",
                paginated_results.len(),
                mode
            );

            for (model, score) in &paginated_results {
                let tags = parse_db_tags(model.tags.as_ref());
                let source = model.source.as_deref().unwrap_or("unknown");
                let tags_str = if tags.is_empty() {
                    "none".to_string()
                } else {
                    tags.join(", ")
                };

                let safe_source = source.replace("|", "\\|").replace("\n", " ");
                let safe_tags = tags_str.replace("|", "\\|").replace("\n", " ");
                let safe_content = model.content.replace("|", "\\|").replace("\n", " ");

                output_text.push_str(&format!(
                    "| `{}` | {:.4} | {} | {} | {} |\n",
                    model.id,
                    score,
                    safe_source,
                    safe_tags,
                    safe_content
                ));
            }

            if paginated_results.is_empty() {
                output_text = "No relevant knowledge found for your query.".to_string();
            } else if has_more {
                output_text.push_str(&format!(
                    "\n*(Showing {} to {} results. Call this tool again with offset: {} to see more)*",
                    offset + 1,
                    offset + paginated_results.len() as u64,
                    offset + limit
                ));
            }

            Ok(
                SuccessHint::new(output_text, vec![]).to_mcp_result_with_data(Some(json!({
                    "mode": mode,
                    "offset": offset,
                    "results": paginated_results.iter().map(|(m, d)| json!({
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
        .with_guidance(vec!["Try using simpler search queries or exploring context around known entities.".to_string()])
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

            let summary = format_graph_context(entity_name, depth, &context);
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

fn format_graph_context(entity_name: &str, depth: u32, context: &Value) -> String {
    let nodes = context
        .get("nodes")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let edges = context
        .get("edges")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let linked_chunks = context
        .get("linked_chunks")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    let mut node_names = HashMap::<i64, String>::new();
    let mut output = format!(
        "Graph context for '{}' (Depth: {})\n\nNodes ({}):\n",
        entity_name,
        depth,
        nodes.len()
    );

    for node in &nodes {
        let id = node
            .get("id")
            .and_then(|value| value.as_i64())
            .unwrap_or_default();
        let name = node
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        let entity_type = node
            .get("type")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        let node_depth = node
            .get("depth")
            .and_then(|value| value.as_i64())
            .unwrap_or_default();
        let description = node
            .get("description")
            .and_then(|value| value.as_str())
            .unwrap_or("");

        node_names.insert(id, name.to_string());
        output.push_str(&format!(
            "- [{}] {} | type: {} | depth: {}{}\n",
            id,
            name,
            entity_type,
            node_depth,
            if description.is_empty() {
                String::new()
            } else {
                format!(" | {}", description)
            }
        ));
    }

    output.push_str(&format!("\nEdges ({}):\n", edges.len()));
    if edges.is_empty() {
        output.push_str("- none\n");
    } else {
        for edge in &edges {
            let source_id = edge
                .get("source_id")
                .and_then(|value| value.as_i64())
                .unwrap_or_default();
            let target_id = edge
                .get("target_id")
                .and_then(|value| value.as_i64())
                .unwrap_or_default();
            let relation_type = edge
                .get("type")
                .and_then(|value| value.as_str())
                .unwrap_or("RELATED_TO");
            let source_name = node_names
                .get(&source_id)
                .map(String::as_str)
                .unwrap_or("unknown");
            let target_name = node_names
                .get(&target_id)
                .map(String::as_str)
                .unwrap_or("unknown");

            output.push_str(&format!(
                "- {} ({}) -[{}]-> {} ({})\n",
                source_name, source_id, relation_type, target_name, target_id
            ));
        }
    }

    output.push_str(&format!(
        "\nLinked knowledge chunks ({}):\n",
        linked_chunks.len()
    ));
    if linked_chunks.is_empty() {
        output.push_str("- none\n");
    } else {
        for chunk in &linked_chunks {
            let chunk_id = chunk
                .get("id")
                .and_then(|value| value.as_i64())
                .unwrap_or_default();
            let source = chunk
                .get("source")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");
            let content = chunk
                .get("content")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            output.push_str(&format!(
                "- Chunk {} | source: {} | {}\n",
                chunk_id, source, content
            ));
        }
    }

    output
}
