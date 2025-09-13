use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::info;

use super::BuiltinMCPServer;
use crate::mcp::{MCPResponse, MCPTool};
use crate::services::SecureFileManager;
use crate::session::SessionManager;

// Module imports
pub mod code_execution;
pub mod export_operations;
pub mod file_operations;
pub mod tools;
pub mod ui_resources;
pub mod utils;

pub struct WorkspaceServer {
    session_manager: Arc<SessionManager>,
}

impl WorkspaceServer {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        info!("WorkspaceServer using session-based workspace management");
        Self { session_manager }
    }

    // Common utility methods
    pub fn get_workspace_dir(&self) -> std::path::PathBuf {
        self.session_manager.get_session_workspace_dir()
    }

    pub fn get_file_manager(&self) -> Arc<SecureFileManager> {
        self.session_manager.get_file_manager()
    }

    // Common response creation methods (wrappers)
    pub fn generate_request_id() -> Value {
        utils::generate_request_id()
    }

    pub fn success_response(request_id: Value, message: &str) -> MCPResponse {
        utils::create_success_response(request_id, message)
    }

    pub fn error_response(request_id: Value, code: i32, message: &str) -> MCPResponse {
        utils::create_error_response(request_id, code, message)
    }
}

#[async_trait]
impl BuiltinMCPServer for WorkspaceServer {
    fn name(&self) -> &str {
        "workspace"
    }

    fn description(&self) -> &str {
        "Integrated workspace for file operations and code execution"
    }

    fn tools(&self) -> Vec<MCPTool> {
        let mut tools = Vec::new();
        tools.extend(tools::file_tools());
        tools.extend(tools::code_tools());
        tools.extend(tools::export_tools());
        tools
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse {
        match tool_name {
            // File operation tools
            "read_file" => self.handle_read_file(args).await,
            "write_file" => self.handle_write_file(args).await,
            "list_directory" => self.handle_list_directory(args).await,
            "search_files" => self.handle_search_files(args).await,
            "replace_lines_in_file" => self.handle_replace_lines_in_file(args).await,
            "grep" => self.handle_grep(args).await,
            "import_file" => self.handle_import_file(args).await,
            // Code execution tools
            "execute_python" => self.handle_execute_python(args).await,
            "execute_typescript" => self.handle_execute_typescript(args).await,
            "execute_shell" => self.handle_execute_shell(args).await,
            // Export tools
            "export_file" => self.handle_export_file(args).await,
            "export_zip" => self.handle_export_zip(args).await,
            _ => {
                let request_id = Self::generate_request_id();
                Self::error_response(request_id, -32601, &format!("Tool '{tool_name}' not found"))
            }
        }
    }
}
