use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, not_found_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::builtin::service_id::BuiltinServiceId;
use crate::mcp::types::{
    MCPResult, MCPServerConfig, OAuthConfig, ServerProvenance, ServerProvenanceSource,
    TransportConfig,
};
use crate::repositories::AssistantRepository;
use serde_json::{json, Value};

use super::super::queries::{find_server_model, get_server_config};
use super::super::ToolServer;
use super::catalog::matching_catalog_preset;
use super::persistence::{
    delete_server_config_db, save_server_config, ContainerCheck, SavedServer,
};
use super::verify::container_stdio_guidance;
use crate::mcp::session_isolation::exec_target::StdioExecTarget;

/// Catalog only for an exact preset transport without custom OAuth; the same
/// rule lets `tool__registerServer` skip hard approval.
pub(crate) fn install_provenance(
    transport: &Value,
    authentication: Option<&OAuthConfig>,
) -> ServerProvenance {
    let preset = authentication
        .is_none()
        .then(|| matching_catalog_preset(transport))
        .flatten();
    ServerProvenance {
        source: if preset.is_some() {
            ServerProvenanceSource::Catalog
        } else {
            ServerProvenanceSource::External
        },
        preset,
        recorded_at: chrono::Utc::now().timestamp_millis(),
    }
}

async fn session_exec_target(session_id: Option<&str>) -> Result<StdioExecTarget, MCPResult> {
    StdioExecTarget::for_session(session_id)
        .await
        .map_err(|error| {
            guided_error(ErrorCategory::DatabaseError, error, ToolGroup::Tool).to_mcp_result()
        })
}

/// The container test failed: the record is saved, but only this response reports the failure.
fn container_check_failed(name: &str, id: &str, error: &str) -> MCPResult {
    guided_error(
        ErrorCategory::OperationFailed,
        format!(
            "Server '{}' (ID: {}) was saved, but it did not start in the session container.\n\n{}",
            name, id, error
        ),
        ToolGroup::Tool,
    )
    .with_guidance(container_stdio_guidance())
    .to_mcp_result_with_data(Some(json!({ "name": name, "id": id })))
}

fn provenance_line(provenance: &ServerProvenance) -> String {
    match &provenance.preset {
        Some(preset) => format!("• Source: catalog preset '{preset}'"),
        None => "• Source: external (not a catalog preset)".to_string(),
    }
}

