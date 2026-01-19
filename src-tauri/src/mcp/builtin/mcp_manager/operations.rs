use crate::entity::{mcp_server, mcp_server::Entity as McpServerEntity};
use crate::mcp::builtin::error_guidance::{
    invalid_input_error, missing_param_error, operation_failed_error, ErrorCategory, ErrorGuidance,
    SuccessHint, ToolGroup,
};
use crate::mcp::types::{MCPResult, MCPServerConfig, TransportConfig};
use crate::state::{get_database_connection, get_mcp_manager};
use sea_orm::*;
use serde_json::{json, Value};

use super::queries::get_server_config;

use super::MCPManagerServer;

async fn save_server_config(config: &MCPServerConfig) -> Result<(), String> {
    let db = get_database_connection();
    let now = chrono::Utc::now().timestamp_millis();
    let config_json = serde_json::to_string(config).map_err(|e| e.to_string())?;

    // Upsert using SeaORM (async without nested runtime)
    let model = mcp_server::ActiveModel {
        name: Set(config.name.clone()),
        config: Set(config_json.clone()),
        created_at: Set(now),
        updated_at: Set(now),
    };

    match McpServerEntity::insert(model.clone()).exec(db).await {
        Ok(_) => Ok(()),
        Err(DbErr::RecordNotInserted) | Err(DbErr::Exec(_)) => {
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

async fn delete_server_config_db(name: String) -> Result<(), String> {
    let db = get_database_connection();
    McpServerEntity::delete_by_id(name)
        .exec(db)
        .await
        .map_err(|e| format!("DB Delete Error: {}", e))?;
    Ok(())
}

/// Create and start a new MCP server
pub async fn create_server(server: &MCPManagerServer, args: Value) -> Result<MCPResult, String> {
    // Get and validate name (required parameter)
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(custom_name) if !custom_name.trim().is_empty() => {
            let sanitized = custom_name.trim().to_string();

            // Validate name format (alphanumeric, hyphens, underscores)
            if !sanitized
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            {
                return Ok(invalid_input_error(
                    "Server name must contain only alphanumeric characters, hyphens, and underscores",
                    ToolGroup::McpManager,
                ));
            }

            // Check if name already exists (uniqueness constraint)
            if get_server_config(&sanitized).await.ok().flatten().is_some() {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::DuplicateResource,
                    format!("Server name '{}' already exists", sanitized),
                    vec![
                        "Choose a different name".to_string(),
                        "Use listServers to see existing server names".to_string(),
                    ],
                    ToolGroup::McpManager,
                )
                .to_mcp_result());
            }

            sanitized
        }
        Some(_) => {
            return Ok(invalid_input_error(
                "Server name cannot be empty",
                ToolGroup::McpManager,
            ))
        }
        None => return Ok(missing_param_error("name", ToolGroup::McpManager)),
    };

    let transport_val = match args.get("transport") {
        Some(t) => t,
        Option::None => return Ok(missing_param_error("transport", ToolGroup::McpManager)),
    };

    let transport: TransportConfig = serde_json::from_value(transport_val.clone())
        .map_err(|e| format!("Invalid transport config: {}", e))?;

    let config = MCPServerConfig {
        name: name.clone(),
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
    // Use a clone to keep `config` available for error handling if start fails
    if let Err(e) = manager.start_server(config.clone()).await {
        let error_msg = e.to_string();
        let mut hints = vec!["Check server command or URL".to_string()];

        // Check for common "not found" errors to provide better guidance
        if error_msg.to_lowercase().contains("program not found")
            || error_msg.contains("No such file or directory")
            || error_msg.contains("The system cannot find the file specified")
        {
            if let TransportConfig::Stdio { command, .. } = &config.transport {
                match command.as_str() {
                    "npx" => hints.push(
                        "Try installing Node.js/npm and ensure 'npx' is in your PATH".to_string(),
                    ),
                    "uv" => hints.push(
                        "Try installing 'uv' (pip install uv) and ensure it is in your PATH"
                            .to_string(),
                    ),
                    "python" | "python3" => {
                        hints.push("Check your Python installation and PATH".to_string())
                    }
                    cmd => hints.push(format!("Ensure '{}' is installed and valid", cmd)),
                }
            }
        }

        return Ok(operation_failed_error(
            "start_server",
            &format!("Server created but failed to start: {}", e),
            hints,
            ToolGroup::McpManager,
        ));
    }

    server.invalidate_cache().await;

    let hint = SuccessHint::new(
        format!(
            "✓ Server created and started\n\nServer Name: {}\nStatus: Connected\n\nUse this name for subsequent management operations (connect, update, delete).",
            name
        ),
        vec![
            "Use listServers to view all registered servers".to_string(),
            format!("Use disconnectServer('{}') to stop this server", name),
        ],
    );
    Ok(hint.to_mcp_result_with_data(Some(json!({ "name": name }))))
}

/// Delete an MCP server
pub async fn delete_server(server: &MCPManagerServer, args: Value) -> Result<MCPResult, String> {
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) if !n.is_empty() => n.to_string(),
        Some(_) => {
            return Ok(invalid_input_error(
                "Target name cannot be empty",
                ToolGroup::McpManager,
            ))
        }
        Option::None => return Ok(missing_param_error("name", ToolGroup::McpManager)),
    };

    // Check if server exists (Hallucination Firewall - Section 3.2)
    if let Ok(Option::None) = get_server_config(&name).await {
        return Ok(ErrorGuidance::with_guidance(
            ErrorCategory::ResourceNotFound,
            format!("Server '{}' not found in configuration", name),
            vec![
                "Use listServers to view all registered servers".to_string(),
                format!("Use searchServer(query='{}') to find similar names", name),
            ],
            ToolGroup::McpManager,
        )
        .to_mcp_result());
    }

    // Stop server first if running
    let manager = get_mcp_manager();
    let _ = manager.stop_server(&name).await;

    // Delete config
    if let Err(e) = delete_server_config_db(name.clone()).await {
        return Ok(operation_failed_error(
            "deleteServer",
            &format!("Failed to exclude server configuration: {}", e),
            vec![
                "Verify database permissions".to_string(),
                "Target 'listServers' to ensure the name exists".to_string(),
            ],
            ToolGroup::McpManager,
        ));
    }

    server.invalidate_cache().await;

    let hint = SuccessHint::new(
        format!("Excluded server '{}' from configuration", name),
        vec!["Use listServers to verify remaining servers".to_string()],
    );
    Ok(hint.to_mcp_result())
}

