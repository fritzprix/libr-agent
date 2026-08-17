use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, SuccessHint, ToolGroup};
use crate::mcp::builtin::utils::load_session_tool_access;
use crate::mcp::types::{MCPResult, MCPServerConfig};
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::state::get_mcp_server_repository;
use serde_json::{json, Value};

use super::verify::test_server_connection;

/// Unified tool discovery across builtin and external MCP servers.
pub async fn list_tools(args: Value, session_id: Option<&str>) -> Result<MCPResult, String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();

    let scope = args.get("scope").and_then(|v| v.as_str()).unwrap_or("all");
    let availability = args
        .get("availability")
        .and_then(|v| v.as_str())
        .unwrap_or("inventory");

    let force_verify = args
        .get("forceVerify")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v.clamp(1, 100))
        .unwrap_or(50);
    let limit = limit.min(usize::MAX as u64) as usize;

    let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0);
    let offset = offset.min(usize::MAX as u64) as usize;

    let include_internal = matches!(scope, "internal" | "all");
    let include_external = matches!(scope, "external" | "all");
    let session_view = availability == "session";
    let access = if session_view {
        load_session_tool_access(session_id).await
    } else {
        load_session_tool_access(None).await
    };

    struct MatchedTool {
        source: String, // "Builtin" or "External: <server_name>"
        name: String,
        description: String,
        status: String,
        external_server: Option<(String, String)>,
    }

    let mut all_matched_tools: Vec<MatchedTool> = Vec::new();

    // --- Internal (builtin) tools ---
    if include_internal {
        for entry in crate::mcp::builtin::service_id::BUILTIN_SERVICE_REGISTRY {
            let all_tools = crate::mcp::server::tools::get_static_tools_for_server(entry.canonical);
            for t in all_tools {
                if query.is_empty()
                    || t.name.to_lowercase().contains(&query)
                    || t.description.to_lowercase().contains(&query)
                {
                    let status = if session_view {
                        let (s, _) = access.builtin_status(entry.canonical);
                        s.to_string()
                    } else {
                        "".to_string()
                    };

                    all_matched_tools.push(MatchedTool {
                        source: "Builtin".to_string(),
                        name: t.name.clone(),
                        description: t.description.clone(),
                        status,
                        external_server: None,
                    });
                }
            }
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
                    ToolGroup::Tool,
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
                            log::warn!("tool::list live verify failed for '{}': {}", model.name, e);
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

            let mut matched_in_server = false;
            for t in &cached_tools {
                let name = t["name"].as_str().unwrap_or("?");
                let desc = t["description"].as_str().unwrap_or("");

                let name_match = name.to_lowercase().contains(&query);
                let desc_match = desc.to_lowercase().contains(&query);

                if query.is_empty() || name_match || desc_match || server_matches_query {
                    let status = if session_view {
                        let (s, _) = access.external_status(&model.id, &model.name);
                        s.to_string()
                    } else {
                        "".to_string()
                    };

                    all_matched_tools.push(MatchedTool {
                        source: format!("External: {}", model.name),
                        name: name.to_string(),
                        description: desc.to_string(),
                        status,
                        external_server: Some((model.name.clone(), model.id.clone())),
                    });
                    matched_in_server = true;
                }
            }

            if !matched_in_server && server_matches_query {
                let status = if session_view {
                    let (s, _) = access.external_status(&model.id, &model.name);
                    s.to_string()
                } else {
                    "".to_string()
                };

                let desc = if tools_json_str.is_none() {
                    "(No tools cached. Run with forceVerify=true to discover tools)"
                } else {
                    "(No tools provided by this server)"
                };

                all_matched_tools.push(MatchedTool {
                    source: format!("External: {}", model.name),
                    name: "-".to_string(),
                    description: desc.to_string(),
                    status,
                    external_server: Some((model.name.clone(), model.id.clone())),
                });
            }
        }
    }

    let total_results = all_matched_tools.len();
    let total_tools = all_matched_tools
        .iter()
        .filter(|tool| tool.name != "-")
        .count();
    let total_server_rows = total_results.saturating_sub(total_tools);

    if total_results == 0 {
        let hint_text = if query.is_empty() {
            "No tools found. Use tool__registerServer to add external MCP servers.".to_string()
        } else {
            format!(
                "No tools found matching '{}'. Try a broader query, scope='all', or availability='inventory'.",
                query
            )
        };
        return Ok(SuccessHint::new(
            hint_text,
            vec![
                "Use scope='all' to search both builtin and external tools".to_string(),
                "Use availability='inventory' to browse platform/server inventory regardless of current session access".to_string(),
                "Use tool__listServers({\"availability\":\"inventory\"}) to browse all available tools".to_string(),
            ],
        )
        .to_mcp_result());
    }

    if offset >= total_results {
        return Ok(SuccessHint::new(
            format!(
                "Offset {} exceeds total results ({}). Try calling again with offset: 0",
                offset, total_results
            ),
            vec!["Reset offset to 0".to_string()],
        )
        .to_mcp_result());
    }

    let paginated_tools: Vec<_> = all_matched_tools
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect();
    let mut visible_external_ids = paginated_tools
        .iter()
        .filter_map(|tool| tool.external_server.clone())
        .collect::<Vec<_>>();
    visible_external_ids.sort();
    visible_external_ids.dedup();

    let result_summary = if total_server_rows > 0 {
        format!(
            "{} tools and {} matching servers without cached tools",
            total_tools, total_server_rows
        )
    } else {
        format!("{} tools", total_tools)
    };

    let header = if query.is_empty() {
        format!(
            "Found {} (scope: {}, availability: {}):\n\n",
            result_summary, scope, availability
        )
    } else {
        format!(
            "Found {} matching '{}' (scope: {}, availability: {}):\n\n",
            result_summary, query, scope, availability
        )
    };

    let mut body =
        String::from("| Source | Tool Name | Status | Description |\n|---|---|---|---|\n");
    for t in &paginated_tools {
        let desc = if t.description.len() > 80 {
            let mut end = 77;
            while end > 0 && !t.description.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...", &t.description[..end])
        } else {
            t.description.clone()
        };
        // Escape pipes and newlines for markdown tables
        let desc = desc.replace("|", "\\|").replace('\n', " ");
        let name = t.name.replace("|", "\\|").replace('\n', " ");
        let source = t.source.replace("|", "\\|").replace('\n', " ");
        let status_str = if t.status.is_empty() {
            "-".to_string()
        } else {
            t.status.replace("|", "\\|").replace('\n', " ")
        };

        body.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            source, name, status_str, desc
        ));
    }

    if offset.saturating_add(limit) < total_results {
        body.push_str(&format!(
            "\n*(Showing {} to {} of {} total results. Call this tool again with offset: {} to see more)*",
            offset + 1,
            offset + paginated_tools.len(),
            total_results,
            offset.saturating_add(limit)
        ));
    }

    let external_action = if !session_view && !visible_external_ids.is_empty() {
        let ids_list: Vec<String> = visible_external_ids
            .iter()
            .map(|(name, id)| format!("  • {} → \"{}\"", name, id))
            .collect();
        format!(
            "\n\n---\n📌 External server IDs (inventory only — not auto-enabled in this session):\n\
            Server IDs found:\n(this page only)\n{}\n\n\
            To attach them to an agent template for future sessions, call:\n  agent__updateAgent(id: \"<agentId>\", externalMcpServers: [\"<id_1>\", \"...\"])\n\n\
            Note: agent__updateAgent cannot add or modify tools in your currently active session. Active session tool access is fixed at session start. Use availability='session' to see what you can call right now; start a new session (or agent__startSession) to run with an updated config.\n\n\
            Use agent__listAgents(type: \"configs\") to find your target agent ID.",
            ids_list.join("\n")
        )
    } else {
        String::new()
    };

    let mut hints = if session_view {
        vec![
            "Session mode shows whether the current session can actually call each tool. Use availability='inventory' to browse registered platform tools regardless of current access.".to_string(),
        ]
    } else {
        vec![
            "Inventory mode lists registered platform tools; listing or verifying a server does not make it callable here. Use availability='session' for tools permitted in the current session.".to_string(),
        ]
    };
    if !force_verify && include_external {
        hints.push(
            "Use forceVerify=true to get a live tool list from external servers (slower)"
                .to_string(),
        );
    }

    let structured_results = paginated_tools
        .iter()
        .map(|tool| {
            json!({
                "source": tool.source,
                "name": tool.name,
                "status": tool.status,
                "description": tool.description,
            })
        })
        .collect::<Vec<_>>();
    let external_servers = visible_external_ids
        .iter()
        .map(|(name, id)| json!({ "name": name, "id": id }))
        .collect::<Vec<_>>();

    Ok(
        SuccessHint::new(format!("{}{}{}", header, body, external_action), hints)
            .to_mcp_result_with_data(Some(json!({
                "query": query,
                "scope": scope,
                "availability": availability,
                "forceVerify": force_verify,
                "offset": offset,
                "limit": limit,
                "totalResults": total_results,
                "totalTools": total_tools,
                "totalServerRows": total_server_rows,
                "results": structured_results,
                "externalServers": external_servers,
            }))),
    )
}
