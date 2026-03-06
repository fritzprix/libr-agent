use crate::agent::events;
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, not_found_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::builtin::service_id::BuiltinServiceId;
use crate::mcp::types::{MCPResult, MCPServerConfig, TransportConfig};
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::state::get_mcp_server_repository;
use serde_json::{json, Value};

use super::queries::{get_server_config, get_server_details};

use super::MCPManagerServer;

async fn save_server_config(config: &MCPServerConfig) -> Result<String, String> {
    let repo = get_mcp_server_repository();
    let server_name = config
        .name
        .as_ref()
        .ok_or_else(|| "Server name is required".to_string())?;

    // 1. Verify the configuration before saving
    let tools =
        crate::services::mcp_server_service::McpServerService::verify_config(config.clone())
            .await
            .map_err(|e| format!("Verification failed: {}", e))?;
    let tools_json_str = crate::mcp::utils::serialize_mcp_tools(&tools);

    let config_value = serde_json::to_value(config).map_err(|e| e.to_string())?;

    // Try to update first (by name lookup), create if doesn't exist
    let id = match repo.get_by_name(server_name).await {
        Ok(Some(existing)) => {
            // Update by ID with new config
            repo.update(&existing.id, None, Some(config_value))
                .await
                .map_err(|e| format!("Failed to update MCP server config: {}", e))?
                .id
        }
        Ok(None) => {
            repo.create(server_name, config_value)
                .await
                .map_err(|e| format!("Failed to create MCP server config: {}", e))?
                .id
        }
        Err(e) => return Err(format!("DB query error: {}", e)),
    };

    // Update the cached tools immediately since we just verified it
    let _ = repo
        .update_cached_tools(&id, tools.len() as i32, tools_json_str)
        .await;

    Ok(id)
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
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) if !n.is_empty() => n.to_string(),
        Some(_) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Server name cannot be empty",
                ToolGroup::McpManager,
            )
            .with_guidance(vec!["Provide a unique name for this MCP server".to_string()])
            .to_mcp_result())
        }
        None => return Ok(missing_param_error("name", ToolGroup::McpManager)),
    };

    if BuiltinServiceId::from_alias(&name).is_some() {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "Server name '{}' is reserved for a builtin service. Choose a different name.",
                name
            ),
            ToolGroup::McpManager,
        )
        .with_guidance(vec!["Use a unique name that doesn't match a builtin service (e.g. planning, browser, workspace)".to_string()])
        .to_mcp_result());
    }

    let transport_val = match args.get("transport") {
        Some(t) => t,
        Option::None => return Ok(missing_param_error("transport", ToolGroup::McpManager)),
    };

    let transport: TransportConfig = match serde_json::from_value(transport_val.clone()) {
        Ok(config) => config,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("Invalid transport config: {}", e),
                ToolGroup::McpManager,
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

    let config = MCPServerConfig {
        name: Some(name.clone()),
        transport,
        authentication: None,
        metadata,
    };

    let id = match save_server_config(&config).await {
        Ok(id) => id,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::DatabaseError,
                format!("Failed to register server: {}", e),
                ToolGroup::McpManager,
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
    server.invalidate_cache().await;

    // Emit resource updated event for frontend cache revalidation
    events::emit_resource_updated("mcpServer", "create", Some(name.to_string()));

    let hint = SuccessHint::new(
        format!(
            "✓ Server configuration saved\n\n• Server Name: {}\n• Server ID: {}\n\nStatus: Configured (ready to use)\n\nExternal servers are managed per-session through MCPServiceProxyManager.",
            name, id
        ),
        vec![
            "Use listTools to view all registered servers".to_string(),
            format!("Use connectServer('{}') to start this server in a session", name),
            "The Server ID is required when configuring an assistant's mcpServerIds".to_string(),
        ],
    );
    Ok(hint.to_mcp_result_with_data(Some(json!({ "name": name, "id": id }))))
}

/// Delete an MCP server
pub async fn delete_server(server: &MCPManagerServer, args: Value) -> Result<MCPResult, String> {
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) if !n.is_empty() => n.to_string(),
        Some(_) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Target name cannot be empty",
                ToolGroup::McpManager,
            )
            .with_guidance(vec!["Specify the name of the server to delete".to_string()])
            .to_mcp_result())
        }
        Option::None => return Ok(missing_param_error("name", ToolGroup::McpManager)),
    };

    // Check if server exists (Hallucination Firewall - Section 3.2)
    if let Ok(Option::None) = get_server_config(&name).await {
        return Ok(not_found_error("Server", &name, ToolGroup::McpManager));
    }

    // Note: Session Isolation means we cannot stop via global manager
    // Servers are managed per-session, not globally

    // Delete config
    if let Err(e) = delete_server_config_db(name.clone()).await {
        return Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Failed to exclude server configuration: {}", e),
            ToolGroup::McpManager,
        )
        .with_guidance(vec![
            "Verify database permissions".to_string(),
            "Use listTools to confirm the name exists".to_string(),
        ])
        .to_mcp_result());
    }

    server.invalidate_cache().await;

    // Emit resource updated event for frontend cache revalidation
    events::emit_resource_updated("mcpServer", "delete", Some(name.to_string()));

    let hint = SuccessHint::new(
        format!("Excluded server '{}' from configuration", name),
        vec!["Use listTools to verify remaining servers".to_string()],
    );
    Ok(hint.to_mcp_result())
}

