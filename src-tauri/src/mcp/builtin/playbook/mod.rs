use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::schema::JSONSchema;
use crate::mcp::types::{MCPResult, MCPTool};
use crate::mcp::utils::schema_builder::*;
use async_trait::async_trait;
use sea_orm::*;
use serde_json::Value; // JSON interaction still needed for tool args
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
    db_conn: DatabaseConnection,
}

impl PlaybookServer {
    pub async fn new(session_id: String, db: Arc<DatabaseConnection>) -> Result<Self, String> {
        let db_conn = (*db).clone();
        let server = Self {
            session_id,
            db_conn,
        };
        Ok(server)
    }

    /// Get database connection (helper method for future operations)
    #[allow(dead_code)]
    fn get_db(&self) -> &DatabaseConnection {
        &self.db_conn
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

    async fn get_service_context(
        &self,
        _options: Option<&Value>,
    ) -> crate::mcp::types::ServiceContext {
        use crate::entity::{playbook, playbook::Entity as PlaybookEntity};

        // Query playbook count for this session
        let total_count = match PlaybookEntity::find()
            .filter(playbook::Column::SessionId.eq(&self.session_id))
            .count(&self.db_conn)
            .await
        {
            Ok(count) => count as i64,
            Err(e) => {
                log::warn!("Failed to count playbooks: {}", e);
                return crate::mcp::types::ServiceContext {
                    context_prompt: "## Playbooks\n\nError loading state".to_string(),
                    structured_state: None,
                };
            }
        };

        // If no playbooks, return minimal context
        if total_count == 0 {
            return crate::mcp::types::ServiceContext {
                context_prompt: "## Playbooks\n\nNo playbooks yet".to_string(),
                structured_state: Some(serde_json::json!({
                    "total_count": 0,
                    "recent_playbooks": []
                })),
            };
        }

        // Fetch recent 3 playbooks (Planning-style detail)
        let models = match PlaybookEntity::find()
            .filter(playbook::Column::SessionId.eq(&self.session_id))
            .order_by_desc(playbook::Column::UpdatedAt)
            .limit(3)
            .all(&self.db_conn)
            .await
        {
            Ok(models) => models,
            Err(e) => {
                log::warn!("Failed to fetch recent playbooks: {}", e);
                return crate::mcp::types::ServiceContext {
                    context_prompt: format!("## Playbooks\n\n{} total", total_count),
                    structured_state: Some(serde_json::json!({
                        "total_count": total_count,
                        "recent_playbooks": []
                    })),
                };
            }
        };

        let playbooks: Vec<types::Playbook> =
            models.iter().map(types::Playbook::from_model).collect();

        // Build context prompt (Planning style: list with details)
        let mut parts = vec![
            "## Playbooks".to_string(),
            String::new(),
            format!("{} total", total_count),
        ];

        if !playbooks.is_empty() {
            parts.push(String::new());
            parts.push("Recent:".to_string());
            for playbook in &playbooks {
                // Truncate goal to 50 chars for token efficiency
                let goal_display = if playbook.goal.len() > 50 {
                    format!("{}...", &playbook.goal[..50])
                } else {
                    playbook.goal.clone()
                };

                // Get short ID (first 8 chars)
                let short_id = if playbook.id.len() > 8 {
                    &playbook.id[..8]
                } else {
                    &playbook.id
                };

                parts.push(format!(
                    "- {} steps: {} ({})",
                    playbook.workflow.len(),
                    goal_display,
                    short_id
                ));
            }
        }

        crate::mcp::types::ServiceContext {
            context_prompt: parts.join("\n"),
            structured_state: Some(serde_json::json!({
                "total_count": total_count,
                "recent_playbooks": playbooks.iter().map(|p| serde_json::json!({
                    "id": p.id,
                    "goal": p.goal,
                    "step_count": p.workflow.len(),
                    "updated_at": p.updated_at
                })).collect::<Vec<_>>()
            })),
        }
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

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        match tool_name {
            "createPlaybook" | "builtin_playbook__createPlaybook" => {
                operations::create_playbook(&self.db_conn, &self.session_id, args).await
            }
            "selectPlaybook" | "builtin_playbook__selectPlaybook" => {
                operations::select_playbook(&self.db_conn, &self.session_id, args).await
            }
            "listPlaybooks" | "builtin_playbook__listPlaybooks" => {
                operations::list_playbooks(&self.db_conn, &self.session_id, args, false).await
            }
            "showPlaybooks" | "builtin_playbook__showPlaybooks" => {
                operations::list_playbooks(&self.db_conn, &self.session_id, args, true).await
            }
            "getPlaybookPage" | "builtin_playbook__getPlaybookPage" => {
                operations::list_playbooks(&self.db_conn, &self.session_id, args, true).await
            }
            "deletePlaybook" | "builtin_playbook__deletePlaybook" => {
                operations::delete_playbook(&self.db_conn, &self.session_id, args).await
            }
            "getPlaybook" | "builtin_playbook__getPlaybook" => {
                operations::get_playbook(&self.db_conn, &self.session_id, args).await
            }
            "updatePlaybook" | "builtin_playbook__updatePlaybook" => {
                operations::update_playbook(&self.db_conn, &self.session_id, args).await
            }
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }
}
