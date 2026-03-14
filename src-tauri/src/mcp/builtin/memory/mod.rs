mod handlers;
mod tools;

use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{BuiltinServerMetadata, MCPResult, ServiceContext};
use crate::mcp::MCPTool;
use crate::repositories::PlanningRepository;
use crate::state::get_planning_repository;
use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};
use std::sync::Arc;

/// Memory MCP Server
///
/// Provides working memory (notes), thinking, and reflection tools for agent sessions.
/// Session-scoped: each session gets dedicated memory state backed by `planning_scratchpad` table.
#[derive(Debug)]
pub struct MemoryServer {
    session_id: String,
    db: Arc<DatabaseConnection>,
}

impl MemoryServer {
    /// Create a new `MemoryServer` for the given session.
    pub async fn new(session_id: String, db: Arc<DatabaseConnection>) -> Result<Self, String> {
        Ok(Self { session_id, db })
    }

    /// Get tools statically (without an instance).
    pub fn tools_static() -> Vec<MCPTool> {
        tools::all_tools()
    }

    /// Get metadata statically.
    pub fn metadata_static() -> BuiltinServerMetadata {
        BuiltinServerMetadata {
            display_name: "Memory".to_string(),
            description: "Working memory notes and thinking tools".to_string(),
            icon: None,
        }
    }
}

pub const NAME: &str = "memory";

#[async_trait]
impl BuiltinMCPServer for MemoryServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Session-scoped working memory: notes, thinking, and reflection tools"
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
        let target_session_id = _session_id.unwrap_or_else(|| self.session_id.clone());
        log::debug!(
            "Memory server tool called: {} for session: {}",
            tool_name,
            target_session_id
        );

        match tool_name {
            "add" => handlers::add(self.db.as_ref(), &target_session_id, args).await,
            "update" => handlers::update(self.db.as_ref(), &target_session_id, args).await,
            "list" => handlers::list(self.db.as_ref(), &target_session_id, args).await,
            "read" => handlers::read(self.db.as_ref(), &target_session_id, args).await,
            "clear" => handlers::clear(self.db.as_ref(), &target_session_id, args).await,
            "think" => handlers::think(args).await,
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        let repo = get_planning_repository();
        let items = repo
            .list_scratchpad(&self.session_id)
            .await
            .unwrap_or_else(|e| {
                log::error!("Failed to fetch memory items: {}", e);
                Vec::new()
            });

        let mut parts = vec!["## Memory".to_string()];

        if items.is_empty() {
            parts.push("\n*No memory notes.*".to_string());
        } else {
            parts.push(format!("\n**Notes:** {} item(s)", items.len()));
            for item in &items {
                let content = item.content.replace(['\n', '\r'], " ");
                let summary = if content.chars().count() > 80 {
                    let s: String = content.chars().take(77).collect();
                    format!("{}...", s)
                } else {
                    content
                };
                let title = item.title.as_deref().unwrap_or("Note");
                parts.push(format!("- **[ID: {}] {}**: {}", item.id, title, summary));
            }
        }

        let structured_state = json!({
            "items": items.iter().map(|i| json!({
                "id": i.id,
                "title": i.title,
                "content": i.content
            })).collect::<Vec<_>>(),
            "count": items.len()
        });

        ServiceContext {
            context_prompt: parts.join("\n"),
            structured_state: Some(structured_state),
        }
    }
}
