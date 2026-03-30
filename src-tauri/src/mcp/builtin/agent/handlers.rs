use serde_json::{json, Value};
use std::sync::Arc;

use crate::mcp::builtin::error_guidance::{
    guided_error, missing_agent_config_error, missing_agent_session_error, ErrorCategory,
    SuccessHint, ToolGroup,
};
use crate::mcp::builtin::session_api::formatting::{
    extract_session_status, latest_assistant_message_text,
};
use crate::mcp::builtin::session_api::utils::{
    count_session_turns, handle_wait_timeout_result, read_required_string,
    wait_until_session_terminal,
};
use crate::mcp::types::MCPResult;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::repositories::message_repository::MessageRepository;

use super::formatting::{
    build_server_name_lookup, extract_string_list, format_capability_list,
    format_external_server_refs, format_registered_external_servers,
    resolve_external_server_labels,
};
use super::AgentServer;

/// Unified create_agent handler (from createAssistant)
pub async fn create_agent(server: &AgentServer, args: Value) -> Result<MCPResult, String> {
    let mut mapped_args = args.clone();

    // Map Agent Domain friendly names to underlying config fields
    if let Some(builtins) = args.get("builtinCapabilities") {
        mapped_args["allowedBuiltInServiceAliases"] = builtins.clone();
    }
    if let Some(externals) = args.get("externalMcpServers") {
        mapped_args["mcpServerIds"] = externals.clone();
    }

    let assistant_server =
        crate::mcp::builtin::assistant::AssistantServer::new(Arc::new(server.get_db().clone()))
            .await?;
    crate::mcp::builtin::assistant::operations::create_assistant(&assistant_server, mapped_args)
        .await
}

/// Unified update_agent handler (from updateAssistant)
pub async fn update_agent(
    server: &AgentServer,
    args: Value,
    caller_session_id: Option<String>,
) -> Result<MCPResult, String> {
    let mut mapped_args = args.clone();

    // Map Agent Domain friendly names to underlying config fields
    if let Some(builtins) = args.get("builtinCapabilities") {
        mapped_args["allowedBuiltInServiceAliases"] = builtins.clone();
    }
    if let Some(externals) = args.get("externalMcpServers") {
        mapped_args["mcpServerIds"] = externals.clone();
    }

    let assistant_server =
        crate::mcp::builtin::assistant::AssistantServer::new(Arc::new(server.get_db().clone()))
            .await?;
    crate::mcp::builtin::assistant::operations::update_assistant(
        &assistant_server,
        mapped_args,
        caller_session_id,
    )
    .await
}

