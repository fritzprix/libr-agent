use crate::mcp::types::MCPResult;
use crate::mcp::types::{BuiltinServerMetadata, ServiceContext};
use crate::mcp::{MCPResponse, MCPTool};
use crate::session::SessionManager;
use async_trait::async_trait;
use serde_json::Value;
use tracing::info;

pub mod assistant;
pub mod bootstrap;
pub mod browser;
pub mod browser_content_store;
pub mod content_store;
pub mod error_guidance;
pub mod knowledge;
pub mod mcp_manager;
pub mod memory;
pub mod planning;
pub mod playbook;
pub mod service_id;
pub mod session_api;
pub mod skills;
pub mod ui;
pub mod utils;
pub mod workspace;

#[cfg(test)]
mod tests;

/// A trait that defines the common interface for all built-in MCP servers.
#[async_trait]
pub trait BuiltinMCPServer: Send + Sync + std::fmt::Debug {
    /// Returns the unique name of the server (e.g., "workspace").
    fn name(&self) -> &str;

    /// Returns a brief description of the server's purpose.
    #[allow(dead_code)]
    fn description(&self) -> &str;

    /// Returns the version of the server.
    #[allow(dead_code)]
    fn version(&self) -> &str {
        "1.0.0"
    }

    /// Returns a human-friendly display name for the UI.
    /// Default: Capitalize the server name
    fn display_name(&self) -> String {
        // Default: capitalize first letter of each word
        self.name()
            .split('_')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Returns complete UI metadata for this server.
    fn metadata(&self) -> BuiltinServerMetadata {
        BuiltinServerMetadata {
            display_name: self.display_name(),
            description: self.description().to_string(),
            icon: None,
        }
    }

    /// Returns a list of all tools provided by this server.
    fn tools(&self) -> Vec<MCPTool>;

    /// Calls a tool on this server with the given arguments.
    ///
    /// # Arguments
    /// * `tool_name` - The name of the tool to call.
    /// * `args` - The arguments for the tool.
    /// * `session_id` - The session ID of the caller, if available.
    ///
    /// # Returns
    /// A `Result` containing the `MCPResult` on success, or an error message on failure.
    /// The wrapping into `MCPResponse` is handled by `BuiltinServerRegistry`.
    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String>;

    /// Returns a markdown-formatted string describing the server's current status and context.
    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        ServiceContext {
            context_prompt: format!(
                "## {}\n**Description**: {}",
                self.display_name(),
                self.description()
            ),
            structured_state: None,
        }
    }

    /// Returns `true` when the server has meaningful state worth including in the
    /// system prompt. Returning `false` causes `get_service_contexts()` to skip
    /// this server entirely, avoiding unnecessary DB round-trips.
    ///
    /// Default: `true` (always included). Override in servers that can be empty.
    async fn has_active_state(&self) -> bool {
        true
    }
}

/// A registry for all built-in MCP servers.
#[derive(Debug)]
pub struct BuiltinServerRegistry {
    servers: std::collections::HashMap<String, Box<dyn BuiltinMCPServer>>,
}

impl BuiltinServerRegistry {
    /// Normalizes LLM-generated JSON arguments to fix common escaping and formatting issues.
    /// This acts as a robust pre-processing step before tool execution.
    ///
    /// # Arguments
    /// * `args` - The `serde_json::Value` representing the arguments.
    ///
    /// # Returns
    /// The normalized `serde_json::Value`.
    fn normalize_json_args(args: Value) -> Value {
        match args {
            Value::Object(mut obj) => {
                // Handle "raw" field from frontend when JSON parsing failed
                if let Some(raw_value) = obj.get("raw").cloned() {
                    info!("Processing raw arguments from frontend JSON parsing failure");
                    if let Value::String(raw_str) = raw_value {
                        // Try to parse the raw JSON string after normalization
                        let normalized_raw = Self::normalize_raw_json_string(&raw_str);
                        match serde_json::from_str::<Value>(&normalized_raw) {
                            Ok(parsed) => {
                                info!("Successfully parsed normalized raw JSON");
                                return Self::normalize_json_args(parsed);
                            }
                            Err(e) => {
                                info!("Failed to parse even after normalization: {}", e);
                                // Fall back to extracting what we can
                                return Self::extract_from_malformed_json(&raw_str);
                            }
                        }
                    }
                }

                // Handle code execution parameters
                if let Some(Value::String(code_str)) = obj.get("code").cloned() {
                    let normalized_code = Self::normalize_code_parameter(&code_str);
                    if normalized_code != code_str {
                        info!("Normalized 'code' parameter for execution");
                        obj.insert("code".to_string(), Value::String(normalized_code));
                    }
                }

                // Handle shell command parameters
                if let Some(Value::String(command_str)) = obj.get("command").cloned() {
                    let normalized_command = Self::normalize_command_parameter(&command_str);
                    if normalized_command != command_str {
                        info!("Normalized 'command' parameter for execution");
                        obj.insert("command".to_string(), Value::String(normalized_command));
                    }
                }

                Value::Object(obj)
            }
            _ => args,
        }
    }

