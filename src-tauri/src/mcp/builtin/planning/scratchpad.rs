use crate::entity::planning_scratchpad;
use crate::mcp::builtin::error_guidance::{
    guided_error, invalid_input_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::repositories::PlanningRepository;
use crate::state::get_planning_repository;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

/// Add scratchpad item (Legacy: addScratchpad)
pub async fn add_scratchpad(
    _db: &DatabaseConnection,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let note = args
        .get("note")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let note_content = match note {
        Some(n) => n,
        None => return Ok(missing_param_error("note", ToolGroup::Planning)),
    };

    let title = args.get("title").and_then(|v| v.as_str()).map(|s| s.trim());
    let source = args
        .get("source")
        .and_then(|v| v.as_str())
        .map(|s| s.trim());
    let tags = args.get("tags").map(|v| v.to_string()); // Store as JSON string

    // Clone for async move
    let session_id_owned = session_id.to_string();
    let note_owned = note_content.to_string();
    let title_owned = title.map(|s| s.to_string());
    let source_owned = source.map(|s| s.to_string());

    let repo = get_planning_repository();

    // 1. Check limit
    match repo.check_scratchpad_limit(&session_id_owned).await {
        Ok(count) => {
            if count >= 10 {
                return Ok(guided_error(
                    ErrorCategory::InvalidState,
                    "Scratchpad limit reached (10 items)",
                    ToolGroup::Planning,
                )
                .with_guidance(vec![
                    "Use updateScratchpad to modify existing notes".to_string(),
                    "Use clearScratchpad to remove old items".to_string(),
                ])
                .to_mcp_result());
            }
        }
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::DatabaseError,
                format!("Database error: {}", e),
                ToolGroup::Planning,
            )
            .with_guidance(vec!["Try again".to_string()])
            .to_mcp_result())
        }
    }

    // 2. Check duplicate title
    if let Some(ref t) = title_owned {
        match repo.check_scratchpad_duplicate(&session_id_owned, t).await {
            Ok(is_dup) => {
                if is_dup {
                    return Ok(guided_error(
                        ErrorCategory::DuplicateResource,
                        format!("Scratchpad item with title '{}' already exists", t),
                        ToolGroup::Planning,
                    )
                    .with_guidance(vec![
                        "Use updateScratchpad to modify the existing note".to_string(),
                        "Choose a different title for the new note".to_string(),
                    ])
                    .to_mcp_result());
                }
            }
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::DatabaseError,
                    format!("Database error: {}", e),
                    ToolGroup::Planning,
                )
                .with_guidance(vec!["Try again".to_string()])
                .to_mcp_result())
            }
        }
    }

    // 3. Insert
    match repo
        .add_scratchpad(
            &session_id_owned,
            title_owned,
            &note_owned,
            source_owned,
            tags,
        )
        .await
    {
        Ok(id) => {
            let count = repo
                .check_scratchpad_limit(&session_id_owned)
                .await
                .unwrap_or(0); // Best effort count
            let response_id = cuid2::create_id();
            let hint = SuccessHint::new(
                format!(
                    "✓ Note added to scratchpad (ID: {})\nScratchpad: {}/10",
                    id, count
                ),
                vec![
                    "Use listScratchpad to see all items".to_string(),
                    "Use readScratchpad to view full content".to_string(),
                ],
            );
            Ok(hint.to_mcp_result_with_data(Some(json!({
                "id": response_id,
                "scratchpadId": id
            }))))
        }
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Database error: {}", e),
            ToolGroup::Planning,
        )
        .with_guidance(vec!["Try again".to_string()])
        .to_mcp_result()),
    }
}

/// Update scratchpad item
pub async fn update_scratchpad(
    _db: &DatabaseConnection,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let id = args.get("id").and_then(|v| v.as_i64());

    let note = args
        .get("note")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let id_val = match id {
        Some(i) => i,
        None => return Ok(missing_param_error("id", ToolGroup::Planning)),
    };
    let note_val = match note {
        Some(n) => n,
        None => return Ok(missing_param_error("note", ToolGroup::Planning)),
    };

    let new_title = args
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let repo = get_planning_repository();

    match repo
        .update_scratchpad_by_id(
            session_id,
            id_val,
            note_val,
            new_title.map(|s| s.to_string()),
        )
        .await
    {
        Ok(found) => {
            if found {
                let hint = SuccessHint::new(
                    format!("✓ Scratchpad note (ID: {}) updated", id_val),
                    vec![
                        "Use readScratchpad or listScratchpad to verify".to_string(),
                        "Use getCurrentState to see updated context".to_string(),
                    ],
                );
                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "id": cuid2::create_id(),
                    "success": true,
                    "scratchpadId": id_val,
                    "note": note_val
                }))))
            } else {
                Ok(guided_error(
                    ErrorCategory::ResourceNotFound,
                    format!("Scratchpad note with ID {} not found", id_val),
                    ToolGroup::Planning,
                )
                .with_guidance(vec![
                    "Use listScratchpad or getCurrentState to see available notes".to_string(),
                    "Verify the ID is correct".to_string(),
                ])
                .to_mcp_result())
            }
        }
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Database error: {}", e),
            ToolGroup::Planning,
        )
        .with_guidance(vec!["Try again".to_string()])
        .to_mcp_result()),
    }
}

