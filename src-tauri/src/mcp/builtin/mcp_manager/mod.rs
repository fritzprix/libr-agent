use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::Row;

use super::BuiltinMCPServer;
use crate::mcp::types::{MCPResult, MCPServerConfig, ServiceContext, TransportConfig};
use crate::mcp::MCPTool;
use crate::state::{get_mcp_manager, get_sqlite_pool};

#[derive(Debug, Default)]
pub struct MCPManagerServer;

impl MCPManagerServer {
    pub fn new() -> Self {
        Self
    }

    async fn ensure_tables(&self) -> Result<(), String> {
        let pool = get_sqlite_pool();
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS mcp_servers (
                name TEXT PRIMARY KEY,
                config JSON NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| format!("DB Init Error: {}", e))?;
        Ok(())
    }

    async fn save_server_config(&self, config: &MCPServerConfig) -> Result<(), String> {
        self.ensure_tables().await?;
        let pool = get_sqlite_pool();
        let now = chrono::Utc::now().timestamp_millis();
        let config_json = serde_json::to_string(config).map_err(|e| e.to_string())?;

        sqlx::query(
            r#"
            INSERT INTO mcp_servers (name, config, created_at, updated_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(name) DO UPDATE SET
                config = excluded.config,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&config.name)
        .bind(config_json)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| format!("DB Save Error: {}", e))?;
        Ok(())
    }

    async fn get_server_config(&self, name: &str) -> Result<Option<MCPServerConfig>, String> {
        self.ensure_tables().await?;
        let pool = get_sqlite_pool();

        let row = sqlx::query("SELECT config FROM mcp_servers WHERE name = ?")
            .bind(name)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("DB Fetch Error: {}", e))?;

        if let Some(row) = row {
            let config_str: String = row.get("config");
            let config = serde_json::from_str(&config_str).map_err(|e| e.to_string())?;
            Ok(Some(config))
        } else {
            Ok(None)
        }
    }

    async fn list_all_configs(&self) -> Result<Vec<MCPServerConfig>, String> {
        self.ensure_tables().await?;
        let pool = get_sqlite_pool();

        let rows = sqlx::query("SELECT config FROM mcp_servers")
            .fetch_all(pool)
            .await
            .map_err(|e| format!("DB List Error: {}", e))?;

        let mut configs = Vec::new();
        for row in rows {
            let config_str: String = row.get("config");
            if let Ok(config) = serde_json::from_str::<MCPServerConfig>(&config_str) {
                configs.push(config);
            }
        }
        Ok(configs)
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

        Ok(MCPResult::success_with_data(
            "Servers listed",
            json!({
                "servers": servers,
                "total": servers.len(),
                "page": 1,
                "pageSize": servers.len()
            }),
        ))
    }

    async fn search_server(&self, args: Value) -> Result<MCPResult, String> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'query' parameter")?
            .to_lowercase();

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

        Ok(MCPResult::success_with_data(
            &format!("Found {} servers", results.len()),
            json!({
                "results": results,
                "count": results.len()
            }),
        ))
    }

    async fn create_server(&self, args: Value) -> Result<MCPResult, String> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("Missing name")?;
        let transport = args.get("transport").ok_or("Missing transport config")?;

        let transport_config: TransportConfig = serde_json::from_value(transport.clone())
            .map_err(|e| format!("Invalid transport config: {}", e))?;

        let config = MCPServerConfig {
            name: name.to_string(),
            transport: transport_config,
            authentication: None,
            metadata: None,
        };

        // Save config first
        self.save_server_config(&config).await?;

        let manager = get_mcp_manager();
        match manager.start_server(config).await {
            Ok(server_name) => Ok(MCPResult::success(&format!(
                "Server '{}' started successfully",
                server_name
            ))),
            Err(e) => Err(format!("Failed to start server: {}", e)),
        }
    }

    async fn connect_server(&self, args: Value) -> Result<MCPResult, String> {
        let server_name = args
            .get("serverName")
            .or_else(|| args.get("serverId"))
            .and_then(|v| v.as_str())
            .ok_or("Missing serverName")?;

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
        let config = self
            .get_server_config(server_name)
            .await?
            .ok_or_else(|| format!("Server '{}' not found in configuration", server_name))?;

        // Start server
        match manager.start_server(config).await {
            Ok(_) => Ok(MCPResult::success(&format!(
                "Server '{}' connected successfully",
                server_name
            ))),
            Err(e) => Err(format!("Failed to connect server: {}", e)),
        }
    }

    async fn disconnect_server(&self, args: Value) -> Result<MCPResult, String> {
        let server_name = args
            .get("serverName")
            .or_else(|| args.get("serverId"))
            .and_then(|v| v.as_str())
            .ok_or("Missing serverName or serverId")?;

        let manager = get_mcp_manager();
        match manager.stop_server(server_name).await {
            Ok(_) => Ok(MCPResult::success(&format!(
                "Server '{}' disconnected",
                server_name
            ))),
            Err(e) => Err(format!("Failed to disconnect server: {}", e)),
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
                        "query": { "type": "string", "description": "Search query" }
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
