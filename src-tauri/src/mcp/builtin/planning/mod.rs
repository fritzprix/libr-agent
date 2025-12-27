use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPContent, MCPResult, ServiceContext, ServiceContextOptions};
use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;
use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;

/// Todo item from database
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TodoItem {
    id: i64,
    content: String,
    active_form: String,
    status: String,
    created_at: i64,
    updated_at: i64,
}

impl sqlx::FromRow<'_, sqlx::sqlite::SqliteRow> for TodoItem {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(TodoItem {
            id: row.try_get("id")?,
            content: row.try_get("content")?,
            active_form: row.try_get("active_form")?,
            status: row.try_get("status")?,
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

        // Create planning_todos table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS planning_todos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                content TEXT NOT NULL,
                active_form TEXT NOT NULL,
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

        // Create planning_scratchpad table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS planning_scratchpad (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL UNIQUE,
                content TEXT NOT NULL,
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

        log::debug!(
            "Planning server tables initialized for session: {}",
            self.session_id
        );

        Ok(())
    }

    /// Set or update the goal for this session
    async fn set_goal(&self, args: Value) -> Result<MCPResult, String> {
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
                Ok(MCPResult {
                    content: Some(vec![MCPContent::Text {
                        text: format!("Goal set successfully: {}", goal),
                    }]),
                    structured_content: Some(json!({
                        "success": true,
                        "id": id,
                        "goal": goal,
                        "session_id": self.session_id
                    })),
                    is_error: Some(false),
                })
            }
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to set goal: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    /// Get the current active goal for this session
    async fn get_goal(&self, _args: Value) -> Result<MCPResult, String> {
        let result = sqlx::query_as::<_, (i64, String, String, i64)>(
            r#"
            SELECT id, goal_text, status, created_at
            FROM planning_goals
            WHERE session_id = ? AND status = 'active'
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(&self.session_id)
        .fetch_optional(self.db_pool.as_ref())
        .await;

        match result {
            Ok(Some((id, goal_text, status, created_at))) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Current goal: {}", goal_text),
                }]),
                structured_content: Some(json!({
                    "id": id,
                    "goal": goal_text,
                    "status": status,
                    "created_at": created_at
                })),
                is_error: Some(false),
            }),
            Ok(None) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: "No active goal set".to_string(),
                }]),
                structured_content: Some(json!({"goal": null})),
                is_error: Some(false),
            }),
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to get goal: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    /// Add a new todo
    async fn add_todo(&self, args: Value) -> Result<MCPResult, String> {
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'content' parameter".to_string())?;

        let active_form = args
            .get("activeForm")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'activeForm' parameter".to_string())?;

        let now = chrono::Utc::now().timestamp_millis();

        let result = sqlx::query(
            r#"
            INSERT INTO planning_todos (session_id, content, active_form, status, created_at, updated_at)
            VALUES (?, ?, ?, 'pending', ?, ?)
            "#,
        )
        .bind(&self.session_id)
        .bind(content)
        .bind(active_form)
        .bind(now)
        .bind(now)
        .execute(self.db_pool.as_ref())
        .await;

        match result {
            Ok(query_result) => {
                let id = query_result.last_insert_rowid();
                Ok(MCPResult {
                    content: Some(vec![MCPContent::Text {
                        text: format!("Todo added: {}", content),
                    }]),
                    structured_content: Some(json!({
                        "success": true,
                        "id": id,
                        "content": content,
                        "active_form": active_form,
                        "status": "pending"
                    })),
                    is_error: Some(false),
                })
            }
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to add todo: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    /// Update a todo's status
    async fn update_todo(&self, args: Value) -> Result<MCPResult, String> {
        let id = args
            .get("id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| "Missing 'id' parameter".to_string())?;

        let status = args
            .get("status")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'status' parameter".to_string())?;

        // Validate status
        if !["pending", "in_progress", "completed"].contains(&status) {
            return Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!(
                        "Invalid status '{}'. Must be one of: pending, in_progress, completed",
                        status
                    ),
                }]),
                structured_content: None,
                is_error: Some(true),
            });
        }

        let now = chrono::Utc::now().timestamp_millis();

        let result = sqlx::query(
            r#"
            UPDATE planning_todos
            SET status = ?, updated_at = ?
            WHERE id = ? AND session_id = ?
            "#,
        )
        .bind(status)
        .bind(now)
        .bind(id)
        .bind(&self.session_id)
        .execute(self.db_pool.as_ref())
        .await;

        match result {
            Ok(query_result) => {
                if query_result.rows_affected() > 0 {
                    Ok(MCPResult {
                        content: Some(vec![MCPContent::Text {
                            text: format!("Todo {} updated to status: {}", id, status),
                        }]),
                        structured_content: Some(json!({
                            "success": true,
                            "id": id,
                            "status": status
                        })),
                        is_error: Some(false),
                    })
                } else {
                    Ok(MCPResult {
                        content: Some(vec![MCPContent::Text {
                            text: format!("Todo {} not found in session", id),
                        }]),
                        structured_content: None,
                        is_error: Some(true),
                    })
                }
            }
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to update todo: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    /// List all todos for this session
    async fn list_todos(&self, _args: Value) -> Result<MCPResult, String> {
        let result = sqlx::query_as::<_, (i64, String, String, String, i64, i64)>(
            r#"
            SELECT id, content, active_form, status, created_at, updated_at
            FROM planning_todos
            WHERE session_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(&self.session_id)
        .fetch_all(self.db_pool.as_ref())
        .await;

        match result {
            Ok(rows) => {
                let todos: Vec<Value> = rows
                    .into_iter()
                    .map(
                        |(id, content, active_form, status, created_at, updated_at)| {
                            json!({
                                "id": id,
                                "content": content,
                                "activeForm": active_form,
                                "status": status,
                                "created_at": created_at,
                                "updated_at": updated_at
                            })
                        },
                    )
                    .collect();

                Ok(MCPResult {
                    content: Some(vec![MCPContent::Text {
                        text: format!("Found {} todos", todos.len()),
                    }]),
                    structured_content: Some(json!({
                        "todos": todos,
                        "count": todos.len()
                    })),
                    is_error: Some(false),
                })
            }
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to list todos: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    /// Update the scratchpad content
    async fn update_scratchpad(&self, args: Value) -> Result<MCPResult, String> {
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'content' parameter".to_string())?;

        let now = chrono::Utc::now().timestamp_millis();

        // Use INSERT OR REPLACE pattern for SQLite
        let result = sqlx::query(
            r#"
            INSERT INTO planning_scratchpad (session_id, content, updated_at)
            VALUES (?, ?, ?)
            ON CONFLICT(session_id)
            DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at
            "#,
        )
        .bind(&self.session_id)
        .bind(content)
        .bind(now)
        .execute(self.db_pool.as_ref())
        .await;

        match result {
            Ok(_) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: "Scratchpad updated successfully".to_string(),
                }]),
                structured_content: Some(json!({
                    "success": true,
                    "content": content
                })),
                is_error: Some(false),
            }),
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to update scratchpad: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    /// Get the scratchpad content
    async fn get_scratchpad(&self, _args: Value) -> Result<MCPResult, String> {
        let result = sqlx::query_as::<_, (String, i64)>(
            r#"
            SELECT content, updated_at
            FROM planning_scratchpad
            WHERE session_id = ?
            "#,
        )
        .bind(&self.session_id)
        .fetch_optional(self.db_pool.as_ref())
        .await;

        match result {
            Ok(Some((content, updated_at))) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: content.clone(),
                }]),
                structured_content: Some(json!({
                    "content": content,
                    "updated_at": updated_at
                })),
                is_error: Some(false),
            }),
            Ok(None) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: "".to_string(),
                }]),
                structured_content: Some(json!({
                    "content": "",
                    "updated_at": null
                })),
                is_error: Some(false),
            }),
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to get scratchpad: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    /// Load planning state from database
    async fn load_planning_state(
        &self,
    ) -> Result<(Option<String>, Vec<TodoItem>, Option<String>), String> {
        // Load goal
        let goal = sqlx::query_scalar::<_, Option<String>>(
            "SELECT goal_text FROM planning_goals
             WHERE session_id = ? AND status = 'active'
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(&self.session_id)
        .fetch_optional(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to query goal: {}", e))?
        .flatten();

        // Load todos
        let todos: Vec<TodoItem> = sqlx::query_as(
            "SELECT id, content, active_form, status, created_at, updated_at
             FROM planning_todos
             WHERE session_id = ?
             ORDER BY created_at ASC",
        )
        .bind(&self.session_id)
        .fetch_all(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to query todos: {}", e))?;

        // Load scratchpad
        let scratchpad = sqlx::query_scalar::<_, Option<String>>(
            "SELECT content FROM planning_scratchpad WHERE session_id = ?",
        )
        .bind(&self.session_id)
        .fetch_optional(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to query scratchpad: {}", e))?
        .flatten();

        Ok((goal, todos, scratchpad))
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
            create_set_goal_tool(),
            create_get_goal_tool(),
            create_add_todo_tool(),
            create_update_todo_tool(),
            create_list_todos_tool(),
            create_update_scratchpad_tool(),
            create_get_scratchpad_tool(),
        ]
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String> {
        log::debug!(
            "Planning server tool called: {} for session: {}",
            tool_name,
            self.session_id
        );

        match tool_name {
            "setGoal" | "builtin_planning__setGoal" => self.set_goal(args).await,
            "getGoal" | "builtin_planning__getGoal" => self.get_goal(args).await,
            "addTodo" | "builtin_planning__addTodo" => self.add_todo(args).await,
            "updateTodo" | "builtin_planning__updateTodo" => self.update_todo(args).await,
            "listTodos" | "builtin_planning__listTodos" => self.list_todos(args).await,
            "updateScratchpad" | "builtin_planning__updateScratchpad" => {
                self.update_scratchpad(args).await
            }
            "getScratchpad" | "builtin_planning__getScratchpad" => {
                self.get_scratchpad(args).await
            }
            _ => Err(format!(
                "Unknown tool: {}. Available tools: setGoal, getGoal, addTodo, updateTodo, listTodos, updateScratchpad, getScratchpad",
                tool_name
            )),
        }
    }

    async fn switch_context(&self, _options: ServiceContextOptions) -> Result<(), String> {
        Err("Context switching not supported for session-bound planning server".to_string())
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        // Load planning state with error handling
        let (goal, todos, scratchpad) = match self.load_planning_state().await {
            Ok(state) => state,
            Err(e) => {
                log::warn!(
                    "Failed to load planning state for session '{}': {}",
                    self.session_id,
                    e
                );
                return ServiceContext {
                    context_prompt: "# Planning\n**Status**: Error loading state".to_string(),
                    structured_state: None,
                };
            }
        };

        // Build context prompt
        let mut parts = vec!["## Planning".to_string()];

        // Goal section
        if let Some(goal_text) = &goal {
            parts.push(format!("\n**Current Goal:** \"{}\"", goal_text));
            parts.push("*Goal is active. Track progress with todos below.*".to_string());
        } else {
            parts.push("\n**No Goal Set**".to_string());
            parts.push(
                "*Consider using createGoal to establish a clear objective for this planning session.*"
                    .to_string(),
            );
        }

        // Todos section
        if !todos.is_empty() {
            let unchecked: Vec<_> = todos.iter().filter(|t| t.status != "completed").collect();
            let checked: Vec<_> = todos.iter().filter(|t| t.status == "completed").collect();

            parts.push(format!(
                "\n**Todos:** {} unchecked / {} checked ({} total)",
                unchecked.len(),
                checked.len(),
                todos.len()
            ));

            // Unchecked items (top 5)
            if !unchecked.is_empty() {
                parts.push("\n**Unchecked Items:**".to_string());
                for (idx, todo) in unchecked.iter().take(5).enumerate() {
                    let status_display = match todo.status.as_str() {
                        "in_progress" => " [IN PROGRESS]",
                        _ => "",
                    };
                    parts.push(format!(
                        "  [{}] ID:{} | {}{}",
                        idx, todo.id, todo.content, status_display
                    ));
                }

                if unchecked.len() > 5 {
                    parts.push(format!(
                        "  ...and {} more (use listTodos to see all)",
                        unchecked.len() - 5
                    ));
                }

                parts.push("\n*Use Index or ID when calling checkTodo/updateTodo*".to_string());
            }

            // Checked items (last 3 completed)
            if !checked.is_empty() {
                parts.push("\n**Checked Items (Completed):**".to_string());
                let recent_completed: Vec<_> = checked.iter().rev().take(3).collect();

                for todo in recent_completed {
                    parts.push(format!("  [✓] ID:{} | {}", todo.id, todo.content));
                }

                if checked.len() > 3 {
                    parts.push(format!("  ...and {} more completed", checked.len() - 3));
                }
            }
        }

        // Scratchpad section
        if let Some(scratchpad_content) = &scratchpad {
            let preview = if scratchpad_content.len() > 500 {
                format!("{}...", &scratchpad_content[..500])
            } else {
                scratchpad_content.clone()
            };
            parts.push(format!("\n**Scratchpad:**\n{}", preview));
        }

        ServiceContext {
            context_prompt: parts.join("\n"),
            structured_state: Some(json!({
                "goal": goal,
                "todos": {
                    "unchecked": todos.iter().filter(|t| t.status != "completed").count(),
                    "checked": todos.iter().filter(|t| t.status == "completed").count(),
                    "total": todos.len()
                },
                "scratchpad": scratchpad.is_some()
            })),
        }
    }
}

/// Create the setGoal tool definition
fn create_set_goal_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "goal".to_string(),
        string_prop_required("The goal description"),
    );

    MCPTool {
        name: "builtin_planning__setGoal".to_string(),
        title: Some("Set Session Goal".to_string()),
        description: "Set or update the primary goal for this session. Deactivates previous goals."
            .to_string(),
        input_schema: object_schema(props, vec!["goal".to_string()]),
        annotations: None,
        output_schema: None,
    }
}

/// Create the getGoal tool definition
fn create_get_goal_tool() -> MCPTool {
    let props = HashMap::new();

    MCPTool {
        name: "builtin_planning__getGoal".to_string(),
        title: Some("Get Session Goal".to_string()),
        description: "Retrieve the current active goal for this session".to_string(),
        input_schema: object_schema(props, vec![]),
        annotations: None,
        output_schema: None,
    }
}

/// Create the addTodo tool definition
fn create_add_todo_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "content".to_string(),
        string_prop_required("The todo task description"),
    );
    props.insert(
        "activeForm".to_string(),
        string_prop_required("The present continuous form (e.g., 'Running tests')"),
    );

    MCPTool {
        name: "builtin_planning__addTodo".to_string(),
        title: Some("Add Todo".to_string()),
        description: "Add a new todo item to the session's task list".to_string(),
        input_schema: object_schema(props, vec!["content".to_string(), "activeForm".to_string()]),
        annotations: None,
        output_schema: None,
    }
}

