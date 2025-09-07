use crate::mcp::{MCPResponse, MCPTool};
use crate::session::SessionManager;
use async_trait::async_trait;
use serde_json::Value;
use tracing::info;

pub mod utils;
pub mod workspace;

/// Trait for built-in MCP servers
#[async_trait]
pub trait BuiltinMCPServer: Send + Sync {
    /// Server name (e.g., "builtin.filesystem")
    fn name(&self) -> &str;

    /// Server description
    #[allow(dead_code)]
    fn description(&self) -> &str;

    /// Server version
    #[allow(dead_code)]
    fn version(&self) -> &str {
        "1.0.0"
    }

    /// List available tools for this server
    fn tools(&self) -> Vec<MCPTool>;

    /// Call a tool on this server
    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse;
}

/// Built-in server registry
pub struct BuiltinServerRegistry {
    servers: std::collections::HashMap<String, Box<dyn BuiltinMCPServer>>,
}

impl BuiltinServerRegistry {
    /// Normalize LLM-generated JSON arguments to fix common escaping issues
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
                if let Some(code_value) = obj.get("code").cloned() {
                    if let Value::String(code_str) = code_value {
                        let normalized_code = Self::normalize_code_parameter(&code_str);
                        if normalized_code != code_str {
                            info!("Normalized 'code' parameter for execution");
                            obj.insert("code".to_string(), Value::String(normalized_code));
                        }
                    }
                }

                // Handle shell command parameters
                if let Some(command_value) = obj.get("command").cloned() {
                    if let Value::String(command_str) = command_value {
                        let normalized_command = Self::normalize_command_parameter(&command_str);
                        if normalized_command != command_str {
                            info!("Normalized 'command' parameter for execution");
                            obj.insert("command".to_string(), Value::String(normalized_command));
                        }
                    }
                }

                Value::Object(obj)
            }
            _ => args,
        }
    }

    /// Normalize raw JSON string from frontend
    fn normalize_raw_json_string(raw_json: &str) -> String {
        let mut normalized = raw_json.to_string();

        // Fix common JSON escaping issues
        // Pattern: "code":"print("hello")" -> "code":"print(\"hello\")"
        if normalized.contains("\":\"") && !normalized.ends_with("\"}") {
            // Try to balance quotes in JSON values
            normalized = Self::fix_json_string_values(&normalized);
        }

        normalized
    }

    /// Fix JSON string values with unescaped quotes
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

    /// Extract parameters from malformed JSON as fallback
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

    /// Extract a parameter value from malformed JSON using regex-like pattern matching
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

    /// Normalize code parameters (Python/TypeScript)
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

    /// Normalize shell command parameters
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

    /// Fix consecutive quotes in shell commands
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

    pub fn new_with_session_manager(session_manager: std::sync::Arc<SessionManager>) -> Self {
        let mut registry = Self {
            servers: std::collections::HashMap::new(),
        };

        // Register built-in workspace server with SessionManager
        registry.register_server(Box::new(workspace::WorkspaceServer::new(session_manager)));

        // Browser Agent server removed to prevent duplicate tools.
        // Browser functionality now provided by frontend BrowserToolProvider.

        registry
    }

    pub fn register_server(&mut self, server: Box<dyn BuiltinMCPServer>) {
        let name = server.name().to_string();
        self.servers.insert(name, server);
    }

    pub fn get_server(&self, name: &str) -> Option<&dyn BuiltinMCPServer> {
        self.servers.get(name).map(|s| s.as_ref())
    }

    pub fn list_servers(&self) -> Vec<String> {
        self.servers.keys().cloned().collect()
    }

    pub fn list_all_tools(&self) -> Vec<MCPTool> {
        let mut all_tools = Vec::new();

        for server in self.servers.values() {
            let tools = server.tools();
            // Prefix tool names with server name for uniqueness
            all_tools.extend(tools);
        }

        all_tools
    }

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

    pub async fn call_tool(&self, server_name: &str, tool_name: &str, args: Value) -> MCPResponse {
        if let Some(server) = self.get_server(server_name) {
            // Apply JSON normalization before calling the tool
            let normalized_args = Self::normalize_json_args(args);
            server.call_tool(tool_name, normalized_args).await
        } else {
            MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(Value::String(uuid::Uuid::new_v4().to_string())),
                result: None,
                error: Some(crate::mcp::MCPError {
                    code: -32601,
                    message: format!("Built-in server '{server_name}' not found"),
                    data: None,
                }),
            }
        }
    }
}
