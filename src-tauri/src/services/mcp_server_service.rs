use crate::mcp::builtin::service_id::BuiltinServiceId;
use crate::mcp::{MCPServerManager, MCPTool};
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::repositories::settings_repository::SettingsRepository;
use serde_json::Value;

pub struct McpServerService;

pub(crate) fn summarize_tool_names(tools: &[MCPTool]) -> String {
    const MAX_NAMES: usize = 5;

    if tools.is_empty() {
        return "none".to_string();
    }

    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_str()).collect();
    let preview = names
        .iter()
        .take(MAX_NAMES)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");

    if names.len() > MAX_NAMES {
        format!("{} (+{} more)", preview, names.len() - MAX_NAMES)
    } else {
        preview
    }
}

impl McpServerService {
    fn requires_reverification(existing_config: &Value, incoming_config: &Value) -> bool {
        let existing_transport = existing_config.get("transport");
        let incoming_transport = incoming_config.get("transport");
        let existing_authentication = existing_config.get("authentication");
        let incoming_authentication = incoming_config.get("authentication");

        existing_transport != incoming_transport
            || existing_authentication != incoming_authentication
    }

    fn spawn_verification_task(server_id: String) {
        tauri::async_runtime::spawn(async move {
            let repo = crate::state::get_mcp_server_repository();

            let verification_result = Self::verify_server_by_id(repo, &server_id).await;
            if let Err(error) = verification_result {
                log::error!(
                    "[verify-bg] Verification task failed for MCP server '{}': {}",
                    server_id,
                    error
                );
            }

            crate::agent::tauri_events::emit_resource_updated(
                "mcpServer",
                "verify",
                Some(server_id),
            );
        });
    }

    async fn verify_server_by_id(
        repo: &dyn MCPServerRepository,
        server_id: &str,
    ) -> Result<(), String> {
        let model = repo
            .get(server_id)
            .await
            .map_err(|e| format!("DB error looking up server '{}': {}", server_id, e))?
            .ok_or_else(|| format!("MCP server '{}' not found", server_id))?;

        let mut config = serde_json::from_str::<crate::mcp::types::MCPServerConfig>(&model.config)
            .map_err(|e| format!("Failed to parse config for '{}': {}", model.name, e))?;
        config.name = Some(model.name.clone());

        match Self::verify_config(config).await {
            Ok(tools) => {
                let tools_json_str = crate::mcp::utils::serialize_mcp_tools(&tools);
                repo.update_cached_tools(server_id, tools.len() as i32, tools_json_str)
                    .await
                    .map_err(|e| format!("Failed to persist verification result: {}", e))?;
                log::info!(
                    "[verify-bg] '{}' ({}) verified successfully with {} tool(s): [{}]",
                    model.name,
                    server_id,
                    tools.len(),
                    summarize_tool_names(&tools)
                );
                Ok(())
            }
            Err(error) => {
                repo.set_verification_error(server_id, error.clone())
                    .await
                    .map_err(|e| format!("Failed to persist verification error: {}", e))?;
                log::warn!(
                    "[verify-bg] '{}' ({}) verification failed: {}",
                    model.name,
                    server_id,
                    error
                );
                Ok(())
            }
        }
    }

    /// Connects to the server defined by `config`, lists its tools, and disconnects.
    /// Returns the list of tools if successful.
    pub async fn verify_config(
        config: crate::mcp::types::MCPServerConfig,
    ) -> Result<Vec<MCPTool>, String> {
        let server_name = config
            .name
            .clone()
            .unwrap_or_else(|| "unnamed_server".to_string());

        let mut timeout_seconds = 30; // Default fallback to 30 seconds
        if let Ok(settings_repo) = std::panic::catch_unwind(crate::state::get_settings_repository) {
            if let Ok(Some(model)) = settings_repo.get("systemSettings").await {
                #[derive(serde::Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct SystemSettings {
                    mcp_server_verification_timeout_seconds: Option<u64>,
                }

                if let Ok(settings) = serde_json::from_str::<SystemSettings>(&model.value) {
                    if let Some(timeout) = settings.mcp_server_verification_timeout_seconds {
                        timeout_seconds = timeout;
                    }
                }
            }
        }

        // Create a throw-away MCPServerManager (no builtins needed)
        let probe_manager = MCPServerManager {
            connections: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            builtin_servers: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            oauth_manager: std::sync::Arc::new(crate::mcp::oauth::OAuthManager::new()),
        };

        let timeout_duration = std::time::Duration::from_secs(timeout_seconds);

        let probe_result = {
            let probe_manager_ref = &probe_manager;
            let server_name_ref = &server_name;
            let config_clone = config.clone();

            let probe_fut = async move {
                // Connect — this blocks until the MCP handshake completes
                probe_manager_ref
                    .start_server(config_clone)
                    .await
                    .map_err(|e| format!("Failed to connect to '{}': {}", server_name_ref, e))?;

                // List tools
                let tools_result = probe_manager_ref.list_tools(server_name_ref).await;

                // Disconnect — explicitly stop the MCP server to ensure subprocess cleanup
                if let Err(e) = probe_manager_ref.stop_server(server_name_ref).await {
                    log::warn!(
                        "[probe] Failed to stop MCP server '{}' cleanly: {}",
                        server_name_ref,
                        e
                    );
                }

                // Return tools or error
                tools_result
                    .map_err(|e| format!("Failed to list tools from '{}': {}", server_name_ref, e))
            };

            tokio::time::timeout(timeout_duration, probe_fut).await
        };

