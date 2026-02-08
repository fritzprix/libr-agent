use crate::agent::events;
use crate::mcp::builtin::error_guidance::{
    invalid_input_error, missing_param_error, operation_failed_error, ErrorCategory, ErrorGuidance,
    SuccessHint, ToolGroup,
};
use crate::mcp::types::{MCPResult, MCPServerConfig, TransportConfig};
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::state::get_mcp_server_repository;
use serde_json::{json, Value};

use super::queries::{get_server_config, get_server_details};

use super::MCPManagerServer;

async fn save_server_config(config: &MCPServerConfig) -> Result<(), String> {
    let repo = get_mcp_server_repository();
    let server_name = config
        .name
        .as_ref()
        .ok_or_else(|| "Server name is required".to_string())?;

    let config_value = serde_json::to_value(config).map_err(|e| e.to_string())?;

    // Try to update first (by name lookup), create if doesn't exist
    match repo.get_by_name(server_name).await {
        Ok(Some(existing)) => {
            // Update by ID with new config
            repo.update(&existing.id, None, Some(config_value))
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

async fn delete_server_config_db(id_or_name: String) -> Result<(), String> {
    let repo = get_mcp_server_repository();

    // Try ID first, then name
    let mut server = repo.get(&id_or_name).await.map_err(|e| e.to_string())?;
    if server.is_none() {
        server = repo
            .get_by_name(&id_or_name)
            .await
            .map_err(|e| e.to_string())?;
    }

    let server = server.ok_or_else(|| format!("MCP server '{}' not found", id_or_name))?;

    repo.delete(&server.id)
        .await
        .map_err(|e| format!("DB Delete Error: {}", e))?;
    Ok(())
}

/// Register a new MCP server configuration
pub async fn register_server(server: &MCPManagerServer, args: Value) -> Result<MCPResult, String> {
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

    let transport: TransportConfig = match serde_json::from_value(transport_val.clone()) {
        Ok(config) => config,
        Err(e) => {
            return Ok(invalid_input_error(
                &format!("Invalid transport config: {}", e),
                ToolGroup::McpManager,
            ))
        }
    };

    // Extract optional description for metadata
    let metadata = args
        .get("description")
        .and_then(|v| v.as_str())
        .map(|desc| crate::mcp::types::ServerMetadata {
            description: Some(desc.to_string()),
            vendor: None,
            version: None,
        });

    let config = MCPServerConfig {
        name: Some(name.clone()),
        transport,
        authentication: None,
        metadata,
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

    // Emit resource updated event for frontend cache revalidation
    events::emit_resource_updated("mcpServer", "create", Some(name.to_string()));

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

    // Emit resource updated event for frontend cache revalidation
    events::emit_resource_updated("mcpServer", "delete", Some(name.to_string()));

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

    // Extract optional description for metadata
    let metadata = args
        .get("description")
        .and_then(|v| v.as_str())
        .map(|desc| crate::mcp::types::ServerMetadata {
            description: Some(desc.to_string()),
            vendor: None,
            version: None,
        });

    let config = MCPServerConfig {
        name: Some(name.to_string()),
        transport: transport_config,
        authentication: None,
        metadata,
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

    // Emit resource updated event for frontend cache revalidation
    events::emit_resource_updated("mcpServer", "update", Some(name.to_string()));

    let hint = SuccessHint::new(
        "Server configuration updated".to_string(),
        vec!["Use listServers to extract status".to_string()],
    );
    Ok(hint.to_mcp_result())
}

/// Verify server configuration and connectivity
pub async fn verify_server(server: &MCPManagerServer, args: Value) -> Result<MCPResult, String> {
    use crate::mcp::types::TransportConfig;
    use std::time::Instant;

    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        Option::None => return Ok(missing_param_error("name", ToolGroup::McpManager)),
    };

    // Get server config
    let (id, config) = match get_server_details(name).await? {
        Some(details) => details,
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

    // Determine transport type
    let (transport_type, transport_details) = match &config.transport {
        TransportConfig::Stdio { command, args, .. } => {
            let args_str = if args.is_empty() {
                "(no arguments)".to_string()
            } else {
                args.join(" ")
            };
            (
                "stdio",
                format!("Command: {}\nArguments: {}", command, args_str),
            )
        }
        TransportConfig::Http { url, .. } => ("http", format!("URL: {}", url)),
    };

    // Test connection and list tools
    let start_time = Instant::now();
    let verification_result = test_server_connection(&config, name).await;
    let latency_ms = start_time.elapsed().as_millis();

    server.invalidate_cache().await;

    // Emit resource updated event for frontend cache revalidation
    events::emit_resource_updated("mcpServer", "verify", Some(name.to_string()));

    match verification_result {
        Ok(tool_count) => {
            // Persist tool count to database for UI display
            let repo = get_mcp_server_repository();
            if let Err(e) = repo.update_tool_count(&id, tool_count as i32).await {
                log::warn!(
                    "Failed to cache tool count for '{}' (ID: {}): {}",
                    name,
                    id,
                    e
                );
                // Continue - don't fail verification if cache update fails
            }

            let result_text = format!(
                "✓ Server '{}' verification successful\n\n\
                Transport: {}\n\
                {}\n\
                Status: Connected and responsive\n\
                Available tools: {} (cached)\n\
                Connection latency: {}ms\n\n\
                The server is properly configured and ready to use.",
                name, transport_type, transport_details, tool_count, latency_ms
            );

            Ok(SuccessHint::new(
                result_text,
                vec!["Server configuration is valid and operational".to_string()],
            )
            .to_mcp_result())
        }
        Err(error) => {
            let error_msg = format!("✗ Server '{}' verification failed", name);
            let error_details = format!(
                "Transport: {}\n\
                {}\n\
                Status: Failed to connect or respond\n\
                Error: {}\n\
                Test duration: {}ms",
                transport_type, transport_details, error, latency_ms
            );

            let suggestions = match transport_type {
                "stdio" => vec![
                    "Verify the command path is correct and executable".to_string(),
                    "Check that all required arguments are provided".to_string(),
                    "Ensure the MCP server package is installed".to_string(),
                    "Test the command manually in terminal".to_string(),
                ],
                "http" => vec![
                    "Verify the URL is correct and accessible".to_string(),
                    "Check that the HTTP server is running".to_string(),
                    "Ensure network connectivity to the endpoint".to_string(),
                    "Verify authentication headers if required".to_string(),
                ],
                _ => vec!["Review server configuration".to_string()],
            };

            Ok(ErrorGuidance::with_guidance(
                ErrorCategory::OperationFailed,
                format!("{}\n\n{}", error_msg, error_details),
                suggestions,
                ToolGroup::McpManager,
            )
            .to_mcp_result())
        }
    }
}

/// Test server connection by spawning/connecting and calling listTools
async fn test_server_connection(
    config: &crate::mcp::types::MCPServerConfig,
    server_name: &str,
) -> Result<usize, String> {
    use crate::mcp::types::TransportConfig;
    use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
    use rmcp::ServiceExt;
    use std::time::Duration;

    match &config.transport {
        TransportConfig::Stdio { command, args, env } => {
            // Prepare command with cross-platform support (Windows .cmd/.bat wrapping)
            let (final_command, final_args) =
                crate::mcp::utils::command_helper::prepare_command(command, args);

            log::debug!(
                "Testing stdio server '{}': {} {:?}",
                server_name,
                final_command,
                final_args
            );

            // Spawn process
            let cmd = tokio::process::Command::new(&final_command).configure(|cmd| {
                for arg in &final_args {
                    cmd.arg(arg);
                }
                for (key, value) in env {
                    cmd.env(key, value);
                }
            });

            let transport = TokioChildProcess::new(cmd)
                .map_err(|e| format!("Failed to spawn process: {}", e))?;

            // Initialize with timeout
            let client = tokio::time::timeout(Duration::from_secs(30), ().serve(transport))
                .await
                .map_err(|_| "Initialization timeout (30s)".to_string())?
                .map_err(|e| format!("Initialization failed: {}", e))?;

            // List tools
            let tools = client
                .list_all_tools()
                .await
                .map_err(|e| format!("Failed to list tools: {}", e))?;

            log::info!(
                "Stdio server '{}' verified: {} tools available",
                server_name,
                tools.len()
            );

            Ok(tools.len())
        }
        TransportConfig::Http {
            url,
            headers,
            enable_sse,
            ..
        } => {
            use rmcp::transport::streamable_http_client::{
                StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
            };

            log::debug!("Testing HTTP server '{}': {}", server_name, url);

            // Build header map
            let mut header_map = reqwest::header::HeaderMap::new();
            if let Some(headers) = headers {
                for (k, v) in headers {
                    if let (Ok(k), Ok(v)) = (
                        reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                        reqwest::header::HeaderValue::from_str(v),
                    ) {
                        header_map.insert(k, v);
                    }
                }
            }

            let http_client = reqwest::Client::builder()
                .default_headers(header_map)
                .build()
                .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

            let mut transport_config = StreamableHttpClientTransportConfig::with_uri(url.as_str());
            if let Some(sse) = enable_sse {
                transport_config.allow_stateless = !sse;
            }

            let transport =
                StreamableHttpClientTransport::with_client(http_client, transport_config);

            // Initialize with timeout
            let client = tokio::time::timeout(Duration::from_secs(30), ().serve(transport))
                .await
                .map_err(|_| "Initialization timeout (30s)".to_string())?
                .map_err(|e| format!("Initialization failed: {}", e))?;

            // List tools
            let tools = client
                .list_all_tools()
                .await
                .map_err(|e| format!("Failed to list tools: {}", e))?;

            log::info!(
                "HTTP server '{}' verified: {} tools available",
                server_name,
                tools.len()
            );

            Ok(tools.len())
        }
    }
}
