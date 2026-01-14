use crate::entity::planning_scratchpad;
use crate::mcp::builtin::error_guidance::{
    invalid_input_error, missing_param_error, ErrorCategory, ErrorGuidance, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set, TransactionTrait,
};
use serde_json::{json, Value};

/// Add scratchpad item (Legacy: addScratchpad)
pub async fn add_scratchpad(
    db: &DatabaseConnection,
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

    // Transaction for atomic check-and-insert
    let result: Result<(i64, i64), sea_orm::TransactionError<String>> = db
        .transaction::<_, (i64, i64), String>(move |txn| {
            Box::pin(async move {
                // 1. Check duplicate title
                if let Some(ref t) = title_owned {
                    let existing = planning_scratchpad::Entity::find()
                        .filter(planning_scratchpad::Column::SessionId.eq(&session_id_owned))
                        .filter(planning_scratchpad::Column::Title.eq(t))
                        .one(txn)
                        .await
                        .map_err(|e| format!("Database error: {}", e))?;

                    if existing.is_some() {
                        return Err(format!("DUPLICATE:{}", t));
                    }
                }

                // 2. Check limit
                let count = planning_scratchpad::Entity::find()
                    .filter(planning_scratchpad::Column::SessionId.eq(&session_id_owned))
                    .count(txn)
                    .await
                    .map_err(|e| format!("Database error: {}", e))?;

                if count >= 10 {
                    return Err("LIMIT_REACHED".to_string());
                }

                // 3. Insert
                let now = chrono::Utc::now().timestamp_millis();
                let new_item = planning_scratchpad::ActiveModel {
                    session_id: Set(session_id_owned.clone()),
                    content: Set(note_owned.clone()),
                    title: Set(title_owned.clone()),
                    source: Set(source_owned.clone()),
                    tags: Set(tags.clone()),
                    created_at: Set(now),
                    updated_at: Set(now),
                    ..Default::default()
                };

                let inserted = new_item
                    .insert(txn)
                    .await
                    .map_err(|e| format!("Failed to insert: {}", e))?;

                // Get new count
                let new_count = planning_scratchpad::Entity::find()
                    .filter(planning_scratchpad::Column::SessionId.eq(&session_id_owned))
                    .count(txn)
                    .await
                    .map_err(|e| format!("Database error: {}", e))?;

                Ok((inserted.id, new_count as i64))
            })
        })
        .await;

    match result {
        Ok((last_id, current_count)) => {
            let response_id = cuid2::create_id();
            let hint = SuccessHint::new(
                format!(
                    "✓ Note added to scratchpad (ID: {})\nScratchpad: {}/10",
                    last_id, current_count
                ),
                vec![
                    "Use listScratchpad to see all items".to_string(),
                    "Use readScratchpad to view full content".to_string(),
                ],
            );
            Ok(hint.to_mcp_result_with_data(Some(json!({
                "id": response_id,
                "scratchpadId": last_id
            }))))
        }
        Err(sea_orm::TransactionError::Transaction(err)) => {
            if err == "LIMIT_REACHED" {
                Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidState,
                    "Scratchpad limit reached (10 items)",
                    vec![
                        "Use updateScratchpad to modify existing notes".to_string(),
                        "Use clearScratchpad to remove old items".to_string(),
                    ],
                    ToolGroup::Planning,
                )
                .to_mcp_result())
            } else if let Some(stripped) = err.strip_prefix("DUPLICATE:") {
                Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::DuplicateResource,
                    format!("Scratchpad item with title '{}' already exists", stripped),
                    vec![
                        "Use updateScratchpad to modify the existing note".to_string(),
                        "Choose a different title for the new note".to_string(),
                    ],
                    ToolGroup::Planning,
                )
                .to_mcp_result())
            } else {
                Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::DatabaseError,
                    format!("Transaction failed: {}", err),
                    vec!["Try again".to_string()],
                    ToolGroup::Planning,
                )
                .to_mcp_result())
            }
        }
        Err(e) => Ok(ErrorGuidance::with_guidance(
            ErrorCategory::DatabaseError,
            format!("Database error: {}", e),
            vec!["Try again".to_string()],
            ToolGroup::Planning,
        )
        .to_mcp_result()),
    }
}