/// Update an existing MCP server configuration
pub async fn update_server(server: &MCPManagerServer, args: Value) -> Result<MCPResult, String> {
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) if !n.is_empty() => n,
        Some(_) => {
            return Ok(invalid_input_error(
                "Target name cannot be empty",
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

    // Check if server exists (Hallucination Firewall - Section 3.2)
    if let Ok(Option::None) = get_server_config(name).await {
        return Ok(ErrorGuidance::with_guidance(
            ErrorCategory::ResourceNotFound,
            format!("Server '{}' not found in configuration", name),
            vec![
                "Use listServers to view all registered servers".to_string(),
                format!("Use searchServer(query='{}') to find similar names", name),
            ],
            ToolGroup::McpManager,
        )
        .to_mcp_result());
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
            &format!("Failed to target server for configuration update: {}", e),
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

    server.invalidate_cache().await;

    let hint = SuccessHint::new(
        status_msg,
        vec!["Use listServers to extract status".to_string()],
    );
    Ok(hint.to_mcp_result())
}

/// Connect to a server
pub async fn connect_server(server: &MCPManagerServer, args: Value) -> Result<MCPResult, String> {
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        Option::None => return Ok(missing_param_error("name", ToolGroup::McpManager)),
    };

    let config = match get_server_config(name).await? {
        Some(c) => c,
        Option::None => {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::ResourceNotFound,
                format!("Server '{}' not found in configuration", name),
                vec![
                    "Use listServers to view all registered servers".to_string(),
                    format!("Use searchServer(query='{}') to find similar names", name),
                ],
                ToolGroup::McpManager,
            )
            .to_mcp_result())
        }
    };

    let manager = get_mcp_manager();
    // Use a clone to keep `config` available for error handling if start fails
    if let Err(e) = manager.start_server(config.clone()).await {
        let error_msg = e.to_string();
        let mut hints = vec!["Check target server logs".to_string()];

        // Check for common "not found" errors to provide better guidance
        if error_msg.to_lowercase().contains("program not found")
            || error_msg.contains("No such file or directory")
            || error_msg.contains("The system cannot find the file specified")
        {
            if let TransportConfig::Stdio { command, .. } = &config.transport {
                match command.as_str() {
                    "npx" => hints.push(
                        "Try installing Node.js/npm and ensure 'npx' is in your PATH".to_string(),
                    ),
                    "uv" => hints.push(
                        "Try installing 'uv' (pip install uv) and ensure it is in your PATH"
                            .to_string(),
                    ),
                    "python" | "python3" => {
                        hints.push("Check your Python installation and PATH".to_string())
                    }
                    cmd => hints.push(format!("Ensure '{}' is installed and valid", cmd)),
                }
            }
        }

        return Ok(operation_failed_error(
            "connectServer",
            &error_msg,
            hints,
            ToolGroup::McpManager,
        ));
    }

    server.invalidate_cache().await;

    Ok(SuccessHint::new(format!("Target server '{}' connected", name), vec![]).to_mcp_result())
}

/// Disconnect a server
pub async fn disconnect_server(
    server: &MCPManagerServer,
    args: Value,
) -> Result<MCPResult, String> {
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        Option::None => return Ok(missing_param_error("name", ToolGroup::McpManager)),
    };

    let manager = get_mcp_manager();

    // Check if server is actually connected (Feedback Logic)
    if !manager.is_server_alive(name).await {
        return Ok(ErrorGuidance::with_guidance(
            ErrorCategory::ResourceNotFound,
            format!("Server '{}' is not currently connected", name),
            vec!["Use listServers to view active connections".to_string()],
            ToolGroup::McpManager,
        )
        .to_mcp_result());
    }

    if let Err(e) = manager.stop_server(name).await {
        return Ok(operation_failed_error(
            "disconnectServer",
            &e.to_string(),
            vec![],
            ToolGroup::McpManager,
        ));
    }

    server.invalidate_cache().await;

    Ok(SuccessHint::new(format!("Target server '{}' disconnected", name), vec![]).to_mcp_result())
}
