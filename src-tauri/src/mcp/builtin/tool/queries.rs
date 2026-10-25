use crate::entity::mcp_server;
use crate::mcp::types::MCPServerConfig;
use crate::mcp::utils::sanitize_mcp_server_name;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::state::get_mcp_server_repository;

/// Find a server row by ID, raw name, or the sanitized name it is stored under.
pub async fn find_server_model(id_or_name: &str) -> Result<Option<mcp_server::Model>, String> {
    let repo = get_mcp_server_repository();
    let fetch_error = |e| format!("DB Fetch Error: {}", e);

    if let Some(model) = repo.get(id_or_name).await.map_err(fetch_error)? {
        return Ok(Some(model));
    }
    if let Some(model) = repo.get_by_name(id_or_name).await.map_err(fetch_error)? {
        return Ok(Some(model));
    }
    let sanitized = sanitize_mcp_server_name(id_or_name);
    if sanitized == id_or_name {
        return Ok(None);
    }
    repo.get_by_name(&sanitized).await.map_err(fetch_error)
}

/// Get server ID and configuration by ID or name
pub async fn get_server_details(
    id_or_name: &str,
) -> Result<Option<(String, MCPServerConfig)>, String> {
    if let Some(model) = find_server_model(id_or_name).await? {
        let mut config: MCPServerConfig =
            serde_json::from_str(&model.config).map_err(|e| e.to_string())?;
        config.name = Some(model.name.clone());
        Ok(Some((model.id, config)))
    } else {
        Ok(None)
    }
}

/// Get a server configuration by name or ID
pub async fn get_server_config(id_or_name: &str) -> Result<Option<MCPServerConfig>, String> {
    get_server_details(id_or_name)
        .await
        .map(|opt| opt.map(|(_, config)| config))
}
