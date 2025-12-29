use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPResult, ServiceContext, ServiceContextOptions};
use crate::mcp::MCPTool;
use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::sync::Arc;

/// Todo item from database
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TodoItem {
    id: i64,
    content: String,
    description: Option<String>,
    priority: String,
    parent_id: Option<i64>,
    is_checked: bool,
    status: String, // Keep for backward compatibility if needed, or map to checked
    created_at: i64,
    updated_at: i64,
}

impl sqlx::FromRow<'_, sqlx::sqlite::SqliteRow> for TodoItem {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(TodoItem {
            id: row.try_get("id")?,
            content: row.try_get("content")?,
            description: row.try_get("description").ok(),
            priority: row.try_get("priority").unwrap_or("medium".to_string()),
            parent_id: row.try_get("parent_id").ok(),
            is_checked: row.try_get::<i64, _>("is_checked")? != 0,
            status: row.try_get("status")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

/// Scratchpad item from database
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ScratchpadItem {
    id: i64,
    content: String,
    title: Option<String>,
    source: Option<String>,
    tags: Option<String>, // JSON array string
    created_at: i64,
    updated_at: i64,
}

impl sqlx::FromRow<'_, sqlx::sqlite::SqliteRow> for ScratchpadItem {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(ScratchpadItem {
            id: row.try_get("id")?,
            content: row.try_get("content")?,
            title: row.try_get("title").ok(),
            source: row.try_get("source").ok(),
            tags: row.try_get("tags").ok(),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

/// Planning MCP Server
///
/// Provides goal/todo/scratchpad management for agent sessions.
/// Session-scoped: Each session gets dedicated planning state.
#[derive(Debug)]
pub struct PlanningServer {
    session_id: String,
    db_pool: Arc<SqlitePool>,
}

impl PlanningServer {
    /// Create a new PlanningServer for the given session
    pub async fn new(session_id: String, db_pool: Arc<SqlitePool>) -> Result<Self, String> {
        let server = Self {
            session_id,
            db_pool,
        };

        // Initialize database tables
        server.init_tables().await?;

        Ok(server)
    }

