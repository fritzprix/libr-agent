use crate::mcp::builtin::service_id::BuiltinServiceId;
use crate::mcp::secret_templates::{
    fill_secrets, parse_secret_names, secret_names_column, secret_store, template_names,
    SecretStore,
};
use crate::mcp::session_isolation::exec_target::StdioExecTarget;
use crate::mcp::session_isolation::{SessionMCPError, SessionMCPManager};
use crate::mcp::session_isolation_config::SessionIsolationConfig;
use crate::mcp::types::TransportConfig;
use crate::mcp::utils::sanitize_mcp_server_name;
use crate::mcp::{MCPServerManager, MCPTool};
use crate::repositories::mcp_server_repository::MCPServerRepository;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

pub struct McpServerService;

/// Verification state a saved config starts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveVerification {
    /// A background probe sets `success` or `error`.
    Pending,
    /// The caller tests the server in a session container; the record keeps no status,
    /// error or cached tools until a check succeeds.
    Unverified,
}

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

    /// Verifies `config` where `target` runs stdio servers. A Docker session tests a stdio
    /// server in its container; everything else uses [`Self::verify_config`].
    ///
    /// `{{secret:NAME}}` templates are filled in from the keychain entries of `server_id`
    /// for this start only, and the values are masked in the error.
    pub async fn verify_config_at(
        config: crate::mcp::types::MCPServerConfig,
        target: &StdioExecTarget,
        server_id: &str,
    ) -> Result<Vec<MCPTool>, String> {
        let filled = fill_secrets(secret_store(), server_id, &config).await?;
        let result = match (target.container_session(), &filled.config.transport) {
            (Some(session), TransportConfig::Stdio { .. }) => {
                Self::verify_stdio_in_container(filled.config.clone(), session).await
            }
            _ => Self::verify_config(filled.config.clone()).await,
        };
        result.map_err(|error| filled.mask(&error))
    }

    /// Starts the server through a short-lived session manager, so the test uses the same
    /// container spawn (and missing-runtime message) as the tool calls of that session.
    /// The container is prepared first: an image pull does not count against the timeout.
    async fn verify_stdio_in_container(
        config: crate::mcp::types::MCPServerConfig,
        session: &crate::repositories::SessionMetadata,
    ) -> Result<Vec<MCPTool>, String> {
        let server_name = config
            .name
            .clone()
            .unwrap_or_else(|| "unnamed_server".to_string());
        crate::services::WorkspaceRuntimeManager::prepare_mcp_container(session)
            .await
            .map_err(|e| format!("Failed to prepare the session container: {}", e))?;
        let session_id = session.id.as_str();
        let (channel_event_tx, _channel_event_rx) =
            crate::mcp::session_isolation::channel_events::create_channel_event_bus();
        let manager = SessionMCPManager::new(
            session_id.to_string(),
            HashMap::from([(server_name.clone(), config)]),
            SessionIsolationConfig::default(),
            std::env::temp_dir(),
            channel_event_tx,
        );

        let timeout = verify_config_timeout();
        let result = tokio::time::timeout(
            timeout,
            manager.list_tools_with_init_timeout(&server_name, timeout),
        )
        .await;
        manager.shutdown_all().await;

        match result {
            Ok(Ok(tools)) => Ok(tools),
            Ok(Err(SessionMCPError::RuntimeMissing(message))) => Err(message),
            Ok(Err(error)) => Err(format!(
                "Failed to start '{}' in the session container: {}",
                server_name, error
            )),
            Err(_) => Err(format!(
                "Timed out starting '{}' in the session container after {}s",
                server_name,
                timeout.as_secs()
            )),
        }
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
        let verify_result = Self::verify_config_at(config, &StdioExecTarget::Host, server_id).await;

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
        mut config: Value,
        verification: SaveVerification,
    ) -> Result<crate::entity::mcp_server::Model, String> {
        let name = sanitize_mcp_server_name(&name);
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
        if let Some(obj) = config.as_object_mut() {
            obj.insert("name".to_string(), Value::String(name.clone()));
        }
        let _ = mcp_config;

        // Save-first: Install UI returns immediately with verification_status=pending.
        // Callers should `schedule_background_probe` (commands / tool path do this).
        let pending = verification == SaveVerification::Pending;
        let model = repo
            .create_with_verification(&name, config, pending)
            .await
            .map_err(|e| format!("Failed to create MCP server config: {}", e))?;

        log::info!(
            "'{}' ({}) saved ({:?} verification)",
            model.name,
            model.id,
            verification
        );

        Ok(model)
    }

    pub async fn update_server_config(
        repo: &dyn MCPServerRepository,
        id: String,
        name: Option<String>,
        config: Option<Value>,
        verification: SaveVerification,
    ) -> Result<crate::entity::mcp_server::Model, String> {
        Self::update_server_config_with_store(repo, secret_store(), id, name, config, verification)
            .await
    }

    /// [`Self::update_server_config`] with the secret store passed in.
    pub(crate) async fn update_server_config_with_store(
        repo: &dyn MCPServerRepository,
        store: &dyn SecretStore,
        id: String,
        name: Option<String>,
        config: Option<Value>,
        verification: SaveVerification,
    ) -> Result<crate::entity::mcp_server::Model, String> {
        // Preserve legacy display names (including whitespace). Session load sanitizes
        // tool prefixes at runtime; do not rewrite existing DB rows on update.
        let name = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
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

        // #2924 (D2): plugin-owned servers are read-only here. They change
        // only with the plugin that installed them (re-install / uninstall).
        if let Some(owner) = crate::mcp::types::plugin_source_of(&existing.config) {
            return Err(format!(
                "MCP server '{}' is owned by plugin '{}' and cannot be edited. Manage it through the plugin (uninstall the plugin to remove it).",
                existing.name, owner
            ));
        }

        let existing_config_val: Value = serde_json::from_str(&existing.config)
            .map_err(|e| format!("Failed to parse existing config from DB: {}", e))?;
        let final_config_val = match config.as_ref() {
            Some(c) => c.clone(),
            None => existing_config_val.clone(),
        };

        // Validate config shape when provided; secrets the new config no longer uses are
        // removed from the keychain (agent and UI edits alike).
        let mut unused_secrets = Vec::new();
        let stored_secrets = parse_secret_names(existing.secret_names.as_deref());
        if config.is_some() {
            let mcp_config: crate::mcp::types::MCPServerConfig =
                serde_json::from_value(final_config_val.clone())
                    .map_err(|e| format!("Invalid MCP server configuration: {}", e))?;
            let used = template_names(&mcp_config.transport)?;
            unused_secrets = stored_secrets.difference(&used).cloned().collect();
        }

        let requires_reverification =
            Self::requires_reverification(&existing_config_val, &final_config_val);

        let updated = repo
            .update(&id, name.as_deref(), config)
            .await
            .map_err(|e| format!("Failed to update MCP server config: {}", e))?;

        if !unused_secrets.is_empty() {
            for secret in &unused_secrets {
                if let Err(e) = store.delete(&id, secret).await {
                    log::warn!(
                        "Failed to delete secret '{}' of server '{}': {}",
                        secret,
                        id,
                        e
                    );
                }
            }
            let kept = stored_secrets
                .iter()
                .filter(|name| !unused_secrets.contains(name))
                .cloned()
                .collect();
            repo.set_secret_names(&id, secret_names_column(&kept))
                .await
                .map_err(|e| format!("Failed to update secret names: {}", e))?;
        }

        if requires_reverification {
            match verification {
                SaveVerification::Pending => {
                    repo.mark_verification_pending(&updated.id, true).await
                }
                SaveVerification::Unverified => repo.clear_verification(&updated.id).await,
            }
            .map_err(|e| format!("Failed to reset verification: {}", e))?;

            let updated = Self::fetch_server_model(repo, &updated.id).await?;
            log::info!(
                "'{}' ({}) updated ({:?} verification)",
                updated.name,
                updated.id,
                verification
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
        Self::delete_server_config_with_store(repo, secret_store(), id).await
    }

    /// Delete a plugin-owned MCP server row (plugin uninstall path only).
    /// Refuses rows that are not plugin-owned — normal removals keep going
    /// through [`Self::delete_server_config`], which rejects plugin-owned
    /// rows (#2924 D2).
    pub(crate) async fn delete_plugin_owned_server(id: &str) -> Result<(), String> {
        let repo = crate::state::get_mcp_server_repository();
        let model = repo
            .get(id)
            .await
            .map_err(|e| format!("DB error: {}", e))?
            .ok_or_else(|| format!("MCP server '{}' not found", id))?;
        if crate::mcp::types::plugin_source_of(&model.config).is_none() {
            return Err(format!(
                "MCP server '{}' is not plugin-owned; delete it through the normal flow",
                model.name
            ));
        }
        Self::delete_server_config_inner(repo, secret_store(), id).await
    }

    /// [`Self::delete_server_config`] with the secret store passed in. Deleting the row
    /// also drops its `encrypted_secrets`.
    pub(crate) async fn delete_server_config_with_store(
        repo: &dyn MCPServerRepository,
        store: &dyn SecretStore,
        id: &str,
    ) -> Result<(), String> {
        // #2924 (D2): plugin-owned servers are removed only with their plugin.
        if let Ok(Some(model)) = repo.get(id).await {
            if let Some(owner) = crate::mcp::types::plugin_source_of(&model.config) {
                return Err(format!(
                    "MCP server '{}' is owned by plugin '{}' and cannot be deleted. Uninstall the plugin to remove it.",
                    model.name, owner
                ));
            }
        }
        Self::delete_server_config_inner(repo, store, id).await
    }

    async fn delete_server_config_inner(
        repo: &dyn MCPServerRepository,
        store: &dyn SecretStore,
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

        if let Ok(Some(model)) = repo.get(id).await {
            for secret in parse_secret_names(model.secret_names.as_deref()) {
                if let Err(e) = store.delete(id, &secret).await {
                    log::warn!(
                        "Failed to delete secret '{}' of server '{}': {}",
                        secret,
                        id,
                        e
                    );
                }
            }
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

#[cfg(all(test, unix))]
mod verify_tests {
    use super::*;

    #[tokio::test]
    async fn verify_starts_the_server_with_the_value_not_the_template() {
        let dir = tempfile::tempdir().unwrap();
        let seen = dir.path().join("token");
        let id = "verify-fill-test";
        secret_store()
            .set(id, "TOKEN", "the-secret-value")
            .await
            .unwrap();
        let config: crate::mcp::types::MCPServerConfig =
            serde_json::from_value(serde_json::json!({
                "name": "probe",
                "transport": {
                    "type": "stdio",
                    "command": "sh",
                    "args": ["-c", format!("printf %s \"$TOKEN\" > '{}'", seen.display())],
                    "env": { "TOKEN": "{{secret:TOKEN}}" }
                }
            }))
            .unwrap();

        let error = McpServerService::verify_config_at(config, &StdioExecTarget::Host, id)
            .await
            .expect_err("sh is not an MCP server");

        assert_eq!(std::fs::read_to_string(&seen).unwrap(), "the-secret-value");
        assert!(!error.contains("the-secret-value"), "{error}");
    }
}

// #2924 (D2, gatekeeper G1.4): plugin-owned MCP server rows are read-only —
// they change only through the plugin lifecycle (install/update/uninstall).
#[cfg(test)]
mod plugin_guard_tests {
    use super::*;
    use crate::repositories::mcp_server_repository::SqliteMCPServerRepository;

    async fn memory_db() -> sea_orm::DatabaseConnection {
        use sea_orm_migration::MigratorTrait;
        crate::lifecycle::database::register_sqlite_vec();
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("memory db");
        crate::migration::Migrator::up(&db, None)
            .await
            .expect("migrations");
        db
    }

    fn plugin_config(name: &str, owner: &str) -> serde_json::Value {
        serde_json::json!({
            "name": name,
            "transport": { "type": "http", "url": format!("https://{name}.example/mcp") },
            "pluginSource": format!("plugin:{owner}")
        })
    }

    fn plain_config(name: &str) -> serde_json::Value {
        serde_json::json!({
            "name": name,
            "transport": { "type": "http", "url": format!("https://{name}.example/mcp") }
        })
    }

    #[tokio::test]
    async fn update_refuses_plugin_owned_servers_only() {
        let db = memory_db().await;
        let repo = SqliteMCPServerRepository::new(db);
        let owned = repo
            .create("owned", plugin_config("owned", "plugin-ee00000000000001"))
            .await
            .unwrap();
        let error = McpServerService::update_server_config(
            &repo,
            owned.id.clone(),
            None,
            Some(plugin_config("owned", "plugin-ee00000000000001")),
            SaveVerification::Pending,
        )
        .await
        .expect_err("plugin-owned rows must not be editable");
        assert!(error.contains("owned by plugin"), "{error}");
        assert!(error.contains("plugin-ee00000000000001"), "{error}");

        // Non-plugin rows keep working through the normal edit path.
        let normal = repo.create("normal", plain_config("normal")).await.unwrap();
        let updated = McpServerService::update_server_config(
            &repo,
            normal.id.clone(),
            Some("renamed".to_string()),
            None,
            SaveVerification::Pending,
        )
        .await
        .expect("name-only update of a normal row");
        assert_eq!(updated.name, "renamed");
    }

    #[tokio::test]
    async fn normal_delete_refuses_plugin_owned_but_uninstall_path_removes_it() {
        let _guard = crate::state::lock_test_global_state().await;
        let db = memory_db().await;
        let repo = SqliteMCPServerRepository::new(db.clone());
        crate::state::set_mcp_server_repository(SqliteMCPServerRepository::new(db));

        let owned = repo
            .create(
                "owned-del",
                plugin_config("owned-del", "plugin-ee00000000000002"),
            )
            .await
            .unwrap();
        let error = McpServerService::delete_server_config_with_store(
            &repo,
            &crate::mcp::secret_templates::MemorySecretStore::default(),
            &owned.id,
        )
        .await
        .expect_err("plugin-owned rows must not be deletable from the UI path");
        assert!(error.contains("cannot be deleted"), "{error}");
        assert!(error.contains("Uninstall the plugin"), "{error}");

        // The plugin-uninstall path removes the row it owns.
        McpServerService::delete_plugin_owned_server(&owned.id)
            .await
            .expect("uninstall path may delete owned rows");
        assert!(repo.get(&owned.id).await.unwrap().is_none());

        // …and refuses rows it does not own.
        let normal = repo.create("plain", plain_config("plain")).await.unwrap();
        let error = McpServerService::delete_plugin_owned_server(&normal.id)
            .await
            .expect_err("only plugin-owned rows go through this path");
        assert!(error.contains("not plugin-owned"), "{error}");
    }
}
