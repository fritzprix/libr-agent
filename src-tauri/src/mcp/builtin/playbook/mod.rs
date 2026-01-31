use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{BuiltinServerMetadata, MCPResult, MCPTool};
use crate::repositories::{PlaybookRepository, SessionRepository};
use async_trait::async_trait;
use sea_orm::*;
use serde_json::Value;
use std::sync::Arc;

mod operations;
mod templates;
mod tools;
mod types;

// Re-export types if needed, or just use them internally
// use self::types::Playbook;

/// Playbook MCP Server
#[derive(Debug)]
pub struct PlaybookServer {
    assistant_id: String,
    db_conn: DatabaseConnection,
}

impl PlaybookServer {
    pub async fn new(session_id: String, db: Arc<DatabaseConnection>) -> Result<Self, String> {
        let db_conn = (*db).clone();

        // Get assistant_id from session
        let assistant_id = get_assistant_id_from_session(&session_id).await?;

        let server = Self {
            assistant_id,
            db_conn,
        };
        Ok(server)
    }

    /// Get database connection (helper method for future operations)
    #[allow(dead_code)]
    fn get_db(&self) -> &DatabaseConnection {
        &self.db_conn
    }

    /// Get tools statically (without an instance)
    pub fn tools_static() -> Vec<MCPTool> {
        Self::tools_static_internal()
    }

    /// Internal static tools definition to avoid duplication
    fn tools_static_internal() -> Vec<MCPTool> {
        tools::all_tools()
    }

    /// Get metadata statically
    pub fn metadata_static() -> BuiltinServerMetadata {
        BuiltinServerMetadata {
            display_name: "Playbook".to_string(),
            description: "Execute and manage reusable playbooks".to_string(),
            icon: None,
        }
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

    async fn get_service_context(
        &self,
        _options: Option<&Value>,
    ) -> crate::mcp::types::ServiceContext {
        let repo = crate::get_playbook_repository();

        // Query playbook count for this assistant
        let total_count = match repo.count_playbooks(&self.assistant_id).await {
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
        let pagination = crate::repositories::PaginationParams {
            page: 1,
            page_size: 3,
        };
        let models = match repo
            .list_playbooks(Some(&self.assistant_id), pagination)
            .await
        {
            Ok(page) => page.items,
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
                let goal_display = crate::utils::truncate_chars(&playbook.goal, 50);

                // Get short ID (first 8 chars)
                let short_id = crate::utils::safe_truncate(&playbook.id, 8);

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
        Self::tools_static()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        match tool_name {
            "createPlaybook" | "builtin_playbook__createPlaybook" => {
                operations::create_playbook(&self.assistant_id, args).await
            }
            "selectPlaybook" | "builtin_playbook__selectPlaybook" => {
                operations::select_playbook(&self.assistant_id, args).await
            }
            "listPlaybooks" | "builtin_playbook__listPlaybooks" => {
                operations::list_playbooks(&self.assistant_id, args, false).await
            }
            "showPlaybooks" | "builtin_playbook__showPlaybooks" => {
                operations::list_playbooks(&self.assistant_id, args, true).await
            }
            "getPlaybookPage" | "builtin_playbook__getPlaybookPage" => {
                operations::list_playbooks(&self.assistant_id, args, true).await
            }
            "deletePlaybook" | "builtin_playbook__deletePlaybook" => {
                operations::delete_playbook(&self.assistant_id, args).await
            }
            "getPlaybook" | "builtin_playbook__getPlaybook" => {
                operations::get_playbook(&self.assistant_id, args).await
            }
            "updatePlaybook" | "builtin_playbook__updatePlaybook" => {
                operations::update_playbook(&self.assistant_id, args).await
            }
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }
}

// Helper functions for tool definitions

async fn get_assistant_id_from_session(session_id: &str) -> Result<String, String> {
    let session = crate::get_session_repository()
        .get_session(session_id)
        .await
        .map_err(|e| format!("Database error fetching session: {}", e))?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    let config_str = session
        .agent_config
        .clone()
        .ok_or_else(|| "Session has no config".to_string())?;

    let config: serde_json::Value = serde_json::from_str(&config_str)
        .map_err(|e| format!("Invalid session config JSON: {}", e))?;

    config
        .get("assistant_id")
        .or_else(|| config.get("assistantId"))
        .or_else(|| config.get("id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "No assistant ID in session config".to_string())
}