/// Register a new MCP server configuration
pub async fn register_server(
    _server: &ToolServer,
    args: Value,
    session_id: Option<&str>,
) -> Result<MCPResult, String> {
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) if !n.is_empty() => n.to_string(),
        Some(_) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Server name cannot be empty",
                ToolGroup::Tool,
            )
            .with_guidance(vec!["Provide a unique name for this MCP server".to_string()])
            .to_mcp_result())
        }
        None => return Ok(missing_param_error("name", ToolGroup::Tool)),
    };

    if BuiltinServiceId::from_alias(&name).is_some() {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "Server name '{}' is reserved for a builtin service. Choose a different name.",
                name
            ),
            ToolGroup::Tool,
        )
        .with_guidance(vec!["Use a unique name that doesn't match a builtin service (e.g. planning, browser, workspace)".to_string()])
        .to_mcp_result());
    }

    match get_server_config(&name).await {
        Ok(Some(_)) => {
            return Ok(guided_error(
                ErrorCategory::DuplicateResource,
                format!("Server name '{}' already exists", name),
                ToolGroup::Tool,
            )
            .with_guidance(vec![
                format!(
                    "Use tool__updateServer(name=\"{}\", transport=...) to change the existing server configuration",
                    name
                ),
                format!(
                    "Use tool__listServers(query=\"{}\") to inspect the existing server before modifying it",
                    name
                ),
                "Choose a different unique name if you want to register a separate server"
                    .to_string(),
            ])
            .to_mcp_result());
        }
        Ok(None) => {}
        Err(error) => {
            return Ok(guided_error(
                ErrorCategory::DatabaseError,
                format!(
                    "Failed to check whether server '{}' already exists: {}",
                    name, error
                ),
                ToolGroup::Tool,
            )
            .with_guidance(vec![
                "The server registry could not be queried, so registration was aborted to avoid mutating an existing server by mistake".to_string(),
                "Retry the operation after the database/service issue is resolved".to_string(),
            ])
            .to_mcp_result());
        }
    }

    let transport_val = match args.get("transport") {
        Some(t) => t,
        Option::None => return Ok(missing_param_error("transport", ToolGroup::Tool)),
    };

    let transport: TransportConfig = match serde_json::from_value(transport_val.clone()) {
        Ok(config) => config,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("Invalid transport config: {}", e),
                ToolGroup::Tool,
            )
            .with_guidance(vec![
                "Verify the 'transport' object matches the expected schema (stdio or http)"
                    .to_string(),
                "For stdio: { \"type\": \"stdio\", \"command\": \"...\", \"args\": [...] }"
                    .to_string(),
                "For http: { \"type\": \"http\", \"url\": \"...\" }".to_string(),
            ])
            .to_mcp_result())
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

    let authentication = match args.get("authentication") {
        Some(v) => match serde_json::from_value::<OAuthConfig>(v.clone()) {
            Ok(config) => Some(config),
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("Invalid authentication config: {}", e),
                    ToolGroup::Tool,
                )
                .to_mcp_result())
            }
        },
        None => None,
    };

    let provenance = install_provenance(transport_val, authentication.as_ref());
    let source_line = provenance_line(&provenance);
    let config = MCPServerConfig {
        name: Some(name.clone()),
        transport,
        authentication,
        metadata,
        provenance: Some(provenance),
    };

    let target = match session_exec_target(session_id).await {
        Ok(target) => target,
        Err(result) => return Ok(result),
    };

    let SavedServer {
        id,
        name,
        container_check,
    } = match save_server_config(&config, &target).await {
        Ok(saved) => saved,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::DatabaseError,
                format!("Failed to register server: {}", e),
                ToolGroup::Tool,
            )
            .with_guidance(vec![
                "Check database connectivity".to_string(),
                "Ensure the server name is unique".to_string(),
            ])
            .to_mcp_result());
        }
    };

    // Note: Session Isolation means we cannot auto-start via global manager
    // External servers are now created per-session through MCPServiceProxyManager

    // Emit resource updated event for frontend cache revalidation
    crate::agent::tauri_events::emit_resource_updated(
        "mcpServer",
        "create",
        Some(name.to_string()),
    );

    let status = match container_check {
        Some(ContainerCheck::Failed(error)) => {
            return Ok(container_check_failed(&name, &id, &error))
        }
        Some(ContainerCheck::Passed { tool_count }) => format!(
            "Saved and verified in the session container ({} tools)",
            tool_count
        ),
        None => "Saved (connectivity verification running in the background)".to_string(),
    };

    let hint = SuccessHint::new(
        format!(
            "✓ Server configuration saved\n\n• Server Name: {}\n• Server ID: {}\n{}\n\nStatus: {}",
            name, id, source_line, status
        ),
        vec![
            "Use tool__listServers({\"availability\":\"inventory\"}) to confirm the registered server.".to_string(),
            "Use tool__verifyServer to wait for / refresh connectivity and tool cache if needed.".to_string(),
            "Attach this Server ID to an agent config with agent__updateAgent(id:\"<id>\", externalMcpServers:[...]). That updates the template for future sessions only — it cannot add tools to your currently active session.".to_string(),
            "Confirm what this session can call with tool__listServers({\"availability\":\"session\"}).".to_string(),
        ],
    );
    Ok(hint.to_mcp_result_with_data(Some(json!({ "name": name, "id": id }))))
}

