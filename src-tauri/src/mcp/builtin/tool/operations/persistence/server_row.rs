//! Finding, writing, and restoring the `mcp_servers` row of a saved server.

use super::secrets::{record_secret_names, store_secrets, store_values};
use crate::mcp::secret_templates::SecretStore;
use crate::mcp::types::MCPServerConfig;
use crate::mcp::utils::sanitize_mcp_server_name;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::services::{McpServerService, SaveVerification};

pub(super) struct WrittenServer {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) target_changed: bool,
    pub(super) storage_warning: Option<String>,
}

/// Write the row and the secrets. On failure a new row is removed, and an existing row
/// keeps its previous config and secret names.
pub(super) async fn write_server(
    repo: &dyn MCPServerRepository,
    store: &dyn SecretStore,
    existing: Option<crate::entity::mcp_server::Model>,
    config: &MCPServerConfig,
    verification: SaveVerification,
    entered: &[(String, String)],
    reused: &[String],
) -> Result<WrittenServer, String> {
    let has_secrets = !entered.is_empty() || !reused.is_empty();
    match existing {
        Some(existing) => {
            // Values before the row: a keychain failure must leave the row unchanged, and
            // the update below deletes the values the new config no longer uses.
            let storage_warning = store_values(store, &existing.id, entered).await?;

            // Keep legacy display names (including whitespace) intact.
            let mut config = config.clone();
            config.name = Some(existing.name.clone());
            let config_value = serde_json::to_value(&config).map_err(|e| e.to_string())?;

            let existing_config_val: serde_json::Value = serde_json::from_str(&existing.config)
                .map_err(|e| format!("Failed to parse existing config from DB: {}", e))?;
            let requires_reverification = existing_config_val.get("transport")
                != config_value.get("transport")
                || existing_config_val.get("authentication") != config_value.get("authentication");

            let updated = McpServerService::update_server_config_with_store(
                repo,
                store,
                existing.id.clone(),
                None,
                Some(config_value),
                verification,
            )
            .await?;

            if has_secrets {
                if let Err(error) = record_secret_names(repo, &updated.id, entered, reused).await {
                    restore_row(repo, &existing, existing_config_val).await;
                    return Err(error);
                }
            }
            Ok(WrittenServer {
                id: updated.id,
                name: existing.name,
                target_changed: requires_reverification,
                storage_warning,
            })
        }
        None => {
            let sanitized_name = sanitize_mcp_server_name(config.name.as_deref().unwrap_or(""));
            let mut config = config.clone();
            config.name = Some(sanitized_name.clone());
            let config_value = serde_json::to_value(&config).map_err(|e| e.to_string())?;
            let created = McpServerService::create_server_config(
                repo,
                sanitized_name,
                config_value,
                verification,
            )
            .await?;
            let mut storage_warning = None;
            if has_secrets {
                match store_secrets(repo, store, &created.id, entered, reused).await {
                    Ok(warning) => storage_warning = warning,
                    Err(error) => {
                        let _ = McpServerService::delete_server_config_with_store(
                            repo,
                            store,
                            &created.id,
                        )
                        .await;
                        return Err(error);
                    }
                }
            }
            Ok(WrittenServer {
                id: created.id,
                name: created.name,
                target_changed: true,
                storage_warning,
            })
        }
    }
}

/// Put back the config and secret names that a failed update replaced.
async fn restore_row(
    repo: &dyn MCPServerRepository,
    previous: &crate::entity::mcp_server::Model,
    previous_config: serde_json::Value,
) {
    if let Err(e) = repo.update(&previous.id, None, Some(previous_config)).await {
        log::warn!(
            "Failed to restore config of MCP server '{}': {}",
            previous.id,
            e
        );
    }
    if let Err(e) = repo
        .set_secret_names(&previous.id, previous.secret_names.clone())
        .await
    {
        log::warn!(
            "Failed to restore secret names of MCP server '{}': {}",
            previous.id,
            e
        );
    }
}

/// The stored row for `raw_name`. Prefers the sanitized name; falls back to the raw name
/// for legacy rows that still contain spaces.
pub(super) async fn find_existing(
    repo: &dyn MCPServerRepository,
    raw_name: &str,
) -> Result<Option<crate::entity::mcp_server::Model>, String> {
    let sanitized_name = sanitize_mcp_server_name(raw_name);
    let query_error = |e| format!("DB query error while saving server config: {}", e);
    match repo
        .get_by_name(&sanitized_name)
        .await
        .map_err(query_error)?
    {
        Some(model) => Ok(Some(model)),
        None if sanitized_name != raw_name => repo.get_by_name(raw_name).await.map_err(query_error),
        None => Ok(None),
    }
}
