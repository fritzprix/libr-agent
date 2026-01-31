use crate::mcp::builtin::error_guidance::{
    missing_param_error, not_found_error, operation_failed_error, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::repositories::AssistantRepository;
use serde_json::{json, Value};

/// List all assistants with pagination support
pub async fn list_assistants(
    db: &sea_orm::DatabaseConnection,
    args: Value,
) -> Result<MCPResult, String> {
    // Use repository from db connection instead of global state
    let repo = crate::repositories::SqliteAssistantRepository::new(db.clone());

    // Legacy support: page/pageSize -> limit/offset
    let page = args
        .get("page")
        .and_then(|v| v.as_i64())
        .unwrap_or(1)
        .max(1) as u64;
    let page_size = args
        .get("pageSize")
        .and_then(|v| v.as_i64())
        .unwrap_or(20)
        .clamp(1, 100) as u64;

    // Modern API: limit/offset (takes precedence)
    let limit = args
        .get("limit")
        .and_then(|v| v.as_i64())
        .map(|v| v.clamp(1, 100) as u64)
        .unwrap_or(page_size);
    let offset = args
        .get("offset")
        .and_then(|v| v.as_i64())
        .map(|v| v.max(0) as u64)
        .unwrap_or((page - 1) * page_size);

    // Get total count for pagination metadata
    let total_count = repo.count_assistants().await.unwrap_or(0);

    // Fetch paginated results using database-level pagination
    let result = repo.list_assistants_paginated(limit, offset).await;

    match result {
        Ok(models) => {
            let assistants: Vec<Value> = models
                .into_iter()
                .map(|model| {
                    // Parse config JSON with error logging
                    let config = serde_json::from_str::<Value>(&model.config).unwrap_or_else(|e| {
                        log::warn!("Failed to parse config for assistant {}: {}", model.id, e);
                        json!({})
                    });

                    json!({
                        "id": model.id,
                        "name": model.name,
                        "config": config,
                        "created_at": model.created_at,
                        "updated_at": model.updated_at
                    })
                })
                .collect();

            let has_more = (offset + limit) < total_count;

            // Format list for AI readability
            let assistants_text = assistants
                .iter()
                .map(|a| {
                    let description = a["config"]
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("No description");
                    format!(
                        "• {} [ID: {}]\n  Description: {}",
                        a["name"].as_str().unwrap_or("?"),
                        a["id"].as_str().unwrap_or("?"),
                        description
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");

            let hint = SuccessHint::new(
                if has_more {
                    format!(
                        "Found {} assistants (showing {} to {}):\n\n{}",
                        total_count,
                        offset + 1,
                        offset + assistants.len() as u64,
                        assistants_text
                    )
                } else {
                    format!(
                        "Found {} {}:\n\n{}",
                        total_count,
                        if total_count == 1 {
                            "assistant"
                        } else {
                            "assistants"
                        },
                        assistants_text
                    )
                },
                if has_more {
                    vec![format!(
                        "Use limit={} offset={} to see more assistants",
                        limit,
                        offset + limit
                    )]
                } else if total_count > 0 {
                    vec!["Use builtin_assistant__getAssistant to view details".to_string()]
                } else {
                    vec![
                        "Use builtin_assistant__createAssistant to create an assistant".to_string(),
                    ]
                },
            );

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "assistants": assistants,
                "total": total_count,
                "limit": limit,
                "offset": offset,
                "returned": assistants.len(),
                "has_more": has_more
            }))))
        }
        Err(e) => Ok(operation_failed_error(
            "List assistants",
            &e.to_string(),
            vec![
                "Check database connectivity".to_string(),
                "Verify pagination parameters are valid integers".to_string(),
            ],
            ToolGroup::Assistant,
        )),
    }
}

/// Search assistants
pub async fn search_assistant(
    db: &sea_orm::DatabaseConnection,
    args: Value,
) -> Result<MCPResult, String> {
    // Use repository from db connection
    let repo = crate::repositories::SqliteAssistantRepository::new(db.clone());

    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(v) => v,
        Option::None => return Ok(missing_param_error("query", ToolGroup::Assistant)),
    };

    let limit = args
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(10)
        .min(100);

    let result = repo.search_assistants(query).await;

    match result {
        Ok(models) => {
            let assistants: Vec<Value> = models
                .into_iter()
                .take(limit as usize)
                .map(|model| {
                    let config = serde_json::from_str::<Value>(&model.config).unwrap_or_else(|e| {
                        log::warn!("Failed to parse config for assistant {}: {}", model.id, e);
                        json!({})
                    });
                    json!({
                        "id": model.id,
                        "name": model.name,
                        "config": config,
                        "created_at": model.created_at,
                        "updated_at": model.updated_at
                    })
                })
                .collect();

            // Format list for AI readability
            let assistants_text = assistants
                .iter()
                .map(|a| {
                    format!(
                        "• {} [ID: {}]",
                        a["name"].as_str().unwrap_or("?"),
                        a["id"].as_str().unwrap_or("?")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            let hint = SuccessHint::new(
                format!(
                    "Found {} assistants matching '{}':\n\n{}",
                    assistants.len(),
                    query,
                    assistants_text
                ),
                if assistants.is_empty() {
                    vec!["Use builtin_assistant__listAssistants to see all assistants".to_string()]
                } else {
                    vec!["Use builtin_assistant__getAssistant to view details".to_string()]
                },
            );

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "assistants": assistants,
                "count": assistants.len()
            }))))
        }
        Err(e) => Ok(operation_failed_error(
            "Search assistants",
            &e.to_string(),
            vec![
                "Check database connectivity".to_string(),
                "Verify query parameter is a valid string".to_string(),
            ],
            ToolGroup::Assistant,
        )),
    }
}

/// Get an assistant by ID
pub async fn get_assistant(
    db: &sea_orm::DatabaseConnection,
    args: Value,
) -> Result<MCPResult, String> {
    // Use repository from db connection
    let repo = crate::repositories::SqliteAssistantRepository::new(db.clone());

    let id = match args.get("id").and_then(|v| v.as_str()) {
        Some(v) => v,
        Option::None => return Ok(missing_param_error("id", ToolGroup::Assistant)),
    };

    let result = repo.get_assistant(id).await;

    match result {
        Ok(Some(model)) => {
            // Parse config JSON with error logging
            let config = serde_json::from_str::<Value>(&model.config).unwrap_or_else(|e| {
                log::warn!("Failed to parse config for assistant {}: {}", model.id, e);
                json!({})
            });

            let config_display =
                serde_json::to_string_pretty(&config).unwrap_or_else(|_| "{}".to_string());

            let hint = SuccessHint::new(
                format!(
                    "Assistant: {}\nID: {}\n\nConfiguration:\n{}",
                    model.name, model.id, config_display
                ),
                vec![
                    "Use builtin_assistant__updateAssistant to modify configuration".to_string(),
                    "Use builtin_assistant__deleteAssistant to remove this assistant".to_string(),
                ],
            );

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "id": model.id,
                "name": model.name,
                "config": config,
                "created_at": model.created_at,
                "updated_at": model.updated_at
            }))))
        }
        Ok(None) => Ok(not_found_error("Assistant", id, ToolGroup::Assistant)),
        Err(e) => Ok(operation_failed_error(
            "Get assistant",
            &e.to_string(),
            vec![
                "Verify the assistant ID is correct".to_string(),
                "Use builtin_assistant__listAssistants to see existing assistants".to_string(),
                "Check database connectivity".to_string(),
            ],
            ToolGroup::Assistant,
        )),
    }
}