        match probe_result {
            Ok(result) => result,
            Err(_) => {
                // If it timed out, try to stop the server if it started.
                if let Err(e) = probe_manager.stop_server(&server_name).await {
                    log::warn!(
                        "[probe-timeout] Failed to stop MCP server '{}' during cleanup: {}",
                        server_name,
                        e
                    );
                }
                Err(format!(
                    "Server '{}' verification timed out after {} seconds",
                    server_name, timeout_seconds
                ))
            }
        }
    }

    /// Probe a single MCP server by ID: connect, list tools, disconnect.
    pub async fn probe_server(
        repo: &dyn MCPServerRepository,
        server_id: &str,
    ) -> Result<Vec<MCPTool>, String> {
        // 1. Load server record from DB
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
            "[probe] '{}' ({}) → {} tool(s): [{}]",
            server_name,
            server_id,
            tools.len(),
            summarize_tool_names(&tools)
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
        repo: &dyn MCPServerRepository,
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
        let _ = mcp_config;

        // 3. Save to database
        let model = repo
            .create(&name, config)
            .await
            .map_err(|e| format!("Failed to create MCP server config: {}", e))?;

        Self::spawn_verification_task(model.id.clone());

        Ok(model)
    }

    pub async fn update_server_config(
        repo: &dyn MCPServerRepository,
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

        // 1. Get the current configuration and merge with updates
        let existing = repo
            .get(&id)
            .await
            .map_err(|e| format!("DB error: {}", e))?
            .ok_or_else(|| format!("MCP server '{}' not found", id))?;

        let final_name = name.clone().unwrap_or_else(|| existing.name.clone());
        let existing_config_val: Value = serde_json::from_str(&existing.config)
            .map_err(|e| format!("Failed to parse existing config from DB: {}", e))?;
        let final_config_val = match config.as_ref() {
            Some(c) => c.clone(),
            None => existing_config_val.clone(),
        };

        // 2. Parse config into MCPServerConfig for verification
        let mut mcp_config: crate::mcp::types::MCPServerConfig =
            serde_json::from_value(final_config_val.clone())
                .map_err(|e| format!("Invalid MCP server configuration: {}", e))?;

        // Ensure name is set in the config
        mcp_config.name = Some(final_name.clone());

        let requires_reverification =
            Self::requires_reverification(&existing_config_val, &final_config_val);
        let _ = mcp_config;

        // 4. Save to database
        let updated = repo
            .update(&id, name.as_deref(), config)
            .await
            .map_err(|e| format!("Failed to update MCP server config: {}", e))?;

        if requires_reverification {
            repo.mark_verification_pending(&updated.id, true)
                .await
                .map_err(|e| format!("Failed to mark verification pending: {}", e))?;
            Self::spawn_verification_task(updated.id.clone());
            return repo
                .get(&updated.id)
                .await
                .map_err(|e| format!("Failed to reload MCP server config after update: {}", e))?
                .ok_or_else(|| format!("MCP server '{}' disappeared after update", updated.id));
        }

        Ok(updated)
    }

    pub async fn delete_server_config(
        repo: &dyn MCPServerRepository,
        id: &str,
    ) -> Result<(), String> {
        repo.delete(id)
            .await
            .map_err(|e| format!("Failed to delete MCP server config: {}", e))?;
        Ok(())
    }

    pub async fn list_server_configs(
        repo: &dyn MCPServerRepository,
    ) -> Result<Vec<crate::entity::mcp_server::Model>, String> {
        let models = repo
            .list()
            .await
            .map_err(|e| format!("Failed to list MCP server configs: {}", e))?;

        log::info!("Loaded {} MCP server configs from repository", models.len());

        if log::log_enabled!(log::Level::Debug) {
            for model in &models {
                let cached_tool_names = model
                    .cached_tools
                    .as_deref()
                    .and_then(|json| serde_json::from_str::<Vec<serde_json::Value>>(json).ok())
                    .map(|tools| {
                        let names: Vec<&str> = tools
                            .iter()
                            .filter_map(|tool| tool.get("name").and_then(|name| name.as_str()))
                            .collect();
                        if names.is_empty() {
                            return "none".to_string();
                        }
                        let preview = names.iter().take(5).copied().collect::<Vec<_>>().join(", ");

                        if names.len() > 5 {
                            format!("{} (+{} more)", preview, names.len() - 5)
                        } else {
                            preview
                        }
                    })
                    .unwrap_or_else(|| "none".to_string());

                log::debug!(
                    "[server-list] '{}' ({}) cached tool_count={:?}, cached_tools=[{}]",
                    model.name,
                    model.id,
                    model.tool_count,
                    cached_tool_names
                );
            }
        }

        Ok(models)
    }
}
