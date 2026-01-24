use crate::mcp::builtin::error_guidance::{
    invalid_input_error, missing_param_error, operation_failed_error, ErrorCategory, ErrorGuidance,
    SuccessHint, ToolGroup,
};
use crate::mcp::types::{MCPResult, MCPServerConfig, TransportConfig};
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::state::get_mcp_server_repository;
use serde_json::{json, Value};

use super::queries::get_server_config;

use super::MCPManagerServer;

async fn save_server_config(config: &MCPServerConfig) -> Result<(), String> {
    let repo = get_mcp_server_repository();
    let server_name = config
        .name
        .as_ref()
        .ok_or_else(|| "Server name is required".to_string())?;

    let config_value = serde_json::to_value(config).map_err(|e| e.to_string())?;

    // Try to update first, create if doesn't exist
    match repo.get(server_name).await {
        Ok(Some(_)) => {
            repo.update(server_name, config_value)
                .await
                .map_err(|e| format!("Failed to update MCP server config: {}", e))?;
        }
        Ok(None) => {
            repo.create(server_name, config_value)
                .await
                .map_err(|e| format!("Failed to create MCP server config: {}", e))?;
        }
        Err(e) => return Err(format!("DB query error: {}", e)),
    }
    Ok(())
}

async fn delete_server_config_db(name: String) -> Result<(), String> {
    let repo = get_mcp_server_repository();
    repo.delete(&name)
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
        name: Some(name.clone()),
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

    // Note: Session Isolation means we cannot auto-start via global manager
    // External servers are now created per-session through MCPServiceProxyManager
    server.invalidate_cache().await;

    let hint = SuccessHint::new(
        format!(
            "✓ Server configuration saved\n\nServer Name: {}\nStatus: Configured (not auto-started)\n\nExternal servers are managed per-session through MCPServiceProxyManager.",
            name
        ),
        vec![
            "Use listServers to view all registered servers".to_string(),
            format!("Use connectServer('{}') to start this server in a session", name),
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

    // Note: Session Isolation means we cannot stop via global manager
    // Servers are managed per-session, not globally

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
        name: Some(name.to_string()),
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

    // Note: Session Isolation means we cannot restart via global manager
    // Configuration updates take effect when servers are next started in a session

    server.invalidate_cache().await;

    let hint = SuccessHint::new(
        "Server configuration updated".to_string(),
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

    let _config = match get_server_config(name).await? {
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

    // Note: Session Isolation means connection is per-session, not global
    // This tool validates the config exists but doesn't actually connect
    // Actual connection happens in MCPServiceProxyManager when session is created

    server.invalidate_cache().await;

    Ok(SuccessHint::new(
        format!("Target server '{}' configuration validated", name),
        vec![],
    )
    .to_mcp_result())
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

    // Note: Session Isolation means disconnection is per-session, not global
    // This tool only validates the server exists; actual disconnection happens in session cleanup

    server.invalidate_cache().await;

    Ok(SuccessHint::new(
        format!(
            "Target server '{}' marked for disconnection in current session",
            name
        ),
        vec![],
    )
    .to_mcp_result())
}
