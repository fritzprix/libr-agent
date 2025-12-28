use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::schema::JSONSchema;
use crate::mcp::types::{MCPContent, MCPResult, ServiceContext, ServiceContextOptions};
use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;
use async_trait::async_trait;
use handlebars::Handlebars;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::sync::Arc;

/// HTML template for playbook list UI with pagination
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
        .playbook-goal { font-weight: bold; font-size: 16px; }
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
        button:disabled {
            opacity: 0.5;
            cursor: not-allowed;
        }
        .btn-select {
            background: linear-gradient(45deg, #2196F3, #21CBF3);
            color: white;
        }
        .btn-select:hover:not(:disabled) { transform: translateY(-2px); box-shadow: 0 4px 8px rgba(33, 150, 243, 0.4); }
        .btn-delete {
            background: linear-gradient(45deg, #f44336, #e91e63);
            color: white;
        }
        .btn-delete:hover:not(:disabled) { transform: translateY(-2px); box-shadow: 0 4px 8px rgba(244, 67, 54, 0.4); }
        .pagination {
            display: flex;
            justify-content: center;
            align-items: center;
            gap: 16px;
            margin-top: 24px;
        }
        .page-btn {
            background: rgba(255, 255, 255, 0.2);
            color: white;
        }
        .page-btn:hover:not(:disabled) {
            background: rgba(255, 255, 255, 0.3);
        }
        .empty-state {
            text-align: center;
            padding: 40px;
            opacity: 0.8;
        }
    </style>
</head>
<body>
    <div class='container'>
        <h2>📚 Playbooks ({{totalItems}})</h2>
        {{#if hasPlaybooks}}
        <div id='playbook-list'>
            {{#each playbooks}}
            <div class='playbook-item'>
                <div class='playbook-info'>
                    <div class='playbook-goal'>{{this.goal}}</div>
                    <div class='playbook-meta'>
                        ID: {{this.id}} | Steps: {{this.step_count}} | Created: {{this.created_at_fmt}}
                    </div>
                </div>
                <div class='btn-group'>
                    <button class='btn-select' data-id='{{this.id}}'>Select</button>
                    <button class='btn-delete' data-id='{{this.id}}'>Delete</button>
                </div>
            </div>
            {{/each}}
        </div>
        <div class='pagination'>
            <button class='page-btn' id='prev-btn' {{prevDisabled}}>Previous</button>
            <span>Page {{page}} of {{totalPages}}</span>
            <button class='page-btn' id='next-btn' {{nextDisabled}}>Next</button>
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
                            tool: 'builtin_playbook__selectPlaybook',
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
            // Pagination
            const page = {{page}};
            document.getElementById('prev-btn')?.addEventListener('click', function() {
                window.parent.postMessage({
                    type: 'ui-action',
                    action: {
                        tool: 'builtin_playbook__getPlaybookPage',
                        params: { page: page - 1, pageSize: {{pageSize}} }
                    }
                }, '*');
            });
            document.getElementById('next-btn')?.addEventListener('click', function() {
                window.parent.postMessage({
                    type: 'ui-action',
                    action: {
                        tool: 'builtin_playbook__getPlaybookPage',
                        params: { page: page + 1, pageSize: {{pageSize}} }
                    }
                }, '*');
            });
        });
    </script>
</body>
</html>"#;

// --- Data Structures ---

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PlaybookAction {
    #[serde(rename = "toolName")]
    tool_name: String,
    purpose: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PlaybookStep {
    #[serde(rename = "stepId")]
    step_id: Option<String>,
    description: String,
    action: PlaybookAction,
    #[serde(rename = "requiredData")]
    required_data: Option<Vec<String>>,
    #[serde(rename = "outputVariable")]
    output_variable: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SuccessCriteria {
    description: String,
    #[serde(rename = "requiredArtifacts")]
    required_artifacts: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Playbook {
    id: String,
    session_id: String,
    goal: String,
    #[serde(rename = "initialCommand")]
    initial_command: Option<String>,
    workflow: Vec<PlaybookStep>,
    #[serde(rename = "successCriteria")]
    success_criteria: Option<SuccessCriteria>,
    created_at: i64,
    updated_at: i64,
}

impl Playbook {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Self {
        let workflow_str: String = row.get("workflow");
        let success_criteria_str: Option<String> = row.get("success_criteria");

        Self {
            id: row.get("id"),
            session_id: row.get("session_id"),
            goal: row.get("goal"),
            initial_command: row.get("initial_command"),
            workflow: serde_json::from_str(&workflow_str).unwrap_or_default(),
            success_criteria: success_criteria_str.and_then(|s| serde_json::from_str(&s).ok()),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }
}

/// Playbook MCP Server
#[derive(Debug)]
pub struct PlaybookServer {
    session_id: String,
    db_pool: Arc<SqlitePool>,
}

impl PlaybookServer {
    pub async fn new(session_id: String, db_pool: Arc<SqlitePool>) -> Result<Self, String> {
        let server = Self {
            session_id,
            db_pool,
        };
        server.init_tables().await?;
        Ok(server)
    }

    async fn init_tables(&self) -> Result<(), String> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS playbooks (
                id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                goal TEXT NOT NULL,
                initial_command TEXT,
                workflow JSON NOT NULL,
                success_criteria JSON,
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

        Ok(())
    }

    // --- Tools Implementation ---

    async fn create_playbook(&self, args: Value) -> Result<MCPResult, String> {
        let goal = args
            .get("goal")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'goal'")?;
        let initial_command = args.get("initialCommand").and_then(|v| v.as_str());
        let workflow = args.get("workflow").ok_or("Missing 'workflow'")?;
        let success_criteria = args.get("successCriteria");

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();

        let workflow_json = serde_json::to_string(workflow).map_err(|e| e.to_string())?;
        let success_criteria_json = success_criteria
            .map(|v| serde_json::to_string(v).map_err(|e| e.to_string()))
            .transpose()?;

        sqlx::query(
            r#"
            INSERT INTO playbooks (id, session_id, goal, initial_command, workflow, success_criteria, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&self.session_id)
        .bind(goal)
        .bind(initial_command)
        .bind(&workflow_json)
        .bind(&success_criteria_json)
        .bind(now)
        .bind(now)
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to create playbook: {}", e))?;

        // Re-fetch to get the full object
        let row = sqlx::query("SELECT * FROM playbooks WHERE id = ? AND session_id = ?")
            .bind(&id)
            .bind(&self.session_id)
            .fetch_one(self.db_pool.as_ref())
            .await
            .map_err(|e| format!("Failed to fetch created playbook: {}", e))?;

        let playbook = Playbook::from_row(&row);
        let formatted = self.format_playbook_summary(&playbook);

        let text_response = format!(
            "Successfully created new playbook.\nID: {}\nGoal: {}\nSteps: {}\n\n{}\n\nThe playbook is now available. Use 'listPlaybooks' to see all playbooks, or 'selectPlaybook' with ID {} to execute it.",
            id, playbook.goal, playbook.workflow.len(), formatted, id
        );

        Ok(MCPResult {
            content: Some(vec![MCPContent::Text {
                text: text_response,
            }]),
            structured_content: Some(json!({
                "success": true,
                "playbook": playbook
            })),
            is_error: Some(false),
        })
    }

    fn format_playbook_summary(&self, p: &Playbook) -> String {
        let created = chrono::DateTime::from_timestamp_millis(p.created_at)
            .map(|dt| dt.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        format!(
            "id:{} goal:\"{}\" initial:\"{}\" steps:{} createdAt:{}",
            p.id,
            p.goal,
            p.initial_command.as_deref().unwrap_or(""),
            p.workflow.len(),
            created
        )
    }

    async fn list_playbooks(&self, args: Value, render_ui: bool) -> Result<MCPResult, String> {
        let page = args
            .get("page")
            .and_then(|v| v.as_i64())
            .unwrap_or(1)
            .max(1);
        let page_size = args.get("pageSize").and_then(|v| v.as_i64()).unwrap_or(10); // Default 10

        let limit = if page_size < 0 { -1 } else { page_size };
        let offset = if page_size < 0 {
            0
        } else {
            (page - 1) * page_size
        };

        // Count total
        let total_items: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM playbooks WHERE session_id = ?")
                .bind(&self.session_id)
                .fetch_one(self.db_pool.as_ref())
                .await
                .unwrap_or(0);

        // Fetch items
        let query = if limit < 0 {
            "SELECT * FROM playbooks WHERE session_id = ? ORDER BY updated_at DESC".to_string()
        } else {
            "SELECT * FROM playbooks WHERE session_id = ? ORDER BY updated_at DESC LIMIT ? OFFSET ?"
                .to_string()
        };

        let mut q = sqlx::query(&query).bind(&self.session_id);
        if limit >= 0 {
            q = q.bind(limit).bind(offset);
        }

        let rows = q
            .fetch_all(self.db_pool.as_ref())
            .await
            .map_err(|e| format!("Failed to list playbooks: {}", e))?;

        let playbooks: Vec<Playbook> = rows.iter().map(Playbook::from_row).collect();
        let total_pages = if page_size > 0 {
            (total_items as f64 / page_size as f64).ceil() as i64
        } else {
            1
        };

        let formatted_list = if playbooks.is_empty() {
            format!("No playbooks found for session {}.", self.session_id)
        } else {
            playbooks
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    format!(
                        "{}. {}",
                        offset + i as i64 + 1,
                        self.format_playbook_summary(p)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        let page_result = json!({
            "page": page,
            "pageSize": page_size,
            "totalItems": total_items,
            "totalPages": total_pages,
            "items": playbooks
        });

        let structured = json!({
            "page": page_result,
            "formattedText": formatted_list
        });

        if render_ui {
            let html = self.render_ui(&playbooks, page, total_pages, total_items, page_size)?;
            let tool_name = if args.get("tool").and_then(|v| v.as_str()) == Some("getPlaybookPage")
            {
                "getPlaybookPage"
            } else {
                "showPlaybooks"
            };

            let action_text = if tool_name == "getPlaybookPage" {
                format!("Navigated to page {}", page)
            } else {
                format!("Displaying {} playbook(s) in interactive UI", total_items)
            };

            let text_response = format!(
                "[{}] {}.\nCurrent page: {} of {}\n\nPlaybooks on this page:\n{}\n\nStatus: Agent paused for user interaction (Select/Delete/Navigate buttons available).",
                tool_name, action_text, page, total_pages, formatted_list
            );

            Ok(MCPResult {
                content: Some(vec![
                    MCPContent::Text {
                        text: text_response,
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
            let text_response = if playbooks.is_empty() {
                format!(
                    "[listPlaybooks] No playbooks found for session {}.",
                    self.session_id
                )
            } else {
                format!(
                    "[listPlaybooks] Found {} playbook(s) for session {}.\nShowing page {} of {} ({} items on this page):\n\n{}\n\nNote: Use 'getPlaybook' to view details or 'selectPlaybook' to execute a playbook.",
                    total_items, self.session_id, page, total_pages, playbooks.len(), formatted_list
                )
            };

            Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: text_response,
                }]),
                structured_content: Some(structured),
                is_error: Some(false),
            })
        }
    }

    fn render_ui(
        &self,
        playbooks: &[Playbook],
        page: i64,
        total_pages: i64,
        total_items: i64,
        page_size: i64,
    ) -> Result<String, String> {
        let mut handlebars = Handlebars::new();
        handlebars.register_escape_fn(handlebars::html_escape);

        let view_models: Vec<Value> = playbooks
            .iter()
            .map(|p| {
                json!({
                    "id": p.id,
                    "goal": p.goal,
                    "step_count": p.workflow.len(),
                    "created_at_fmt": chrono::DateTime::from_timestamp_millis(p.created_at)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_default()
                })
            })
            .collect();

        let data = json!({
            "playbooks": view_models,
            "hasPlaybooks": !playbooks.is_empty(),
            "page": page,
            "totalPages": total_pages,
            "totalItems": total_items,
            "pageSize": page_size,
            "prevDisabled": if page <= 1 { "disabled" } else { "" },
            "nextDisabled": if page >= total_pages { "disabled" } else { "" }
        });

        handlebars
            .render_template(PLAYBOOK_LIST_TEMPLATE, &data)
            .map_err(|e| format!("Failed to render UI: {}", e))
    }

    async fn get_playbook(&self, args: Value) -> Result<MCPResult, String> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'id'")?;

        let row = sqlx::query("SELECT * FROM playbooks WHERE id = ? AND session_id = ?")
            .bind(id)
            .bind(&self.session_id)
            .fetch_optional(self.db_pool.as_ref())
            .await
            .map_err(|e| format!("DB Error: {}", e))?;

        match row {
            Some(row) => {
                let playbook = Playbook::from_row(&row);
                let formatted = self.format_playbook_detailed(&playbook);

                let text_response = format!(
                    "[get_playbook] Retrieved playbook details for ID: {}\n\n{}\n\nNote: Use 'selectPlaybook' to execute this playbook, or 'updatePlaybook' to modify it.",
                    id, formatted
                );

                Ok(MCPResult {
                    content: Some(vec![MCPContent::Text {
                        text: text_response,
                    }]),
                    structured_content: Some(json!({ "playbook": playbook })),
                    is_error: Some(false),
                })
            }
            None => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Playbook '{}' not found", id),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    async fn select_playbook(&self, args: Value) -> Result<MCPResult, String> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'id'")?;

        let row = sqlx::query("SELECT * FROM playbooks WHERE id = ? AND session_id = ?")
            .bind(id)
            .bind(&self.session_id)
            .fetch_optional(self.db_pool.as_ref())
            .await
            .map_err(|e| format!("DB Error: {}", e))?;

        match row {
            Some(row) => {
                let playbook = Playbook::from_row(&row);
                let details = self.format_playbook_detailed(&playbook);
                let prompt = format!(
                    "[select_playbook] Playbook \"{}\" (ID: {}) has been selected for execution.\n\nPlaybook Details:\n---\n{}\n---\n\nInstructions:\n1. Review the workflow steps and success criteria above\n2. Establish todos based on the workflow steps\n3. Begin executing the tasks according to the defined steps\n4. Track progress and verify against success criteria\n\nYou may now proceed with execution.",
                    playbook.goal, playbook.id, details
                );

                Ok(MCPResult {
                    content: Some(vec![MCPContent::Text { text: prompt }]),
                    structured_content: Some(json!({ "playbook": playbook })),
                    is_error: Some(false),
                })
            }
            None => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Playbook '{}' not found", id),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    fn format_playbook_detailed(&self, p: &Playbook) -> String {
        let mut lines = Vec::new();
        lines.push(format!("ID: {}", p.id));
        lines.push(format!("Goal: {}", p.goal));
        if let Some(cmd) = &p.initial_command {
            lines.push(format!("Initial Command: {}", cmd));
        }
        lines.push(format!("Steps: {}", p.workflow.len()));

        if !p.workflow.is_empty() {
            lines.push("\n--- Workflow ---".to_string());
            for (i, step) in p.workflow.iter().enumerate() {
                lines.push(format!(
                    "{}. Step ID: {}",
                    i + 1,
                    step.step_id.as_deref().unwrap_or("N/A")
                ));
                lines.push(format!("   Description: {}", step.description));
                lines.push(format!(
                    "   Tool: {} (Purpose: {})",
                    step.action.tool_name, step.action.purpose
                ));
                if let Some(req) = &step.required_data {
                    lines.push(format!("   Required Data: {}", req.join(", ")));
                }
                lines.push(format!("   Output Variable: {}", step.output_variable));
            }
        }

        if let Some(sc) = &p.success_criteria {
            lines.push("\n--- Success Criteria ---".to_string());
            lines.push(format!("Description: {}", sc.description));
            if let Some(arts) = &sc.required_artifacts {
                lines.push(format!("Required Artifacts: {}", arts.join(", ")));
            }
        }

        lines.join("\n")
    }

    async fn delete_playbook(&self, args: Value) -> Result<MCPResult, String> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'id'")?;

        let result = sqlx::query("DELETE FROM playbooks WHERE id = ? AND session_id = ?")
            .bind(id)
            .bind(&self.session_id)
            .execute(self.db_pool.as_ref())
            .await
            .map_err(|e| format!("DB Error: {}", e))?;

        if result.rows_affected() > 0 {
            Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Playbook '{}' deleted", id),
                }]),
                structured_content: Some(json!({ "success": true, "id": id })),
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

    async fn update_playbook(&self, args: Value) -> Result<MCPResult, String> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'id'")?;

        let playbook_obj = args.get("playbook").ok_or("Missing 'playbook' object")?;

        // Fetch existing to merge
        let existing_row = sqlx::query("SELECT * FROM playbooks WHERE id = ? AND session_id = ?")
            .bind(id)
            .bind(&self.session_id)
            .fetch_optional(self.db_pool.as_ref())
            .await
            .map_err(|e| format!("DB Error: {}", e))?;

        let mut existing = match existing_row {
            Some(row) => Playbook::from_row(&row),
            None => {
                return Ok(MCPResult {
                    content: Some(vec![MCPContent::Text {
                        text: format!("Playbook '{}' not found", id),
                    }]),
                    structured_content: None,
                    is_error: Some(true),
                })
            }
        };

        // Update fields if present
        if let Some(g) = playbook_obj.get("goal").and_then(|v| v.as_str()) {
            existing.goal = g.to_string();
        }
        if let Some(c) = playbook_obj.get("initialCommand").and_then(|v| v.as_str()) {
            existing.initial_command = Some(c.to_string());
        }
        if let Some(w) = playbook_obj.get("workflow") {
            existing.workflow = serde_json::from_value(w.clone()).map_err(|e| e.to_string())?;
        }
        if let Some(s) = playbook_obj.get("successCriteria") {
            existing.success_criteria =
                serde_json::from_value(s.clone()).map_err(|e| e.to_string())?;
        }

        let now = chrono::Utc::now().timestamp_millis();
        let workflow_json = serde_json::to_string(&existing.workflow).unwrap();
        let success_criteria_json = serde_json::to_string(&existing.success_criteria).unwrap();

        sqlx::query(
            r#"
            UPDATE playbooks 
            SET goal = ?, initial_command = ?, workflow = ?, success_criteria = ?, updated_at = ?
            WHERE id = ? AND session_id = ?
            "#,
        )
        .bind(&existing.goal)
        .bind(&existing.initial_command)
        .bind(workflow_json)
        .bind(success_criteria_json)
        .bind(now)
        .bind(id)
        .bind(&self.session_id)
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Update failed: {}", e))?;

        let formatted = self.format_playbook_summary(&existing);
        let text_response = format!(
            "Successfully updated playbook ID: {}\n\nUpdated Details:\n{}\n\nThe playbook has been modified. Changes are immediately available.",
            id, formatted
        );

        Ok(MCPResult {
            content: Some(vec![MCPContent::Text {
                text: text_response,
            }]),
            structured_content: Some(json!({
                "success": true,
                "playbook": existing
            })),
            is_error: Some(false),
        })
    }
}

#[async_trait]
impl BuiltinMCPServer for PlaybookServer {
    fn name(&self) -> &str {
        "playbook"
    }

    fn description(&self) -> &str {
        "Playbook management for reusable workflows"
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            create_tool_def(
                "createPlaybook",
                "Create a new playbook",
                json!({
                    "goal": string_prop_required("Goal description"),
                    "initialCommand": string_prop(None, None, Some("Original command")),
                    "workflow": serde_json::to_value(array_schema(playbook_step_schema(), Some("List of steps"))).unwrap(),
                    "successCriteria": serde_json::to_value(success_criteria_schema()).unwrap()
                }),
                vec!["goal", "workflow"],
            ),
            create_tool_def(
                "selectPlaybook",
                "Select and prepare a playbook",
                json!({
                    "id": string_prop_required("Playbook ID")
                }),
                vec!["id"],
            ),
            create_tool_def(
                "listPlaybooks",
                "List playbooks (text only)",
                json!({
                    "page": integer_prop(Some(1), None, Some("Page number")),
                    "pageSize": integer_prop(Some(10), None, Some("Items per page"))
                }),
                vec![],
            ),
            create_tool_def(
                "showPlaybooks",
                "Show playbooks (interactive UI)",
                json!({
                    "page": integer_prop(Some(1), None, Some("Page number")),
                    "pageSize": integer_prop(Some(10), None, Some("Items per page"))
                }),
                vec![],
            ),
            create_tool_def(
                "getPlaybookPage",
                "Navigate playbook UI",
                json!({
                    "page": integer_prop(Some(1), None, Some("Page number")),
                    "pageSize": integer_prop(Some(10), None, Some("Items per page"))
                }),
                vec!["page"],
            ),
            create_tool_def(
                "deletePlaybook",
                "Delete a playbook",
                json!({
                    "id": string_prop_required("Playbook ID")
                }),
                vec!["id"],
            ),
            create_tool_def(
                "getPlaybook",
                "Get playbook details",
                json!({
                    "id": string_prop_required("Playbook ID")
                }),
                vec!["id"],
            ),
            create_tool_def(
                "updatePlaybook",
                "Update a playbook",
                json!({
                    "id": string_prop_required("Playbook ID"),
                    "playbook": serde_json::to_value(object_prop(
                        vec![
                            ("goal".to_string(), string_prop(None, None, Some("Goal description"))),
                            ("initialCommand".to_string(), string_prop(None, None, Some("Original command"))),
                            ("workflow".to_string(), array_schema(playbook_step_schema(), Some("List of steps"))),
                            ("successCriteria".to_string(), success_criteria_schema()),
                        ],
                        vec![],
                        Some("Fields to update")
                    )).unwrap()
                }),
                vec!["id", "playbook"],
            ),
        ]
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String> {
        match tool_name {
            "createPlaybook" | "builtin_playbook__createPlaybook" => {
                self.create_playbook(args).await
            }
            "selectPlaybook" | "builtin_playbook__selectPlaybook" => {
                self.select_playbook(args).await
            }
            "listPlaybooks" | "builtin_playbook__listPlaybooks" => {
                self.list_playbooks(args, false).await
            }
            "showPlaybooks" | "builtin_playbook__showPlaybooks" => {
                self.list_playbooks(args, true).await
            }
            "getPlaybookPage" | "builtin_playbook__getPlaybookPage" => {
                self.list_playbooks(args, true).await
            }
            "deletePlaybook" | "builtin_playbook__deletePlaybook" => {
                self.delete_playbook(args).await
            }
            "getPlaybook" | "builtin_playbook__getPlaybook" => self.get_playbook(args).await,
            "updatePlaybook" | "builtin_playbook__updatePlaybook" => {
                self.update_playbook(args).await
            }
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }

    async fn switch_context(&self, _options: ServiceContextOptions) -> Result<(), String> {
        Err("Context switching not supported".to_string())
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        ServiceContext {
            context_prompt: format!(
                "# Playbook Server\nSession: {}\nManage reusable workflows.",
                self.session_id
            ),
            structured_state: None,
        }
    }
}

// Helper to create tool definitions concisely
fn create_tool_def(name: &str, desc: &str, props: Value, required: Vec<&str>) -> MCPTool {
    let mut prop_map = HashMap::new();
    if let Value::Object(map) = props {
        for (k, v) in map {
            // Fix: Use serde_json::from_value to convert Value to JSONSchema
            // This avoids the "trait bound JSONSchema: From<Value> is not satisfied" error
            let schema: JSONSchema = serde_json::from_value(v).unwrap();
            prop_map.insert(k, schema);
        }
    }

    MCPTool {
        name: format!("builtin_playbook__{}", name),
        title: Some(name.to_string()),
        description: desc.to_string(),
        input_schema: object_schema(prop_map, required.into_iter().map(String::from).collect()),
        annotations: None,
        output_schema: None,
    }
}

fn playbook_step_schema() -> JSONSchema {
    object_prop(
        vec![
            (
                "stepId".to_string(),
                string_prop(None, None, Some("Optional step ID")),
            ),
            (
                "description".to_string(),
                string_prop_required("Step description"),
            ),
            (
                "action".to_string(),
                object_prop(
                    vec![
                        (
                            "toolName".to_string(),
                            string_prop_required("Tool to execute"),
                        ),
                        (
                            "purpose".to_string(),
                            string_prop_required("Purpose of the tool call"),
                        ),
                    ],
                    vec!["toolName".to_string(), "purpose".to_string()],
                    Some("Action to perform"),
                ),
            ),
            (
                "requiredData".to_string(),
                array_schema(
                    string_prop(None, None, None),
                    Some("List of required data keys"),
                ),
            ),
            (
                "outputVariable".to_string(),
                string_prop_required("Variable to store output"),
            ),
        ],
        vec![
            "description".to_string(),
            "action".to_string(),
            "outputVariable".to_string(),
        ],
        Some("A step in the playbook workflow"),
    )
}

fn success_criteria_schema() -> JSONSchema {
    object_prop(
        vec![
            (
                "description".to_string(),
                string_prop_required("Success criteria description"),
            ),
            (
                "requiredArtifacts".to_string(),
                array_schema(
                    string_prop(None, None, None),
                    Some("List of required artifacts"),
                ),
            ),
        ],
        vec!["description".to_string()],
        Some("Criteria for playbook success"),
    )
}
