use super::MCPServerManager;
use crate::mcp::types::{MCPConnection, MCPServerConfig, TransportConfig};
use crate::mcp::utils::command_helper::CommandExt;
use anyhow::Result;
use log::{debug, error, info};
use rmcp::{
    transport::{ConfigureCommandExt, StreamableHttpClientTransport, TokioChildProcess},
    ServiceExt,
};
use std::collections::HashMap;
use tokio::process::Command;

/// Start an MCP server with the given configuration
pub async fn start_server(manager: &MCPServerManager, config: MCPServerConfig) -> Result<String> {
    // Clone the transport info we need before moving config
    match config.transport.clone() {
        TransportConfig::Stdio { command, args, env } => {
            start_stdio_server(manager, config, &command, &args, &env).await
        }
        TransportConfig::Http {
            url,
            protocol_version,
            session_id,
            headers,
            enable_sse,
            ..
        } => {
            start_http_server(
                manager,
                config,
                url,
                protocol_version,
                session_id,
                headers,
                enable_sse,
            )
            .await
        }
    }
}

async fn start_stdio_server(
    manager: &MCPServerManager,
    config: MCPServerConfig,
    command: &str,
    args: &[String],
    env: &HashMap<String, String>,
) -> Result<String> {
    let name = config
        .name
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Server name is required"))?
        .clone();

    // Prepare command with cross-platform handling
    // On Windows, this wraps .cmd/.bat files with cmd.exe
    let (final_command, final_args) =
        crate::mcp::utils::command_helper::prepare_command(command, args);

    log::info!(
        "Starting MCP server '{}': {} {:?} (env vars: {})",
        name,
        final_command,
        final_args,
        env.len()
    );

    // Create command with rmcp - configure returns the modified command
    let cmd = Command::new(&final_command).configure(|cmd| {
        for arg in &final_args {
            cmd.arg(arg);
        }

        // Set environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        cmd.silent();
    });

    // Create transport and connect using RMCP pattern
    log::info!("Attempting to spawn process: {} {:?}", command, args);
    let transport = TokioChildProcess::new(cmd)?;
    log::info!("Successfully spawned process");
    debug!("Created transport for command: {command} {args:?}");

    let client = ().serve(transport).await?;
    info!("Successfully connected to MCP server: {name}");

    let connection = MCPConnection { client, config };

    // Store connection
    {
        let mut connections = manager.connections.lock().await;
        connections.insert(name.clone(), connection);
        debug!("Stored connection for server: {name}");
    }

    Ok(format!("Started and connected to MCP server: {name}"))
}

use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;

async fn start_http_server(
    manager: &MCPServerManager,
    config: MCPServerConfig,
    url: String,
    protocol_version: String,
    session_id: Option<String>,
    headers: Option<HashMap<String, String>>,
    enable_sse: Option<bool>,
) -> Result<String> {
    let name = config
        .name
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Server name is required"))?
        .clone();
    info!("Starting HTTP MCP server: {name} at {url}");

    // prepare headers
    let mut header_map = reqwest::header::HeaderMap::new();

    // Add custom headers
    if let Some(headers) = headers {
        for (k, v) in headers {
            if let (Ok(k), Ok(v)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                reqwest::header::HeaderValue::from_str(&v),
            ) {
                header_map.insert(k, v);
            } else {
                error!("Invalid header ignored: {}: {}", k, v);
            }
        }
    }

    // Add session ID if provided
    if let Some(sid) = session_id {
        if let Ok(v) = reqwest::header::HeaderValue::from_str(&sid) {
            // Use string key to avoid dependency on internal constants if possible,
            // but Mcp-Session-Id is standard.
            if let Ok(k) = reqwest::header::HeaderName::from_bytes(b"Mcp-Session-Id") {
                header_map.insert(k, v);
            }
        }
    }

    // Build reqwest client
    let client = reqwest::Client::builder()
        .default_headers(header_map)
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {e}"))?;

    // Create configuration
    let mut transport_config = StreamableHttpClientTransportConfig::with_uri(url.clone());

    // Process enable_sse if applicable (mapping to allow_stateless if inverse)
    // If enable_sse is explicitly false, we might want to enable stateless if likely?
    // But for now, let's keep config default unless we know for sure.
    // The previous error showed allow_stateless is available.
    // If streaming (SSE) is disabled, it might mean "stateless request/response".
    // Let's assume enable_sse=false -> allow_stateless=true.
    if let Some(sse) = enable_sse {
        transport_config.allow_stateless = !sse;
    }

    // Create transport with custom client
    let transport = StreamableHttpClientTransport::with_client(client, transport_config);

    debug!("Created HTTP transport for {name} (protocol: {protocol_version})");

    // Connect to the HTTP server
    let client = ().serve(transport).await.map_err(|e| {
        error!("Failed to connect to HTTP MCP server {name}: {e}");
        anyhow::anyhow!("HTTP connection failed: {e}")
    })?;

    info!("Successfully connected to HTTP MCP server: {name}");

    let connection = MCPConnection { client, config };

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
