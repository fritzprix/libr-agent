use crate::entity::{mcp_server, mcp_server::Entity as McpServerEntity};
use crate::mcp::builtin::error_guidance::{
    duplicate_error, invalid_input_error, missing_param_error, not_found_error,
    operation_failed_error, SuccessHint, ToolGroup,
};
use crate::mcp::types::{MCPResult, MCPServerConfig, TransportConfig};
use crate::state::{get_database_connection, get_mcp_manager};
use sea_orm::*;
use serde_json::Value;

use super::queries::get_server_config;

async fn save_server_config(config: &MCPServerConfig) -> Result<(), String> {
    let db = get_database_connection();
    let now = chrono::Utc::now().timestamp_millis();
    let config_json = serde_json::to_string(config).map_err(|e| e.to_string())?;

    // Upsert using SeaORM
    let model = mcp_server::ActiveModel {
        name: Set(config.name.clone()),
        config: Set(config_json.clone()),
        created_at: Set(now),
        updated_at: Set(now),
    };

    // Try to insert, if conflict update
    match McpServerEntity::insert(model.clone()).exec(db).await {
        Ok(_) => Ok(()),
        Err(DbErr::RecordNotInserted) | Err(DbErr::Exec(_)) => {
            // Try update instead
            let update_model = mcp_server::ActiveModel {
                name: Set(config.name.clone()),
                config: Set(config_json),
                created_at: NotSet,
                updated_at: Set(now),
            };
            McpServerEntity::update(update_model)
                .exec(db)
                .await
                .map_err(|e| format!("DB Update Error: {}", e))?;
            Ok(())
        }
        Err(e) => Err(format!("DB Save Error: {}", e)),
    }
}

async fn delete_server_config_db(name: &str) -> Result<(), String> {
    let db = get_database_connection();

    McpServerEntity::delete_by_id(name.to_string())
        .exec(db)
        .await
        .map_err(|e| format!("DB Delete Error: {}", e))?;

    Ok(())
}

/// Create and start a new MCP server
pub async fn create_server(args: Value) -> Result<MCPResult, String> {
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) if !n.is_empty() => n,
        Some(_) => {
            return Ok(invalid_input_error(
                "Server name cannot be empty",
                ToolGroup::McpManager,
            ))
        }
        Option::None => return Ok(missing_param_error("name", ToolGroup::McpManager)),
    };

    if let Ok(Some(_)) = get_server_config(name).await {
        return Ok(duplicate_error("Server", name, ToolGroup::McpManager));
    }

    let transport_val = match args.get("transport") {
        Some(t) => t,
        Option::None => return Ok(missing_param_error("transport", ToolGroup::McpManager)),
    };

    let transport: TransportConfig = serde_json::from_value(transport_val.clone())
        .map_err(|e| format!("Invalid transport config: {}", e))?;

    let config = MCPServerConfig {
        name: name.to_string(),
        transport,
        authentication: None,
        metadata: None,
    };

    if let Err(e) = save_server_config(&config).await {
        return Ok(operation_failed_error(
            "save_server_config",
            &e,
            vec!["Check database connectivity".to_string()],
            ToolGroup::McpManager,
        ));
    }

    // Auto-start
    let manager = get_mcp_manager();
    if let Err(e) = manager.start_server(config).await {
        return Ok(operation_failed_error(
            "start_server",
            &format!("Server created but failed to start: {}", e),
            vec!["Check server command or URL".to_string()],
            ToolGroup::McpManager,
        ));
    }

    let hint = SuccessHint::new(
        format!("Server '{}' created and started successfully", name),
        vec!["Use listServers to check status".to_string()],
    );
    Ok(hint.to_mcp_result())
}