/// Update scratchpad item
pub async fn update_scratchpad(
    db: &DatabaseConnection,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let note = args
        .get("note")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let title_val = match title {
        Some(t) => t,
        None => return Ok(missing_param_error("title", ToolGroup::Planning)),
    };
    let note_val = match note {
        Some(n) => n,
        None => return Ok(missing_param_error("note", ToolGroup::Planning)),
    };

    let new_title = args
        .get("newTitle")
        .and_then(|v| v.as_str())
        .map(|s| s.trim());

    // Find item
    let existing = planning_scratchpad::Entity::find()
        .filter(planning_scratchpad::Column::SessionId.eq(session_id))
        .filter(planning_scratchpad::Column::Title.eq(title_val))
        .one(db)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    let item = match existing {
        Some(i) => i,
        None => {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::ResourceNotFound,
                format!("No scratchpad item found with title '{}'", title_val),
                vec![
                    "Use listScratchpad to see available items".to_string(),
                    "Use addScratchpad to create a new note".to_string(),
                    "Check for typos in the title".to_string(),
                ],
                ToolGroup::Planning,
            )
            .to_mcp_result());
        }
    };

    let now = chrono::Utc::now().timestamp_millis();
    let final_title = new_title.unwrap_or(title_val);

    let mut active_item: planning_scratchpad::ActiveModel = item.into();
    active_item.content = Set(note_val.to_string());
    active_item.title = Set(Some(final_title.to_string()));
    active_item.updated_at = Set(now);

    match active_item.update(db).await {
        Ok(_) => {
            let hint = SuccessHint::new(
                format!("✓ Scratchpad note '{}' updated", final_title),
                vec![
                    "Use readScratchpad to verify content".to_string(),
                    "Use listScratchpad to see all items".to_string(),
                ],
            );
            Ok(hint.to_mcp_result())
        }
        Err(e) => Ok(ErrorGuidance::with_guidance(
            ErrorCategory::DatabaseError,
            format!("Failed to update note: {}", e),
            vec![
                "Try again".to_string(),
                "Use getCurrentState to verify item status".to_string(),
            ],
            ToolGroup::Planning,
        )
        .to_mcp_result()),
    }
}

/// List scratchpad items (Legacy: listScratchpad)
pub async fn list_scratchpad(
    db: &DatabaseConnection,
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

    // Fetch all items
    let all_items = planning_scratchpad::Entity::find()
        .filter(planning_scratchpad::Column::SessionId.eq(session_id))
        .order_by_desc(planning_scratchpad::Column::CreatedAt)
        .all(db)
        .await
        .map_err(|e| format!("Failed to list scratchpad: {}", e))?;

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
            // .filter(|item| ...) logic
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

    let json_items: Vec<Value> = paged_items.into_iter().map(|item| {
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
    }).collect();

    let hint = SuccessHint::new(
        text_output,
        vec![
            "Use readScratchpad(ids) to read full content of specific items".to_string(),
            "Use addScratchpad to create new notes".to_string(),
        ],
    );

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
    db: &DatabaseConnection,
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

    let mut items = Vec::new();
    for id_val in ids_array {
        if let Some(id) = id_val.as_i64() {
            if id < 0 {
                return Ok(invalid_input_error(
                    &format!("Invalid id '{}'. Must be >= 0", id),
                    ToolGroup::Planning,
                ));
            }

            let item = planning_scratchpad::Entity::find_by_id(id)
                .filter(planning_scratchpad::Column::SessionId.eq(session_id))
                .one(db)
                .await
                .map_err(|e| format!("Failed to read item {}: {}", id, e))?;

            if let Some(i) = item {
                items.push(json!({
                    "id": i.id,
                    "title": i.title,
                    "content": i.content,
                    "source": i.source,
                    "tags": i.tags.and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok())
                }));
            }
        }
    }

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

    let hint = SuccessHint::new(
        text_output,
        vec![
            "Use updateScratchpad to modify these items".to_string(),
            "Use clearScratchpad to remove them".to_string(),
        ],
    );

    Ok(hint.to_mcp_result_with_data(Some(json!({ "items": items }))))
}

/// Clear scratchpad item (Legacy: clearScratchpad)
pub async fn clear_scratchpad(
    db: &DatabaseConnection,
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

    let result = planning_scratchpad::Entity::delete_by_id(target_id)
        .filter(planning_scratchpad::Column::SessionId.eq(session_id))
        .exec(db)
        .await;

    match result {
        Ok(_) => {
            let hint = SuccessHint::new(
                "✓ Scratchpad item cleared",
                vec![
                    "Use addScratchpad to add new items".to_string(),
                    "Use listScratchpad to see remaining items".to_string(),
                ],
            );
            Ok(hint.to_mcp_result())
        }
        Err(e) => Ok(ErrorGuidance::with_guidance(
            ErrorCategory::DatabaseError,
            format!("Failed to clear item: {}", e),
            vec![
                "Try again".to_string(),
                "Use listScratchpad to verify item exists".to_string(),
            ],
            ToolGroup::Planning,
        )
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

    if critique.is_none() {
        return Ok(missing_param_error("critique", ToolGroup::Planning));
    }
    if reflection.is_none() {
        return Ok(missing_param_error("reflection", ToolGroup::Planning));
    }
    if next_action.is_none() {
        return Ok(missing_param_error("nextAction", ToolGroup::Planning));
    }

    let response_id = cuid2::create_id();

    let message = format!(
        "## Reflection & Critique Analysis\n\n**Critique:**\n{}\n\n**Reflection:**\n{}\n\n**Next Action:**\n{}\n\n> Based on this reflection, please proceed with the \"Next Action\" carefully. Do not repeat this reflection unless new information surfaces.",
        critique.unwrap(), reflection.unwrap(), next_action.unwrap()
    );

    let hint = SuccessHint::new(
        message.clone(),
        vec![format!("Proceed with: {}", next_action.unwrap())],
    );

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "id": response_id,
        "args": args
    }))))
}