/// Create the updateTodo tool definition
fn create_update_todo_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert("id".to_string(), string_prop_required("The todo ID"));
    props.insert(
        "status".to_string(),
        string_prop_required("New status (pending/in_progress/completed)"),
    );

    MCPTool {
        name: "builtin_planning__updateTodo".to_string(),
        title: Some("Update Todo Status".to_string()),
        description: "Update a todo's status. Valid statuses: pending, in_progress, completed"
            .to_string(),
        input_schema: object_schema(props, vec!["id".to_string(), "status".to_string()]),
        annotations: None,
        output_schema: None,
    }
}

/// Create the listTodos tool definition
fn create_list_todos_tool() -> MCPTool {
    let props = HashMap::new();

    MCPTool {
        name: "builtin_planning__listTodos".to_string(),
        title: Some("List Todos".to_string()),
        description: "List all todos for this session, ordered by creation time".to_string(),
        input_schema: object_schema(props, vec![]),
        annotations: None,
        output_schema: None,
    }
}

/// Create the updateScratchpad tool definition
fn create_update_scratchpad_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "content".to_string(),
        string_prop_required("The scratchpad content"),
    );

    MCPTool {
        name: "builtin_planning__updateScratchpad".to_string(),
        title: Some("Update Scratchpad".to_string()),
        description: "Update the scratchpad notes for this session. Useful for temporary context."
            .to_string(),
        input_schema: object_schema(props, vec!["content".to_string()]),
        annotations: None,
        output_schema: None,
    }
}

