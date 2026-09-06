use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, not_found_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::builtin::utils::load_session_tool_access;
use crate::mcp::types::{MCPResult, TransportConfig};
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::state::get_mcp_server_repository;
use serde_json::{json, Value};
use std::time::Instant;

use super::super::queries::get_server_details;
use super::super::ToolServer;

/// Verify server configuration and connectivity
pub async fn verify_server(
    _server: &ToolServer,
    args: Value,
    session_id: Option<&str>,
) -> Result<MCPResult, String> {
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        Option::None => return Ok(missing_param_error("name", ToolGroup::Tool)),
    };

    // Get server config
    let (id, config) = match get_server_details(name).await? {
        Some(details) => details,
        Option::None => return Ok(not_found_error("Server", name, ToolGroup::Tool)),
    };

    let access = load_session_tool_access(session_id).await;
    let (session_status, session_access_line) =
        access.external_access_report(session_id, &id, name);

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

    // Emit resource updated event for frontend cache revalidation
    crate::agent::tauri_events::emit_resource_updated(
        "mcpServer",
        "verify",
        Some(name.to_string()),
    );

    match verification_result {
        Ok((tool_count, tools_json)) => {
            // Persist tool list to database (count + names/descriptions)
            let repo = get_mcp_server_repository();
            if let Err(e) = repo
                .update_cached_tools(&id, tool_count as i32, tools_json)
                .await
            {
                log::warn!(
                    "Failed to cache tool list for '{}' (ID: {}): {}",
                    name,
                    id,
                    e
                );
                // Continue - don't fail verification if cache update fails
            }

            let result_text = format!(
                "✓ Server '{}' (ID: {}) connectivity verification successful\n\n\
                Transport: {}\n\
                {}\n\
                Status: Connected and responsive\n\
                Tools discovered: {} (cached metadata — not the same as session-callable access)\n\
                Connection latency: {}ms\n\
                {}\n\n\
                Verification only confirms the server config can connect. It does not enable tools in your currently active session.",
                name,
                id,
                transport_type,
                transport_details,
                tool_count,
                latency_ms,
                session_access_line
            );

            Ok(SuccessHint::new(
                result_text,
                vec![
                    "Use tool__listServers({\"availability\":\"session\"}) to see tools callable in this session.".to_string(),
                    "agent__updateAgent attaches servers to an agent template for future sessions only; start a new session (or agent__startSession) to run with that access.".to_string(),
                ],
            )
            .to_mcp_result_with_data(Some(json!({
                "name": name,
                "id": id,
                "toolCount": tool_count,
                "sessionStatus": session_status,
            }))))
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

            Ok(guided_error(
                ErrorCategory::OperationFailed,
                format!("{}\n\n{}", error_msg, error_details),
                ToolGroup::Tool,
            )
            .with_guidance(suggestions)
            .to_mcp_result())
        }
    }
}

/// Test server connection by spawning/connecting and calling the remote MCP ListTools API.
/// Returns `(tool_count, tools_json)` where `tools_json` is a JSON array of
/// `{"name": "...", "description": "..."}` entries for caching.
pub(super) async fn test_server_connection(
    config: &crate::mcp::types::MCPServerConfig,
    server_name: &str,
) -> Result<(usize, String), String> {
    let mut cloned = config.clone();
    if cloned.name.is_none() {
        cloned.name = Some(server_name.to_string());
    }

    let tools =
        crate::services::mcp_server_service::McpServerService::verify_config(cloned).await?;
    let tools_json = crate::mcp::utils::serialize_mcp_tools(&tools);
    Ok((tools.len(), tools_json))
}
