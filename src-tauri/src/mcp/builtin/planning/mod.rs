use crate::mcp::builtin::error_guidance::{
    duplicate_error, invalid_input_error, missing_param_error, not_found_error, ErrorCategory,
    ErrorGuidance, SuccessHint, ToolGroup,
};
use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPResult, ServiceContext, ServiceContextOptions};
use crate::mcp::MCPTool;
use async_trait::async_trait;
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;

/// Todo item for frontend display
#[derive(Debug, Serialize)]
struct TodoDTO {
    id: i64,
    title: String,
    description: Option<String>,
    priority: String,
    checked: bool,
    subtasks: Vec<TodoDTO>,
}

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
#[derive(Debug, Clone, Serialize)]
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
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "Missing or empty 'goal' parameter".to_string())?;

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
                let response_id = cuid2::create_id();
                Ok(MCPResult::success_with_data(
                    &format!("✓ Goal created: {}", goal),
                    json!({
                        "id": response_id,
                        "success": true,
                        "goal": goal,
                        "goalId": id
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
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "Missing or empty 'goal' parameter".to_string())?;

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
                    let response_id = cuid2::create_id();
                    Ok(MCPResult::success_with_data(
                        &format!("✓ Goal updated: {}", goal),
                        json!({
                            "id": response_id,
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
            Ok(_) => Ok(MCPResult::success("✓ Goal cleared")),
            Err(e) => Ok(MCPResult::error(&format!("Failed to clear goal: {}", e))),
        }
    }

    /// Add a new todo (Legacy: addTodo)
    async fn add_todo(&self, args: Value) -> Result<MCPResult, String> {
        // Validate title parameter
        let title = match args
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            Some(t) => t,
            None => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::MissingRequiredParam,
                    "Missing or empty 'title' parameter",
                    vec![
                        "Provide a non-empty title string".to_string(),
                        "Example: {\"title\": \"Implement feature X\"}".to_string(),
                        "Use list_todos to see existing todos".to_string(),
                    ],
                    ToolGroup::Planning,
                )
                .to_mcp_result());
            }
        };

        let description = args.get("description").and_then(|v| v.as_str());
        let priority = args
            .get("priority")
            .and_then(|v| v.as_str())
            .unwrap_or("medium");
        let parent_id = args.get("parentId").and_then(|v| v.as_i64());

        // 1. Validate priority
        let valid_priorities = ["low", "medium", "high"];
        if !valid_priorities.contains(&priority) {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                format!(
                    "Invalid priority '{}'. Must be one of: low, medium, high",
                    priority
                ),
                vec![
                    "Use 'low', 'medium', or 'high' for priority".to_string(),
                    format!(
                        "Example: {{\"priority\": \"high\"}} (you used: \"{}\")",
                        priority
                    ),
                    "Omit priority parameter to use default 'medium'".to_string(),
                ],
                ToolGroup::Planning,
            )
            .to_mcp_result());
        }

        // 2. Validate nesting constraints (cannot have both parentId and subtasks)
        if parent_id.is_some() && args.get("subtasks").is_some() {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::NestingTooDeep,
                "Cannot add subtasks to a child todo (max 1 level of nesting)",
                vec![
                    "Create the todo without subtasks, then add subtasks separately".to_string(),
                    "Create as top-level todo by omitting parentId".to_string(),
                    "Use list_todos to see the current hierarchy".to_string(),
                ],
                ToolGroup::Planning,
            )
            .to_mcp_result());
        }

        // 3. Check for duplicate title (case-insensitive)
        let duplicate_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM planning_todos WHERE session_id = ? AND lower(content) = lower(?)",
        )
        .bind(&self.session_id)
        .bind(title)
        .fetch_one(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to check duplicates: {}", e))?;

        if duplicate_count > 0 {
            return Ok(duplicate_error("Todo", title, ToolGroup::Planning));
        }

        // 4. If parent_id is provided, validate parent exists and is top-level
        if let Some(pid) = parent_id {
            let parent: Option<(Option<i64>,)> = sqlx::query_as(
                "SELECT parent_id FROM planning_todos WHERE id = ? AND session_id = ?",
            )
            .bind(pid)
            .bind(&self.session_id)
            .fetch_optional(self.db_pool.as_ref())
            .await
            .map_err(|e| format!("Failed to fetch parent: {}", e))?;

            match parent {
                Some((grandparent_id,)) => {
                    if grandparent_id.is_some() {
                        return Ok(ErrorGuidance::with_guidance(
                            ErrorCategory::NestingTooDeep,
                            "Cannot add subtask to a subtask (max 1 level of nesting)",
                            vec![
                                "Create as top-level todo instead".to_string(),
                                "Attach to a different parent that has no parent".to_string(),
                                "Use list_todos to see the current hierarchy".to_string(),
                            ],
                            ToolGroup::Planning,
                        )
                        .to_mcp_result());
                    }
                }
                None => {
                    return Ok(not_found_error(
                        "Parent todo",
                        &pid.to_string(),
                        ToolGroup::Planning,
                    ));
                }
            }
        }

        // 5. Validate subtasks if present
        if let Some(subtasks) = args.get("subtasks").and_then(|v| v.as_array()) {
            for (idx, subtask) in subtasks.iter().enumerate() {
                // Validate subtask title is non-empty
                let sub_title = subtask
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty());

                if sub_title.is_none() {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::InvalidInput,
                        format!("Subtask at index {} has an empty or missing title", idx),
                        vec![
                            format!("Provide a non-empty title for subtask #{}", idx + 1),
                            "All subtasks must have non-empty titles".to_string(),
                            "Example: {\"title\": \"Implement X\"}".to_string(),
                        ],
                        ToolGroup::Planning,
                    )
                    .to_mcp_result());
                }

                // Validate subtask priority
                let sub_prio = subtask
                    .get("priority")
                    .and_then(|v| v.as_str())
                    .unwrap_or("medium");

                if !valid_priorities.contains(&sub_prio) {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::InvalidInput,
                        format!(
                            "Invalid subtask #{} priority '{}'. Must be one of: low, medium, high",
                            idx + 1,
                            sub_prio
                        ),
                        vec![
                            "Use 'low', 'medium', or 'high' for priority".to_string(),
                            "Omit priority to use default 'medium'".to_string(),
                        ],
                        ToolGroup::Planning,
                    )
                    .to_mcp_result());
                }
            }
        }

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
                        // Title already validated in step 5, safe to unwrap
                        let sub_title = subtask
                            .get("title")
                            .and_then(|v| v.as_str())
                            .map(|s| s.trim())
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

                let response_id = cuid2::create_id();
                let hint = SuccessHint::new(
                    format!("Todo added with ID {}: {}", id, title),
                    SuccessHint::for_tool("addTodo", ToolGroup::Planning),
                );
                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "id": response_id,
                    "success": true,
                    "todoId": id,
                    "todo": title
                }))))
            }
            Err(e) => Ok(ErrorGuidance::with_guidance(
                ErrorCategory::DatabaseError,
                format!("Failed to add todo: {}", e),
                vec![
                    "Try again - this may be a transient database error".to_string(),
                    "Verify the session is active".to_string(),
                    "Use list_todos to check if the todo was created despite the error".to_string(),
                ],
                ToolGroup::Planning,
            )
            .to_mcp_result()),
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
            if tid < 1 {
                return Ok(invalid_input_error(
                    "Invalid 'id'. Must be >= 1",
                    ToolGroup::Planning,
                ));
            }
            tid
        } else if let Some(idx) = index {
            if idx < 0 {
                return Ok(invalid_input_error(
                    "Invalid 'index'. Must be >= 0",
                    ToolGroup::Planning,
                ));
            }
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
                Option::None => {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::ResourceNotFound,
                        format!("Todo not found at index {}", idx),
                        vec![
                            "Use list_todos to see available todos".to_string(),
                            format!("Index {} may be out of range", idx),
                            "Indices are 0-based and ordered by creation time".to_string(),
                        ],
                        ToolGroup::Planning,
                    )
                    .to_mcp_result());
                }
            }
        } else {
            return Ok(missing_param_error("'id' or 'index'", ToolGroup::Planning));
        };

        let now = chrono::Utc::now().timestamp_millis();
        let status = if checked { "completed" } else { "pending" };

        // Store summary in description field if provided
        let result = if let Some(s) = summary {
            sqlx::query(
                r#"
                UPDATE planning_todos
                SET is_checked = ?, status = ?, description = COALESCE(description || ' - ' || ?, description, ?), updated_at = ?
                WHERE id = ? AND session_id = ?
                "#,
            )
            .bind(if checked { 1 } else { 0 })
            .bind(status)
            .bind(s)
            .bind(s)
            .bind(now)
            .bind(target_id)
            .bind(&self.session_id)
            .execute(self.db_pool.as_ref())
            .await
        } else {
            sqlx::query(
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
            .await
        };

        match result {
            Ok(_) => {
                let response_id = cuid2::create_id();
                let action = if checked { "checked" } else { "unchecked" };
                let summary_text = if let Some(s) = summary {
                    format!(" - {}", s)
                } else {
                    String::new()
                };
                let hint = SuccessHint::new(
                    format!("Todo {} (ID: {}){}", action, target_id, summary_text),
                    SuccessHint::for_tool("checkTodo", ToolGroup::Planning),
                );
                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "id": response_id,
                    "success": true,
                    "todoId": target_id,
                    "checked": checked,
                    "summary": summary
                }))))
            }
            Err(e) => Ok(ErrorGuidance::with_guidance(
                ErrorCategory::DatabaseError,
                format!("Failed to update todo: {}", e),
                vec![
                    "Try again - this may be a transient error".to_string(),
                    "Use list_todos to verify the todo exists".to_string(),
                    "Verify the session is active".to_string(),
                ],
                ToolGroup::Planning,
            )
            .to_mcp_result()),
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
                    "✓ Cleared {} todos",
                    r.rows_affected()
                ))),
                Err(e) => Ok(MCPResult::error(&format!("Failed to clear todos: {}", e))),
            };
        }

        let mut target_ids: Vec<i64> = Vec::new();

        // Collect explicit IDs
        if let Some(id_list) = ids {
            for id_val in id_list {
                if let Some(id) = id_val.as_i64() {
                    if id < 1 {
                        return Ok(MCPResult::error(&format!(
                            "Invalid id '{}'. Must be >= 1",
                            id
                        )));
                    }
                    target_ids.push(id);
                }
            }
        }

        // Collect IDs from indices
        if let Some(idx_list) = indices {
            // Fetch all IDs ordered by created_at to map indices
            let all_todos: Vec<(i64,)> = sqlx::query_as(
                "SELECT id FROM planning_todos WHERE session_id = ? ORDER BY created_at ASC",
            )
            .bind(&self.session_id)
            .fetch_all(self.db_pool.as_ref())
            .await
            .map_err(|e| format!("Failed to fetch todos for index mapping: {}", e))?;

            for idx_val in idx_list {
                if let Some(idx) = idx_val.as_i64() {
                    if idx < 0 {
                        return Ok(MCPResult::error(&format!(
                            "Invalid index '{}'. Must be >= 0",
                            idx
                        )));
                    }
                    let idx = idx as usize;
                    if idx < all_todos.len() {
                        target_ids.push(all_todos[idx].0);
                    }
                }
            }
        }

        if target_ids.is_empty() {
            return Ok(MCPResult::success("✓ No todos found to clear"));
        }

        // Remove duplicates
        target_ids.sort();
        target_ids.dedup();

        // Construct DELETE query with IN clause
        let placeholders: Vec<String> = target_ids.iter().map(|_| "?".to_string()).collect();
        let query = format!(
            "DELETE FROM planning_todos WHERE session_id = ? AND id IN ({})",
            placeholders.join(",")
        );

        let mut query_builder = sqlx::query(&query).bind(&self.session_id);
        for id in target_ids {
            query_builder = query_builder.bind(id);
        }

        let result = query_builder.execute(self.db_pool.as_ref()).await;

        match result {
            Ok(r) => Ok(MCPResult::success(&format!(
                "✓ Cleared {} todos",
                r.rows_affected()
            ))),
            Err(e) => Ok(MCPResult::error(&format!("Failed to clear todos: {}", e))),
        }
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

        Ok(MCPResult::success("✓ Session planning state cleared"))
    }

    /// Add scratchpad item (Legacy: addScratchpad)
    async fn add_scratchpad(&self, args: Value) -> Result<MCPResult, String> {
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

    /// List scratchpad items (Legacy: listScratchpad)
    async fn list_scratchpad(&self, args: Value) -> Result<MCPResult, String> {
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

        if id < 0 {
            return Ok(MCPResult::error("Invalid 'id'. Must be >= 0"));
        }

        let result = sqlx::query("DELETE FROM planning_scratchpad WHERE id = ? AND session_id = ?")
            .bind(id)
            .bind(&self.session_id)
            .execute(self.db_pool.as_ref())
            .await;

        match result {
            Ok(_) => Ok(MCPResult::success("✓ Scratchpad item cleared")),
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
        let response_id = cuid2::create_id();
        // Ephemeral, just echo back
        Ok(MCPResult::success_with_data(
            "✓ Thought recorded",
            json!({
                "id": response_id,
                "thought": thought
            }),
        ))
    }

    /// Critique and reflection (Legacy: critiqueAndReflection)
    async fn critique_and_reflection(&self, args: Value) -> Result<MCPResult, String> {
        let response_id = cuid2::create_id();
        // Ephemeral, just echo back
        Ok(MCPResult::success_with_data(
            "✓ Reflection recorded",
            json!({
                "id": response_id,
                "args": args
            }),
        ))
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
                description: "Create a single goal for the session. Use when starting a new or complex task.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": { "goal": { "type": "string", "description": "The goal text to set for the session (e.g., \"Complete project setup\")." } },
                    "required": ["goal"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "updateGoal".to_string(),
                title: Some("Update Goal".to_string()),
                description: "Update the current goal. Use when the goal needs refinement or correction without clearing context.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": { "goal": { "type": "string", "description": "The new goal text." } },
                    "required": ["goal"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "clearGoal".to_string(),
                title: Some("Clear Goal".to_string()),
                description: "Clear the current goal. Use when finishing or abandoning the current goal.".to_string(),
                input_schema: serde_json::from_value(json!({ "type": "object", "properties": {} }))
                    .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "addTodo".to_string(),
                title: Some("Add Todo".to_string()),
                description: "Add a todo item to the goal. Supports 1-level nesting: you can add subtasks inline or specify a parentId to create a child task. Use to break down a goal into actionable steps.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "description": "Short summary of the task (e.g., \"Write documentation\")." },
                        "description": { "type": "string", "description": "Detailed instructions or context for the task." },
                        "priority": { "type": "string", "enum": ["low", "medium", "high"], "description": "The priority of the todo item." },
                        "parentId": { "type": "number", "description": "Parent todo ID to create a subtask. Only top-level todos (without parentId) can be parents. Maximum 1-level nesting." },
                        "subtasks": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "title": { "type": "string", "description": "Subtask title" },
                                    "description": { "type": "string", "description": "Subtask description" },
                                    "priority": { "type": "string", "enum": ["low", "medium", "high"] }
                                },
                                "required": ["title"]
                            },
                            "description": "Array of subtasks to create with this todo. Only allowed when creating a top-level todo (no parentId)."
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
                description: "Mark a todo item as checked (completed) or unchecked, optionally with a completion summary. You can specify either id (database ID) or index (0-based position in the list).".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "number", "minimum": 1, "description": "The database ID of the todo to update" },
                        "index": { "type": "number", "minimum": 0, "description": "The 0-based index position of the todo in the current list" },
                        "checked": { "type": "boolean", "description": "Whether to mark the todo as checked (true) or unchecked (false). Defaults to true." },
                        "summary": { "type": "string", "description": "Optional summary or completion note for the todo (e.g., \"Completed with PR #42\")." }
                    }
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "clearTodos".to_string(),
                title: Some("Clear Todos".to_string()),
                description: "Clear specific todos by their indices (0-based) or IDs. If no indices or IDs are provided, all todos will be cleared. Use to remove completed tasks or reset the todo list.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "ids": { "type": "array", "items": { "type": "number", "minimum": 1 }, "description": "Array of todo IDs to clear." },
                        "indices": { "type": "array", "items": { "type": "number", "minimum": 0 }, "description": "Array of todo indices (0-based) to clear." }
                    }
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "clearSession".to_string(),
                title: Some("Clear Session".to_string()),
                description: "Clear all session state (goal, todos, and scratchpad items). Use to reset everything and start fresh.".to_string(),
                input_schema: serde_json::from_value(json!({ "type": "object", "properties": {} }))
                    .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "addScratchpad".to_string(),
                title: Some("Add Scratchpad".to_string()),
                description: "Add a note to your Scratchpad (Working Memory). Content here is ALWAYS visible in your context. Use this for keeping track of important findings, file paths, IDs, or intermediate analysis results that you need to reference frequently during the task.\n\nOptional source parameter: Provide the source of information for citation tracking (e.g., URLs, file paths, or tool result IDs like \"https://example.com/article\" or \"file://path/to/doc.txt\").".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "note": { "type": "string", "description": "The content to add to the scratchpad (e.g., \"User requested feature X\", \"File path: src/main.ts\")." },
                        "title": { "type": "string", "description": "Optional title for the note. Helps in identifying the note in the list." },
                        "source": { "type": "string", "description": "Optional source of the information for citation tracking. Examples: \"https://example.com/article\", \"file://workspace/docs/readme.md\", \"tool_result_id:abc123\"" },
                        "tags": { "type": "array", "items": { "type": "string" }, "description": "Optional tags for categorization and filtering." }
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
                description: "List scratchpad items with metadata (ID, title, tags) and content preview. Use this to find the IDs of items you want to read fully. Supports pagination and tag filtering.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "page": { "type": "number", "minimum": 1, "description": "Page number (default: 1)" },
                        "pageSize": { "type": "number", "minimum": 1, "description": "Items per page (default: 10)" },
                        "tags": { "type": "array", "items": { "type": "string" }, "description": "Filter items by tags" }
                    }
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "readScratchpad".to_string(),
                title: Some("Read Scratchpad".to_string()),
                description: "Read the FULL content of specific scratchpad items by their IDs. You must provide the IDs of the items you want to read. Use listScratchpad first to find IDs.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "ids": { "type": "array", "items": { "type": "number", "minimum": 0 }, "description": "List of scratchpad IDs to read (Required)." }
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
                description: "Remove a note from your Scratchpad. Use this to clear information that is no longer relevant to free up context window space.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": { "id": { "type": "number", "minimum": 0, "description": "The ID of the scratchpad item to clear." } },
                    "required": ["id"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "getCurrentState".to_string(),
                title: Some("Get Current State".to_string()),
                description: "Get current planning state including Goal, Todos, and Scratchpad as structured JSON data for UI visualization".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "include_checked": { "type": "boolean", "description": "Whether to include checked todos in the output. Defaults to true." },
                        "include_scratchpad": { "type": "boolean", "description": "Whether to include scratchpad items in the output. Defaults to true." }
                    }
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "pauseAndThink".to_string(),
                title: Some("Pause and Think".to_string()),
                description: "Pause to think about the problem, plan your approach, or analyze results before taking action. Use this when you need to reason through complex decisions or maintain context. Simpler alternative to sequentialthinking.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "thought": { "type": "string", "description": "Your current thought, analysis, or plan. Be clear and specific about what you are thinking through." },
                        "nextAction": { "type": "string", "description": "Optional: The specific next action you plan to take after this thought. Helps maintain continuity." }
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
                description: "Reflect on the current state and provide a critique of the progress. Use this tool to pause, analyze what has been done, identify potential issues or missed steps, and plan the next actions carefully.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "critique": { "type": "string", "description": "A critical evaluation of the results achieved so far." },
                        "reflection": { "type": "string", "description": "Self-reflection on any shortcomings or areas for improvement in the process." },
                        "nextAction": { "type": "string", "description": "The expected next action based on the reflection." }
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

        // Build Todo Tree for structured state
        let mut todo_map: HashMap<i64, Vec<TodoItem>> = HashMap::new();
        let mut root_todos: Vec<TodoItem> = Vec::new();

        for todo in &todos {
            if let Some(parent_id) = todo.parent_id {
                todo_map.entry(parent_id).or_default().push(todo.clone());
            } else {
                root_todos.push(todo.clone());
            }
        }

        let structured_todos: Vec<TodoDTO> = root_todos
            .into_iter()
            .map(|t| {
                let subtasks = todo_map
                    .remove(&t.id)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|st| TodoDTO {
                        id: st.id,
                        title: st.content,
                        description: st.description,
                        priority: st.priority,
                        checked: st.is_checked,
                        subtasks: Vec::new(), // Max 1 level nesting supported
                    })
                    .collect();

                TodoDTO {
                    id: t.id,
                    title: t.content,
                    description: t.description,
                    priority: t.priority,
                    checked: t.is_checked,
                    subtasks,
                }
            })
            .collect();

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
             "lastClearedGoal": null,
             "todos": structured_todos,
             "scratchpad": scratchpad,
             "todos_count": todos.len(),
             "scratchpad_count": scratchpad.len()
        });

        ServiceContext {
            context_prompt: parts.join("\n"),
            structured_state: Some(structured_state),
        }
    }
}
