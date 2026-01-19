use crate::entity::mcp_server::Entity as McpServerEntity;
use crate::mcp::builtin::error_guidance::{missing_param_error, SuccessHint, ToolGroup};
use crate::mcp::types::{MCPResult, MCPServerConfig};
use crate::state::get_database_connection;
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
    // Cap page_size to prevent context overflow (Section 8.2 - Context Economy)
    let page_size = args
        .get("pageSize")
        .and_then(|v| v.as_i64())
        .unwrap_or(20)
        .min(50) as usize;

    let configs = list_all_configs().await?;

    // Note: Session Isolation means we cannot query global connection state
    // All servers are shown as "disconnected" in this view because connections are per-session
    let mut servers = Vec::new();
    for config in configs {
        let server_name = config
            .name
            .as_ref()
            .ok_or_else(|| "Server config missing name".to_string())?;

        let status = "configured"; // Changed from "connected"/"disconnected" to "configured"

        servers.push(json!({
            "name": server_name,
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
        let end = (start + page_size).min(total);
        servers[start..end].to_vec()
    };

    // Generate human-readable list with transport details (Section 4.2 - Narrative Requirement)
    let servers_text = servers_slice
        .iter()
        .map(|s| {
            let transport_type = s["transport"]["type"].as_str().unwrap_or("?");
            let detail = match transport_type {
                "stdio" => {
                    let cmd = s["transport"]["command"].as_str().unwrap_or("unknown");
                    format!(" | Command: {}", cmd)
                }
                "http" => {
                    let url = s["transport"]["url"].as_str().unwrap_or("unknown");
                    format!(" | URL: {}", url)
                }
                _ => String::new(),
            };
            format!(
                "• {} [Status: {}] (Type: {}{})",
                s["name"].as_str().unwrap_or("?"),
                s["status"].as_str().unwrap_or("?"),
                transport_type,
                detail
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let total_pages = (total as f64 / page_size as f64).ceil() as u64;

    let hint = SuccessHint::new(
        format!(
            "Found {} servers (Page {}/{}):\n\n{}",
            total, page, total_pages, servers_text
        ),
        vec![
            "Use createServer to initiate a new server configuration".to_string(),
            "Use connectServer/disconnectServer to target connection status".to_string(),
        ],
    );

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "servers": servers_slice,
        "total": total,
        "page": page,
        "pageSize": page_size
    }))))
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
        .filter_map(|c| {
            c.name.as_ref().and_then(|name| {
                if name.to_lowercase().contains(&query) {
                    Some(json!({ "name": name, "transport": c.transport }))
                } else {
                    None
                }
            })
        })
        .collect();

    // Pagination logic (Context Economy)
    let page = args.get("page").and_then(|v| v.as_u64()).unwrap_or(1);
    let page_size = args
        .get("pageSize")
        .and_then(|v| v.as_i64())
        .unwrap_or(20)
        .min(50) as usize;

    let total = filtered.len();
    let start = ((page - 1) * page_size as u64) as usize;
    let sliced_results = if start >= total {
        Vec::new()
    } else {
        let end = (start + page_size).min(total);
        filtered[start..end].to_vec()
    };

    let servers_text = sliced_results
        .iter()
        .map(|s| {
            format!(
                "• {} ({})",
                s["name"].as_str().unwrap_or("?"),
                s["transport"]["type"].as_str().unwrap_or("?")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let total_pages = (total as f64 / page_size as f64).ceil() as u64;

    let hint = SuccessHint::new(
        format!(
            "Search complete. Found {} servers matching '{}' (Page {}/{}):\n\n{}",
            total, query, page, total_pages, servers_text
        ),
        if filtered.is_empty() {
            vec!["Use listServers to extract all servers".to_string()]
        } else {
            vec!["Use connectServer to target a server for connection".to_string()]
        },
    );

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "servers": sliced_results,
        "total": total,
        "page": page,
        "pageSize": page_size
    }))))
}

/// List all builtin tools from the registry
pub async fn list_builtin_tools(_args: Value) -> Result<MCPResult, String> {
    // Note: Session Isolation means builtin tools are instantiated per-session
    // This returns static schema information, not per-session instances

    let tools_text = "Builtin tools are managed per-session through MCPServiceProxy.\n\n\
                     To get actual tools available in your session, check the agent's session tools.".to_string();

    Ok(
        SuccessHint::new(tools_text, vec![]).to_mcp_result_with_data(Some(json!({
            "tools": [],
            "note": "Session-specific tools unavailable from global view"
        }))),
    )
}
