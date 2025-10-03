use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::info;

use super::BuiltinMCPServer;
use crate::mcp::types::{ServiceContext, ServiceContextOptions};
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

#[derive(Debug)]
pub struct WorkspaceServer {
    session_manager: Arc<SessionManager>,
    isolation_manager: crate::session_isolation::SessionIsolationManager,
}

impl WorkspaceServer {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        info!("WorkspaceServer using session-based workspace management");
        Self {
            session_manager,
            isolation_manager: crate::session_isolation::SessionIsolationManager::new(),
        }
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

    fn get_workspace_tree(&self, path: &str, max_depth: usize) -> String {
        use std::fs;

        fn build_tree(
            dir: &std::path::Path,
            prefix: &str,
            depth: usize,
            max_depth: usize,
        ) -> String {
            if depth >= max_depth {
                return String::new();
            }

            let mut result = String::new();
            if let Ok(entries) = fs::read_dir(dir) {
                let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
                entries.sort_by_key(|e| e.file_name());

                let mut limited_entries = entries.iter().take(10).peekable();

                while let Some(entry) = limited_entries.next() {
                    let is_last = limited_entries.peek().is_none();
                    let connector = if is_last { "└── " } else { "├── " };
                    let name = entry.file_name().to_string_lossy().to_string();

                    result.push_str(&format!("{prefix}{connector}{name}\n"));

                    if entry.path().is_dir() {
                        let new_prefix =
                            format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                        if depth < max_depth - 1 {
                            result.push_str(&build_tree(
                                &entry.path(),
                                &new_prefix,
                                depth + 1,
                                max_depth,
                            ));
                        }
                    }
                }
            }
            result
        }

        build_tree(std::path::Path::new(path), "", 0, max_depth)
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

    fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        // Get session-specific workspace directory
        let workspace_dir_path = self.get_workspace_dir();
        let workspace_dir = workspace_dir_path.to_string_lossy().to_string();

        // Generate directory tree (2 levels deep)
        let tree_output = self.get_workspace_tree(&workspace_dir, 2);

        let context_prompt = format!(
            "# Workspace Server Status\n\
            **Server**: workspace\n\
            **Status**: Active\n\
            **Working Directory**: {}\n\
            **Available Tools**: {} tools\n\
            \n\
            ## Current Directory Structure\n\
            ```\n\
            {}\n\
            ```",
            workspace_dir,
            self.tools().len(),
            tree_output
        );

        ServiceContext {
            context_prompt,
            structured_state: None,
        }
    }

    async fn switch_context(&self, options: ServiceContextOptions) -> Result<(), String> {
        // Update session context if session_id is provided
        if let Some(session_id) = options.session_id {
            info!("Switching workspace context to session: {}", session_id);

            // Switch session in session_manager
            if let Err(e) = self.session_manager.set_session(session_id.clone()) {
                return Err(format!("Failed to switch session in session_manager: {e}"));
            }

            // The session manager handles session-specific workspace directories
            // No additional action needed as get_workspace_dir() uses session context
        }

        // Update assistant context if assistant_id is provided
        if let Some(assistant_id) = options.assistant_id {
            info!("Switching workspace context to assistant: {}", assistant_id);
            // Workspace server doesn't filter by assistant, but logs for awareness
        }

        Ok(())
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
