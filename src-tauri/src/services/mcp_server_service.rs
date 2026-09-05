use crate::mcp::builtin::service_id::BuiltinServiceId;
use crate::mcp::{MCPServerManager, MCPTool};
use crate::repositories::mcp_server_repository::MCPServerRepository;
use serde_json::Value;
use std::time::Duration;

pub struct McpServerService;

/// Probe/install verification can include cold `npx`/`uvx` downloads; keep a
/// floor above the session startup default (30s) so first installs do not fail
/// prematurely while the UI shows pending verification.
fn verify_config_timeout() -> Duration {
    Duration::from_secs(std::cmp::max(
        crate::config::mcp_startup_timeout_seconds(),
        120,
    ))
}

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

        let timeout = verify_config_timeout();
        // Connect — bounded so hung handshakes / slow downloads fail visibly
        match tokio::time::timeout(timeout, probe_manager.start_server(config)).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                return Err(format!("Failed to connect to '{}': {}", server_name, e));
            }
            Err(_) => {
                return Err(format!(
                    "Timed out connecting to '{}' after {}s",
                    server_name,
                    timeout.as_secs()
                ));
            }
        }

        // List tools (also bounded — handshake success does not guarantee a timely tools/list)
        let tools_result =
            match tokio::time::timeout(timeout, probe_manager.list_tools(&server_name)).await {
                Ok(result) => result,
                Err(_) => {
                    if let Err(e) = probe_manager.stop_server(&server_name).await {
                        log::warn!(
                            "[probe] Failed to stop MCP server '{}' after list_tools timeout: {}",
                            server_name,
                            e
                        );
                    }
                    return Err(format!(
                        "Timed out listing tools from '{}' after {}s",
                        server_name,
                        timeout.as_secs()
                    ));
                }
            };

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

    /// Run connectivity verification in the background and notify the UI when done.
    ///
    /// Used after save-first create/update so Install does not block on cold package downloads.
    pub fn schedule_background_probe(server_id: String) {
        tokio::spawn(async move {
            let repo = crate::state::get_mcp_server_repository();
            let name_for_event = repo
                .get(&server_id)
                .await
                .ok()
                .flatten()
                .map(|m| m.name)
                .unwrap_or_else(|| server_id.clone());

            match Self::probe_server(repo, &server_id).await {
                Ok(tools) => {
                    log::info!(
                        "[probe] background verify '{}' ({}) → {} tool(s)",
                        name_for_event,
                        server_id,
                        tools.len()
                    );
                }
                Err(err) => {
                    log::warn!(
                        "[probe] background verify '{}' ({}) failed: {}",
                        name_for_event,
                        server_id,
                        err
                    );
                }
            }

            crate::agent::tauri_events::emit_resource_updated(
                "mcpServer",
                "verify",
                Some(name_for_event),
            );
        });
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
        let verify_result = Self::verify_config(config).await;

        match verify_result {
            Ok(tools) => {
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
            Err(err) => {
                // Set verification status to error so it doesn't get stuck in pending
                if let Err(e) = repo.set_verification_error(server_id, err.clone()).await {
                    log::warn!(
                        "[probe] Failed to update verification error status for '{}': {}",
                        server_id,
                        e
                    );
                }
                Err(err)
            }
        }
    }

    /// Reloads the server row from the database after a write. Does not re-verify connectivity.
    async fn fetch_server_model(
        repo: &dyn MCPServerRepository,
        server_id: &str,
    ) -> Result<crate::entity::mcp_server::Model, String> {
        repo.get(server_id)
            .await
            .map_err(|e| format!("DB error: {}", e))?
            .ok_or_else(|| format!("MCP server '{}' not found", server_id))
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

        // Validate config shape before persisting (connectivity is verified asynchronously).
        let mut mcp_config: crate::mcp::types::MCPServerConfig =
            serde_json::from_value(config.clone())
                .map_err(|e| format!("Invalid MCP server configuration: {}", e))?;
        mcp_config.name = Some(name.clone());
        let _ = mcp_config;

        // Save-first: Install UI returns immediately with verification_status=pending.
        // Callers should `schedule_background_probe` (commands / tool path do this).
        let model = repo
            .create(&name, config)
            .await
            .map_err(|e| format!("Failed to create MCP server config: {}", e))?;

        log::info!(
            "'{}' ({}) saved with pending verification (background probe expected)",
            model.name,
            model.id
        );

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

        let existing_config_val: Value = serde_json::from_str(&existing.config)
            .map_err(|e| format!("Failed to parse existing config from DB: {}", e))?;
        let final_config_val = match config.as_ref() {
            Some(c) => c.clone(),
            None => existing_config_val.clone(),
        };

        // Validate config shape when provided
        if config.is_some() {
            let mut mcp_config: crate::mcp::types::MCPServerConfig =
                serde_json::from_value(final_config_val.clone())
                    .map_err(|e| format!("Invalid MCP server configuration: {}", e))?;
            let final_name = name.clone().unwrap_or_else(|| existing.name.clone());
            mcp_config.name = Some(final_name);
            let _ = mcp_config;
        }

        let requires_reverification =
            Self::requires_reverification(&existing_config_val, &final_config_val);

        let updated = repo
            .update(&id, name.as_deref(), config)
            .await
            .map_err(|e| format!("Failed to update MCP server config: {}", e))?;

        if requires_reverification {
            repo.mark_verification_pending(&updated.id, true)
                .await
                .map_err(|e| format!("Failed to mark verification pending: {}", e))?;

            let updated = Self::fetch_server_model(repo, &updated.id).await?;
            log::info!(
                "'{}' ({}) updated; verification marked pending (background probe expected)",
                updated.name,
                updated.id
            );
            return Ok(updated);
        }

        // Name-only or metadata-only updates do not require transport re-verification.
        Ok(updated)
    }

    pub async fn delete_server_config(
        repo: &dyn MCPServerRepository,
        id: &str,
    ) -> Result<(), String> {
        // Cleanup token from keychain if present
        if let Err(e) = crate::mcp::keychain::delete_token(id).await {
            log::warn!(
                "Failed to delete OAuth token from keychain for server '{}': {}",
                id,
                e
            );
        }

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
