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
    server: &KnowledgeServer,
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

    let repo = server.repository();
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
            let mut output_text = format!(
                "Found {} relevant knowledge entries (mode: {}):\n\n",
                results.len(),
                mode
            );
            for (model, score) in &results {
                let tags = parse_db_tags(model.tags.as_ref());
                let source = model.source.as_deref().unwrap_or("unknown");
                output_text.push_str(&format!(
                    "### Chunk ID: {} (Score: {:.4})\nSource: {}\nTags: {}\n{}\n\n",
                    model.id,
                    score,
                    source,
                    if tags.is_empty() {
                        "none".to_string()
                    } else {
                        tags.join(", ")
                    },
                    model.content
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
    server: &KnowledgeServer,
    args: Value,
    assistant_id: &str,
) -> Result<MCPResult, String> {
    let entity_name = match args.get("entity_name").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => return Ok(missing_param_error("entity_name", ToolGroup::Knowledge)),
    };

    let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(1) as u32;

    let repo = server.repository();
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

    if nodes.is_empty() {
        output.push_str("- none\n");
    } else {
        output.push_str("| ID | Name | Type | Depth | Description |\n|---|---|---|---|---|\n");
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
                .unwrap_or("")
                .replace('\n', " ");

            node_names.insert(id, name.to_string());
            output.push_str(&format!(
                "| `{}` | {} | {} | {} | {} |\n",
                id, name, entity_type, node_depth, description
            ));
        }
    }

    output.push_str(&format!("\nEdges ({}):\n", edges.len()));
    if edges.is_empty() {
        output.push_str("- none\n");
    } else {
        output.push_str("| Source | Relation | Target |\n|---|---|---|\n");
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
                "| `{}` ({}) | {} | `{}` ({}) |\n",
                source_id, source_name, relation_type, target_id, target_name
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
        output.push_str("| Chunk ID | Source | Content |\n|---|---|---|\n");
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
                .unwrap_or("")
                .replace('\n', "<br>");
            output.push_str(&format!("| `{}` | {} | {} |\n", chunk_id, source, content));
        }
    }

    output
}
