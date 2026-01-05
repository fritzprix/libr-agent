use async_trait::async_trait;
use sea_orm::*;
use serde_json::{json, Value};

use super::BuiltinMCPServer;
use crate::entity::{mcp_server, mcp_server::Entity as McpServerEntity};
use crate::mcp::builtin::error_guidance::{
    invalid_input_error, missing_param_error, not_found_error, operation_failed_error, ToolGroup,
};
use crate::mcp::types::{MCPResult, MCPServerConfig, ServiceContext, TransportConfig};
use crate::mcp::MCPTool;
use crate::state::{get_mcp_manager, get_sqlite_pool};

#[derive(Debug, Default, Clone)]
pub struct MCPManagerServer;

impl MCPManagerServer {
    pub fn new() -> Self {
        Self
    }

    fn get_db(&self) -> DatabaseConnection {
        let pool = get_sqlite_pool();
        SqlxSqliteConnector::from_sqlx_sqlite_pool(pool.clone())
    }

    async fn save_server_config(&self, config: &MCPServerConfig) -> Result<(), String> {
        let db = self.get_db();
        let now = chrono::Utc::now().timestamp_millis();
        let config_json = serde_json::to_string(config).map_err(|e| e.to_string())?;

        // Upsert using SeaORM
        let model = mcp_server::ActiveModel {
            name: Set(config.name.clone()),
            config: Set(config_json.clone()),
            created_at: Set(now),
            updated_at: Set(now),
        };

        // Try to insert, if conflict update
        match McpServerEntity::insert(model.clone()).exec(&db).await {
            Ok(_) => Ok(()),
            Err(DbErr::RecordNotInserted) | Err(DbErr::Exec(_)) => {
                // Try update instead
                let update_model = mcp_server::ActiveModel {
                    name: Set(config.name.clone()),
                    config: Set(config_json),
                    created_at: NotSet,
                    updated_at: Set(now),
                };
                McpServerEntity::update(update_model)
                    .exec(&db)
                    .await
                    .map_err(|e| format!("DB Update Error: {}", e))?;
                Ok(())
            }
            Err(e) => Err(format!("DB Save Error: {}", e)),
        }
    }

    async fn get_server_config(&self, name: &str) -> Result<Option<MCPServerConfig>, String> {
        let db = self.get_db();

        let model = McpServerEntity::find_by_id(name.to_string())
            .one(&db)
            .await
            .map_err(|e| format!("DB Fetch Error: {}", e))?;

        if let Some(model) = model {
            let config = serde_json::from_str(&model.config).map_err(|e| e.to_string())?;
            Ok(Some(config))
        } else {
            Ok(None)
        }
    }

    async fn list_all_configs(&self) -> Result<Vec<MCPServerConfig>, String> {
        let db = self.get_db();

        let models = McpServerEntity::find()
            .all(&db)
            .await
            .map_err(|e| format!("DB List Error: {}", e))?;

        let mut configs = Vec::new();
        for model in models {
            if let Ok(config) = serde_json::from_str::<MCPServerConfig>(&model.config) {
                configs.push(config);
            }
        }
        Ok(configs)
    }

    /// Delete server configuration (not currently used, kept for API completeness)
    #[allow(dead_code)]
    async fn delete_server_config(&self, name: &str) -> Result<(), String> {
        let db = self.get_db();

        McpServerEntity::delete_by_id(name.to_string())
            .exec(&db)
            .await
            .map_err(|e| format!("DB Delete Error: {}", e))?;

        Ok(())
    }

