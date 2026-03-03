use crate::mcp::{MCPServerManager, MCPTool};

pub struct McpServerService;

impl McpServerService {
    /// Probe a single MCP server by ID: connect, list tools, disconnect.
    pub async fn probe_server(server_id: &str) -> Result<Vec<MCPTool>, String> {
        use crate::repositories::mcp_server_repository::MCPServerRepository;

        // 1. Load server record from DB
        let repo = crate::state::get_mcp_server_repository();
        let model = repo
            .get(server_id)
            .await
            .map_err(|e| format!("DB error looking up server '{}': {}", server_id, e))?
            .ok_or_else(|| format!("MCP server '{}' not found", server_id))?;

        // 2. Parse config JSON stored in DB
        let mut config = serde_json::from_str::<crate::mcp::types::MCPServerConfig>(&model.config)
            .map_err(|e| format!("Failed to parse config for '{}': {}", model.name, e))?;

        // Populate name from DB row if absent in JSON
        let server_name = config.name.unwrap_or_else(|| model.name.clone());
        config.name = Some(server_name.clone());

        // 3. Create a throw-away MCPServerManager (no builtins needed)
        let probe_manager = MCPServerManager {
            connections: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            builtin_servers: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            oauth_manager: std::sync::Arc::new(crate::mcp::oauth::OAuthManager::new()),
        };

        // 4. Connect — this blocks until the MCP handshake completes
        probe_manager
            .start_server(config)
            .await
            .map_err(|e| format!("Failed to connect to '{}': {}", server_name, e))?;

        // 5. List tools
        let tools_result = probe_manager.list_tools(&server_name).await;

        // 7. Disconnect — explicitly stop the MCP server to ensure subprocess cleanup
        // We do this here (before early return) to guarantee cleanup even if tool listing fails
        if let Err(e) = probe_manager.stop_server(&server_name).await {
            log::warn!(
                "[probe] Failed to stop MCP server '{}' cleanly: {}",
                server_name,
                e
            );
        }

        // Now process the tool listing result
        let tools = tools_result
            .map_err(|e| format!("Failed to list tools from '{}': {}", server_name, e))?;

        log::info!(
            "[probe] '{}' ({}) → {} tool(s)",
            server_name,
            server_id,
            tools.len()
        );

        // 6. Persist tool list (names + descriptions) to DB (best-effort)
        let tools_json_str = crate::mcp::utils::serialize_mcp_tools(&tools);

        if let Err(e) = repo
            .update_cached_tools(server_id, tools.len() as i32, tools_json_str)
            .await
        {
            log::warn!(
                "[probe] Failed to cache tool list for '{}': {}",
                server_id,
                e
            );
        }

        Ok(tools)
    }
}
