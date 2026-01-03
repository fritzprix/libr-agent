use crate::mcp::builtin::planning::models::ScratchpadItem;
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};
use sqlx::SqlitePool;

/// Add scratchpad item (Legacy: addScratchpad)
pub async fn add_scratchpad(
    pool: &SqlitePool,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let note = args
        .get("note")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or("Missing or empty 'note'")?;
    let title = args.get("title").and_then(|v| v.as_str()).map(|s| s.trim());
    let source = args
        .get("source")
        .and_then(|v| v.as_str())
        .map(|s| s.trim());
    let tags = args.get("tags").map(|v| v.to_string()); // Store as JSON string

    // Check for duplicate title if title is provided
    if let Some(t) = title {
        let existing: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM planning_scratchpad WHERE session_id = ? AND title = ?")
                .bind(session_id)
                .bind(t)
                .fetch_optional(pool)
                .await
                .map_err(|e| format!("Database error checking duplicate: {}", e))?;

        if existing.is_some() {
            // Return guidance as the error message (text content)
            return Ok(MCPResult::error(&format!(
                "Scratchpad item with title '{}' already exists. Please use the `updateScratchpad` tool to modify the existing note or choose a different title.",
                t
            )));
        }
    }

    let now = chrono::Utc::now().timestamp_millis();

    let result = sqlx::query(
        r#"
        INSERT INTO planning_scratchpad (session_id, content, title, source, tags, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(session_id)
    .bind(note)
    .bind(title)
    .bind(source)
    .bind(tags)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await;

    match result {
        Ok(r) => {
            let response_id = cuid2::create_id();
            Ok(MCPResult::success_with_data(
                &format!("✓ Note added to scratchpad (ID: {})", r.last_insert_rowid()),
                json!({
                    "id": response_id,
                    "scratchpadId": r.last_insert_rowid()
                }),
            ))
        }
        Err(e) => Ok(MCPResult::error(&format!("Failed to add note: {}", e))),
    }
}

/// Update scratchpad item
pub async fn update_scratchpad(
    pool: &SqlitePool,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or("Missing or empty 'title'")?;

    let note = args
        .get("note")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or("Missing or empty 'note'")?;

    // Optional: Allow renaming via newTitle
    let new_title = args
        .get("newTitle")
        .and_then(|v| v.as_str())
        .map(|s| s.trim());

    // Check if item exists
    let existing: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM planning_scratchpad WHERE session_id = ? AND title = ?")
            .bind(session_id)
            .bind(title)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Database error checking existence: {}", e))?;

    let id = match existing {
        Some((id,)) => id,
        None => {
            return Ok(MCPResult::error(&format!(
                "No scratchpad item found with title '{}'. Use `addScratchpad` to create a new note.",
                title
            )));
        }
    };

    let now = chrono::Utc::now().timestamp_millis();
    let final_title = new_title.unwrap_or(title);

    // Update
    let result = sqlx::query(
        r#"
        UPDATE planning_scratchpad 
        SET content = ?, title = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(note)
    .bind(final_title)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(MCPResult::success(&format!(
            "✓ Scratchpad note '{}' updated",
            final_title
        ))),
        Err(e) => Ok(MCPResult::error(&format!("Failed to update note: {}", e))),
    }
}

/// List scratchpad items (Legacy: listScratchpad)
pub async fn list_scratchpad(
    pool: &SqlitePool,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("pageSize").and_then(|v| v.as_i64()).unwrap_or(10);

    if page < 1 {
        return Ok(MCPResult::error("Invalid 'page'. Must be >= 1"));
    }
    if page_size < 1 {
        return Ok(MCPResult::error("Invalid 'pageSize'. Must be >= 1"));
    }

    let filter_tags = args.get("tags").and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Vec<String>>()
    });

    // Fetch all items for session (optimize later if needed)
    let all_items: Vec<ScratchpadItem> = sqlx::query_as(
        "SELECT id, content, title, source, tags, created_at, updated_at FROM planning_scratchpad WHERE session_id = ? ORDER BY created_at DESC"
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to list scratchpad: {}", e))?;

    // Filter
    let filtered_items: Vec<&ScratchpadItem> = if let Some(tags) = &filter_tags {
        if tags.is_empty() {
            all_items.iter().collect()
        } else {
            all_items
                .iter()
                .filter(|item| {
                    if let Some(item_tags_json) = &item.tags {
                        if let Ok(item_tags) = serde_json::from_str::<Vec<String>>(item_tags_json) {
                            // Check if any filter tag is present in item tags
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

    // Format Text Output
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

    Ok(MCPResult::success_with_data(
        &text_output,
        json!({
            "items": json_items,
            "pagination": {
                "page": page,
                "pageSize": page_size,
                "total": total_items
            }
        }),
    ))
}

/// Read scratchpad item (Legacy: readScratchpad)
pub async fn read_scratchpad(
    pool: &SqlitePool,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let ids = args
        .get("ids")
        .and_then(|v| v.as_array())
        .ok_or("Missing 'ids' parameter")?;

    let mut items = Vec::new();
    for id_val in ids {
        if let Some(id) = id_val.as_i64() {
            if id < 0 {
                return Ok(MCPResult::error(&format!(
                    "Invalid id '{}'. Must be >= 0",
                    id
                )));
            }
            let item: Option<ScratchpadItem> = sqlx::query_as(
                "SELECT id, content, title, source, tags, created_at, updated_at FROM planning_scratchpad WHERE id = ? AND session_id = ?"
            )
            .bind(id)
            .bind(session_id)
            .fetch_optional(pool)
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

    Ok(MCPResult::success_with_data(
        &text_output,
        json!({ "items": items }),
    ))
}

/// Clear scratchpad item (Legacy: clearScratchpad)
pub async fn clear_scratchpad(
    pool: &SqlitePool,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let id = args
        .get("id")
        .and_then(|v| v.as_i64())
        .ok_or("Missing 'id'")?;

    if id < 0 {
        return Ok(MCPResult::error("Invalid 'id'. Must be >= 0"));
    }

    let result = sqlx::query("DELETE FROM planning_scratchpad WHERE id = ? AND session_id = ?")
        .bind(id)
        .bind(session_id)
        .execute(pool)
        .await;

    match result {
        Ok(_) => Ok(MCPResult::success("✓ Scratchpad item cleared")),
        Err(e) => Ok(MCPResult::error(&format!("Failed to clear item: {}", e))),
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

    Ok(MCPResult::success_with_data(
        &message,
        json!({
            "id": response_id,
            "thought": thought,
            "nextAction": next_action
        }),
    ))
}

/// Critique and reflection (Legacy: critiqueAndReflection)
/// Critique and reflection (Legacy: critiqueAndReflection)
pub async fn critique_and_reflection(args: Value) -> Result<MCPResult, String> {
    let critique = args
        .get("critique")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'critique'")?;
    let reflection = args
        .get("reflection")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'reflection'")?;
    let next_action = args
        .get("nextAction")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'nextAction'")?;

    let response_id = cuid2::create_id();

    let message = format!(
        "## Reflection & Critique Analysis\n\n**Critique:**\n{}\n\n**Reflection:**\n{}\n\n**Next Action:**\n{}\n\n> Based on this reflection, please proceed with the \"Next Action\" carefully. Do not repeat this reflection unless new information surfaces.",
        critique, reflection, next_action
    );

    Ok(MCPResult::success_with_data(
        &message,
        json!({
            "id": response_id,
            "args": args
        }),
    ))
}