    async fn list_servers(&self, args: Value) -> Result<MCPResult, String> {
        let include_inactive = args
            .get("includeInactive")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let manager = get_mcp_manager();
        let connections = manager.connections.lock().await;

        // Get active servers
        let mut server_map: std::collections::HashMap<String, Value> =
            std::collections::HashMap::new();

        for (name, conn) in connections.iter() {
            server_map.insert(
                name.clone(),
                json!({
                    "name": name,
                    "status": "connected",
                    "config": conn.config,
                    "transport": conn.config.transport
                }),
            );
        }

        // Get persisted servers and merge
        if include_inactive {
            if let Ok(configs) = self.list_all_configs().await {
                for config in configs {
                    if !server_map.contains_key(&config.name) {
                        server_map.insert(
                            config.name.clone(),
                            json!({
                                "name": config.name,
                                "status": "disconnected",
                                "config": config,
                                "transport": config.transport
                            }),
                        );
                    }
                }
            }
        }

        let mut servers: Vec<Value> = server_map.into_values().collect();
        // Sort by name
        servers.sort_by(|a, b| {
            let name_a = a["name"].as_str().unwrap_or("");
            let name_b = b["name"].as_str().unwrap_or("");
            name_a.cmp(name_b)
        });

        let text_output = if servers.is_empty() {
            "No servers found".to_string()
        } else {
            let mut s = format!("MCP Servers List ({} total):\n", servers.len());
            for server in &servers {
                let name = server.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let status = server.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                let transport = server
                    .get("transport")
                    .and_then(|t| t.get("type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                s.push_str(&format!(
                    "- **{}** ({}) [Transport: {}]\n",
                    name, status, transport
                ));
            }
            s
        };

        Ok(MCPResult::success_with_data(
            &text_output,
            json!({
                "servers": servers,
                "total": servers.len(),
                "page": 1,
                "pageSize": servers.len()
            }),
        ))
    }

    async fn search_server(&self, args: Value) -> Result<MCPResult, String> {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.is_empty() => q.to_lowercase(),
            Some(_) => {
                return Ok(invalid_input_error(
                    "Query parameter cannot be empty",
                    ToolGroup::McpManager,
                ))
            }
            None => return Ok(missing_param_error("query", ToolGroup::McpManager)),
        };

        let manager = get_mcp_manager();
        let connections = manager.connections.lock().await;

        let mut results_map: std::collections::HashMap<String, Value> =
            std::collections::HashMap::new();

        // Search active connections
        for (name, conn) in connections.iter() {
            if name.to_lowercase().contains(&query) {
                results_map.insert(
                    name.clone(),
                    json!({
                        "name": name,
                        "status": "connected",
                        "config": conn.config,
                        "transport": conn.config.transport
                    }),
                );
            }
        }

        // Search persisted configs
        if let Ok(configs) = self.list_all_configs().await {
            for config in configs {
                if config.name.to_lowercase().contains(&query)
                    && !results_map.contains_key(&config.name)
                {
                    results_map.insert(
                        config.name.clone(),
                        json!({
                            "name": config.name,
                            "status": "disconnected",
                            "config": config,
                            "transport": config.transport
                        }),
                    );
                }
            }
        }

        let results: Vec<Value> = results_map.into_values().collect();

        let text_output = if results.is_empty() {
            format!("No servers found matching '{}'", query)
        } else {
            let mut s = format!("Found {} servers matching '{}':\n", results.len(), query);
            for server in &results {
                let name = server.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let status = server.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                s.push_str(&format!("- **{}** ({})\n", name, status));
            }
            s
        };

