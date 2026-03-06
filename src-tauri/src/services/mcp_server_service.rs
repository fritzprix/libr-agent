use crate::mcp::builtin::service_id::BuiltinServiceId;
use crate::mcp::{MCPServerManager, MCPTool};
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::state::get_mcp_server_repository;
use serde_json::Value;

pub struct McpServerService;

impl McpServerService {
    /// Connects to the server defined by `config`, lists its tools, and disconnects.
    /// Returns the list of tools if successful.
    pub async fn verify_config(
        config: crate::mcp::types::MCPServerConfig,
    ) -> Result<Vec<MCPTool>, String> {
        let server_name = config
            .name
            .clone()
            .unwrap_or_else(|| "unnamed_server".to_string());

        // Create a throw-away MCPServerManager (no builtins needed)
        let probe_manager = MCPServerManager {
            connections: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            builtin_servers: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            oauth_manager: std::sync::Arc::new(crate::mcp::oauth::OAuthManager::new()),
        };

        // Connect — this blocks until the MCP handshake completes
        probe_manager
            .start_server(config)
            .await
            .map_err(|e| format!("Failed to connect to '{}': {}", server_name, e))?;

        // List tools
        let tools_result = probe_manager.list_tools(&server_name).await;

        // Disconnect — explicitly stop the MCP server to ensure subprocess cleanup
        if let Err(e) = probe_manager.stop_server(&server_name).await {
            log::warn!(
                "[probe] Failed to stop MCP server '{}' cleanly: {}",
                server_name,
                e
            );
        }

        // Return tools or error
        tools_result.map_err(|e| format!("Failed to list tools from '{}': {}", server_name, e))
    }

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

        // 3. Verify config
        let tools = Self::verify_config(config).await?;

        log::info!(
            "[probe] '{}' ({}) → {} tool(s)",
            server_name,
            server_id,
            tools.len()
        );

        // 4. Persist tool list (names + descriptions) to DB (best-effort)
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

    pub async fn create_server_config(
        name: String,
        config: Value,
    ) -> Result<crate::entity::mcp_server::Model, String> {
        if BuiltinServiceId::from_alias(&name).is_some() {
            return Err(format!(
                "Server name '{}' is reserved for a builtin service.",
                name
            ));
        }

        // 1. Parse config into MCPServerConfig for verification
        let mut mcp_config: crate::mcp::types::MCPServerConfig =
            serde_json::from_value(config.clone())
                .map_err(|e| format!("Invalid MCP server configuration: {}", e))?;

        // Ensure name is set in the config
        mcp_config.name = Some(name.clone());

        // 2. Verify the configuration connects and provides tools before saving
        let tools = Self::verify_config(mcp_config).await?;
        let tools_json_str = crate::mcp::utils::serialize_mcp_tools(&tools);

        // 3. Save to database
        let repo = get_mcp_server_repository();
        let model = repo
            .create(&name, config)
            .await
            .map_err(|e| format!("Failed to create MCP server config: {}", e))?;

        // 4. Update the cached tools immediately since we just verified it
        repo.update_cached_tools(&model.id, tools.len() as i32, tools_json_str)
            .await
            .map_err(|e| format!("Failed to update cached tools: {}", e))?;

        // Reload the model to get the updated tool count
        let model = repo
            .get(&model.id)
            .await
            .map_err(|e| format!("Failed to reload MCP server config after creation: {}", e))?
            .unwrap_or(model);

        Ok(model)
    }

    pub async fn update_server_config(
        id: String,
        name: Option<String>,
        config: Option<Value>,
    ) -> Result<crate::entity::mcp_server::Model, String> {
        if let Some(ref n) = name {
            if BuiltinServiceId::from_alias(n).is_some() {
                return Err(format!(
                    "Server name '{}' is reserved for a builtin service.",
                    n
                ));
            }
        }

        let repo = get_mcp_server_repository();

        // 1. Get the current configuration and merge with updates
        let existing = repo
            .get(&id)
            .await
            .map_err(|e| format!("DB error: {}", e))?
            .ok_or_else(|| format!("MCP server '{}' not found", id))?;

        let final_name = name.clone().unwrap_or_else(|| existing.name.clone());
        let final_config_val = match config.as_ref() {
            Some(c) => c.clone(),
            None => serde_json::from_str(&existing.config)
                .map_err(|e| format!("Failed to parse existing config from DB: {}", e))?,
        };

        // 2. Parse config into MCPServerConfig for verification
        let mut mcp_config: crate::mcp::types::MCPServerConfig =
            serde_json::from_value(final_config_val.clone())
                .map_err(|e| format!("Invalid MCP server configuration: {}", e))?;

        // Ensure name is set in the config
        mcp_config.name = Some(final_name.clone());

        // 3. Verify the configuration connects and provides tools before saving
        let tools = Self::verify_config(mcp_config).await?;
        let tools_json_str = crate::mcp::utils::serialize_mcp_tools(&tools);

        // 4. Save to database
        let updated = repo
            .update(&id, name.as_deref(), config)
            .await
            .map_err(|e| format!("Failed to update MCP server config: {}", e))?;

        // 5. Update the cached tools immediately since we just verified it
        repo.update_cached_tools(&updated.id, tools.len() as i32, tools_json_str)
            .await
            .map_err(|e| format!("Failed to update cached tools: {}", e))?;

        // Reload the model to get the updated tool count
        let updated = repo
            .get(&updated.id)
            .await
            .map_err(|e| format!("Failed to reload MCP server config after update: {}", e))?
            .unwrap_or(updated);

        Ok(updated)
    }

    pub async fn delete_server_config(id: &str) -> Result<(), String> {
        let repo = get_mcp_server_repository();
        repo.delete(id)
            .await
            .map_err(|e| format!("Failed to delete MCP server config: {}", e))?;
        Ok(())
    }

    pub async fn list_server_configs() -> Result<Vec<crate::entity::mcp_server::Model>, String> {
        let repo = get_mcp_server_repository();
        repo.list()
            .await
            .map_err(|e| format!("Failed to list MCP server configs: {}", e))
    }
}
