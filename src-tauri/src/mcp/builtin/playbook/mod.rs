use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPContent, MCPResult, ServiceContext, ServiceContextOptions};
use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;
use async_trait::async_trait;
use handlebars::Handlebars;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;

/// HTML template for playbook list UI
const PLAYBOOK_LIST_TEMPLATE: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset='utf-8'>
    <meta name='viewport' content='width=device-width,initial-scale=1'>
    <style>
        body {
            font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            margin: 0;
            padding: 16px;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            min-height: 100vh;
        }
        .container {
            max-width: 800px;
            margin: 0 auto;
            background: rgba(255, 255, 255, 0.1);
            border-radius: 15px;
            padding: 24px;
            backdrop-filter: blur(10px);
            box-shadow: 0 8px 32px 0 rgba(31, 38, 135, 0.37);
        }
        h2 { margin-top: 0; text-align: center; }
        .playbook-item {
            background: rgba(255, 255, 255, 0.15);
            padding: 16px;
            margin: 12px 0;
            border-radius: 8px;
            display: flex;
            justify-content: space-between;
            align-items: center;
        }
        .playbook-info { flex: 1; }
        .playbook-title { font-weight: bold; font-size: 16px; }
        .playbook-desc { font-size: 14px; opacity: 0.9; margin-top: 4px; }
        .playbook-meta { font-size: 12px; opacity: 0.7; margin-top: 4px; }
        .btn-group { display: flex; gap: 8px; }
        button {
            padding: 8px 16px;
            border: none;
            border-radius: 6px;
            cursor: pointer;
            font-size: 14px;
            transition: all 0.2s;
        }
        .btn-select {
            background: linear-gradient(45deg, #2196F3, #21CBF3);
            color: white;
        }
        .btn-select:hover { transform: translateY(-2px); box-shadow: 0 4px 8px rgba(33, 150, 243, 0.4); }
        .btn-delete {
            background: linear-gradient(45deg, #f44336, #e91e63);
            color: white;
        }
        .btn-delete:hover { transform: translateY(-2px); box-shadow: 0 4px 8px rgba(244, 67, 54, 0.4); }
        .empty-state {
            text-align: center;
            padding: 40px;
            opacity: 0.8;
        }
    </style>
</head>
<body>
    <div class='container'>
        <h2>📚 Playbooks ({{count}})</h2>
        {{#if hasPlaybooks}}
        <div id='playbook-list'>
            {{#each playbooks}}
            <div class='playbook-item'>
                <div class='playbook-info'>
                    <div class='playbook-title'>{{this.title}}</div>
                    {{#if this.description}}
                    <div class='playbook-desc'>{{this.description}}</div>
                    {{/if}}
                    <div class='playbook-meta'>ID: {{this.id}} | Updated: {{this.updated_at}}</div>
                </div>
                <div class='btn-group'>
                    <button class='btn-select' data-id='{{this.id}}'>Select</button>
                    <button class='btn-delete' data-id='{{this.id}}'>Delete</button>
                </div>
            </div>
            {{/each}}
        </div>
        {{else}}
        <div class='empty-state'>
            <p>No playbooks found</p>
            <p>Create your first playbook to get started!</p>
        </div>
        {{/if}}
    </div>
    <script>
        document.addEventListener('DOMContentLoaded', function() {
            // Select buttons
            document.querySelectorAll('.btn-select').forEach(function(btn) {
                btn.addEventListener('click', function() {
                    const id = this.getAttribute('data-id');
                    window.parent.postMessage({
                        type: 'ui-action',
                        action: {
                            tool: 'builtin_playbook__getPlaybook',
                            params: { id: id }
                        }
                    }, '*');
                });
            });
            // Delete buttons
            document.querySelectorAll('.btn-delete').forEach(function(btn) {
                btn.addEventListener('click', function() {
                    const id = this.getAttribute('data-id');
                    if (confirm('Delete playbook "' + id + '"?')) {
                        window.parent.postMessage({
                            type: 'ui-action',
                            action: {
                                tool: 'builtin_playbook__deletePlaybook',
                                params: { id: id }
                            }
                        }, '*');
                    }
                });
            });
        });
    </script>
</body>
</html>"#;

/// Playbook MCP Server
///
/// Provides reusable workflow template management for agent sessions.
/// Session-scoped: Each session can save and render playbooks.
#[derive(Debug)]
pub struct PlaybookServer {
    session_id: String,
    db_pool: Arc<SqlitePool>,
}

impl PlaybookServer {
    /// Create a new PlaybookServer for the given session
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
        // Create playbooks table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS playbooks (
                id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT,
                template TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (id, session_id),
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to create playbooks table: {}", e))?;

        // Create indexes
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_playbooks_session ON playbooks(session_id)")
            .execute(self.db_pool.as_ref())
            .await
            .map_err(|e| format!("Failed to create index: {}", e))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_playbooks_updated ON playbooks(updated_at DESC)",
        )
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to create index: {}", e))?;

        log::debug!(
            "Playbook server tables initialized for session: {}",
            self.session_id
        );

        Ok(())
    }

    /// Save or update a playbook
    async fn save_playbook(&self, args: Value) -> Result<MCPResult, String> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'id' parameter".to_string())?;

        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'title' parameter".to_string())?;

        let description = args.get("description").and_then(|v| v.as_str());

        let template = args
            .get("template")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'template' parameter".to_string())?;

        let now = chrono::Utc::now().timestamp_millis();

        // Upsert playbook (INSERT OR REPLACE)
        let result = sqlx::query(
            r#"
            INSERT INTO playbooks (id, session_id, title, description, template, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id, session_id)
            DO UPDATE SET
                title = excluded.title,
                description = excluded.description,
                template = excluded.template,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(id)
        .bind(&self.session_id)
        .bind(title)
        .bind(description)
        .bind(template)
        .bind(now)
        .bind(now)
        .execute(self.db_pool.as_ref())
        .await;

        match result {
            Ok(_) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Playbook '{}' saved successfully", id),
                }]),
                structured_content: Some(json!({
                    "success": true,
                    "id": id,
                    "title": title,
                    "session_id": self.session_id
                })),
                is_error: Some(false),
            }),
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to save playbook: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    /// Get a playbook by ID
    async fn get_playbook(&self, args: Value) -> Result<MCPResult, String> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'id' parameter".to_string())?;

        let result = sqlx::query_as::<_, (String, String, Option<String>, String, i64, i64)>(
            r#"
            SELECT id, title, description, template, created_at, updated_at
            FROM playbooks
            WHERE id = ? AND session_id = ?
            "#,
        )
        .bind(id)
        .bind(&self.session_id)
        .fetch_optional(self.db_pool.as_ref())
        .await;

        match result {
            Ok(Some((id, title, description, template, created_at, updated_at))) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Playbook: {}", title),
                }]),
                structured_content: Some(json!({
                    "id": id,
                    "title": title,
                    "description": description,
                    "template": template,
                    "created_at": created_at,
                    "updated_at": updated_at
                })),
                is_error: Some(false),
            }),
            Ok(None) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Playbook '{}' not found", id),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to get playbook: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    /// List all playbooks for this session
    async fn list_playbooks(&self, args: Value) -> Result<MCPResult, String> {
        let render_ui = args
            .get("renderUI")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let result = sqlx::query_as::<_, (String, String, Option<String>, i64, i64)>(
            r#"
            SELECT id, title, description, created_at, updated_at
            FROM playbooks
            WHERE session_id = ?
            ORDER BY updated_at DESC
            "#,
        )
        .bind(&self.session_id)
        .fetch_all(self.db_pool.as_ref())
        .await;

        match result {
            Ok(rows) => {
                let playbooks: Vec<Value> = rows
                    .into_iter()
                    .map(|(id, title, description, created_at, updated_at)| {
                        let updated_str = chrono::DateTime::from_timestamp_millis(updated_at)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| "N/A".to_string());
                        json!({
                            "id": id,
                            "title": title,
                            "description": description,
                            "created_at": created_at,
                            "updated_at": updated_str
                        })
                    })
                    .collect();

                let count = playbooks.len();
                let structured = json!({
                    "playbooks": playbooks,
                    "count": count
                });

                // If renderUI is requested, return UI resource
                if render_ui {
                    let html = self.render_playbooks_ui(&playbooks)?;
                    Ok(MCPResult {
                        content: Some(vec![
                            MCPContent::Text {
                                text: format!("Found {} playbooks", count),
                            },
                            MCPContent::Resource {
                                resource: json!({
                                    "uri": format!("ui://playbook/list/{}", self.session_id),
                                    "mimeType": "text/html",
                                    "text": html
                                }),
                            },
                        ]),
                        structured_content: Some(structured),
                        is_error: Some(false),
                    })
                } else {
                    Ok(MCPResult {
                        content: Some(vec![MCPContent::Text {
                            text: format!("Found {} playbooks", count),
                        }]),
                        structured_content: Some(structured),
                        is_error: Some(false),
                    })
                }
            }
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to list playbooks: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    /// Render playbooks list as HTML using Handlebars
    fn render_playbooks_ui(&self, playbooks: &[Value]) -> Result<String, String> {
        let mut handlebars = Handlebars::new();
        handlebars.register_escape_fn(handlebars::html_escape);

        let template_data = json!({
            "playbooks": playbooks,
            "count": playbooks.len(),
            "hasPlaybooks": !playbooks.is_empty()
        });

        handlebars
            .render_template(PLAYBOOK_LIST_TEMPLATE, &template_data)
            .map_err(|e| format!("Failed to render playbook UI: {}", e))
    }

    /// Render a playbook template with provided context
    async fn render_playbook(&self, args: Value) -> Result<MCPResult, String> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'id' parameter".to_string())?;

        let context = args.get("context").cloned().unwrap_or(json!({}));

        // Fetch playbook
        let playbook_result = sqlx::query_as::<_, (String, String)>(
            r#"
            SELECT title, template
            FROM playbooks
            WHERE id = ? AND session_id = ?
            "#,
        )
        .bind(id)
        .bind(&self.session_id)
        .fetch_optional(self.db_pool.as_ref())
        .await;

        match playbook_result {
            Ok(Some((title, template))) => {
                // Simple template rendering using string replacement
                let rendered = self.render_template(&template, &context)?;

                Ok(MCPResult {
                    content: Some(vec![MCPContent::Text {
                        text: rendered.clone(),
                    }]),
                    structured_content: Some(json!({
                        "id": id,
                        "title": title,
                        "rendered": rendered
                    })),
                    is_error: Some(false),
                })
            }
            Ok(None) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Playbook '{}' not found", id),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to render playbook: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    /// Simple template rendering using {{variable}} syntax
    ///
    /// This is a basic implementation similar to workspace/ui_resources.rs
    /// Replaces {{key}} with values from context JSON object
    fn render_template(&self, template: &str, context: &Value) -> Result<String, String> {
        let mut result = template.to_string();

        if let Value::Object(map) = context {
            for (key, value) in map {
                let placeholder = format!("{{{{{}}}}}", key);
                let replacement = match value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Null => "null".to_string(),
                    _ => serde_json::to_string(value).unwrap_or_default(),
                };
                result = result.replace(&placeholder, &replacement);
            }
        }

        Ok(result)
    }

    /// Delete a playbook
    async fn delete_playbook(&self, args: Value) -> Result<MCPResult, String> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'id' parameter".to_string())?;

        let result = sqlx::query(
            r#"
            DELETE FROM playbooks
            WHERE id = ? AND session_id = ?
            "#,
        )
        .bind(id)
        .bind(&self.session_id)
        .execute(self.db_pool.as_ref())
        .await;

        match result {
            Ok(query_result) => {
                if query_result.rows_affected() > 0 {
                    Ok(MCPResult {
                        content: Some(vec![MCPContent::Text {
                            text: format!("Playbook '{}' deleted successfully", id),
                        }]),
                        structured_content: Some(json!({
                            "success": true,
                            "id": id
                        })),
                        is_error: Some(false),
                    })
                } else {
                    Ok(MCPResult {
                        content: Some(vec![MCPContent::Text {
                            text: format!("Playbook '{}' not found", id),
                        }]),
                        structured_content: None,
                        is_error: Some(true),
                    })
                }
            }
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to delete playbook: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }
}

