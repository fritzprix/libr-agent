use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::repositories::AssistantRepository;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

type ServerMapCache = Mutex<Option<(Instant, HashMap<String, String>)>>;

static SERVER_MAP_CACHE: once_cell::sync::Lazy<ServerMapCache> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));

async fn get_server_id_to_name_map() -> HashMap<String, String> {
    const CACHE_TTL: Duration = Duration::from_secs(30);

    // Try to get from cache first
    if let Ok(cache) = SERVER_MAP_CACHE.lock() {
        if let Some((timestamp, map)) = &*cache {
            if timestamp.elapsed() < CACHE_TTL {
                return map.clone();
            }
        }
    }

    // Cache miss or expired - fetch from DB
    let repo = crate::state::get_mcp_server_repository();
    let map: HashMap<String, String> = if let Ok(models) = repo.list().await {
        models.into_iter().map(|m| (m.id, m.name)).collect()
    } else {
        HashMap::new()
    };

    // Update cache
    if let Ok(mut cache) = SERVER_MAP_CACHE.lock() {
        *cache = Some((Instant::now(), map.clone()));
    }

    map
}

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
            let mut assistants_text = String::new();
            assistants_text.push_str("| Name | ID | Description |\n");
            assistants_text.push_str("|---|---|---|\n");
            for a in &assistants {
                let description = extract_assistant_description(&a["config"]);
                let name = a["name"].as_str().unwrap_or("?");
                let id = a["id"].as_str().unwrap_or("?");

                let name_clean = name.replace('|', "\\|").replace('\n', " ");
                let id_clean = id.replace('|', "\\|").replace('\n', " ");
                let description_clean = description.replace('|', "\\|").replace('\n', " ");

                assistants_text.push_str(&format!("| {} | `{}` | {} |\n", name_clean, id_clean, description_clean));
            }

            let hint = SuccessHint::new(
                if has_more {
                    format!(
                        "Found {} assistants (showing {} to {}):\n\n{}\n\n*(Showing {} to {} of {} items. Call this tool again with offset: {} to see more)*",
                        total_count,
                        offset + 1,
                        offset + assistants.len() as u64,
                        assistants_text,
                        offset + 1,
                        offset + assistants.len() as u64,
                        total_count,
                        offset + limit
                    )
                } else if assistants.is_empty() {
                    format!(
                        "No more assistants at offset {} (total: {}).",
                        offset, total_count
                    )
                } else {
                    let returned = assistants.len();
                    format!(
                        "Found {} {}:\n\n{}",
                        returned,
                        if returned == 1 {
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
                } else if assistants.is_empty() && total_count > 0 {
                    vec![format!(
                        "Offset {} is past the end (total: {}). Use offset=0 to list from the beginning.",
                        offset, total_count
                    )]
                } else if total_count > 0 {
                    vec!["Inspect a configuration entry to view full details".to_string()]
                } else {
                    vec!["Create a new agent configuration to get started".to_string()]
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
            ToolGroup::Agent,
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
        Option::None => return Ok(missing_param_error("query", ToolGroup::Agent)),
    };

    let limit = args
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(10)
        .min(100);

    let offset = args
        .get("offset")
        .and_then(|v| v.as_i64())
        .map(|v| v.max(0) as u64)
        .unwrap_or(0);

    let result = repo.search_assistants(query).await;

    match result {
        Ok(models) => {
            let total = models.len();
            let assistants: Vec<Value> = models
                .into_iter()
                .skip(offset as usize)
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
            let mut assistants_text = String::new();
            if !assistants.is_empty() {
                assistants_text.push_str("| Name | ID | Description |\n");
                assistants_text.push_str("|---|---|---|\n");
                for a in &assistants {
                    let description = extract_assistant_description(&a["config"]);
                    let name = a["name"].as_str().unwrap_or("?");
                    let id = a["id"].as_str().unwrap_or("?");

                    let name_clean = name.replace('|', "\\|").replace('\n', " ");
                    let id_clean = id.replace('|', "\\|").replace('\n', " ");
                    let description_clean = description.replace('|', "\\|").replace('\n', " ");

                    assistants_text.push_str(&format!("| {} | `{}` | {} |\n", name_clean, id_clean, description_clean));
                }
            }

            let start = offset + 1;
            let end = offset + assistants.len() as u64;

            let message = if assistants.is_empty() {
                if total == 0 {
                    format!("Found 0 assistants matching '{}'", query)
                } else {
                    format!("No results for this page (offset {}, limit {}). Try a smaller offset.", offset, limit)
                }
            } else if end < total as u64 {
                format!(
                    "Found {} assistants matching '{}':\n\n{}\n*(Showing {} to {} of {} items. Call this tool again with offset: {} to see more)*",
                    total,
                    query,
                    assistants_text,
                    start,
                    end,
                    total,
                    offset + limit as u64
                )
            } else if offset > 0 {
                format!(
                    "Found {} assistants matching '{}':\n\n{}\n*(Showing {} to {} of {} items)*",
                    total,
                    query,
                    assistants_text,
                    start,
                    end,
                    total
                )
            } else {
                format!(
                    "Found {} assistants matching '{}':\n\n{}",
                    total,
                    query,
                    assistants_text
                )
            };

            let hint = SuccessHint::new(
                message,
                if assistants.is_empty() {
                    vec!["List all agent configurations to broaden the search".to_string()]
                } else {
                    vec!["Inspect a matching configuration to view full details".to_string()]
                },
            );

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "assistants": assistants,
                "count": assistants.len(),
                "total": total,
                "limit": limit,
                "offset": offset
            }))))
        }
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Failed to search assistants for '{}': {}", query, e),
            ToolGroup::Agent,
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
        Option::None => return Ok(missing_param_error("id", ToolGroup::Agent)),
    };

    let result = repo.get_assistant(id).await;

    match result {
        Ok(Some(model)) => {
            // Parse config JSON with error logging
            let config = serde_json::from_str::<Value>(&model.config).unwrap_or_else(|e| {
                log::warn!("Failed to parse config for assistant {}: {}", model.id, e);
                json!({})
            });

            // Resolve server names for display
            let server_map = get_server_id_to_name_map().await;

            // Extract fields for report
            let system_prompt = config
                .get("systemPrompt")
                .and_then(|v| v.as_str())
                .unwrap_or("(No system prompt)");

            let model_name = config
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");

            let provider = config
                .get("provider")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");

            // Format Tools
            let tools_list = config
                .get("allowedBuiltInServiceAliases")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_else(|| "None".to_string());

            // Format MCP Servers
            let mcp_servers_list = config
                .get("mcpServerIds")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    if arr.is_empty() {
                        return "None".to_string();
                    }
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|id| {
                            let name = server_map.get(id).map(|s| s.as_str()).unwrap_or("Unknown");
                            format!("- {} (ID: {})", name, id)
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_else(|| "None".to_string());

            let report = format!(
                "## Assistant Report: {}\n\n**ID:** `{}`\n**Model:** {} ({})\n\n### System Prompt (Personality & Instructions)\n```\n{}\n```\n\n### Capabilities (Weapons)\n**Built-in Tools:**\n{}\n\n**External Capability Servers (MCP):**\n{}",
                model.name, model.id, model_name, provider, system_prompt, tools_list, mcp_servers_list
            );

            let hint = SuccessHint::new(
                report,
                vec![
                    "Update this configuration to refine its prompt, temperature, or capabilities"
                        .to_string(),
                    "List other agent configurations to compare available specialists".to_string(),
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
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            "List agent configurations to find the correct ID".to_string()
        ])
        .to_mcp_result()),
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Failed to get assistant '{}': {}", id, e),
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            "Check database connectivity".to_string(),
            "Verify the assistant ID format".to_string(),
        ])
        .to_mcp_result()),
    }
}