/// Delete an MCP server
pub async fn delete_server(args: Value) -> Result<MCPResult, String> {
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) if !n.is_empty() => n,
        Some(_) => {
            return Ok(invalid_input_error(
                "Server name cannot be empty",
                ToolGroup::McpManager,
            ))
        }
        Option::None => return Ok(missing_param_error("name", ToolGroup::McpManager)),
    };

    // Stop server first if running
    let manager = get_mcp_manager();
    let _ = manager.stop_server(name).await;

    // Delete config
    if let Err(e) = delete_server_config_db(name).await {
        return Ok(operation_failed_error(
            "deleteServer",
            &format!("Failed to delete server configuration: {}", e),
            vec![
                "Verify database permissions".to_string(),
                "Check if server name exists".to_string(),
            ],
            ToolGroup::McpManager,
        ));
    }

    let hint = SuccessHint::new(
        format!("Server '{}' deleted successfully", name),
        vec![
            "Use listServers to see remaining servers".to_string(),
            "Use createServer to add a new server".to_string(),
        ],
    );
    Ok(hint.to_mcp_result())
}

/// Update an existing MCP server configuration
pub async fn update_server(args: Value) -> Result<MCPResult, String> {
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) if !n.is_empty() => n,
        Some(_) => {
            return Ok(invalid_input_error(
                "Server name cannot be empty",
                ToolGroup::McpManager,
            ))
        }
        Option::None => return Ok(missing_param_error("name", ToolGroup::McpManager)),
    };

    let transport = match args.get("transport") {
        Some(t) => t,
        Option::None => return Ok(missing_param_error("transport", ToolGroup::McpManager)),
    };

    let transport_config: TransportConfig = match serde_json::from_value(transport.clone()) {
        Ok(config) => config,
        Err(e) => {
            return Ok(invalid_input_error(
                &format!("Invalid transport config: {}", e),
                ToolGroup::McpManager,
            ))
        }
    };

    // Check if server exists
    if let Ok(None) = get_server_config(name).await {
        return Ok(not_found_error("server", name, ToolGroup::McpManager));
    }

    let config = MCPServerConfig {
        name: name.to_string(),
        transport: transport_config,
        authentication: None,
        metadata: None,
    };

    // Update config
    if let Err(e) = save_server_config(&config).await {
        return Ok(operation_failed_error(
            "updateServer",
            &format!("Failed to update server configuration: {}", e),
            vec!["Check database connectivity".to_string()],
            ToolGroup::McpManager,
        ));
    }

    // Restart if running
    let manager = get_mcp_manager();
    let was_running = {
        let connections = manager.connections.lock().await;
        connections.contains_key(name)
    };

    let mut status_msg = "Server configuration updated".to_string();
    if was_running {
        let _ = manager.stop_server(name).await;
        if let Err(e) = manager.start_server(config).await {
            status_msg.push_str(&format!(", but failed to restart: {}", e));
        } else {
            status_msg.push_str(" and restarted successfully");
        }
    }

    let hint = SuccessHint::new(
        status_msg,
        vec!["Use listServers to verify status".to_string()],
    );
    Ok(hint.to_mcp_result())
}

/// Connect to a server
pub async fn connect_server(args: Value) -> Result<MCPResult, String> {
    let name = match args.get("serverName").and_then(|v| v.as_str()) {
        Some(n) => n,
        Option::None => return Ok(missing_param_error("serverName", ToolGroup::McpManager)),
    };

    let config = match get_server_config(name).await? {
        Some(c) => c,
        None => return Ok(not_found_error("Server", name, ToolGroup::McpManager)),
    };

    let manager = get_mcp_manager();
    if let Err(e) = manager.start_server(config).await {
        return Ok(operation_failed_error(
            "connectServer",
            &e.to_string(),
            vec!["Check server logs".to_string()],
            ToolGroup::McpManager,
        ));
    }

    Ok(SuccessHint::new(format!("Connected to '{}'", name), vec![]).to_mcp_result())
}

/// Disconnect a server
pub async fn disconnect_server(args: Value) -> Result<MCPResult, String> {
    let name = match args.get("serverName").and_then(|v| v.as_str()) {
        Some(n) => n,
        Option::None => return Ok(missing_param_error("serverName", ToolGroup::McpManager)),
    };

    let manager = get_mcp_manager();
    if let Err(e) = manager.stop_server(name).await {
        return Ok(operation_failed_error(
            "disconnectServer",
            &e.to_string(),
            vec![],
            ToolGroup::McpManager,
        ));
    }

    Ok(SuccessHint::new(format!("Disconnected '{}'", name), vec![]).to_mcp_result())
}