/// Update an existing MCP server configuration
pub async fn update_server(server: &MCPManagerServer, args: Value) -> Result<MCPResult, String> {
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) if !n.is_empty() => n,
        Some(_) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Target name cannot be empty",
                ToolGroup::McpManager,
            )
            .with_guidance(vec!["Specify the name of the server to update".to_string()])
            .to_mcp_result())
        }
        Option::None => return Ok(missing_param_error("name", ToolGroup::McpManager)),
    };

    if BuiltinServiceId::from_alias(name).is_some() {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "Server name '{}' is reserved for a builtin service. Choose a different name.",
                name
            ),
            ToolGroup::McpManager,
        )
        .with_guidance(vec!["Use a unique name that doesn't match a builtin service (e.g. planning, browser, workspace)".to_string()])
        .to_mcp_result());
    }

    let transport = match args.get("transport") {
        Some(t) => t,
        Option::None => return Ok(missing_param_error("transport", ToolGroup::McpManager)),
    };

    let transport_config: TransportConfig = match serde_json::from_value(transport.clone()) {
        Ok(config) => config,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("Invalid transport config: {}", e),
                ToolGroup::McpManager,
            )
            .with_guidance(vec![
                "Verify the 'transport' object matches the expected schema".to_string(),
            ])
            .to_mcp_result())
        }
    };

    // Check if server exists (Hallucination Firewall - Section 3.2)
    if let Ok(Option::None) = get_server_config(name).await {
        return Ok(not_found_error("Server", name, ToolGroup::McpManager));
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

    // Update config and get ID
    let id = match save_server_config(&config).await {
        Ok(id) => id,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::DatabaseError,
                format!("Failed to update server configuration: {}", e),
                ToolGroup::McpManager,
            )
            .with_guidance(vec!["Check database connectivity".to_string()])
            .to_mcp_result());
        }
    };

    // Note: Session Isolation means we cannot restart via global manager
    // Configuration updates take effect when servers are next started in a session

    server.invalidate_cache().await;

    // Emit resource updated event for frontend cache revalidation
    events::emit_resource_updated("mcpServer", "update", Some(name.to_string()));

    let hint = SuccessHint::new(
        format!("✓ Server configuration updated for '{}' (ID: {})", name, id),
        vec!["Use listTools to verify changes".to_string()],
    );
    Ok(hint.to_mcp_result_with_data(Some(json!({ "name": name, "id": id }))))
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
        Option::None => return Ok(not_found_error("Server", name, ToolGroup::McpManager)),
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
                "✓ Server '{}' (ID: {}) verification successful\n\n\
                Transport: {}\n\
                {}\n\
                Status: Connected and responsive\n\
                Available tools: {} (cached — use listTools to see)\n\
                Connection latency: {}ms\n\n\
                The server is properly configured and ready to use.",
                name, id, transport_type, transport_details, tool_count, latency_ms
            );

            Ok(SuccessHint::new(
                result_text,
                vec!["Server configuration is valid and operational".to_string()],
            )
            .to_mcp_result_with_data(Some(
                json!({ "name": name, "id": id, "toolCount": tool_count }),
            )))
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
                ToolGroup::McpManager,
            )
            .with_guidance(suggestions)
            .to_mcp_result())
        }
    }
}