        Ok(MCPResult::success_with_data(
            &text_output,
            json!({
                "results": results,
                "count": results.len()
            }),
        ))
    }

    async fn create_server(&self, args: Value) -> Result<MCPResult, String> {
        let name = match args.get("name").and_then(|v| v.as_str()) {
            Some(n) if !n.is_empty() => n,
            Some(_) => {
                return Ok(invalid_input_error(
                    "Server name cannot be empty",
                    ToolGroup::McpManager,
                ))
            }
            None => return Ok(missing_param_error("name", ToolGroup::McpManager)),
        };

        let transport = match args.get("transport") {
            Some(t) => t,
            None => return Ok(missing_param_error("transport", ToolGroup::McpManager)),
        };

        let transport_config: TransportConfig = match serde_json::from_value(transport.clone()) {
            Ok(config) => config,
            Err(e) => {
                return Ok(invalid_input_error(
                    &format!("Invalid transport config: {}. Must include 'type' field (stdio or http) and appropriate parameters", e),
                    ToolGroup::McpManager,
                ))
            }
        };

        let config = MCPServerConfig {
            name: name.to_string(),
            transport: transport_config,
            authentication: None,
            metadata: None,
        };

        // Save config first
        if let Err(e) = self.save_server_config(&config).await {
            return Ok(operation_failed_error(
                "createServer",
                &format!("Failed to save server configuration: {}", e),
                vec![
                    "Verify database permissions".to_string(),
                    "Check if server name already exists".to_string(),
                    "Use listServers to see existing servers".to_string(),
                ],
                ToolGroup::McpManager,
            ));
        }

        let manager = get_mcp_manager();
        match manager.start_server(config).await {
            Ok(server_name) => Ok(MCPResult::success(&format!(
                "Server '{}' started successfully",
                server_name
            ))),
            Err(e) => Ok(operation_failed_error(
                "createServer",
                &format!("Failed to start server: {}", e),
                vec![
                    "Verify transport configuration is correct".to_string(),
                    "For stdio: check command path and arguments".to_string(),
                    "For http: verify URL is accessible".to_string(),
                    "Use listServers to see server status".to_string(),
                ],
                ToolGroup::McpManager,
            )),
        }
    }

    async fn connect_server(&self, args: Value) -> Result<MCPResult, String> {
        let server_name = match args
            .get("serverName")
            .or_else(|| args.get("serverId"))
            .and_then(|v| v.as_str())
        {
            Some(name) if !name.is_empty() => name,
            Some(_) => {
                return Ok(invalid_input_error(
                    "Server name cannot be empty",
                    ToolGroup::McpManager,
                ))
            }
            None => return Ok(missing_param_error("serverName", ToolGroup::McpManager)),
        };

        // Check if already connected
        let manager = get_mcp_manager();
        {
            let connections = manager.connections.lock().await;
            if connections.contains_key(server_name) {
                return Ok(MCPResult::success(&format!(
                    "Server '{}' is already connected",
                    server_name
                )));
            }
        }

        // Load config from DB
        let config = match self.get_server_config(server_name).await {
            Ok(Some(cfg)) => cfg,
            Ok(None) => {
                return Ok(not_found_error(
                    "server configuration",
                    server_name,
                    ToolGroup::McpManager,
                ))
            }
            Err(e) => {
                return Ok(operation_failed_error(
                    "connectServer",
                    &format!("Failed to load server configuration: {}", e),
                    vec![
                        "Verify database is accessible".to_string(),
                        "Use listServers to see available servers".to_string(),
                    ],
                    ToolGroup::McpManager,
                ))
            }
        };

        // Start server
        match manager.start_server(config).await {
            Ok(_) => Ok(MCPResult::success(&format!(
                "Server '{}' connected successfully",
                server_name
            ))),
            Err(e) => Ok(operation_failed_error(
                "connectServer",
                &format!("Failed to connect server: {}", e),
                vec![
                    "Verify server configuration is correct".to_string(),
                    "Check if the server process is available".to_string(),
                    "Use listServers to see server status".to_string(),
                ],
                ToolGroup::McpManager,
            )),
        }
    }

    async fn disconnect_server(&self, args: Value) -> Result<MCPResult, String> {
        let server_name = match args
            .get("serverName")
            .or_else(|| args.get("serverId"))
            .and_then(|v| v.as_str())
        {
            Some(name) if !name.is_empty() => name,
            Some(_) => {
                return Ok(invalid_input_error(
                    "Server name cannot be empty",
                    ToolGroup::McpManager,
                ))
            }
            None => return Ok(missing_param_error("serverName", ToolGroup::McpManager)),
        };

        let manager = get_mcp_manager();
        match manager.stop_server(server_name).await {
            Ok(_) => Ok(MCPResult::success(&format!(
                "Server '{}' disconnected",
                server_name
            ))),
            Err(e) => Ok(operation_failed_error(
                "disconnectServer",
                &format!("Failed to disconnect server: {}", e),
                vec![
                    "Verify server is currently connected".to_string(),
                    "Use listServers to see active servers".to_string(),
                ],
                ToolGroup::McpManager,
            )),
        }
    }
}

