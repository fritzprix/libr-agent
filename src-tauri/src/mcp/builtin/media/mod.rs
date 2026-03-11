mod handlers;
mod tools;

use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{BuiltinServerMetadata, MCPResult, ServiceContext};
use crate::mcp::MCPTool;
use crate::session::SessionManager;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

pub const NAME: &str = "media";

/// Media MCP Server
///
/// Provides `seeContent` and `listenContent` tools that fetch image and audio
/// content from web URLs or local workspace files and inject them directly into
/// the LLM conversation as multimodal content items.
///
/// Session-isolated: each session gets its own instance bound to the session
/// workspace directory, which is used to validate local file paths.
#[derive(Debug)]
pub struct MediaServer {
    session_id: String,
    session_manager: Arc<SessionManager>,
}

impl MediaServer {
    /// Create a new `MediaServer` for the given session.
    pub fn new(session_id: String, session_manager: Arc<SessionManager>) -> Self {
        Self {
            session_id,
            session_manager,
        }
    }

    /// Get tools statically (without an instance).
    pub fn tools_static() -> Vec<MCPTool> {
        tools::all_tools()
    }

    /// Get metadata statically.
    pub fn metadata_static() -> BuiltinServerMetadata {
        BuiltinServerMetadata {
            display_name: "Media".to_string(),
            description: "Fetch images and audio into the agent context".to_string(),
            icon: None,
        }
    }

    /// Resolve the workspace directory for the current session.
    fn workspace_dir(&self) -> std::path::PathBuf {
        self.session_manager
            .get_session_workspace_dir_by_id(&self.session_id)
    }
}

#[async_trait]
impl BuiltinMCPServer for MediaServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Fetch images and audio from URLs or workspace files and inject them into the agent context"
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
        let workspace_dir = match _session_id {
            Some(ref sid) => self
                .session_manager
                .get_session_workspace_dir_by_id(sid),
            None => self.workspace_dir(),
        };

        log::debug!(
            "Media server tool called: {} (workspace: {})",
            tool_name,
            workspace_dir.display()
        );

        match tool_name {
            "seeContent" => handlers::handle_see_content(args, workspace_dir).await,
            "listenContent" => handlers::handle_listen_content(args, workspace_dir).await,
            _ => Err(format!("Unknown tool: {tool_name}")),
        }
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        // Media server has no persistent state to expose in system prompt.
        ServiceContext {
            context_prompt: String::new(),
            structured_state: Some(json!({})),
        }
    }
}