#[async_trait]
impl BuiltinMCPServer for PlaybookServer {
    fn name(&self) -> &str {
        "playbook"
    }

    fn description(&self) -> &str {
        "Session-scoped playbook tools for reusable workflow templates"
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            create_save_playbook_tool(),
            create_get_playbook_tool(),
            create_list_playbooks_tool(),
            create_render_playbook_tool(),
            create_delete_playbook_tool(),
        ]
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String> {
        log::debug!(
            "Playbook server tool called: {} for session: {}",
            tool_name,
            self.session_id
        );

        match tool_name {
            "savePlaybook" | "builtin_playbook__savePlaybook" => {
                self.save_playbook(args).await
            }
            "getPlaybook" | "builtin_playbook__getPlaybook" => self.get_playbook(args).await,
            "listPlaybooks" | "builtin_playbook__listPlaybooks" => {
                self.list_playbooks(args).await
            }
            "renderPlaybook" | "builtin_playbook__renderPlaybook" => {
                self.render_playbook(args).await
            }
            "deletePlaybook" | "builtin_playbook__deletePlaybook" => {
                self.delete_playbook(args).await
            }
            _ => Err(format!(
                "Unknown tool: {}. Available tools: savePlaybook, getPlaybook, listPlaybooks, renderPlaybook, deletePlaybook",
                tool_name
            )),
        }
    }