/// Unified list handler: lists configs or sub-sessions
pub async fn list_agents_or_sessions(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let list_type = args
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("configs");

    match list_type {
        "configs" => {
            use crate::repositories::AssistantRepository;
            let repo = crate::repositories::SqliteAssistantRepository::new(server.get_db().clone());
            let mut agents = repo.list_assistants().await.map_err(|e| e.to_string())?;

            // Filter by query if provided
            if let Some(query) = args.get("query").and_then(|v| v.as_str()) {
                let q = query.to_lowercase();
                agents.retain(|a| {
                    a.name.to_lowercase().contains(&q) || a.config.to_lowercase().contains(&q)
                });
            }

            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
            let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

            let total = agents.len();
            let paged_agents: Vec<_> = agents.into_iter().skip(offset).take(limit).collect();
            let mcp_repo = crate::state::get_mcp_server_repository();
            let external_servers = mcp_repo.list().await.map_err(|e| e.to_string())?;
            let server_name_lookup = build_server_name_lookup(&external_servers);

            let mut results = Vec::new();
            let mut text_summary = format!("Found {} agent configurations.\n\n", total);

            for agent in paged_agents {
                let config: Value = serde_json::from_str(&agent.config).unwrap_or_default();
                let desc = config
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No description");
                let builtins = extract_string_list(config.get("allowedBuiltInServiceAliases"));
                let external_ids = extract_string_list(config.get("mcpServerIds"));
                let external_labels =
                    resolve_external_server_labels(&external_ids, &server_name_lookup);

                text_summary.push_str(&format!(
                    "- **{}** (ID: `{}`)\n  Description: {}\n  Builtin Capabilities: {}\n  External MCP Servers: {}\n\n",
                    agent.name,
                    agent.id,
                    desc,
                    format_capability_list(&builtins),
                    format_external_server_refs(&external_ids, &server_name_lookup)
                ));

                results.push(json!({
                    "id": agent.id,
                    "name": agent.name,
                    "description": desc,
                    "builtinCapabilities": builtins,
                    "externalMcpServers": external_ids,
                    "externalMcpServerLabels": external_labels
                }));
            }

            // Add system capability catalog to summary
            use crate::mcp::builtin::service_id::BUILTIN_SERVICE_REGISTRY;
            let available_builtins: Vec<String> = BUILTIN_SERVICE_REGISTRY
                .iter()
                .filter(|e| {
                    !e.canonical.is_empty() && e.canonical != "agent" && e.canonical != "tool"
                })
                .map(|e| e.canonical.to_string())
                .collect();

            text_summary.push_str("\n--- \n## System Capability Catalog\n");
            text_summary.push_str(&format!(
                "Available Builtins: {}\n",
                format_capability_list(&available_builtins)
            ));
            text_summary.push_str(&format!(
                "Available External MCPs:\n{}\n",
                format_registered_external_servers(&external_servers)
            ));

            let hint = SuccessHint::new(
                text_summary,
                vec!["Use startSession(agentId=\"...\") to delegate work".to_string()],
            );
            Ok(hint.to_mcp_result_with_data(Some(json!({ "agents": results, "total": total }))))
        }
        "sessions" => {
            // Logic from getChildAgents
            let session_repo = crate::state::get_session_repository();
            use crate::repositories::session_repository::SessionRepository;

            let child_ids = match session_repo.get_child_session_ids(caller_session_id).await {
                Ok(ids) => ids,
                Err(_) => {
                    // Fallback to lineage store if DB fails or doesn't have the relationship yet
                    let store = crate::services::agent_service::lineage_store().read().await;
                    store
                        .iter()
                        .filter_map(|(id, meta)| {
                            if meta.parent_session_id.as_deref() == Some(caller_session_id) {
                                Some(id.clone())
                            } else {
                                None
                            }
                        })
                        .collect()
                }
            };

            let mut results = Vec::new();
            for child_id in &child_ids {
                if let Ok(Some(child_data)) = session_repo.get_session(child_id).await {
                    let status = format!("{:?}", child_data.status).to_lowercase();
                    results.push(json!({
                        "id": child_id,
                        "name": child_data.name.unwrap_or_else(|| "Unnamed".to_string()),
                        "status": status
                    }));
                }
            }

            let mut message = format!("Found {} sub-agent sessions.", results.len());
            if !results.is_empty() {
                message.push_str("\n\nActive roster:\n");
                for r in &results {
                    message.push_str(&format!(
                        "- {} (ID: {}) status={}\n",
                        r["name"], r["id"], r["status"]
                    ));
                }
            }

            let hint = SuccessHint::new(
                message,
                vec!["Use checkSession(sessionId) to get results".to_string()],
            );
            Ok(hint.to_mcp_result_with_data(Some(json!({ "sessions": results }))))
        }
        _ => Ok(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "Invalid list type '{}'. Use 'configs' or 'sessions'.",
                list_type
            ),
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            "Use list(type=\"configs\") to see agent configurations".to_string(),
            "Use list(type=\"sessions\") to inspect delegated sub-agent sessions".to_string(),
        ])
        .to_mcp_result()),
    }
}

/// startSession handler (from spawnAgent)
pub async fn start_session(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;

    let body: crate::agent::types::CreateSessionRequest = serde_json::from_value(json!({
        "parentSessionId": caller_session_id,
        "assistantId": read_required_string(&args, "agentId")?,
        "request": read_required_string(&args, "task")?,
        "workspacePath": args.get("workspaceOverride").and_then(|v| v.as_str()),
        "maxDepth": args.get("maxDepth").and_then(|v| v.as_u64()),
        "maxFanout": args.get("maxFanout").and_then(|v| v.as_u64()),
    }))
    .map_err(|e| format!("Invalid arguments for start_session: {}", e))?;

    let wait_for_result = args
        .get("waitForResult")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let response = match crate::services::AgentService::spawn_agent(manager, body).await {
        Ok(res) => res,
        Err(err) if err.contains("Assistant not found:") => {
            let agent_id = args
                .get("agentId")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            return Ok(missing_agent_config_error(agent_id));
        }
        Err(err) => return Err(err),
    };

    let session_id = response.id;

    if wait_for_result {
        return check_session(
            server,
            json!({ "sessionId": session_id, "wait": true }),
            caller_session_id,
        )
        .await;
    }

    let hint = SuccessHint::new(
        format!("Session started successfully (ID: {}).", session_id),
        vec![format!(
            "Use checkSession(\"{}\", wait=true) to wait for the answer.",
            session_id
        )],
    );

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "sessionId": session_id,
        "status": "started"
    }))))
}

