use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::schema::JSONSchema;
use crate::mcp::types::{MCPResult, MCPTool};
use crate::mcp::utils::schema_builder::*;
use async_trait::async_trait;
use serde_json::Value; // JSON interaction still needed for tool args
use sqlx::SqlitePool;
use std::sync::Arc;

mod operations;
mod templates;
mod types;

// Re-export types if needed, or just use them internally
// use self::types::Playbook;

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
}

// Helper to create tool definitions concisely
fn create_tool_def(name: &str, description: &str, input_schema: JSONSchema) -> MCPTool {
    MCPTool {
        name: name.to_string(),
        title: Some(name.to_string()),
        description: description.to_string(),
        input_schema,
        output_schema: None,
        annotations: None,
    }
}

fn playbook_step_schema() -> JSONSchema {
    object_prop(
        vec![
            (
                "stepId".to_string(),
                string_prop(None, None, Some("ID for referencing this step")),
            ),
            (
                "description".to_string(),
                string_prop_required("What needs to be done"),
            ),
            (
                "action".to_string(),
                object_prop(
                    vec![
                        (
                            "toolName".to_string(),
                            string_prop_required("Name of tool to use"),
                        ),
                        (
                            "purpose".to_string(),
                            string_prop_required("Why this tool is needed"),
                        ),
                    ],
                    vec!["toolName".to_string(), "purpose".to_string()],
                    Some("Tool execution details"),
                ),
            ),
            (
                "requiredData".to_string(),
                array_schema(
                    string_prop(None, None, None),
                    Some("Data needed from previous steps"),
                ),
            ),
            (
                "outputVariable".to_string(),
                string_prop_required("Variable name to store result"),
            ),
        ],
        vec![
            "description".to_string(),
            "action".to_string(),
            "outputVariable".to_string(),
        ],
        Some("A single step in the playbook"),
    )
}

fn success_criteria_schema() -> JSONSchema {
    object_prop(
        vec![
            (
                "description".to_string(),
                string_prop_required("What constitutes success"),
            ),
            (
                "requiredArtifacts".to_string(),
                array_schema(
                    string_prop(None, None, None),
                    Some("Files/Outputs required"),
                ),
            ),
        ],
        vec!["description".to_string()],
        Some("Criteria for verifying completion"),
    )
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
                object_prop(
                    vec![
                        ("goal".to_string(), string_prop_required("Goal description")),
                        (
                            "initialCommand".to_string(),
                            string_prop(None, None, Some("Original command")),
                        ),
                        (
                            "workflow".to_string(),
                            array_schema(playbook_step_schema(), Some("List of steps")),
                        ),
                        ("successCriteria".to_string(), success_criteria_schema()),
                    ],
                    vec!["goal".to_string(), "workflow".to_string()],
                    None,
                ),
            ),
            create_tool_def(
                "selectPlaybook",
                "Select and prepare a playbook",
                object_prop(
                    vec![("id".to_string(), string_prop_required("Playbook ID"))],
                    vec!["id".to_string()],
                    None,
                ),
            ),
            create_tool_def(
                "listPlaybooks",
                "List playbooks (text only)",
                object_prop(
                    vec![
                        (
                            "page".to_string(),
                            integer_prop(Some(1), None, Some("Page number")),
                        ),
                        (
                            "pageSize".to_string(),
                            integer_prop(Some(10), None, Some("Items per page")),
                        ),
                    ],
                    vec![],
                    None,
                ),
            ),
            create_tool_def(
                "showPlaybooks",
                "Show playbooks (interactive UI)",
                object_prop(
                    vec![
                        (
                            "page".to_string(),
                            integer_prop(Some(1), None, Some("Page number")),
                        ),
                        (
                            "pageSize".to_string(),
                            integer_prop(Some(10), None, Some("Items per page")),
                        ),
                    ],
                    vec![],
                    None,
                ),
            ),
            create_tool_def(
                "getPlaybookPage",
                "Navigate playbook UI",
                object_prop(
                    vec![
                        (
                            "page".to_string(),
                            integer_prop(Some(1), None, Some("Page number")),
                        ),
                        (
                            "pageSize".to_string(),
                            integer_prop(Some(10), None, Some("Items per page")),
                        ),
                    ],
                    vec!["page".to_string()],
                    None,
                ),
            ),
            create_tool_def(
                "deletePlaybook",
                "Delete a playbook",
                object_prop(
                    vec![("id".to_string(), string_prop_required("Playbook ID"))],
                    vec!["id".to_string()],
                    None,
                ),
            ),
            create_tool_def(
                "getPlaybook",
                "Get playbook details",
                object_prop(
                    vec![("id".to_string(), string_prop_required("Playbook ID"))],
                    vec!["id".to_string()],
                    None,
                ),
            ),
            create_tool_def(
                "updatePlaybook",
                "Update a playbook",
                object_prop(
                    vec![
                        ("id".to_string(), string_prop_required("Playbook ID")),
                        (
                            "playbook".to_string(),
                            object_prop(
                                vec![
                                    (
                                        "goal".to_string(),
                                        string_prop(None, None, Some("Goal description")),
                                    ),
                                    (
                                        "initialCommand".to_string(),
                                        string_prop(None, None, Some("Original command")),
                                    ),
                                    (
                                        "workflow".to_string(),
                                        array_schema(playbook_step_schema(), Some("List of steps")),
                                    ),
                                    ("successCriteria".to_string(), success_criteria_schema()),
                                ],
                                vec![],
                                Some("Fields to update"),
                            ),
                        ),
                    ],
                    vec!["id".to_string(), "playbook".to_string()],
                    None,
                ),
            ),
        ]
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String> {
        match tool_name {
            "createPlaybook" | "builtin_playbook__createPlaybook" => {
                operations::create_playbook(&self.db_pool, &self.session_id, args).await
            }
            "selectPlaybook" | "builtin_playbook__selectPlaybook" => {
                operations::select_playbook(&self.db_pool, &self.session_id, args).await
            }
            "listPlaybooks" | "builtin_playbook__listPlaybooks" => {
                operations::list_playbooks(&self.db_pool, &self.session_id, args, false).await
            }
            "showPlaybooks" | "builtin_playbook__showPlaybooks" => {
                operations::list_playbooks(&self.db_pool, &self.session_id, args, true).await
            }
            "getPlaybookPage" | "builtin_playbook__getPlaybookPage" => {
                operations::list_playbooks(&self.db_pool, &self.session_id, args, true).await
            }
            "deletePlaybook" | "builtin_playbook__deletePlaybook" => {
                operations::delete_playbook(&self.db_pool, &self.session_id, args).await
            }
            "getPlaybook" | "builtin_playbook__getPlaybook" => {
                operations::get_playbook(&self.db_pool, &self.session_id, args).await
            }
            "updatePlaybook" | "builtin_playbook__updatePlaybook" => {
                operations::update_playbook(&self.db_pool, &self.session_id, args).await
            }
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }
}
