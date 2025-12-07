use super::MCPServerManager;
use crate::mcp::types::{MCPConnection, MCPServerConfig, MCPServerConfigV2, TransportConfig};
use anyhow::Result;
use log::{debug, error, info};
use rmcp::{
    transport::{ConfigureCommandExt, StreamableHttpClientTransport, TokioChildProcess},
    ServiceExt,
};
use std::collections::HashMap;
use tokio::process::Command;

pub async fn start_server_v2(
    manager: &MCPServerManager,
    config: MCPServerConfigV2,
) -> Result<String> {
    match &config.transport {
        TransportConfig::Stdio { command, args, env } => {
            // Convert to legacy format for stdio
            let legacy_config = MCPServerConfig {
                name: config.name.clone(),
                command: Some(command.clone()),
                args: Some(args.clone()),
                env: Some(env.clone()),
                transport: "stdio".to_string(),
                url: None,
                port: None,
            };
            start_stdio_server(manager, legacy_config).await
        }
        TransportConfig::Http {
            url,
            protocol_version,
            session_id,
            headers,
            ..
        } => {
            start_http_server(
                manager,
                config.name.clone(),
                url.clone(),
                protocol_version.clone(),
                session_id.clone(),
                headers.clone(),
            )
            .await
        }
    }
}

pub async fn start_server(manager: &MCPServerManager, config: MCPServerConfig) -> Result<String> {
    match config.transport.as_str() {
        "stdio" => start_stdio_server(manager, config).await,
        "http" => {
            // Convert to V2 and use new HTTP handler
            if let Some(url) = config.url {
                start_http_server(
                    manager,
                    config.name,
                    url,
                    "2025-06-18".to_string(),
                    None,
                    None,
                )
                .await
            } else {
                Err(anyhow::anyhow!("HTTP transport requires URL"))
            }
        }
        "websocket" => Err(anyhow::anyhow!("WebSocket transport not yet implemented")),
        _ => Err(anyhow::anyhow!(
            "Unsupported transport: {}",
            config.transport
        )),
    }
}

async fn start_stdio_server(manager: &MCPServerManager, config: MCPServerConfig) -> Result<String> {
    let command = config
        .command
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Command is required for stdio transport"))?;

    let default_args = vec![];
    let args = config.args.as_ref().unwrap_or(&default_args);

    // Create command with rmcp - configure returns the modified command
    let cmd = Command::new(command).configure(|cmd| {
        for arg in args {
            cmd.arg(arg);
        }

        // Set environment variables if any
        if let Some(env) = &config.env {
            for (key, value) in env {
                cmd.env(key, value);
            }
        }
    });

    // Create transport and connect using RMCP pattern
    let transport = TokioChildProcess::new(cmd)?;
    debug!("Created transport for command: {command} {args:?}");

    let client = ().serve(transport).await?;
    info!("Successfully connected to MCP server: {}", config.name);

    let connection = MCPConnection { client };

    // Store connection
    {
        let mut connections = manager.connections.lock().await;
        connections.insert(config.name.clone(), connection);
        debug!("Stored connection for server: {}", config.name);
    }

    Ok(format!(
        "Started and connected to MCP server: {}",
        config.name
    ))
}

async fn start_http_server(
    manager: &MCPServerManager,
    name: String,
    url: String,
    protocol_version: String,
    _session_id: Option<String>,
    _headers: Option<HashMap<String, String>>,
) -> Result<String> {
    info!("Starting HTTP MCP server: {name} at {url}");

    // Create HTTP transport using RMCP's built-in StreamableHttpClientTransport
    let transport = StreamableHttpClientTransport::from_uri(url.clone());

    // Add MCP protocol version header
    // Note: RMCP's HTTP client automatically adds required headers

    debug!("Created HTTP transport for {name} (protocol: {protocol_version})");

    // Connect to the HTTP server
    let client = ().serve(transport).await.map_err(|e| {
        error!("Failed to connect to HTTP MCP server {name}: {e}");
        anyhow::anyhow!("HTTP connection failed: {e}")
    })?;

    info!("Successfully connected to HTTP MCP server: {name}");

    let connection = MCPConnection { client };

    // Store connection
    {
        let mut connections = manager.connections.lock().await;
        connections.insert(name.clone(), connection);
        debug!("Stored HTTP connection for server: {name}");
    }

    Ok(format!(
        "Started and connected to HTTP MCP server: {name} at {url}"
    ))
}

pub async fn stop_server(manager: &MCPServerManager, server_name: &str) -> Result<()> {
    let mut connections = manager.connections.lock().await;

    if let Some(connection) = connections.remove(server_name) {
        // Cancel the client connection
        let _ = connection.client.cancel().await;
        info!("Stopped MCP server: {server_name}");
    }

    Ok(())
}