    /// Normalizes a raw JSON string, attempting to fix common escaping issues.
    /// @internal
    fn normalize_raw_json_string(raw_json: &str) -> String {
        let mut normalized = raw_json.to_string();

        // Fix common JSON escaping issues
        // e.g., "code":"print("hello")" -> "code":"print(\"hello\")"
        if normalized.contains("\":\"") && !normalized.ends_with("\"}") {
            // Try to balance quotes in JSON values
            normalized = Self::fix_json_string_values(&normalized);
        }

        normalized
    }

    /// A helper to fix unescaped quotes within JSON string values.
    /// @internal
    fn fix_json_string_values(json_str: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = json_str.chars().collect();
        let mut i = 0;
        let mut in_string_value = false;

        while i < chars.len() {
            let current = chars[i];

            if current == '"' {
                if i > 0 && chars[i - 1] == ':' && !in_string_value {
                    // Starting a string value
                    in_string_value = true;
                    result.push(current);
                } else if in_string_value
                    && (i + 1 >= chars.len() || chars[i + 1] == ',' || chars[i + 1] == '}')
                {
                    // Ending a string value
                    in_string_value = false;
                    result.push(current);
                } else if in_string_value {
                    // Quote inside string value - escape it
                    result.push('\\');
                    result.push(current);
                } else {
                    result.push(current);
                }
            } else {
                result.push(current);
            }
            i += 1;
        }

        result
    }

    /// A fallback method to extract parameters from a malformed JSON string using pattern matching.
    /// @internal
    fn extract_from_malformed_json(malformed: &str) -> Value {
        let mut result = serde_json::Map::new();

        // Try to extract code parameter
        if let Some(code_match) = Self::extract_parameter_value(malformed, "code") {
            result.insert("code".to_string(), Value::String(code_match));
        }

        // Try to extract command parameter
        if let Some(command_match) = Self::extract_parameter_value(malformed, "command") {
            result.insert("command".to_string(), Value::String(command_match));
        }

        info!("Extracted parameters from malformed JSON: {:?}", result);
        Value::Object(result)
    }

    /// Extracts a parameter value from a string using regex-like pattern matching.
    /// @internal
    fn extract_parameter_value(json_str: &str, param_name: &str) -> Option<String> {
        let pattern = format!("\"{param_name}\":\"");
        if let Some(start_idx) = json_str.find(&pattern) {
            let value_start = start_idx + pattern.len();
            let remaining = &json_str[value_start..];

            // Find the end of the value (looking for closing quote or end of object)
            let mut end_idx = 0;
            let mut quote_count = 0;
            for (i, c) in remaining.chars().enumerate() {
                if c == '"' {
                    quote_count += 1;
                    // If we have an odd number of quotes and we're at a logical end point
                    if quote_count % 2 == 1
                        && (i + 1 >= remaining.len()
                            || remaining.chars().nth(i + 1) == Some('}')
                            || remaining.chars().nth(i + 1) == Some(','))
                    {
                        end_idx = i;
                        break;
                    }
                }
            }

            if end_idx > 0 {
                let extracted = remaining[..end_idx].to_string();
                info!("Extracted {} parameter: '{}'", param_name, extracted);
                return Some(extracted);
            }
        }
        None
    }