    async fn switch_context(&self, _options: ServiceContextOptions) -> Result<(), String> {
        Err("Context switching not supported for session-bound playbook server".to_string())
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        ServiceContext {
            context_prompt: format!(
                "# Playbook Server Status\n\
                **Session**: {}\n\
                **Status**: Active\n\
                **Features**: Save, render, and manage reusable workflow templates",
                self.session_id
            ),
            structured_state: None,
        }
    }
}

/// Create the savePlaybook tool definition
fn create_save_playbook_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "id".to_string(),
        string_prop_required("Unique playbook identifier"),
    );
    props.insert("title".to_string(), string_prop_required("Playbook title"));
    props.insert(
        "description".to_string(),
        string_prop(None, None, Some("Optional description")),
    );
    props.insert(
        "template".to_string(),
        string_prop_required("Playbook template with {{variable}} placeholders"),
    );

    MCPTool {
        name: "builtin_playbook__savePlaybook".to_string(),
        title: Some("Save Playbook".to_string()),
        description: "Save or update a reusable workflow template".to_string(),
        input_schema: object_schema(
            props,
            vec![
                "id".to_string(),
                "title".to_string(),
                "template".to_string(),
            ],
        ),
        annotations: None,
        output_schema: None,
    }
}

/// Create the getPlaybook tool definition
fn create_get_playbook_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "id".to_string(),
        string_prop_required("Playbook identifier"),
    );

    MCPTool {
        name: "builtin_playbook__getPlaybook".to_string(),
        title: Some("Get Playbook".to_string()),
        description: "Retrieve a playbook by ID".to_string(),
        input_schema: object_schema(props, vec!["id".to_string()]),
        annotations: None,
        output_schema: None,
    }
}