/// messageToSession handler (from messageAgent)
pub async fn message_to_session(
    server: &AgentServer,
    args: Value,
    _caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let session_id = read_required_string(&args, "sessionId")?;
    let message_text = read_required_string(&args, "message")?;
    let response = match crate::services::AgentService::send_message_to_session(
        manager,
        &session_id,
        message_text,
        Some("agent_tool".to_string()),
    )
    .await
    {
        Ok(response) => response,
        Err(err) if err.contains("Session not found:") => {
            return Ok(missing_agent_session_error(&session_id));
        }
        Err(err) => return Err(err),
    };

    let hint = SuccessHint::new(
        format!("Message {} for session {}.", response.status, session_id),
        vec![format!(
            "Use checkSession(\"{}\", wait=true) to see the response.",
            session_id
        )],
    );

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "sessionId": session_id,
        "messageId": response.message_id,
        "status": response.status
    }))))
}

/// checkSession handler (from awaitAgent / getAgentStatus)
pub async fn check_session(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let session_id = read_required_string(&args, "sessionId")?;
    let wait = args.get("wait").and_then(|v| v.as_bool()).unwrap_or(false);
    let timeout_secs = args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(3600);

    if wait {
        let wait_result = {
            let gate = crate::state::get_concurrency_gate();
            gate.suspend_agent().await?;
            let res = wait_until_session_terminal(
                manager,
                &session_id,
                timeout_secs,
                Some(caller_session_id),
            )
            .await;
            gate.resume_agent().await?;
            res
        };

        let (session_data, _) =
            match handle_wait_timeout_result(wait_result, &session_id, timeout_secs, false) {
                Ok(res) => res,
                Err(mcp_res) => return mcp_res,
            };

        let status = extract_session_status(&session_data);
        let turn_count = count_session_turns(&session_id).await;

        // Fetch latest messages from DB directly
        let repo = crate::state::get_message_repository();
        let messages = repo
            .get_messages_by_session(&session_id, 5)
            .await
            .map_err(|e| format!("Failed to fetch session messages: {}", e))?;

        // Convert messages to Value for formatting functions
        let messages_value: Vec<Value> = messages
            .into_iter()
            .map(|m| serde_json::to_value(m).unwrap_or_default())
            .collect();

        let (_, mut assistant_text) = latest_assistant_message_text(&messages_value, None)
            .unwrap_or(("none".to_string(), "No final answer yet.".to_string()));

        if assistant_text == "[assistant message has no text content]" {
            if let Some(tool_text) =
                crate::mcp::builtin::session_api::formatting::latest_tool_message_text(
                    &messages_value,
                )
            {
                assistant_text = format!("[Tool Response Fallback]\n{}", tool_text);
            }
        }

        let hint = SuccessHint::new(
            format!(
                "Session {} is terminal ({}).\n\nResult:\n{}",
                session_id, status, assistant_text
            ),
            vec![],
        );

        return Ok(hint.to_mcp_result_with_data(Some(json!({
            "sessionId": session_id,
            "status": status,
            "turnCount": turn_count,
            "result": assistant_text
        }))));
    }

    // Just check status via manager
    let session_meta = manager
        .get_session(&session_id)
        .await?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    let status = format!("{:?}", session_meta.status).to_lowercase();
    let turn_count = count_session_turns(&session_id).await;

    let hint = SuccessHint::new(
        format!(
            "Session {} is currently {} (Turns elapsed: {}).",
            session_id, status, turn_count
        ),
        if status != "finished" && status != "error" {
            vec![format!(
                "Use checkSession(\"{}\", wait=true) to wait for completion.",
                session_id
            )]
        } else {
            vec![]
        },
    );

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "sessionId": session_id,
        "status": status,
        "turnCount": turn_count
    }))))
}

/// stopSession handler (from terminateAgent)
pub async fn stop_session(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let session_id = read_required_string(&args, "sessionId")?;

    if caller_session_id == session_id {
        return Ok(guided_error(
            ErrorCategory::InvalidState,
            "Self-termination is not allowed via stopSession.",
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            "Use stopSession only for child or delegated sessions".to_string(),
            "If the current workflow should stop, use the normal session cancellation controls instead"
                .to_string(),
        ])
        .to_mcp_result());
    }

    manager
        .terminate_session(session_id.clone())
        .await
        .map_err(|e| {
            if e.contains("not found") {
                format!("Session not found: {}", session_id)
            } else {
                e
            }
        })?;

    // Also remove from lineage store if present
    crate::services::agent_service::lineage_store()
        .write()
        .await
        .remove(&session_id);

    let hint = SuccessHint::new(format!("Session {} stopped.", session_id), vec![]);

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "sessionId": session_id,
        "stopped": true
    }))))
}