    /// Normalizes code parameters by fixing unmatched quotes.
    /// @internal
    fn normalize_code_parameter(code: &str) -> String {
        let mut normalized = code.to_string();

        // Fix unmatched quotes
        let double_quote_count = normalized.chars().filter(|&c| c == '"').count();
        let single_quote_count = normalized.chars().filter(|&c| c == '\'').count();

        if double_quote_count % 2 != 0 {
            normalized.push('"');
            info!("Fixed unmatched double quote in code parameter");
        }

        if single_quote_count % 2 != 0 {
            normalized.push('\'');
            info!("Fixed unmatched single quote in code parameter");
        }

        normalized
    }

    /// Normalizes shell command parameters by fixing unmatched or consecutive quotes.
    /// @internal
    fn normalize_command_parameter(command: &str) -> String {
        let mut normalized = command.to_string();

        // Fix unmatched quotes
        let double_quote_count = normalized.chars().filter(|&c| c == '"').count();
        let single_quote_count = normalized.chars().filter(|&c| c == '\'').count();

        if double_quote_count % 2 != 0 {
            normalized.push('"');
            info!("Fixed unmatched double quote in command parameter");
        }

        if single_quote_count % 2 != 0 {
            normalized.push('\'');
            info!("Fixed unmatched single quote in command parameter");
        }

        // Fix consecutive quotes pattern like echo "hello""
        if normalized.contains("\"\"") {
            normalized = Self::fix_consecutive_quotes_in_command(&normalized);
        }

        normalized
    }

    /// A helper to fix consecutive quotes in shell commands.
    /// @internal
    fn fix_consecutive_quotes_in_command(input: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if i + 1 < chars.len() && chars[i] == '"' && chars[i + 1] == '"' {
                // Found consecutive quotes - add only one
                result.push('"');
                i += 2;
                info!("Fixed consecutive quotes in command");
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }

        result
    }

    /// Creates a new `BuiltinServerRegistry` and registers the default servers
    /// using the provided `SessionManager`.
    ///
    /// Note: Only registers stateless servers. Stateful servers (knowledge, planning, playbook,
    /// assistant, browser) are instantiated per-session in MCPServiceProxy.
    pub fn new_with_session_manager(session_manager: std::sync::Arc<SessionManager>) -> Self {
        let mut registry = Self {
            servers: std::collections::HashMap::new(),
        };

        // Register stateless builtin servers
        registry.register_server(Box::new(bootstrap::BootstrapServer::new()));

        registry.register_server(Box::new(workspace::WorkspaceServer::new(
            "default".to_string(),
            session_manager.clone(),
        )));

        registry.register_server(Box::new(
            crate::mcp::builtin::content_store::ContentStoreServer::new(
                "default".to_string(),
                session_manager.clone(),
            ),
        ));

        registry.register_server(Box::new(ui::UiServer::new()));
        registry.register_server(Box::new(mcp_manager::MCPManagerServer::new()));
        registry.register_server(Box::new(session_api::SessionApiServer::new()));

        // Session-specific servers (knowledge, planning, playbook, assistant, browser) are
        // instantiated per-session in MCPServiceProxy::create_builtin_server()

        registry
    }

    /// Creates a new `BuiltinServerRegistry` with SQLite support.
    ///
    /// # Arguments
    /// * `session_manager` - A shared reference to the `SessionManager`.
    /// * `sqlite_db_url` - The connection URL for the SQLite database.
    pub async fn new_with_session_manager_and_sqlite(
        session_manager: std::sync::Arc<SessionManager>,
        sqlite_db_url: String,
    ) -> Self {
        let mut registry = Self {
            servers: std::collections::HashMap::new(),
        };

        // V1 LEGACY: Only register servers that don't need session-specific parameters
        // Agent V2 uses MCPServiceProxy per-session instead
        registry.register_server(Box::new(bootstrap::BootstrapServer::new()));
        // knowledge, planning, playbook, assistant require session_id + db - can't instantiate globally
        // browser requires AppHandle + session_id - can't instantiate globally

        registry.register_server(Box::new(workspace::WorkspaceServer::new(
            "default".to_string(),
            session_manager.clone(),
        )));

        let content_store_server =
            crate::mcp::builtin::content_store::ContentStoreServer::new_with_sqlite(
                "default".to_string(),
                session_manager.clone(),
                sqlite_db_url,
            )
            .await
            .expect("Failed to initialize content store with SQLite");

        registry.register_server(Box::new(content_store_server));

        registry.register_server(Box::new(ui::UiServer::new()));
        // browser requires AppHandle - can't instantiate without Tauri app context
        registry.register_server(Box::new(mcp_manager::MCPManagerServer::new()));
        registry.register_server(Box::new(session_api::SessionApiServer::new()));

        registry
    }