/// Delete an MCP server
pub async fn delete_server(_server: &ToolServer, args: Value) -> Result<MCPResult, String> {
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) if !n.is_empty() => n.to_string(),
        Some(_) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Target name cannot be empty",
                ToolGroup::Tool,
            )
            .with_guidance(vec!["Specify the name of the server to delete".to_string()])
            .to_mcp_result())
        }
        Option::None => return Ok(missing_param_error("name", ToolGroup::Tool)),
    };

    let server = match find_server_model(&name).await? {
        Some(s) => s,
        None => return Ok(not_found_error("Server", &name, ToolGroup::Tool)),
    };

    // Get all agent configs to check for dependencies
    let assistant_repo = crate::state::get_assistant_repository();
    let assistants = assistant_repo
        .list_assistants()
        .await
        .map_err(|e| format!("Failed to list agent configurations: {}", e))?;

    let mut affected_agents = Vec::new();
    for assistant in assistants {
        let parsed_config: Value = serde_json::from_str(&assistant.config).unwrap_or(json!({}));
        if let Some(mcp_server_ids) = parsed_config.get("mcpServerIds").and_then(|v| v.as_array()) {
            let contains_server = mcp_server_ids.iter().any(|id_val| {
                if let Some(id_str) = id_val.as_str() {
                    id_str == server.id || id_str == server.name
                } else {
                    false
                }
            });
            if contains_server {
                affected_agents.push(format!("{} (ID: {})", assistant.name, assistant.id));
            }
        }
    }

    // Note: Session Isolation means we cannot stop via global manager
    // Servers are managed per-session, not globally

    // Delete config
    if let Err(e) = delete_server_config_db(server.name.clone()).await {
        return Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Failed to exclude server configuration: {}", e),
            ToolGroup::Tool,
        )
        .with_guidance(vec![
            "Verify database permissions".to_string(),
            format!(
                "Use tool__listServers({{\"availability\":\"inventory\",\"query\":\"{}\"}}) to confirm the name exists",
                server.name
            ),
        ])
        .to_mcp_result());
    }

    // Emit resource updated event for frontend cache revalidation
    crate::agent::tauri_events::emit_resource_updated(
        "mcpServer",
        "delete",
        Some(server.name.to_string()),
    );

    let mut message = format!(
        "Excluded server '{}' (ID: {}) from configuration. WARNING: This operation is irreversible.",
        server.name, server.id
    );

    if !affected_agents.is_empty() {
        message.push_str("\n\n⚠️ WARNING: The following agent configurations referenced this server and will lose access to its tools:\n");
        for agent in &affected_agents {
            message.push_str(&format!("- {}\n", agent));
        }
        message.push_str("\nPlease update these agent configurations using agent__updateAgent to remove this server reference.");
    }

    let hint = SuccessHint::new(
        message,
        vec![
            "Use tool__listServers({\"availability\":\"inventory\"}) to verify remaining servers"
                .to_string(),
            "Use agent__updateAgent to clean up orphaned server references if needed".to_string(),
        ],
    );
    Ok(hint.to_mcp_result_with_data(Some(json!({
        "name": server.name,
        "id": server.id,
        "status": "deleted",
        "affectedAgents": affected_agents
    }))))
}

