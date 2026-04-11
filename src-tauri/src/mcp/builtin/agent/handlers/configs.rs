use serde_json::{json, Value};
use std::sync::Arc;

use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, SuccessHint, ToolGroup};
use crate::mcp::builtin::session_api::utils::build_agent_tool_data;
use crate::mcp::types::MCPResult;
use crate::repositories::mcp_server_repository::MCPServerRepository;

use super::super::formatting::{
    build_server_name_lookup, extract_string_list, format_capability_list,
    format_external_server_refs, resolve_external_server_labels,
};
use super::super::AgentServer;
use super::normalize_agent_config_result;

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
    let result = crate::mcp::builtin::assistant::operations::create_assistant(
        &assistant_server,
        mapped_args,
    )
    .await?;
    Ok(normalize_agent_config_result(
        result,
        "create",
        vec![json!({
            "toolName": "list",
            "reason": "Review the available agent configurations after creating this one.",
            "args": {
                "type": "configs"
            }
        })],
    ))
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
    let result = crate::mcp::builtin::assistant::operations::update_assistant(
        &assistant_server,
        mapped_args,
        caller_session_id,
    )
    .await?;
    Ok(normalize_agent_config_result(
        result,
        "update",
        vec![json!({
            "toolName": "list",
            "reason": "Review the updated agent configurations after this change.",
            "args": {
                "type": "configs"
            }
        })],
    ))
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
        "configs" => list_agent_configs(server, &args).await,
        "sessions" => list_delegated_sessions(caller_session_id).await,
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

async fn list_agent_configs(server: &AgentServer, args: &Value) -> Result<MCPResult, String> {
    use crate::repositories::AssistantRepository;

    let repo = crate::repositories::SqliteAssistantRepository::new(server.get_db().clone());
    let mut agents = repo.list_assistants().await.map_err(|e| e.to_string())?;

    if let Some(query) = args.get("query").and_then(|v| v.as_str()) {
        let q = query.to_lowercase();
        agents
            .retain(|a| a.name.to_lowercase().contains(&q) || a.config.to_lowercase().contains(&q));
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
        let external_labels = resolve_external_server_labels(&external_ids, &server_name_lookup);

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

    let hint = SuccessHint::new(
        text_summary,
        vec!["Use startSession(agentId=\"...\") to delegate work".to_string()],
    );
    let response_message = hint.message.clone();
    let mut response_data = build_agent_tool_data(
        "list",
        "agentConfigCollection",
        None,
        &response_message,
        "success",
        vec![json!({
            "toolName": "startSession",
            "reason": "Start a delegated session with one of the listed agent configurations.",
        })],
    );
    response_data.insert("type".to_string(), Value::String("configs".to_string()));
    response_data.insert("agents".to_string(), Value::Array(results));
    response_data.insert("total".to_string(), json!(total));
    Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
}

async fn list_delegated_sessions(caller_session_id: &str) -> Result<MCPResult, String> {
    let session_repo = crate::state::get_session_repository();
    use crate::repositories::session_repository::SessionRepository;

    let child_ids = match session_repo.get_child_session_ids(caller_session_id).await {
        Ok(ids) => ids,
        Err(_) => {
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
        message.push_str("\n\nActive roster:\n| Name | ID | Status |\n|---|---|---|\n");
        for result in &results {
            let name = result["name"]
                .as_str()
                .unwrap_or("Unnamed")
                .replace("|", "\\|")
                .replace('\n', " ");
            let id = result["id"]
                .as_str()
                .unwrap_or("")
                .replace("|", "\\|")
                .replace('\n', " ");
            let status = result["status"]
                .as_str()
                .unwrap_or("")
                .replace("|", "\\|")
                .replace('\n', " ");

            message.push_str(&format!("| {} | `{}` | {} |\n", name, id, status));
        }
    }

    let hint = SuccessHint::new(
        message,
        vec!["Use checkSession(sessionId) to get results".to_string()],
    );
    let response_message = hint.message.clone();
    let mut response_data = build_agent_tool_data(
        "list",
        "sessionCollection",
        None,
        &response_message,
        "success",
        vec![json!({
            "toolName": "checkSession",
            "reason": "Inspect one of the listed delegated sessions in more detail.",
        })],
    );
    response_data.insert("type".to_string(), Value::String("sessions".to_string()));
    response_data.insert("sessions".to_string(), Value::Array(results));
    response_data.insert("total".to_string(), json!(child_ids.len()));
    Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
}