    /// Creates a new `BuiltinServerRegistry` with SeaORM DatabaseConnection.
    ///
    /// # Arguments
    /// * `session_manager` - A shared reference to the `SessionManager`.
    /// * `db` - The SeaORM DatabaseConnection instance.
    pub async fn new_with_session_manager_and_db(
        session_manager: std::sync::Arc<SessionManager>,
        db: sea_orm::DatabaseConnection,
    ) -> Self {
        let mut registry = Self {
            servers: std::collections::HashMap::new(),
        };

        // V1 LEGACY: Only register servers that don't need session-specific parameters
        // Agent V2 uses MCPServiceProxy per-session instead
        registry.register_server(Box::new(bootstrap::BootstrapServer::new()));
        // knowledge, planning, playbook, assistant require session_id + db - can't instantiate globally
        // browser requires AppHandle + session_id - can't instantiate globally

        registry.register_server(Box::new(workspace::WorkspaceServer::new(
            "default".to_string(),
            session_manager.clone(),
        )));

        let content_store_server =
            crate::mcp::builtin::content_store::ContentStoreServer::new_with_db(
                "default".to_string(),
                session_manager.clone(),
                db,
            )
            .await
            .expect("Failed to initialize content store with DatabaseConnection");

        registry.register_server(Box::new(content_store_server));

        registry.register_server(Box::new(ui::UiServer::new()));
        // browser requires AppHandle - can't instantiate without Tauri app context
        registry.register_server(Box::new(mcp_manager::MCPManagerServer::new()));
        registry.register_server(Box::new(session_api::SessionApiServer::new()));

        registry
    }

    /// Registers a new built-in server with the registry.
    ///
    /// # Arguments
    /// * `server` - A `Box` containing a type that implements the `BuiltinMCPServer` trait.
    pub fn register_server(&mut self, server: Box<dyn BuiltinMCPServer>) {
        let name = server.name().to_string();
        self.servers.insert(name, server);
    }

    /// Gets a reference to a server in the registry by name.
    ///
    /// # Arguments
    /// * `name` - The name of the server to retrieve.
    pub fn get_server(&self, name: &str) -> Option<&dyn BuiltinMCPServer> {
        self.servers.get(name).map(|s| s.as_ref())
    }

    /// Lists the names of all registered built-in servers.
    pub fn list_servers(&self) -> Vec<String> {
        self.servers.keys().cloned().collect()
    }

    /// Lists all tools from all registered built-in servers.
    pub fn list_all_tools(&self) -> Vec<MCPTool> {
        let mut all_tools = Vec::new();

        for server in self.servers.values() {
            let tools = server.tools();
            // Prefix tool names with server name for uniqueness
            all_tools.extend(tools);
        }

        all_tools
    }

    /// Lists the tools for a specific registered built-in server.
    ///
    /// # Arguments
    /// * `server_name` - The name of the server. It can optionally have a "builtin." prefix.
    pub fn list_tools_for_server(&self, server_name: &str) -> Vec<MCPTool> {
        // Remove "builtin." prefix if present
        let normalized_server_name = if let Some(stripped) = server_name.strip_prefix("builtin.") {
            stripped
        } else {
            server_name
        };

        if let Some(server) = self.get_server(normalized_server_name) {
            server.tools()
        } else {
            Vec::new()
        }
    }