/// Create the getScratchpad tool definition
fn create_get_scratchpad_tool() -> MCPTool {
    let props = HashMap::new();

    MCPTool {
        name: "builtin_planning__getScratchpad".to_string(),
        title: Some("Get Scratchpad".to_string()),
        description: "Retrieve the scratchpad notes for this session".to_string(),
        input_schema: object_schema(props, vec![]),
        annotations: None,
        output_schema: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    async fn create_test_pool() -> SqlitePool {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("Invalid database URL")
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .connect_with(options)
            .await
            .expect("Failed to create test pool");

        // Create sessions table for FOREIGN KEY constraint
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                name TEXT,
                status TEXT DEFAULT 'idle',
                agent_config TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create sessions table");

        // Insert test session
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO sessions (id, name, status, created_at, updated_at)
            VALUES ('test-session', 'Test Session', 'idle', 0, 0)
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to insert test session");

        sqlx::query(
            r#"
            INSERT OR IGNORE INTO sessions (id, name, status, created_at, updated_at)
            VALUES ('session-1', 'Session 1', 'idle', 0, 0)
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to insert session 1");

        sqlx::query(
            r#"
            INSERT OR IGNORE INTO sessions (id, name, status, created_at, updated_at)
            VALUES ('session-2', 'Session 2', 'idle', 0, 0)
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to insert session 2");

        pool
    }

    #[tokio::test]
    async fn test_goal_management() {
        let pool = Arc::new(create_test_pool().await);
        let server = PlanningServer::new("test-session".to_string(), pool)
            .await
            .expect("Failed to create server");

        // Set goal
        let set_result = server
            .set_goal(json!({"goal": "Complete Phase 2 implementation"}))
            .await
            .expect("Failed to set goal");

        assert!(set_result.is_error == Some(false));

        // Get goal
        let get_result = server
            .get_goal(json!({}))
            .await
            .expect("Failed to get goal");

        assert!(get_result.is_error == Some(false));
        let structured = get_result.structured_content.unwrap();
        assert_eq!(structured["goal"], "Complete Phase 2 implementation");
    }

    #[tokio::test]
    async fn test_todo_workflow() {
        let pool = Arc::new(create_test_pool().await);
        let server = PlanningServer::new("test-session".to_string(), pool)
            .await
            .expect("Failed to create server");

        // Add todo
        let add_result = server
            .add_todo(json!({
                "content": "Write tests",
                "activeForm": "Writing tests"
            }))
            .await
            .expect("Failed to add todo");

        assert!(add_result.is_error == Some(false));
        let structured = add_result.structured_content.unwrap();
        let todo_id = structured["id"].as_i64().unwrap();

        // List todos
        let list_result = server
            .list_todos(json!({}))
            .await
            .expect("Failed to list todos");

        assert!(list_result.is_error == Some(false));
        let structured = list_result.structured_content.unwrap();
        assert_eq!(structured["count"], 1);

        // Update todo status
        let update_result = server
            .update_todo(json!({
                "id": todo_id,
                "status": "in_progress"
            }))
            .await
            .expect("Failed to update todo");

        assert!(update_result.is_error == Some(false));
    }

    #[tokio::test]
    async fn test_scratchpad() {
        let pool = Arc::new(create_test_pool().await);
        let server = PlanningServer::new("test-session".to_string(), pool)
            .await
            .expect("Failed to create server");

        // Update scratchpad
        let update_result = server
            .update_scratchpad(json!({
                "content": "Temporary notes here"
            }))
            .await
            .expect("Failed to update scratchpad");

        assert!(update_result.is_error == Some(false));

        // Get scratchpad
        let get_result = server
            .get_scratchpad(json!({}))
            .await
            .expect("Failed to get scratchpad");

        assert!(get_result.is_error == Some(false));
        let structured = get_result.structured_content.unwrap();
        assert_eq!(structured["content"], "Temporary notes here");
    }

    #[tokio::test]
    async fn test_session_isolation() {
        let pool = Arc::new(create_test_pool().await);
        let server1 = PlanningServer::new("session-1".to_string(), pool.clone())
            .await
            .expect("Failed to create server 1");
        let server2 = PlanningServer::new("session-2".to_string(), pool)
            .await
            .expect("Failed to create server 2");

        // Add todo to session 1
        server1
            .add_todo(json!({
                "content": "Session 1 task",
                "activeForm": "Working on session 1 task"
            }))
            .await
            .expect("Failed to add todo to session 1");

        // Add todo to session 2
        server2
            .add_todo(json!({
                "content": "Session 2 task",
                "activeForm": "Working on session 2 task"
            }))
            .await
            .expect("Failed to add todo to session 2");

        // List todos for session 1 - should only see 1 todo
        let list1 = server1
            .list_todos(json!({}))
            .await
            .expect("Failed to list session 1 todos");

        let structured1 = list1.structured_content.unwrap();
        assert_eq!(structured1["count"], 1);
        assert_eq!(structured1["todos"][0]["content"], "Session 1 task");

        // List todos for session 2 - should only see 1 todo
        let list2 = server2
            .list_todos(json!({}))
            .await
            .expect("Failed to list session 2 todos");

        let structured2 = list2.structured_content.unwrap();
        assert_eq!(structured2["count"], 1);
        assert_eq!(structured2["todos"][0]["content"], "Session 2 task");
    }
}