/// List scratchpad items (Legacy: listScratchpad)
pub async fn list_scratchpad(
    _db: &DatabaseConnection,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("pageSize").and_then(|v| v.as_i64()).unwrap_or(10);

    if page < 1 {
        return Ok(invalid_input_error(
            "page must be >= 1",
            ToolGroup::Planning,
        ));
    }
    if page_size < 1 {
        return Ok(invalid_input_error(
            "pageSize must be >= 1",
            ToolGroup::Planning,
        ));
    }

    let filter_tags = args.get("tags").and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Vec<String>>()
    });

    let repo = get_planning_repository();
    let all_items = match repo.list_scratchpad(session_id).await {
        Ok(items) => items,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::DatabaseError,
                format!("Failed to list scratchpad: {}", e),
                ToolGroup::Planning,
            )
            .with_guidance(vec!["Try again".to_string()])
            .to_mcp_result())
        }
    };

    // Filter in memory
    let filtered_items: Vec<&planning_scratchpad::Model> = if let Some(tags) = &filter_tags {
        if tags.is_empty() {
            all_items.iter().collect()
        } else {
            all_items
                .iter()
                .filter(|item| {
                    if let Some(item_tags_json) = &item.tags {
                        if let Ok(item_tags) = serde_json::from_str::<Vec<String>>(item_tags_json) {
                            tags.iter().any(|t| item_tags.contains(t))
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                })
                .collect()
        }
    } else {
        all_items.iter().collect()
    };

    // Paginate
    let total_items = filtered_items.len();
    let skip = ((page - 1) * page_size) as usize;
    let take = page_size as usize;
    let paged_items = filtered_items
        .into_iter()
        .skip(skip)
        .take(take)
        .collect::<Vec<_>>();

    // Format Output (Same as before)
    let mut text_output = String::new();
    if paged_items.is_empty() {
        if total_items > 0 {
            text_output.push_str(&format!(
                "No items on page {} (Total: {}).",
                page, total_items
            ));
        } else {
            text_output.push_str("No scratchpad notes found.");
        }
    } else {
        text_output.push_str(&format!(
            "Scratchpad Notes (Page {}/{}):\n",
            page,
            (total_items as f64 / page_size as f64).ceil() as u64
        ));
        for item in &paged_items {
            let id = item.id;
            let title = item.title.clone().unwrap_or_else(|| "Untitled".to_string());
            let preview = if item.content.chars().count() > 200 {
                let truncated: String = item.content.chars().take(200).collect();
                format!("{}...", truncated.replace('\n', " "))
            } else {
                item.content.replace('\n', " ")
            };
            let tags_str = if let Some(t) = &item.tags {
                if let Ok(parsed) = serde_json::from_str::<Vec<String>>(t) {
                    if !parsed.is_empty() {
                        format!(" [{}]", parsed.join(", "))
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            text_output.push_str(&format!(
                "- **ID: {}** | {} | {}{}\n",
                id, title, preview, tags_str
            ));
        }
    }

    let json_items: Vec<Value> = paged_items
        .into_iter()
        .map(|item| {
            json!({
                "id": item.id,
                "title": item.title,
                "preview": if item.content.chars().count() > 200 {
                    let truncated: String = item.content.chars().take(200).collect();
                    format!("{}...", truncated)
                } else {
                    item.content.clone()
                },
                "tags": item.tags.clone().and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok()),
                "created_at": item.created_at
            })
        })
        .collect();

    let guidance = if total_items == 0 {
        vec!["Use addScratchpad to create new notes".to_string()]
    } else {
        vec![
            "Use readScratchpad(ids) to read full content of specific items".to_string(),
            "Use addScratchpad to create new notes".to_string(),
        ]
    };

    let hint = SuccessHint::new(text_output, guidance);

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "items": json_items,
        "pagination": {
            "page": page,
            "pageSize": page_size,
            "total": total_items
        }
    }))))
}