    /// Gets the service context for a specific built-in server.
    ///
    /// # Arguments
    /// * `server_name` - The name of the server.
    /// * `options` - Optional `Value` to pass to the context function.
    ///
    /// # Returns
    /// A `Result` containing the service context, or an error if the server is not found.
    pub async fn get_server_context(
        &self,
        server_name: &str,
        options: Option<Value>,
    ) -> Result<ServiceContext, String> {
        // Remove "builtin." prefix if present
        let normalized_server_name = if let Some(stripped) = server_name.strip_prefix("builtin.") {
            stripped
        } else {
            server_name
        };

        if let Some(server) = self.get_server(normalized_server_name) {
            Ok(server.get_service_context(options.as_ref()).await)
        } else {
            Err(format!("Built-in server '{server_name}' not found"))
        }
    }

    /// Calls a tool on a specific built-in server.
    ///
    /// # Arguments
    /// * `server_name` - The name of the server.
    /// * `tool_name` - The name of the tool to call.
    /// * `args` - The arguments for the tool.
    /// * `request_id` - Optional request ID from the client. If None, a new UUID is generated.
    /// * `session_id` - Optional session ID of the caller.
    ///
    /// # Returns
    /// An `MCPResponse` containing the result of the tool call.
    /// This method wraps the `MCPResult` from the server's `call_tool` into an `MCPResponse`.
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        args: Value,
        request_id: Option<Value>,
        session_id: Option<String>,
    ) -> MCPResponse {
        // Generate request_id if not provided
        let id = request_id.unwrap_or_else(|| Value::String(uuid::Uuid::new_v4().to_string()));

        if let Some(server) = self.get_server(server_name) {
            // Apply JSON normalization before calling the tool
            let normalized_args = Self::normalize_json_args(args);

            // Call the server's tool - returns Result<MCPResult, String>
            match server
                .call_tool(tool_name, normalized_args, session_id.clone())
                .await
            {
                Ok(mcp_result) => {
                    // Success: wrap MCPResult in MCPResponse with proper type
                    // Convert Value id to JsonRpcId
                    let json_rpc_id = match id {
                        serde_json::Value::String(s) => crate::mcp::types::JsonRpcId::String(s),
                        serde_json::Value::Number(n) => {
                            crate::mcp::types::JsonRpcId::Number(n.as_i64().unwrap_or(0))
                        }
                        serde_json::Value::Null => crate::mcp::types::JsonRpcId::Null,
                        _ => crate::mcp::types::JsonRpcId::String(id.to_string()),
                    };
                    MCPResponse {
                        jsonrpc: "2.0".to_string(),
                        id: Some(json_rpc_id),
                        result: Some(crate::mcp::types::MCPResponseResult::ToolCall(mcp_result)),
                        error: None,
                    }
                }
                Err(err_msg) => {
                    // Error from tool execution: return as MCPError
                    let json_rpc_id = match id {
                        serde_json::Value::String(s) => crate::mcp::types::JsonRpcId::String(s),
                        serde_json::Value::Number(n) => {
                            crate::mcp::types::JsonRpcId::Number(n.as_i64().unwrap_or(0))
                        }
                        serde_json::Value::Null => crate::mcp::types::JsonRpcId::Null,
                        _ => crate::mcp::types::JsonRpcId::String(id.to_string()),
                    };
                    MCPResponse {
                        jsonrpc: "2.0".to_string(),
                        id: Some(json_rpc_id),
                        result: None,
                        error: Some(crate::mcp::MCPError {
                            code: -32603, // Internal error
                            message: err_msg,
                            data: None,
                        }),
                    }
                }
            }
        } else {
            // Server not found
            let json_rpc_id = match id {
                serde_json::Value::String(s) => crate::mcp::types::JsonRpcId::String(s),
                serde_json::Value::Number(n) => {
                    crate::mcp::types::JsonRpcId::Number(n.as_i64().unwrap_or(0))
                }
                serde_json::Value::Null => crate::mcp::types::JsonRpcId::Null,
                _ => crate::mcp::types::JsonRpcId::String(id.to_string()),
            };
            MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(json_rpc_id),
                result: None,
                error: Some(crate::mcp::MCPError {
                    code: -32601, // Method not found
                    message: format!("Built-in server '{server_name}' not found"),
                    data: None,
                }),
            }
        }
    }
}