#[async_trait]
impl BuiltinMCPServer for MCPManagerServer {
    fn name(&self) -> &str {
        "mcp_manager"
    }

    fn description(&self) -> &str {
        "Manage MCP servers and connections"
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            MCPTool {
                name: "listServers".to_string(),
                title: Some("List Servers".to_string()),
                description: "List all registered MCP servers".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "page": { "type": "integer", "minimum": 1, "description": "Page number for pagination" },
                        "pageSize": { "type": "integer", "minimum": -1, "description": "Number of items per page" },
                        "filterByAssistant": { "type": "boolean", "description": "Filter servers by assistant capability" },
                        "includeInactive": { "type": "boolean", "description": "Include inactive/disconnected servers" }
                    }
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "searchServer".to_string(),
                title: Some("Search Server".to_string()),
                description: "Search for MCP servers by name".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" },
                        "searchMode": { "type": "string", "enum": ["simple", "bm25"], "description": "Search mode (simple or bm25)" },
                        "weights": { 
                            "type": "object", 
                            "properties": {
                                "name": { "type": "number" },
                                "description": { "type": "number" }
                            },
                            "description": "Search weights for fields"
                        }
                    },
                    "required": ["query"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "createServer".to_string(),
                title: Some("Create Server".to_string()),
                description: "Register and start a new MCP server".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "minLength": 1, "description": "Unique name for the server" },
                        "transport": {
                            "type": "object",
                            "description": "Transport configuration",
                            "properties": {
                                "type": { "type": "string", "enum": ["stdio", "http"], "description": "Transport type" },
                                "command": { "type": "string", "description": "Command to execute (stdio only)" },
                                "args": { "type": "array", "items": { "type": "string" }, "description": "Command arguments (stdio only)" },
                                "env": { "type": "object", "description": "Environment variables" },
                                "url": { "type": "string", "description": "Server URL (http only)" },
                                "headers": { "type": "object", "description": "HTTP headers" }
                            },
                            "required": ["type"]
                        }
                    },
                    "required": ["name", "transport"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "connectServer".to_string(),
                title: Some("Connect Server".to_string()),
                description: "Connect to an existing MCP server".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "serverName": { "type": "string", "description": "Name of the server to connect" },
                        "serverId": { "type": "string", "description": "ID of the server (optional)" },
                        "scope": { "type": "string", "description": "Connection scope (optional)" }
                    }
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "disconnectServer".to_string(),
                title: Some("Disconnect Server".to_string()),
                description: "Disconnect an MCP server".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "serverName": { "type": "string", "description": "Name of the server to disconnect" },
                        "serverId": { "type": "string", "description": "ID of the server (optional)" },
                        "scope": { "type": "string", "description": "Connection scope (optional)" }
                    }
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
        ]
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String> {
        match tool_name {
            "listServers" => self.list_servers(args).await,
            "searchServer" => self.search_server(args).await,
            "createServer" => self.create_server(args).await,
            "connectServer" => self.connect_server(args).await,
            "disconnectServer" => self.disconnect_server(args).await,
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        let manager = get_mcp_manager();
        let count = manager.connections.lock().await.len();

        ServiceContext {
            context_prompt: format!("MCP Manager: {} active servers", count),
            structured_state: Some(json!({
                "active_servers_count": count
            })),
        }
    }
}
