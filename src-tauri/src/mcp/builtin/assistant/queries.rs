use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::repositories::AssistantRepository;
use serde_json::{json, Value};

fn truncate_text(input: &str, max_chars: usize) -> String {
    let normalized = input.replace('\n', " ").trim().to_string();
    if normalized.chars().count() <= max_chars {
        return normalized;
    }

    let mut truncated = String::new();
    for ch in normalized.chars().take(max_chars) {
        truncated.push(ch);
    }
    truncated.push_str("...");
    truncated
}

fn extract_assistant_description(config: &Value) -> String {
    if let Some(description) = config.get("description").and_then(|v| v.as_str()) {
        let cleaned = description.trim();
        if !cleaned.is_empty() {
            return truncate_text(cleaned, 140);
        }
    }

    if let Some(system_prompt) = config.get("systemPrompt").and_then(|v| v.as_str()) {
        let first_meaningful_line = system_prompt
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("");

        if !first_meaningful_line.is_empty() {
            return truncate_text(first_meaningful_line, 140);
        }
    }

    "No description".to_string()
}

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
                    let description = extract_assistant_description(&a["config"]);
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
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Failed to list assistants: {}", e),
            ToolGroup::Assistant,
        )
        .with_guidance(vec![
            "Check database connectivity".to_string(),
            "Verify pagination parameters are valid integers".to_string(),
        ])
        .to_mcp_result()),
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
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Failed to search assistants for '{}': {}", query, e),
            ToolGroup::Assistant,
        )
        .with_guidance(vec![
            "Check database connectivity".to_string(),
            "Verify query parameter is a valid string".to_string(),
        ])
        .to_mcp_result()),
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
        Ok(None) => Ok(guided_error(
            ErrorCategory::ResourceNotFound,
            format!("Assistant '{}' not found", id),
            ToolGroup::Assistant,
        )
        .with_guidance(vec![
            "Use builtin_assistant__listAssistants to find the correct ID".to_string(),
        ])
        .to_mcp_result()),
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Failed to get assistant '{}': {}", id, e),
            ToolGroup::Assistant,
        )
        .with_guidance(vec![
            "Check database connectivity".to_string(),
            "Verify the assistant ID format".to_string(),
        ])
        .to_mcp_result()),
    }
}