/// Create the listPlaybooks tool definition
fn create_list_playbooks_tool() -> MCPTool {
    let props = HashMap::new();

    MCPTool {
        name: "builtin_playbook__listPlaybooks".to_string(),
        title: Some("List Playbooks".to_string()),
        description: "List all playbooks for this session".to_string(),
        input_schema: object_schema(props, vec![]),
        annotations: None,
        output_schema: None,
    }
}

/// Create the renderPlaybook tool definition
fn create_render_playbook_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "id".to_string(),
        string_prop_required("Playbook identifier"),
    );
    props.insert(
        "context".to_string(),
        string_prop(None, None, Some("JSON object with template variables")),
    );

    MCPTool {
        name: "builtin_playbook__renderPlaybook".to_string(),
        title: Some("Render Playbook".to_string()),
        description: "Render a playbook template with provided context variables".to_string(),
        input_schema: object_schema(props, vec!["id".to_string()]),
        annotations: None,
        output_schema: None,
    }
}

/// Create the deletePlaybook tool definition
fn create_delete_playbook_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "id".to_string(),
        string_prop_required("Playbook identifier"),
    );

    MCPTool {
        name: "builtin_playbook__deletePlaybook".to_string(),
        title: Some("Delete Playbook".to_string()),
        description: "Delete a playbook by ID".to_string(),
        input_schema: object_schema(props, vec!["id".to_string()]),
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

        // Insert test sessions
        sqlx::query("INSERT OR IGNORE INTO sessions (id, name, status, created_at, updated_at) VALUES ('test-session', 'Test', 'idle', 0, 0)")
            .execute(&pool)
            .await
            .expect("Failed to insert test session");

        sqlx::query("INSERT OR IGNORE INTO sessions (id, name, status, created_at, updated_at) VALUES ('session-1', 'Session 1', 'idle', 0, 0)")
            .execute(&pool)
            .await
            .expect("Failed to insert session 1");

        sqlx::query("INSERT OR IGNORE INTO sessions (id, name, status, created_at, updated_at) VALUES ('session-2', 'Session 2', 'idle', 0, 0)")
            .execute(&pool)
            .await
            .expect("Failed to insert session 2");

        pool
    }

    #[tokio::test]
    async fn test_save_and_get_playbook() {
        let pool = Arc::new(create_test_pool().await);
        let server = PlaybookServer::new("test-session".to_string(), pool)
            .await
            .expect("Failed to create server");

        // Save playbook
        let save_result = server
            .save_playbook(json!({
                "id": "test-playbook",
                "title": "Test Workflow",
                "description": "A test playbook",
                "template": "Hello {{name}}, welcome to {{place}}!"
            }))
            .await
            .expect("Failed to save playbook");

        assert!(save_result.is_error == Some(false));

        // Get playbook
        let get_result = server
            .get_playbook(json!({"id": "test-playbook"}))
            .await
            .expect("Failed to get playbook");

        assert!(get_result.is_error == Some(false));
        let structured = get_result.structured_content.unwrap();
        assert_eq!(structured["title"], "Test Workflow");
        assert_eq!(
            structured["template"],
            "Hello {{name}}, welcome to {{place}}!"
        );
    }

    #[tokio::test]
    async fn test_render_playbook() {
        let pool = Arc::new(create_test_pool().await);
        let server = PlaybookServer::new("test-session".to_string(), pool)
            .await
            .expect("Failed to create server");

        // Save playbook with template
        server
            .save_playbook(json!({
                "id": "greeting",
                "title": "Greeting Template",
                "template": "Hello {{name}}, you have {{count}} messages!"
            }))
            .await
            .expect("Failed to save playbook");

        // Render with context
        let render_result = server
            .render_playbook(json!({
                "id": "greeting",
                "context": {
                    "name": "Alice",
                    "count": 5
                }
            }))
            .await
            .expect("Failed to render playbook");

        assert!(render_result.is_error == Some(false));
        let structured = render_result.structured_content.unwrap();
        assert_eq!(structured["rendered"], "Hello Alice, you have 5 messages!");
    }

    #[tokio::test]
    async fn test_list_and_delete_playbooks() {
        let pool = Arc::new(create_test_pool().await);
        let server = PlaybookServer::new("test-session".to_string(), pool)
            .await
            .expect("Failed to create server");

        // Save multiple playbooks
        server
            .save_playbook(json!({
                "id": "pb1",
                "title": "Playbook 1",
                "template": "Template 1"
            }))
            .await
            .expect("Failed to save playbook 1");

        server
            .save_playbook(json!({
                "id": "pb2",
                "title": "Playbook 2",
                "template": "Template 2"
            }))
            .await
            .expect("Failed to save playbook 2");

        // List playbooks (without UI)
        let list_result = server
            .list_playbooks(json!({"renderUI": false}))
            .await
            .expect("Failed to list playbooks");

        assert!(list_result.is_error == Some(false));
        let structured = list_result.structured_content.unwrap();
        assert_eq!(structured["count"], 2);

        // Delete one playbook
        let delete_result = server
            .delete_playbook(json!({"id": "pb1"}))
            .await
            .expect("Failed to delete playbook");

        assert!(delete_result.is_error == Some(false));

        // List again - should have 1 playbook
        let list_result2 = server
            .list_playbooks(json!({"renderUI": false}))
            .await
            .expect("Failed to list playbooks");

        let structured2 = list_result2.structured_content.unwrap();
        assert_eq!(structured2["count"], 1);
    }

    #[tokio::test]
    async fn test_render_playbook_ui() {
        let pool = Arc::new(create_test_pool().await);
        let server = PlaybookServer::new("test-session".to_string(), pool)
            .await
            .expect("Failed to create server");

        // Save test playbooks
        server
            .save_playbook(json!({
                "id": "playbook-1",
                "title": "First Playbook",
                "description": "First description",
                "template": "Template 1"
            }))
            .await
            .expect("Failed to save playbook 1");

        server
            .save_playbook(json!({
                "id": "playbook-2",
                "title": "Second Playbook",
                "template": "Template 2"
            }))
            .await
            .expect("Failed to save playbook 2");

        // List with UI rendering (default)
        let list_result = server
            .list_playbooks(json!({}))
            .await
            .expect("Failed to list playbooks");

        assert!(list_result.is_error == Some(false));

        // Check content has both text and resource
        let content = list_result.content.as_ref().unwrap();
        assert_eq!(content.len(), 2);

        // Check resource content
        if let MCPContent::Resource { resource } = &content[1] {
            let html = resource["text"].as_str().unwrap();
            assert!(html.contains("<!DOCTYPE html>"));
            assert!(html.contains("First Playbook"));
            assert!(html.contains("Second Playbook"));
            assert!(html.contains("btn-select"));
            assert!(html.contains("btn-delete"));
            assert!(html.contains("playbook-1"));
            assert!(html.contains("playbook-2"));
        } else {
            panic!("Expected Resource content");
        }

        // Check structured content
        let structured = list_result.structured_content.unwrap();
        assert_eq!(structured["count"], 2);
    }

    #[tokio::test]
    async fn test_empty_playbook_list_ui() {
        let pool = Arc::new(create_test_pool().await);
        let server = PlaybookServer::new("test-session".to_string(), pool)
            .await
            .expect("Failed to create server");

        // List with no playbooks
        let list_result = server
            .list_playbooks(json!({}))
            .await
            .expect("Failed to list playbooks");

        assert!(list_result.is_error == Some(false));

        // Check resource content
        let content = list_result.content.as_ref().unwrap();
        if let MCPContent::Resource { resource } = &content[1] {
            let html = resource["text"].as_str().unwrap();
            assert!(html.contains("No playbooks found"));
            assert!(html.contains("Create your first playbook"));
        } else {
            panic!("Expected Resource content");
        }
    }

    #[tokio::test]
    async fn test_session_isolation() {
        let pool = Arc::new(create_test_pool().await);
        let server1 = PlaybookServer::new("session-1".to_string(), pool.clone())
            .await
            .expect("Failed to create server 1");
        let server2 = PlaybookServer::new("session-2".to_string(), pool)
            .await
            .expect("Failed to create server 2");

        // Save playbook in session 1
        server1
            .save_playbook(json!({
                "id": "shared-id",
                "title": "Session 1 Playbook",
                "template": "Session 1 template"
            }))
            .await
            .expect("Failed to save playbook in session 1");

        // Save playbook with same ID in session 2
        server2
            .save_playbook(json!({
                "id": "shared-id",
                "title": "Session 2 Playbook",
                "template": "Session 2 template"
            }))
            .await
            .expect("Failed to save playbook in session 2");

        // Get from session 1
        let get1 = server1
            .get_playbook(json!({"id": "shared-id"}))
            .await
            .expect("Failed to get playbook from session 1");

        let structured1 = get1.structured_content.unwrap();
        assert_eq!(structured1["title"], "Session 1 Playbook");

        // Get from session 2
        let get2 = server2
            .get_playbook(json!({"id": "shared-id"}))
            .await
            .expect("Failed to get playbook from session 2");

        let structured2 = get2.structured_content.unwrap();
        assert_eq!(structured2["title"], "Session 2 Playbook");
    }
}
