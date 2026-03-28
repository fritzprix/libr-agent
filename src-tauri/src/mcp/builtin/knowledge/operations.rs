use super::{
    embed,
    extraction::{self, ExtractedEntity, ExtractedRelationship},
    KnowledgeServer,
};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::repositories::KnowledgeV2Repository;
use serde_json::{json, Value};
use std::collections::HashMap;

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

    let tags = match args.get("tags") {
        None => None,
        Some(value) if value.is_array() => {
            match serde_json::from_value::<Vec<String>>(value.clone()) {
                Ok(parsed_tags) => Some(parsed_tags),
                Err(error) => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!("Invalid tags payload: {}", error),
                        ToolGroup::Knowledge,
                    )
                    .to_mcp_result())
                }
            }
        }
        Some(_) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Parameter 'tags' must be an array of strings.".to_string(),
                ToolGroup::Knowledge,
            )
            .to_mcp_result())
        }
    };
    let explicit_entities = match args.get("entities") {
        None => Vec::new(),
        Some(value) if value.is_array() => {
            match serde_json::from_value::<Vec<ExtractedEntity>>(value.clone()) {
                Ok(parsed_entities) => parsed_entities,
                Err(error) => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!("Invalid entities payload: {}", error),
                        ToolGroup::Knowledge,
                    )
                    .to_mcp_result())
                }
            }
        }
        Some(_) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Parameter 'entities' must be an array of objects.".to_string(),
                ToolGroup::Knowledge,
            )
            .to_mcp_result())
        }
    };
    let explicit_relationships = match args.get("relationships") {
        None => Vec::new(),
        Some(value) if value.is_array() => {
            match serde_json::from_value::<Vec<ExtractedRelationship>>(value.clone()) {
                Ok(parsed_relationships) => parsed_relationships,
                Err(error) => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!("Invalid relationships payload: {}", error),
                        ToolGroup::Knowledge,
                    )
                    .to_mcp_result())
                }
            }
        }
        Some(_) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Parameter 'relationships' must be an array of objects.".to_string(),
                ToolGroup::Knowledge,
            )
            .to_mcp_result())
        }
    };

    let auto_extract = args
        .get("auto_extract")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let explicit_plan =
        match extraction::normalize_graph_plan(explicit_entities, explicit_relationships) {
            Ok(plan) => plan,
            Err(error) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("Invalid graph payload: {}", error),
                    ToolGroup::Knowledge,
                )
                .to_mcp_result())
            }
        };
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
    let tags_json = tags.as_ref().map(serde_json::to_string).transpose();
    let tags_json = match tags_json {
        Ok(serialized_tags) => serialized_tags,
        Err(error) => {
            return Ok(guided_error(
                ErrorCategory::InternalError,
                format!("Failed to serialize tags: {}", error),
                ToolGroup::Knowledge,
            )
            .to_mcp_result())
        }
    };

    match repo
        .record_chunk(
            assistant_id.to_string(),
            content.to_string(),
            tags_json,
            source.clone(),
            embedding,
        )
        .await
    {
        Ok(chunk_id) => {
            let heuristic_plan = if auto_extract
                && (explicit_plan.entities.is_empty() || explicit_plan.relationships.is_empty())
            {
                extraction::extract_graph_from_content(content, tags.as_deref().unwrap_or(&[]))
            } else {
                extraction::ExtractionPlan::default()
            };
            let extraction_plan = extraction::merge_plans(&explicit_plan, &heuristic_plan);

            let enrichment_summary = if !extraction_plan.entities.is_empty()
                || !extraction_plan.relationships.is_empty()
            {
                match persist_extraction_plan(
                    repo,
                    assistant_id,
                    chunk_id,
                    &extraction_plan,
                    &explicit_plan,
                    &heuristic_plan,
                )
                .await
                {
                    Ok(summary) => Some(summary),
                    Err(error) => {
                        return Ok(guided_error(
                            ErrorCategory::DatabaseError,
                            format!(
                                "Knowledge chunk {} was saved, but graph enrichment failed: {}",
                                chunk_id, error
                            ),
                            ToolGroup::Knowledge,
                        )
                        .to_mcp_result());
                    }
                }
            } else {
                None
            };

            let mut details = vec![format!(
                "Knowledge recorded successfully (ID: {})",
                chunk_id
            )];
            if let Some(summary) = &enrichment_summary {
                details.push(format!(
                    "Graph persistence linked {} entities and created {} relationships.",
                    summary.entity_count, summary.relationship_count
                ));
                details.push(format!(
                    "Structured graph input: {} entities, {} relationships. Heuristic fallback: {} entities, {} relationships.",
                    summary.explicit_entity_count,
                    summary.explicit_relationship_count,
                    summary.heuristic_entity_count,
                    summary.heuristic_relationship_count
                ));
            } else {
                details.push("Graph enrichment was skipped for this entry.".to_string());
            }

            let mut next_steps = vec!["Use search_knowledge to query this information".to_string()];
            if !extraction_plan.entities.is_empty() {
                next_steps.push(
                    "Use explore_context with one of the extracted entities to inspect the relationship graph."
                        .to_string(),
                );
            }

            let hint = SuccessHint::new(details.join("\n"), next_steps);
            Ok(hint.to_mcp_result_with_data(Some(json!({
                "success": true,
                "id": chunk_id,
                "source": source,
                "auto_extract": auto_extract,
                "tags": tags,
                "supplied_entities": explicit_plan.entities,
                "supplied_relationships": explicit_plan.relationships,
                "heuristic_entities": heuristic_plan.entities,
                "heuristic_relationships": heuristic_plan.relationships,
                "extracted_entities": extraction_plan.entities.iter().map(|entity| json!({
                    "name": entity.name,
                    "entity_type": entity.entity_type,
                    "description": entity.description,
                })).collect::<Vec<_>>(),
                "extracted_relationships": extraction_plan.relationships.iter().map(|relationship| json!({
                    "source": relationship.source,
                    "target": relationship.target,
                    "relation_type": relationship.relation_type,
                })).collect::<Vec<_>>(),
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

#[derive(Debug, Clone, Copy)]
struct EnrichmentSummary {
    entity_count: usize,
    relationship_count: usize,
    explicit_entity_count: usize,
    explicit_relationship_count: usize,
    heuristic_entity_count: usize,
    heuristic_relationship_count: usize,
}

async fn persist_extraction_plan(
    repo: &dyn KnowledgeV2Repository,
    assistant_id: &str,
    chunk_id: i32,
    extraction_plan: &extraction::ExtractionPlan,
    explicit_plan: &extraction::ExtractionPlan,
    heuristic_plan: &extraction::ExtractionPlan,
) -> Result<EnrichmentSummary, String> {
    let mut entity_ids = HashMap::<String, i32>::new();

    for entity in &extraction_plan.entities {
        let entity_id = repo
            .upsert_entity(
                assistant_id.to_string(),
                entity.name.clone(),
                entity.entity_type.clone(),
                entity.description.clone(),
            )
            .await
            .map_err(|error| error.to_string())?;
        repo.link_chunk_to_entity(chunk_id, entity_id)
            .await
            .map_err(|error| error.to_string())?;
        entity_ids.insert(entity.name.to_ascii_lowercase(), entity_id);
    }

    let mut relationship_count = 0;
    for relationship in &extraction_plan.relationships {
        let source_id = entity_ids
            .get(&relationship.source.to_ascii_lowercase())
            .copied()
            .ok_or_else(|| {
                format!(
                    "Missing source entity '{}' while persisting relationships",
                    relationship.source
                )
            })?;
        let target_id = entity_ids
            .get(&relationship.target.to_ascii_lowercase())
            .copied()
            .ok_or_else(|| {
                format!(
                    "Missing target entity '{}' while persisting relationships",
                    relationship.target
                )
            })?;

        if source_id != target_id {
            repo.create_relationship(
                assistant_id.to_string(),
                source_id,
                target_id,
                relationship.relation_type.clone(),
            )
            .await
            .map_err(|error| error.to_string())?;
            relationship_count += 1;
        }
    }

    Ok(EnrichmentSummary {
        entity_count: extraction_plan.entities.len(),
        relationship_count,
        explicit_entity_count: explicit_plan.entities.len(),
        explicit_relationship_count: explicit_plan.relationships.len(),
        heuristic_entity_count: heuristic_plan.entities.len(),
        heuristic_relationship_count: heuristic_plan.relationships.len(),
    })
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
        _ => Ok(guided_error(
            ErrorCategory::InvalidInput,
            format!("Action '{}' is not supported for prune_knowledge.", action),
            ToolGroup::Knowledge,
        )
        .with_guidance(vec![
            "Use action='delete' to remove knowledge chunks.".to_string()
        ])
        .to_mcp_result()),
    }
}