/// Read scratchpad item (Legacy: readScratchpad)
pub async fn read_scratchpad(
    _db: &DatabaseConnection,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let ids = args
        .get("ids")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Missing 'ids' parameter".to_string());

    let ids_array = match ids {
        Ok(arr) => arr,
        Err(_) => return Ok(missing_param_error("ids", ToolGroup::Planning)),
    };

    let mut target_ids = Vec::new();
    for id_val in ids_array {
        if let Some(id) = id_val.as_i64() {
            if id < 0 {
                return Ok(invalid_input_error(
                    &format!("Invalid id '{}'. Must be >= 0", id),
                    ToolGroup::Planning,
                ));
            }
            target_ids.push(id);
        }
    }

    let repo = get_planning_repository();
    let retrieved_items = match repo.get_scratchpad_by_ids(target_ids).await {
        Ok(items) => items,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::DatabaseError,
                format!("Failed to read items: {}", e),
                ToolGroup::Planning,
            )
            .with_guidance(vec!["Try again".to_string()])
            .to_mcp_result())
        }
    };

    let items: Vec<Value> = retrieved_items
        .into_iter()
        .filter(|i| i.session_id == session_id)
        .map(|i| {
            json!({
                "id": i.id,
                "title": i.title,
                "content": i.content,
                "source": i.source,
                "tags": i.tags.and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok())
            })
        })
        .collect();

    let mut text_output = String::new();
    if items.is_empty() {
        text_output.push_str("No items found for the provided IDs.");
    } else {
        text_output.push_str("Read Scratchpad Items:\n");
        for item in &items {
            let title = item
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("Untitled");
            let content = item.get("content").and_then(|c| c.as_str()).unwrap_or("");
            let id = item.get("id").and_then(|i| i.as_i64()).unwrap_or(0);

            text_output.push_str(&format!("## [ID: {}] {}\n{}\n\n", id, title, content));
        }
    }

    let guidance = if items.is_empty() {
        vec![
            "Use listScratchpad to see all available items and their IDs".to_string(),
            "Verify the IDs you provided are correct".to_string(),
        ]
    } else {
        vec![
            "Use updateScratchpad to modify these items".to_string(),
            "Use clearScratchpad to remove them".to_string(),
        ]
    };

    let hint = SuccessHint::new(text_output, guidance);

    Ok(hint.to_mcp_result_with_data(Some(json!({ "items": items }))))
}

/// Clear scratchpad item (Legacy: clearScratchpad)
pub async fn clear_scratchpad(
    _db: &DatabaseConnection,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let id = args.get("id").and_then(|v| v.as_i64());

    let target_id = match id {
        Some(i) => i,
        None => return Ok(missing_param_error("id", ToolGroup::Planning)),
    };

    if target_id < 0 {
        return Ok(invalid_input_error("id must be >= 0", ToolGroup::Planning));
    }

    let repo = get_planning_repository();

    // Note: repo.delete_scratchpad_item handles session_id check
    match repo.delete_scratchpad_item(session_id, target_id).await {
        Ok(found) => {
            if found {
                let hint = SuccessHint::new(
                    format!("✓ Scratchpad item {} cleared", target_id),
                    vec![
                        "Use addScratchpad to add new items".to_string(),
                        "Use listScratchpad to see remaining items".to_string(),
                    ],
                );
                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "id": cuid2::create_id(),
                    "success": true,
                    "scratchpadId": target_id
                }))))
            } else {
                Ok(guided_error(
                    ErrorCategory::ResourceNotFound,
                    format!("Scratchpad item {} not found in this session", target_id),
                    ToolGroup::Planning,
                )
                .with_guidance(vec!["Use listScratchpad to verify item exists".to_string()])
                .to_mcp_result())
            }
        }
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Failed to clear item: {}", e),
            ToolGroup::Planning,
        )
        .with_guidance(vec![
            "Try again".to_string(),
            "Use listScratchpad to verify item exists".to_string(),
        ])
        .to_mcp_result()),
    }
}

/// Pause and think (Legacy: pauseAndThink)
pub async fn pause_and_think(args: Value) -> Result<MCPResult, String> {
    let thought = args.get("thought").and_then(|v| v.as_str()).unwrap_or("");
    let next_action = args.get("nextAction").and_then(|v| v.as_str());

    let response_id = cuid2::create_id();

    let mut message = format!("## Thinking Process\n\n**Thought:**\n{}\n", thought);

    if let Some(action) = next_action {
        message.push_str(&format!("\n**Next Action:**\n{}", action));
    }

    let hint = SuccessHint::new(
        message.clone(),
        if let Some(action) = next_action {
            vec![format!("Proceed with next action: {}", action)]
        } else {
            vec!["Continue with the plan".to_string()]
        },
    );

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "id": response_id,
        "thought": thought,
        "nextAction": next_action
    }))))
}

/// Critique and reflection (Legacy: critiqueAndReflection)
pub async fn critique_and_reflection(args: Value) -> Result<MCPResult, String> {
    let critique = args.get("critique").and_then(|v| v.as_str());
    let reflection = args.get("reflection").and_then(|v| v.as_str());
    let next_action = args.get("nextAction").and_then(|v| v.as_str());

    let critique_val = match critique {
        Some(v) => v,
        None => return Ok(missing_param_error("critique", ToolGroup::Planning)),
    };

    let reflection_val = match reflection {
        Some(v) => v,
        None => return Ok(missing_param_error("reflection", ToolGroup::Planning)),
    };

    let next_action_val = match next_action {
        Some(v) => v,
        None => return Ok(missing_param_error("nextAction", ToolGroup::Planning)),
    };

    let response_id = cuid2::create_id();

    let message = format!(
        "## Reflection & Critique Analysis\n\n**Critique:**\n{}\n\n**Reflection:**\n{}\n\n**Next Action:**\n{}\n\n> Based on this reflection, please proceed with the \"Next Action\" carefully. Do not repeat this reflection unless new information surfaces.",
        critique_val, reflection_val, next_action_val
    );

    let hint = SuccessHint::new(
        message.clone(),
        vec![format!("Proceed with: {}", next_action_val)],
    );

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "id": response_id,
        "args": args
    }))))
}
