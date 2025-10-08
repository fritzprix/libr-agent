use async_trait::async_trait;
use serde_json::{from_value, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

use super::BuiltinMCPServer;
use crate::mcp::types::{ServiceContext, ServiceContextOptions};
use crate::mcp::{MCPResponse, MCPTool};
use crate::services::SecureFileManager;
use crate::session::SessionManager;
use crate::terminal_manager::TerminalManager;

// Module imports
pub mod code_execution;
pub mod export_operations;
pub mod file_operations;
pub mod terminal_manager;
pub mod tools;
pub mod ui_resources;
pub mod utils;

#[derive(Debug)]
pub struct WorkspaceServer {
    session_manager: Arc<SessionManager>,
    terminal_manager: Arc<TerminalManager>,
}

impl WorkspaceServer {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        info!("WorkspaceServer using session-based workspace management");
        Self {
            session_manager,
            terminal_manager: Arc::new(TerminalManager::new()),
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

    // Terminal handlers
    pub async fn handle_open_new_terminal(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();
        let session_id = self.session_manager.get_current_session();
        let shell: Option<String> = args.get("shell").and_then(|v| v.as_str()).map(String::from);
        let env: Option<HashMap<String, String>> =
            from_value(args.get("env").cloned().unwrap_or(Value::Null)).unwrap_or_default();

        match self
            .terminal_manager
            .open_new_terminal(session_id, shell, env)
            .await
        {
            Ok(terminal_id) => {
                let result_val = serde_json::json!({ "terminal_id": terminal_id });
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: Some(result_val),
                    error: None,
                }
            }
            Err(e) => Self::error_response(request_id, -32603, &e),
        }
    }

    pub async fn handle_read_terminal_output(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();
        let terminal_id = match args.get("terminal_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => return Self::error_response(request_id, -32602, "Missing 'terminal_id'"),
        };
        let since_index: Option<usize> = args
            .get("since_index")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        match self
            .terminal_manager
            .read_terminal_output(terminal_id, since_index)
            .await
        {
            Ok(result) => {
                let result_json = serde_json::to_string(&result).unwrap_or_default();
                Self::success_response(request_id, &result_json)
            }
            Err(e) => Self::error_response(request_id, -32603, &e),
        }
    }

    pub async fn handle_close_terminal(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();
        let terminal_id = match args.get("terminal_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => return Self::error_response(request_id, -32602, "Missing 'terminal_id'"),
        };

        match self.terminal_manager.close_terminal(terminal_id).await {
            Ok(_) => {
                Self::success_response(request_id, &format!("Terminal '{terminal_id}' closed."))
            }
            Err(e) => Self::error_response(request_id, -32603, &e),
        }
    }

    pub async fn handle_list_terminals(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();
        let session_id: Option<&str> = args.get("session_id").and_then(|v| v.as_str());

        let summaries = self.terminal_manager.list_terminals(session_id).await;
        let result_json = serde_json::to_string(&summaries).unwrap_or_default();
        Self::success_response(request_id, &result_json)
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

        info!(
            "Workspace service context - workspace_dir: {}, tree_output length: {}",
            workspace_dir,
            tree_output.len()
        );

        let context_prompt = format!(
            "workspace: Active, {} tools, dir: {}",
            self.tools().len(),
            workspace_dir
        );

        ServiceContext {
            context_prompt,
            structured_state: Some(Value::String(workspace_dir)),
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
            // Terminal tools
            "open_new_terminal" => self.handle_open_new_terminal(args).await,
            "read_terminal_output" => self.handle_read_terminal_output(args).await,
            "close_terminal" => self.handle_close_terminal(args).await,
            "list_terminals" => self.handle_list_terminals(args).await,
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