/// Test server connection by spawning/connecting and calling listTools.
/// Returns `(tool_count, tools_json)` where `tools_json` is a JSON array of
/// `{"name": "...", "description": "..."}` entries for caching.
async fn test_server_connection(
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

/// Unified tool discovery across builtin and external MCP servers.
pub async fn list_tools(args: Value) -> Result<MCPResult, String> {
    use crate::mcp::types::MCPServerConfig;

    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();

    let scope = args.get("scope").and_then(|v| v.as_str()).unwrap_or("all");

    let force_verify = args
        .get("forceVerify")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let include_internal = matches!(scope, "internal" | "all");
    let include_external = matches!(scope, "external" | "all");

    let mut result_sections: Vec<String> = Vec::new();
    let mut total_tools = 0usize;
    let mut sections_added = 0usize;
    let mut found_external_ids: Vec<(String, String)> = Vec::new(); // (name, id)

    // --- Internal (builtin) tools ---
    if include_internal {
        let all_tools = crate::mcp::server::tools::get_all_static_builtin_tools();
        let matched: Vec<_> = all_tools
            .iter()
            .filter(|t| {
                query.is_empty()
                    || t.name.to_lowercase().contains(&query)
                    || t.description.to_lowercase().contains(&query)
            })
            .collect();

        if !matched.is_empty() {
            let lines: Vec<String> = if query.is_empty() {
                // Compact view when no query is specified
                let names: Vec<String> = matched.iter().map(|t| t.name.clone()).collect();
                vec![format!("Tools: {}", names.join(", "))]
            } else {
                // Detailed view
                matched
                    .iter()
                    .map(|t| {
                        // Truncate description to keep output compact
                        let desc = if t.description.len() > 80 {
                            let mut end = 77;
                            while end > 0 && !t.description.is_char_boundary(end) {
                                end -= 1;
                            }
                            format!("{}...", &t.description[..end])
                        } else {
                            t.description.clone()
                        };
                        format!("• {} — {}", t.name, desc)
                    })
                    .collect()
            };

            result_sections.push(format!(
                "## Builtin Tools ({} matched)\n{}",
                matched.len(),
                lines.join("\n")
            ));
            total_tools += matched.len();
            sections_added += 1;
        }
    }

    // --- External (user-registered) tools ---
    if include_external {
        let repo = get_mcp_server_repository();
        let models = match repo.list().await {
            Ok(m) => m,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::DatabaseError,
                    format!("Failed to query MCP server list: {}", e),
                    ToolGroup::McpManager,
                )
                .with_guidance(vec!["Check database connectivity".to_string()])
                .to_mcp_result())
            }
        };

        for model in &models {
            let config_opt: Option<MCPServerConfig> = serde_json::from_str(&model.config).ok();

            // Determine tool source: live (forceVerify) or cached
            let tools_json_str: Option<String> = if force_verify {
                if let Some(ref config) = config_opt {
                    match test_server_connection(config, &model.name).await {
                        Ok((_, json_str)) => Some(json_str),
                        Err(e) => {
                            log::warn!("listTools: live verify failed for '{}': {}", model.name, e);
                            None
                        }
                    }
                } else {
                    None
                }
            } else {
                model.cached_tools.clone()
            };

            let cached_tools: Vec<Value> = tools_json_str
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();

            let server_matches_query =
                query.is_empty() || model.name.to_lowercase().contains(&query);

            let matched_tools: Vec<&Value> = if query.is_empty() {
                cached_tools.iter().collect()
            } else {
                cached_tools
                    .iter()
                    .filter(|t| {
                        let name_match = t["name"]
                            .as_str()
                            .map(|n| n.to_lowercase().contains(&query))
                            .unwrap_or(false);
                        let desc_match = t["description"]
                            .as_str()
                            .map(|d| d.to_lowercase().contains(&query))
                            .unwrap_or(false);
                        name_match || desc_match || server_matches_query
                    })
                    .collect()
            };

            // Display the server if it matches the query, even if no tools matched.
            // If the server matches the query, matched_tools will contain ALL tools of that server due to `|| server_matches_query` above.
            // However, if the server has NO tools cached/available, matched_tools is empty. We still want to show the server.
            let should_display = !matched_tools.is_empty() || server_matches_query;

            if should_display {
                let verify_note = if force_verify { " [live]" } else { " [cached]" };

                let server_desc = config_opt
                    .as_ref()
                    .and_then(|c| c.metadata.as_ref())
                    .and_then(|m| m.description.as_deref())
                    .map(|d| format!("  Description: {}\n", d))
                    .unwrap_or_default();

                let tool_list_str = if matched_tools.is_empty() {
                    if tools_json_str.is_none() {
                        "  (No tools cached. Run with forceVerify=true to discover tools)"
                            .to_string()
                    } else {
                        "  (No tools provided by this server)".to_string()
                    }
                } else if query.is_empty() {
                    // Compact view for tools
                    let names: Vec<String> = matched_tools
                        .iter()
                        .map(|t| t["name"].as_str().unwrap_or("?").to_string())
                        .collect();
                    format!("  Tools: {}", names.join(", "))
                } else {
                    // Detailed view
                    let tool_lines: Vec<String> = matched_tools
                        .iter()
                        .map(|t| {
                            let name = t["name"].as_str().unwrap_or("?");
                            let desc = t["description"].as_str().unwrap_or("");
                            format!("• {} — {}", name, desc)
                        })
                        .collect();
                    tool_lines.join("\n")
                };

                result_sections.push(format!(
                    "## External: {}{} (ID: {})\n{}{}",
                    model.name, verify_note, model.id, server_desc, tool_list_str
                ));
                total_tools += matched_tools.len();
                sections_added += 1;
                found_external_ids.push((model.name.clone(), model.id.clone()));
            }
        }
    }

    if sections_added == 0 {
        let hint_text = if query.is_empty() {
            "No tools found. Use registerServer to add external MCP servers.".to_string()
        } else {
            format!(
                "No tools found matching '{}'. Try a broader query or scope='all'.",
                query
            )
        };
        return Ok(SuccessHint::new(
            hint_text,
            vec![
                "Use scope='all' to search both builtin and external tools".to_string(),
                "Use listTools to browse all available tools".to_string(),
            ],
        )
        .to_mcp_result());
    }

    let header = if query.is_empty() {
        format!("Found {} tools (scope: {}):\n\n", total_tools, scope)
    } else {
        format!(
            "Found {} tools matching '{}' (scope: {}):\n\n",
            total_tools, query, scope
        )
    };

    let body = result_sections.join("\n\n");

    // Build actionable next-step section for external servers
    let external_action = if !found_external_ids.is_empty() {
        let ids_list: Vec<String> = found_external_ids
            .iter()
            .map(|(name, id)| format!("  • {} → \"{}\"", name, id))
            .collect();
        format!(
            "\n\n---\n📌 To attach external server(s) to an assistant:\n\
            Server IDs found:\n{}\n\n\
            To attach, call:\n  updateAssistant(id: \"<assistantId>\", mcpServerIds: [\"<id_1>\", \"...\"])\n\n\
            Use listAssistants to find your assistant ID.",
            ids_list.join("\n")
        )
    } else {
        String::new()
    };

    let mut hints = vec![
        "Builtin tools are always available; external tools must be attached via updateAssistant(mcpServerIds: [...])".to_string(),
    ];
    if !force_verify && include_external {
        hints.push(
            "Use forceVerify=true to get a live tool list from external servers (slower)"
                .to_string(),
        );
    }

    Ok(SuccessHint::new(format!("{}{}{}", header, body, external_action), hints).to_mcp_result())
}