/// Update an existing MCP server configuration
pub async fn update_server(
    _server: &ToolServer,
    args: Value,
    session_id: Option<&str>,
) -> Result<MCPResult, String> {
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) if !n.is_empty() => n,
        Some(_) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Target name cannot be empty",
                ToolGroup::Tool,
            )
            .with_guidance(vec!["Specify the name of the server to update".to_string()])
            .to_mcp_result())
        }
        Option::None => return Ok(missing_param_error("name", ToolGroup::Tool)),
    };

    if BuiltinServiceId::from_alias(name).is_some() {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "Server name '{}' is reserved for a builtin service. Choose a different name.",
                name
            ),
            ToolGroup::Tool,
        )
        .with_guidance(vec!["Use a unique name that doesn't match a builtin service (e.g. planning, browser, workspace)".to_string()])
        .to_mcp_result());
    }

    let transport = match args.get("transport") {
        Some(t) => t,
        Option::None => return Ok(missing_param_error("transport", ToolGroup::Tool)),
    };

    let transport_config: TransportConfig = match serde_json::from_value(transport.clone()) {
        Ok(config) => config,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("Invalid transport config: {}", e),
                ToolGroup::Tool,
            )
            .with_guidance(vec![
                "Verify the 'transport' object matches the expected schema".to_string(),
            ])
            .to_mcp_result())
        }
    };

    // Check if server exists (Hallucination Firewall - Section 3.2)
    if let Ok(Option::None) = get_server_config(name).await {
        return Ok(not_found_error("Server", name, ToolGroup::Tool));
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

    let authentication = match args.get("authentication") {
        Some(v) => match serde_json::from_value::<OAuthConfig>(v.clone()) {
            Ok(config) => Some(config),
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("Invalid authentication config: {}", e),
                    ToolGroup::Tool,
                )
                .to_mcp_result())
            }
        },
        None => None,
    };

    let provenance = install_provenance(transport, authentication.as_ref());
    let source_line = provenance_line(&provenance);
    let config = MCPServerConfig {
        name: Some(name.to_string()),
        transport: transport_config,
        authentication,
        metadata,
        provenance: Some(provenance),
    };

    let target = match session_exec_target(session_id).await {
        Ok(target) => target,
        Err(result) => return Ok(result),
    };

    // Update config and get ID
    let SavedServer {
        id,
        name,
        container_check,
    } = match save_server_config(&config, &target).await {
        Ok(saved) => saved,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::DatabaseError,
                format!("Failed to update server configuration: {}", e),
                ToolGroup::Tool,
            )
            .with_guidance(vec!["Check database connectivity".to_string()])
            .to_mcp_result());
        }
    };

    // Note: Session Isolation means we cannot restart via global manager
    // Configuration updates take effect when servers are next started in a session

    // Emit resource updated event for frontend cache revalidation
    crate::agent::tauri_events::emit_resource_updated(
        "mcpServer",
        "update",
        Some(name.to_string()),
    );

    let status = match container_check {
        Some(ContainerCheck::Failed(error)) => {
            return Ok(container_check_failed(&name, &id, &error))
        }
        Some(ContainerCheck::Passed { tool_count }) => format!(
            "\nStatus: verified in the session container ({} tools)",
            tool_count
        ),
        None => String::new(),
    };

    let hint = SuccessHint::new(
        format!(
            "✓ Server configuration updated for '{}' (ID: {})\n{}{}",
            name, id, source_line, status
        ),
        vec!["Use tool__listServers({\"availability\":\"inventory\",\"query\":\"<server-name>\"}) to verify changes".to_string()],
    );
    Ok(hint.to_mcp_result_with_data(Some(json!({ "name": name, "id": id }))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_records_catalog_preset_or_external() {
        let catalog = json!({
            "type": "stdio",
            "command": "npx",
            "args": ["-y", "@fre4x/jules@latest"]
        });
        let provenance = install_provenance(&catalog, None);
        assert_eq!(provenance.source, ServerProvenanceSource::Catalog);
        assert_eq!(provenance.preset.as_deref(), Some("jules"));

        let external = json!({ "type": "stdio", "command": "npx", "args": ["-y", "left-pad-mcp"] });
        let provenance = install_provenance(&external, None);
        assert_eq!(provenance.source, ServerProvenanceSource::External);
        assert!(provenance.preset.is_none());

        let stored = serde_json::to_value(&provenance).expect("serialize");
        assert_eq!(stored["source"], "external");
        assert!(stored.get("preset").is_none());
        assert!(stored["recordedAt"].is_i64());
    }
}