    /// Initialize database tables and indexes
    async fn init_tables(&self) -> Result<(), String> {
        // Create planning_goals table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS planning_goals (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                goal_text TEXT NOT NULL,
                status TEXT DEFAULT 'active',
                created_at INTEGER NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to create planning_goals table: {}", e))?;

        // Create planning_todos table (Updated schema)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS planning_todos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                content TEXT NOT NULL,
                description TEXT,
                priority TEXT DEFAULT 'medium',
                parent_id INTEGER,
                is_checked INTEGER DEFAULT 0,
                status TEXT DEFAULT 'pending',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to create planning_todos table: {}", e))?;

        // Create planning_scratchpad table (Updated schema)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS planning_scratchpad (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                content TEXT NOT NULL,
                title TEXT,
                source TEXT,
                tags TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to create planning_scratchpad table: {}", e))?;

        // Create indexes
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_planning_goals_session ON planning_goals(session_id)",
        )
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to create index: {}", e))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_planning_todos_session ON planning_todos(session_id)",
        )
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to create index: {}", e))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_planning_scratchpad_session ON planning_scratchpad(session_id)",
        )
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to create index: {}", e))?;

        log::debug!(
            "Planning server tables initialized for session: {}",
            self.session_id
        );

        Ok(())
    }

    /// Create a new goal (Legacy: createGoal)
    async fn create_goal(&self, args: Value) -> Result<MCPResult, String> {
        let goal = args
            .get("goal")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'goal' parameter".to_string())?;

        let now = chrono::Utc::now().timestamp_millis();

        // Deactivate existing active goals
        sqlx::query("UPDATE planning_goals SET status = 'archived' WHERE session_id = ? AND status = 'active'")
            .bind(&self.session_id)
            .execute(self.db_pool.as_ref())
            .await
            .map_err(|e| format!("Failed to archive old goals: {}", e))?;

        // Insert new goal
        let result = sqlx::query(
            r#"
            INSERT INTO planning_goals (session_id, goal_text, status, created_at)
            VALUES (?, ?, 'active', ?)
            "#,
        )
        .bind(&self.session_id)
        .bind(goal)
        .bind(now)
        .execute(self.db_pool.as_ref())
        .await;

        match result {
            Ok(query_result) => {
                let id = query_result.last_insert_rowid();
                Ok(MCPResult::success_with_data(
                    &format!("Goal created: {}", goal),
                    json!({
                        "success": true,
                        "goal": goal,
                        "id": id
                    }),
                ))
            }
            Err(e) => Ok(MCPResult::error(&format!("Failed to create goal: {}", e))),
        }
    }

    /// Update current goal (Legacy: updateGoal)
    async fn update_goal(&self, args: Value) -> Result<MCPResult, String> {
        let goal = args
            .get("goal")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'goal' parameter".to_string())?;

        let result = sqlx::query(
            r#"
            UPDATE planning_goals 
            SET goal_text = ? 
            WHERE session_id = ? AND status = 'active'
            "#,
        )
        .bind(goal)
        .bind(&self.session_id)
        .execute(self.db_pool.as_ref())
        .await;

        match result {
            Ok(query_result) => {
                if query_result.rows_affected() > 0 {
                    Ok(MCPResult::success_with_data(
                        &format!("Goal updated: {}", goal),
                        json!({
                            "success": true,
                            "goal": goal
                        }),
                    ))
                } else {
                    // If no active goal, create one
                    self.create_goal(args).await
                }
            }
            Err(e) => Ok(MCPResult::error(&format!("Failed to update goal: {}", e))),
        }
    }

    /// Clear current goal (Legacy: clearGoal)
    async fn clear_goal(&self, _args: Value) -> Result<MCPResult, String> {
        let result = sqlx::query(
            r#"
            UPDATE planning_goals 
            SET status = 'cleared' 
            WHERE session_id = ? AND status = 'active'
            "#,
        )
        .bind(&self.session_id)
        .execute(self.db_pool.as_ref())
        .await;

        match result {
            Ok(_) => Ok(MCPResult::success("Goal cleared")),
            Err(e) => Ok(MCPResult::error(&format!("Failed to clear goal: {}", e))),
        }
    }

    /// Add a new todo (Legacy: addTodo)
    async fn add_todo(&self, args: Value) -> Result<MCPResult, String> {
        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'title' parameter".to_string())?;

        let description = args.get("description").and_then(|v| v.as_str());
        let priority = args
            .get("priority")
            .and_then(|v| v.as_str())
            .unwrap_or("medium");
        let parent_id = args.get("parentId").and_then(|v| v.as_i64());

        let now = chrono::Utc::now().timestamp_millis();

        let result = sqlx::query(
            r#"
            INSERT INTO planning_todos (session_id, content, description, priority, parent_id, status, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, 'pending', ?, ?)
            "#,
        )
        .bind(&self.session_id)
        .bind(title)
        .bind(description)
        .bind(priority)
        .bind(parent_id)
        .bind(now)
        .bind(now)
        .execute(self.db_pool.as_ref())
        .await;

        match result {
            Ok(query_result) => {
                let id = query_result.last_insert_rowid();

                // Handle subtasks if present
                if let Some(subtasks) = args.get("subtasks").and_then(|v| v.as_array()) {
                    for subtask in subtasks {
                        let sub_title = subtask
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Untitled");
                        let sub_desc = subtask.get("description").and_then(|v| v.as_str());
                        let sub_prio = subtask
                            .get("priority")
                            .and_then(|v| v.as_str())
                            .unwrap_or("medium");

                        let _ = sqlx::query(
                            r#"
                            INSERT INTO planning_todos (session_id, content, description, priority, parent_id, status, created_at, updated_at)
                            VALUES (?, ?, ?, ?, ?, 'pending', ?, ?)
                            "#,
                        )
                        .bind(&self.session_id)
                        .bind(sub_title)
                        .bind(sub_desc)
                        .bind(sub_prio)
                        .bind(id)
                        .bind(now)
                        .bind(now)
                        .execute(self.db_pool.as_ref())
                        .await;
                    }
                }

                Ok(MCPResult::success_with_data(
                    &format!("Todo added with ID {}: {}", id, title),
                    json!({
                        "success": true,
                        "id": id,
                        "todo": title
                    }),
                ))
            }
            Err(e) => Ok(MCPResult::error(&format!("Failed to add todo: {}", e))),
        }
    }

    /// Check/Uncheck todo (Legacy: checkTodo)
    async fn check_todo(&self, args: Value) -> Result<MCPResult, String> {
        let id = args.get("id").and_then(|v| v.as_i64());
        let index = args.get("index").and_then(|v| v.as_i64());
        let checked = args
            .get("checked")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let summary = args.get("summary").and_then(|v| v.as_str());

        let target_id = if let Some(tid) = id {
            tid
        } else if let Some(idx) = index {
            // Find ID by index (ordered by created_at)
            let row: Option<(i64,)> = sqlx::query_as(
                "SELECT id FROM planning_todos WHERE session_id = ? ORDER BY created_at ASC LIMIT 1 OFFSET ?"
            )
            .bind(&self.session_id)
            .bind(idx)
            .fetch_optional(self.db_pool.as_ref())
            .await
            .map_err(|e| format!("Failed to find todo by index: {}", e))?;

            match row {
                Some((tid,)) => tid,
                Option::None => return Ok(MCPResult::error("Todo not found at index")),
            }
        } else {
            return Ok(MCPResult::error("Missing 'id' or 'index' parameter"));
        };

        let now = chrono::Utc::now().timestamp_millis();
        let status = if checked { "completed" } else { "pending" };

        let result = sqlx::query(
            r#"
            UPDATE planning_todos
            SET is_checked = ?, status = ?, updated_at = ?
            WHERE id = ? AND session_id = ?
            "#,
        )
        .bind(if checked { 1 } else { 0 })
        .bind(status)
        .bind(now)
        .bind(target_id)
        .bind(&self.session_id)
        .execute(self.db_pool.as_ref())
        .await;

        match result {
            Ok(_) => Ok(MCPResult::success_with_data(
                &format!(
                    "Todo {} {}",
                    target_id,
                    if checked { "checked" } else { "unchecked" }
                ),
                json!({
                    "success": true,
                    "id": target_id,
                    "checked": checked,
                    "summary": summary
                }),
            )),
            Err(e) => Ok(MCPResult::error(&format!("Failed to update todo: {}", e))),
        }
    }

    /// Clear todos (Legacy: clearTodos)
    async fn clear_todos(&self, args: Value) -> Result<MCPResult, String> {
        let ids = args.get("ids").and_then(|v| v.as_array());
        let indices = args.get("indices").and_then(|v| v.as_array());

        if ids.is_none() && indices.is_none() {
            // Clear all
            let result = sqlx::query("DELETE FROM planning_todos WHERE session_id = ?")
                .bind(&self.session_id)
                .execute(self.db_pool.as_ref())
                .await;

            return match result {
                Ok(r) => Ok(MCPResult::success(&format!(
                    "Cleared {} todos",
                    r.rows_affected()
                ))),
                Err(e) => Ok(MCPResult::error(&format!("Failed to clear todos: {}", e))),
            };
        }

        // TODO: Implement specific ID/Index clearing if needed, but for now "Clear all" is most common
        // Implementing specific clearing requires more complex SQL construction
        Ok(MCPResult::error(
            "Partial clearing not fully implemented yet, please clear all or use checkTodo",
        ))
    }

    /// Clear session (Legacy: clearSession)
    async fn clear_session(&self, _args: Value) -> Result<MCPResult, String> {
        let mut tx = self.db_pool.begin().await.map_err(|e| e.to_string())?;

        sqlx::query("DELETE FROM planning_goals WHERE session_id = ?")
            .bind(&self.session_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

        sqlx::query("DELETE FROM planning_todos WHERE session_id = ?")
            .bind(&self.session_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

        sqlx::query("DELETE FROM planning_scratchpad WHERE session_id = ?")
            .bind(&self.session_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

        tx.commit().await.map_err(|e| e.to_string())?;

        Ok(MCPResult::success("Session planning state cleared"))
    }

    /// Add scratchpad item (Legacy: addScratchpad)
    async fn add_scratchpad(&self, args: Value) -> Result<MCPResult, String> {
        let note = args
            .get("note")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'note'")?;
        let title = args.get("title").and_then(|v| v.as_str());
        let source = args.get("source").and_then(|v| v.as_str());
        let tags = args.get("tags").map(|v| v.to_string()); // Store as JSON string

        let now = chrono::Utc::now().timestamp_millis();

        let result = sqlx::query(
            r#"
            INSERT INTO planning_scratchpad (session_id, content, title, source, tags, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&self.session_id)
        .bind(note)
        .bind(title)
        .bind(source)
        .bind(tags)
        .bind(now)
        .bind(now)
        .execute(self.db_pool.as_ref())
        .await;

        match result {
            Ok(r) => Ok(MCPResult::success_with_data(
                &format!("Note added to scratchpad (ID: {})", r.last_insert_rowid()),
                json!({ "id": r.last_insert_rowid() }),
            )),
            Err(e) => Ok(MCPResult::error(&format!("Failed to add note: {}", e))),
        }
    }

    /// List scratchpad items (Legacy: listScratchpad)
    async fn list_scratchpad(&self, args: Value) -> Result<MCPResult, String> {
        let page = args.get("page").and_then(|v| v.as_u64()).unwrap_or(1);
        let page_size = args.get("pageSize").and_then(|v| v.as_u64()).unwrap_or(10);
        let filter_tags = args.get("tags").and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>()
        });

        // Fetch all items for session (optimize later if needed)
        let all_items: Vec<ScratchpadItem> = sqlx::query_as(
            "SELECT id, content, title, source, tags, created_at, updated_at FROM planning_scratchpad WHERE session_id = ? ORDER BY created_at DESC"
        )
        .bind(&self.session_id)
        .fetch_all(self.db_pool.as_ref())
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
                            if let Ok(item_tags) =
                                serde_json::from_str::<Vec<String>>(item_tags_json)
                            {
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
                let preview = if item.content.len() > 200 {
                    format!("{}...", &item.content[..200].replace('\n', " "))
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
                "preview": if item.content.len() > 200 { format!("{}...", &item.content[..200]) } else { item.content.clone() },
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
    async fn read_scratchpad(&self, args: Value) -> Result<MCPResult, String> {
        let ids = args
            .get("ids")
            .and_then(|v| v.as_array())
            .ok_or("Missing 'ids' parameter")?;

        let mut items = Vec::new();
        for id_val in ids {
            if let Some(id) = id_val.as_i64() {
                let item: Option<ScratchpadItem> = sqlx::query_as(
                    "SELECT id, content, title, source, tags, created_at, updated_at FROM planning_scratchpad WHERE id = ? AND session_id = ?"
                )
                .bind(id)
                .bind(&self.session_id)
                .fetch_optional(self.db_pool.as_ref())
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
    async fn clear_scratchpad(&self, args: Value) -> Result<MCPResult, String> {
        let id = args
            .get("id")
            .and_then(|v| v.as_i64())
            .ok_or("Missing 'id'")?;

        let result = sqlx::query("DELETE FROM planning_scratchpad WHERE id = ? AND session_id = ?")
            .bind(id)
            .bind(&self.session_id)
            .execute(self.db_pool.as_ref())
            .await;

        match result {
            Ok(_) => Ok(MCPResult::success("Scratchpad item cleared")),
            Err(e) => Ok(MCPResult::error(&format!("Failed to clear item: {}", e))),
        }
    }

    /// Get current state (Legacy: getCurrentState)
    async fn get_current_state(&self, _args: Value) -> Result<MCPResult, String> {
        // Reuse get_service_context logic but return as tool result
        let context = self.get_service_context(None).await;
        Ok(MCPResult::success_with_data(
            &context.context_prompt,
            context.structured_state.unwrap_or(json!({})),
        ))
    }

    /// Pause and think (Legacy: pauseAndThink)
    async fn pause_and_think(&self, args: Value) -> Result<MCPResult, String> {
        let thought = args.get("thought").and_then(|v| v.as_str()).unwrap_or("");
        // Ephemeral, just echo back
        Ok(MCPResult::success_with_data(
            "Thought recorded",
            json!({ "thought": thought }),
        ))
    }

    /// Critique and reflection (Legacy: critiqueAndReflection)
    async fn critique_and_reflection(&self, args: Value) -> Result<MCPResult, String> {
        // Ephemeral, just echo back
        Ok(MCPResult::success_with_data("Reflection recorded", args))
    }
}

#[async_trait]
impl BuiltinMCPServer for PlanningServer {
    fn name(&self) -> &str {
        "planning"
    }

    fn description(&self) -> &str {
        "Session-scoped planning tools for goal/todo/scratchpad management"
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            MCPTool {
                name: "createGoal".to_string(),
                title: Some("Create Goal".to_string()),
                description: "Create a single goal for the session.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": { "goal": { "type": "string" } },
                    "required": ["goal"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "updateGoal".to_string(),
                title: Some("Update Goal".to_string()),
                description: "Update the current goal.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": { "goal": { "type": "string" } },
                    "required": ["goal"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "clearGoal".to_string(),
                title: Some("Clear Goal".to_string()),
                description: "Clear the current goal.".to_string(),
                input_schema: serde_json::from_value(json!({ "type": "object", "properties": {} }))
                    .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "addTodo".to_string(),
                title: Some("Add Todo".to_string()),
                description: "Add a todo item to the goal.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "description": { "type": "string" },
                        "priority": { "type": "string", "enum": ["low", "medium", "high"] },
                        "parentId": { "type": "number" },
                        "subtasks": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "title": { "type": "string" },
                                    "description": { "type": "string" },
                                    "priority": { "type": "string" }
                                }
                            }
                        }
                    },
                    "required": ["title"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "checkTodo".to_string(),
                title: Some("Check Todo".to_string()),
                description: "Mark a todo item as checked/unchecked.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "number" },
                        "index": { "type": "number" },
                        "checked": { "type": "boolean" },
                        "summary": { "type": "string" }
                    }
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "clearTodos".to_string(),
                title: Some("Clear Todos".to_string()),
                description: "Clear specific todos or all todos.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "ids": { "type": "array", "items": { "type": "number" } },
                        "indices": { "type": "array", "items": { "type": "number" } }
                    }
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "clearSession".to_string(),
                title: Some("Clear Session".to_string()),
                description: "Clear all session state.".to_string(),
                input_schema: serde_json::from_value(json!({ "type": "object", "properties": {} }))
                    .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "addScratchpad".to_string(),
                title: Some("Add Scratchpad".to_string()),
                description: "Add a note to Scratchpad.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "note": { "type": "string" },
                        "title": { "type": "string" },
                        "source": { "type": "string" },
                        "tags": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["note"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "listScratchpad".to_string(),
                title: Some("List Scratchpad".to_string()),
                description: "List scratchpad items.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "page": { "type": "number" },
                        "pageSize": { "type": "number" },
                        "tags": { "type": "array", "items": { "type": "string" } }
                    }
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "readScratchpad".to_string(),
                title: Some("Read Scratchpad".to_string()),
                description: "Read full content of scratchpad items.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "ids": { "type": "array", "items": { "type": "number" } }
                    },
                    "required": ["ids"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "clearScratchpad".to_string(),
                title: Some("Clear Scratchpad".to_string()),
                description: "Remove a note from Scratchpad.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": { "id": { "type": "number" } },
                    "required": ["id"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "getCurrentState".to_string(),
                title: Some("Get Current State".to_string()),
                description: "Get current planning state.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "include_checked": { "type": "boolean" },
                        "include_scratchpad": { "type": "boolean" }
                    }
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "pauseAndThink".to_string(),
                title: Some("Pause and Think".to_string()),
                description: "Pause to think about the problem.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "thought": { "type": "string" },
                        "nextAction": { "type": "string" }
                    },
                    "required": ["thought"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "critiqueAndReflection".to_string(),
                title: Some("Critique and Reflection".to_string()),
                description: "Reflect on the current state.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "critique": { "type": "string" },
                        "reflection": { "type": "string" },
                        "nextAction": { "type": "string" }
                    },
                    "required": ["critique", "reflection", "nextAction"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
        ]
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String> {
        log::debug!(
            "Planning server tool called: {} for session: {}",
            tool_name,
            self.session_id
        );

        match tool_name {
            "createGoal" | "builtin_planning__createGoal" => self.create_goal(args).await,
            "updateGoal" | "builtin_planning__updateGoal" => self.update_goal(args).await,
            "clearGoal" | "builtin_planning__clearGoal" => self.clear_goal(args).await,
            "addTodo" | "builtin_planning__addTodo" => self.add_todo(args).await,
            "checkTodo" | "builtin_planning__checkTodo" => self.check_todo(args).await,
            "clearTodos" | "builtin_planning__clearTodos" => self.clear_todos(args).await,
            "clearSession" | "builtin_planning__clearSession" => self.clear_session(args).await,
            "addScratchpad" | "builtin_planning__addScratchpad" => self.add_scratchpad(args).await,
            "listScratchpad" | "builtin_planning__listScratchpad" => {
                self.list_scratchpad(args).await
            }
            "readScratchpad" | "builtin_planning__readScratchpad" => {
                self.read_scratchpad(args).await
            }
            "clearScratchpad" | "builtin_planning__clearScratchpad" => {
                self.clear_scratchpad(args).await
            }
            "getCurrentState" | "builtin_planning__getCurrentState" => {
                self.get_current_state(args).await
            }
            "pauseAndThink" | "builtin_planning__pauseAndThink" => self.pause_and_think(args).await,
            "critiqueAndReflection" | "builtin_planning__critiqueAndReflection" => {
                self.critique_and_reflection(args).await
            }
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }

    async fn switch_context(&self, _options: ServiceContextOptions) -> Result<(), String> {
        Err("Context switching not supported for session-bound planning server".to_string())
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        // 1. Fetch Active Goal
        let goal: Option<String> = sqlx::query_scalar(
            "SELECT goal_text FROM planning_goals WHERE session_id = ? AND status = 'active' LIMIT 1",
        )
        .bind(&self.session_id)
        .fetch_optional(self.db_pool.as_ref())
        .await
        .unwrap_or_else(|e| {
            log::error!("Failed to fetch goal: {}", e);
            None
        });

        // 2. Fetch Todos (All)
        // We fetch all to calculate counts and separate checked/unchecked
        let todos: Vec<TodoItem> = sqlx::query_as(
            "SELECT * FROM planning_todos WHERE session_id = ? ORDER BY created_at ASC",
        )
        .bind(&self.session_id)
        .fetch_all(self.db_pool.as_ref())
        .await
        .unwrap_or_else(|e| {
            log::error!("Failed to fetch todos: {}", e);
            Vec::new()
        });

        // 3. Fetch Scratchpad (Recent)
        let scratchpad: Vec<ScratchpadItem> = sqlx::query_as(
            "SELECT * FROM planning_scratchpad WHERE session_id = ? ORDER BY created_at DESC LIMIT 6",
        )
        .bind(&self.session_id)
        .fetch_all(self.db_pool.as_ref())
        .await
        .unwrap_or_else(|e| {
            log::error!("Failed to fetch scratchpad: {}", e);
            Vec::new()
        });

        // --- Format Output ---

        let mut parts = vec!["## Planning".to_string()];

        // Goal Section
        if let Some(g) = &goal {
            parts.push(format!("\n**Current Goal:** \"{}\"", g));
            parts.push("*Goal is active. Track progress with todos below.*".to_string());
        } else {
            parts.push("\n**No Goal Set**".to_string());
            parts.push("*Consider using createGoal to establish a clear objective for this planning session.*".to_string());
        }

        // Todos Section
        let (checked_todos, unchecked_todos): (Vec<&TodoItem>, Vec<&TodoItem>) =
            todos.iter().partition(|t| t.is_checked);

        if !todos.is_empty() {
            parts.push(format!(
                "\n**Todos:** {} unchecked / {} checked ({} total)",
                unchecked_todos.len(),
                checked_todos.len(),
                todos.len()
            ));

            // Unchecked Todos (Top 5)
            if !unchecked_todos.is_empty() {
                parts.push("\n**Unchecked Items:**".to_string());
                for (idx, t) in unchecked_todos.iter().take(5).enumerate() {
                    let priority = if t.priority != "medium" {
                        format!("Priority:{}", t.priority)
                    } else {
                        "Priority:medium".to_string()
                    };

                    let description = if let Some(desc) = &t.description {
                        if !desc.is_empty() {
                            let truncated = if desc.len() > 80 {
                                format!("{}...", &desc[0..80])
                            } else {
                                desc.clone()
                            };
                            format!("\n     {}", truncated)
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };

                    parts.push(format!(
                        "  [{}] ID:{} | {} | {}{}",
                        idx, t.id, t.content, priority, description
                    ));
                }

                if unchecked_todos.len() > 5 {
                    parts.push(format!(
                        "  ...and {} more (use listTodos to see all)",
                        unchecked_todos.len() - 5
                    ));
                }
                parts.push("\n*Use ID when calling checkTodo/updateTodo*".to_string());
            }

            // Checked Todos (Top 3 recent)
            if !checked_todos.is_empty() {
                parts.push("\n**Checked Items (Completed):**".to_string());
                // We want the most recently updated/created ones (which are at the end of the list since we ordered by ASC)
                // So we reverse iteration
                for t in checked_todos.iter().rev().take(3) {
                    let priority = if t.priority != "medium" {
                        format!("[{}]", t.priority)
                    } else {
                        String::new()
                    };
                    parts.push(format!("  [✓] ID:{} | {} {}", t.id, t.content, priority));
                }

                if checked_todos.len() > 3 {
                    parts.push(format!(
                        "  ...and {} more completed",
                        checked_todos.len() - 3
                    ));
                }
            }
        }

        // Scratchpad Section
        if !scratchpad.is_empty() {
            // Check if we have more than the limit (we fetched limit 6 to check for 'more')
            let (visible_scratchpad, has_more_scratchpad) = if scratchpad.len() > 5 {
                (&scratchpad[0..5], true)
            } else {
                (&scratchpad[..], false)
            };

            parts.push(format!("\n**Scratchpad:** {} items", scratchpad.len()));
            parts.push("".to_string()); // Spacer

            for (idx, item) in visible_scratchpad.iter().enumerate() {
                let title_part = if let Some(title) = &item.title {
                    format!("**{}**", title)
                } else {
                    String::new()
                };

                let tags_part = if let Some(tags_json) = &item.tags {
                    if let Ok(tags) = serde_json::from_str::<Vec<String>>(tags_json) {
                        if !tags.is_empty() {
                            format!(" [{}]", tags.join("] ["))
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                let content_preview = if item.title.is_some() {
                    if item.content.len() > 50 {
                        format!(" - {}...", &item.content[0..50])
                    } else {
                        format!(" - {}", item.content)
                    }
                } else if item.content.len() > 60 {
                    format!("{}...", &item.content[0..60])
                } else {
                    item.content.clone()
                };

                parts.push(format!(
                    "  {}. **ID:{}** {}{}{}",
                    idx + 1,
                    item.id,
                    title_part,
                    content_preview,
                    tags_part
                ));
            }

            if has_more_scratchpad {
                parts.push(format!(
                    "  ...and {} more items. Use listScratchpad to view all.",
                    scratchpad.len() - 5
                ));
            }
        }

        let structured_state = json!({
             "goal": goal,
             "todos_count": todos.len(),
             "scratchpad_count": scratchpad.len()
        });

        ServiceContext {
            context_prompt: parts.join("\n"),
            structured_state: Some(structured_state),
        }
    }
}
