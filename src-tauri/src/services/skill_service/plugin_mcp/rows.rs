use super::config::{plugin_source_marker, snapshot_to_mcp_config_json_in};
use super::PluginExpansion;
use crate::mcp::utils::sanitize_mcp_server_name;
use crate::services::hooks::registry::PluginMcpServerSnapshot;
#[cfg(test)]
use std::path::Path;

#[cfg(test)]
pub async fn create_plugin_mcp_server_rows(
    source_id: &str,
    plugin_root: &Path,
    servers: &[PluginMcpServerSnapshot],
) -> Result<Vec<String>, String> {
    create_plugin_mcp_server_rows_in(source_id, &PluginExpansion::root_only(plugin_root), servers)
        .await
}

/// As [`create_plugin_mcp_server_rows`], with the plugin's data directory and
/// `userConfig` values available to each entry (#2926).
pub async fn create_plugin_mcp_server_rows_in(
    source_id: &str,
    expansion: &PluginExpansion<'_>,
    servers: &[PluginMcpServerSnapshot],
) -> Result<Vec<String>, String> {
    use crate::services::{McpServerService, SaveVerification};
    let repo = crate::state::get_mcp_server_repository();

    let mut warnings = Vec::new();
    for snapshot in servers {
        let name = sanitize_mcp_server_name(&snapshot.name);
        let outcome: Result<String, String> = async {
            let config = snapshot_to_mcp_config_json_in(snapshot, expansion, source_id)?;
            let model = McpServerService::create_server_config(
                repo,
                name.clone(),
                config,
                SaveVerification::Pending,
            )
            .await?;
            Ok(model.id)
        }
        .await;
        match outcome {
            Ok(id) => McpServerService::schedule_background_probe(id),
            Err(error) => {
                log::warn!(
                    "Plugin {source_id}: could not install MCP server '{}' into the global store: {}",
                    snapshot.name,
                    error
                );
                warnings.push(format!(
                    "Plugin {source_id}: MCP server '{}' was approved but could not be installed into the global store: {error}",
                    snapshot.name
                ));
            }
        }
    }
    Ok(warnings)
}

/// Delete every global MCP server row owned by `source_id` (plugin uninstall).
pub async fn remove_global_plugin_mcp_servers(source_id: &str) -> Result<(), String> {
    use crate::repositories::mcp_server_repository::MCPServerRepository;
    use crate::services::mcp_server_service::McpServerService;
    let repo = crate::state::get_mcp_server_repository();
    let marker = plugin_source_marker(source_id);
    let models = repo
        .list()
        .await
        .map_err(|e| format!("list MCP servers: {e}"))?;
    for model in models {
        if crate::mcp::types::plugin_source_of(&model.config).as_deref() == Some(marker.as_str()) {
            McpServerService::delete_plugin_owned_server(&model.id).await?;
        }
    }
    Ok(())
}
