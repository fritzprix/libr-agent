use crate::entity::mcp_server::Entity as McpServerEntity;
use crate::mcp::builtin::error_guidance::{missing_param_error, ToolGroup};
use crate::mcp::types::{MCPResult, MCPServerConfig};
use crate::state::{get_database_connection, get_mcp_manager};
use sea_orm::*;
use serde_json::{json, Value};

/// Get a server configuration by name
pub async fn get_server_config(name: &str) -> Result<Option<MCPServerConfig>, String> {
    let db = get_database_connection();

    let model = McpServerEntity::find_by_id(name.to_string())
        .one(db)
        .await
        .map_err(|e| format!("DB Fetch Error: {}", e))?;

    if let Some(model) = model {
        let config = serde_json::from_str(&model.config).map_err(|e| e.to_string())?;
        Ok(Some(config))
    } else {
        Ok(None)
    }
}

/// List all server configurations
pub async fn list_all_configs() -> Result<Vec<MCPServerConfig>, String> {
    let db = get_database_connection();

    let models = McpServerEntity::find()
        .all(db)
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

/// List servers with pagination
pub async fn list_servers(args: Value) -> Result<MCPResult, String> {
    let page = args.get("page").and_then(|v| v.as_u64()).unwrap_or(1);
    let page_size = args.get("pageSize").and_then(|v| v.as_i64()).unwrap_or(20);

    let configs = list_all_configs().await?;
    let manager = get_mcp_manager();
    let connections = manager.connections.lock().await;

    let mut servers = Vec::new();
    for config in configs {
        let status = if connections.contains_key(&config.name) {
            "connected"
        } else {
            "disconnected"
        };

        servers.push(json!({
            "name": config.name,
            "transport": config.transport,
            "status": status
        }));
    }

    // Pagination
    let total = servers.len();
    let start = ((page - 1) * page_size as u64) as usize;
    let servers_slice = if start >= total {
        Vec::new()
    } else {
        let end = (start + page_size as usize).min(total);
        servers[start..end].to_vec()
    };

    Ok(MCPResult::success_with_data(
        "Servers listed successfully",
        json!({
            "servers": servers_slice,
            "total": total,
            "page": page,
            "pageSize": page_size
        }),
    ))
}

/// Search servers by name
pub async fn search_server(args: Value) -> Result<MCPResult, String> {
    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(q) => q.to_lowercase(),
        Option::None => return Ok(missing_param_error("query", ToolGroup::McpManager)),
    };

    let configs = list_all_configs().await?;
    let filtered: Vec<Value> = configs
        .into_iter()
        .filter(|c| c.name.to_lowercase().contains(&query))
        .map(|c| json!({ "name": c.name, "transport": c.transport }))
        .collect();

    Ok(MCPResult::success_with_data(
        "Search complete",
        json!({ "servers": filtered }),
    ))
}
