use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::super::utils::build_agent_tool_data;
use crate::mcp::builtin::error_guidance::{
    duplicate_error, guided_error, invalid_input_error, not_found_error, ErrorCategory,
    SuccessHint, ToolGroup,
};
use crate::mcp::builtin::service_id::BuiltinServiceId;
use crate::mcp::types::MCPResult;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::AssistantRepository;
use sea_orm::DatabaseConnection;

use super::super::formatting::{
    build_server_name_lookup, format_capability_list, format_external_server_refs,
    resolve_external_server_labels,
};
use super::super::AgentServer;
use super::normalize_agent_config_result;

/// Request structure for creating an agent config
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateAgentRequest {
    name: String,
    #[serde(rename = "systemPrompt")]
    system_prompt: Option<String>,
    description: Option<String>,
    #[serde(rename = "allowedBuiltInServiceAliases")]
    allowed_builtin_service_aliases: Option<Vec<BuiltinServiceId>>,
    #[serde(rename = "mcpServerIds")]
    mcp_server_ids: Option<Vec<String>>,
}

/// Request structure for updating an agent config
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateAgentRequest {
    id: String,
    name: Option<String>,
    #[serde(rename = "systemPrompt")]
    system_prompt: Option<String>,
    description: Option<String>,
    #[serde(rename = "allowedBuiltInServiceAliases")]
    allowed_builtin_service_aliases: Option<Vec<BuiltinServiceId>>,
    #[serde(rename = "mcpServerIds")]
    mcp_server_ids: Option<Vec<String>>,
}

struct ConfigMergeParams<'a> {
    base_config: Option<Value>,
    system_prompt: Option<&'a str>,
    description: Option<&'a str>,
    allowed_builtin_service_aliases: Option<&'a Vec<BuiltinServiceId>>,
    mcp_server_ids: Option<&'a Vec<String>>,
}

fn extract_string_array(config: &Value, key: &str) -> Vec<String> {
    config
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn format_summary_list(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

fn build_agent_config_response_data(id: &str, name: &str, config: &Value) -> Value {
    let configured_builtin_capabilities =
        extract_string_array(config, "allowedBuiltInServiceAliases");
    let effective_builtin_capabilities =
        crate::agent::tools::runtime_allowed_builtin_service_aliases_from_value(config);
    let external_mcp_servers = extract_string_array(config, "mcpServerIds");

    json!({
        "success": true,
        "id": id,
        "name": name,
        "description": config.get("description").and_then(Value::as_str),
        "systemPrompt": config.get("systemPrompt").and_then(Value::as_str),
        "builtinCapabilities": effective_builtin_capabilities.clone(),
        "configuredBuiltinCapabilities": configured_builtin_capabilities.clone(),
        "effectiveBuiltinCapabilities": effective_builtin_capabilities,
        "externalMcpServers": external_mcp_servers.clone(),
        "mcpServerIds": external_mcp_servers,
    })
}

fn build_agent_config_echo_message(action: &str, name: &str, id: &str, config: &Value) -> String {
    let configured_builtin_capabilities =
        extract_string_array(config, "allowedBuiltInServiceAliases");
    let effective_builtin_capabilities =
        crate::agent::tools::runtime_allowed_builtin_service_aliases_from_value(config);
    let external_mcp_servers = extract_string_array(config, "mcpServerIds");
    let description = config
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("none");

    format!(
        "Agent configuration '{}' {} (ID: {})\n\nDescription: {}\nConfigured builtin capabilities: {}\nEffective builtin capabilities: {}\nExternal MCP servers: {}",
        name,
        action,
        id,
        description,
        format_summary_list(&configured_builtin_capabilities),
        format_summary_list(&effective_builtin_capabilities),
        format_summary_list(&external_mcp_servers),
    )
}

fn merge_config_from_request(params: ConfigMergeParams<'_>) -> Value {
    let mut config = params.base_config.unwrap_or_else(|| json!({}));

    if let Some(v) = params.system_prompt {
        config["systemPrompt"] = json!(v);
    }
    if let Some(v) = params.description {
        config["description"] = json!(v);
    }

    if let Some(v) = params.allowed_builtin_service_aliases {
        config["allowedBuiltInServiceAliases"] = json!(v);
    }

    if let Some(v) = params.mcp_server_ids {
        config["mcpServerIds"] = json!(v);
    }

    config
}

async fn validate_mcp_server_ids(
    db: &sea_orm::DatabaseConnection,
    server_ids: &[String],
) -> Result<(), String> {
    if server_ids.is_empty() {
        return Ok(());
    }

    let repo = crate::repositories::SqliteMCPServerRepository::new(db.clone());
    let all_servers = repo
        .list()
        .await
        .map_err(|e| format!("Failed to validate MCP server IDs: {}", e))?;

    let existing_ids: std::collections::HashSet<_> =
        all_servers.iter().map(|s| s.id.as_str()).collect();

    let invalid_ids: Vec<_> = server_ids
        .iter()
        .filter(|id| !existing_ids.contains(id.as_str()))
        .collect();

    if !invalid_ids.is_empty() {
        return Err(format!(
            "Invalid MCP server IDs: {}. Use tool__listServers to see available servers with their IDs.",
            invalid_ids
                .iter()
                .map(|id| format!("'{}'", id))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    Ok(())
}

fn trim_optional_text(value: Option<&str>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

async fn get_caller_assistant_id(session_id: &str) -> Result<String, String> {
    let session = crate::get_session_repository()
        .get_session(session_id)
        .await
        .map_err(|e| format!("DB error: {}", e))?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    if let Some(assistant_id) = crate::agent::extract_assistant_id_from_session(&session) {
        return Ok(assistant_id);
    }

    Err("No assistant_id in session config".to_string())
}

/// Unified create_agent handler
pub async fn create_agent(server: &AgentServer, args: Value) -> Result<MCPResult, String> {
    let mut mapped_args = args.clone();

    if let Some(builtins) = args.get("builtinCapabilities") {
        mapped_args["allowedBuiltInServiceAliases"] = builtins.clone();
    }
    if let Some(externals) = args.get("externalMcpServers") {
        mapped_args["mcpServerIds"] = externals.clone();
    }

    let request: CreateAgentRequest = serde_json::from_value(mapped_args).map_err(|e| {
        log::error!("Failed to parse CreateAgentRequest: {}", e);
        format!("Invalid request format: {}", e)
    })?;
    let normalized_name =
        match crate::services::assistant_service::normalize_assistant_name(&request.name) {
            Ok(name) => name,
            Err(err) => return Ok(invalid_input_error(&err, ToolGroup::Agent)),
        };

    let id = uuid::Uuid::new_v4().to_string();
    let repo = crate::repositories::SqliteAssistantRepository::new(server.get_db().clone());

    let exists = repo
        .check_assistant_exists(&normalized_name)
        .await
        .map_err(|e| format!("Failed to check for duplicate name: {}", e))?;

    if exists {
        return Ok(duplicate_error(
            "Assistant",
            &normalized_name,
            ToolGroup::Agent,
        ));
    }

    // Omit / null builtinCapabilities → persist an explicit empty optional list so
    // runtime enables CORE only (not every optional builtin). Explicit [] is the same.
    let builtin_aliases = request.allowed_builtin_service_aliases.unwrap_or_default();

    let config = merge_config_from_request(ConfigMergeParams {
        base_config: None,
        system_prompt: trim_optional_text(request.system_prompt.as_deref()).as_deref(),
        description: trim_optional_text(request.description.as_deref()).as_deref(),
        allowed_builtin_service_aliases: Some(&builtin_aliases),
        mcp_server_ids: request.mcp_server_ids.as_ref(),
    });

    if let Some(server_ids_value) = config.get("mcpServerIds") {
        if let Some(server_ids_array) = server_ids_value.as_array() {
            let server_ids: Vec<String> = server_ids_array
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();

            if let Err(err_msg) = validate_mcp_server_ids(server.get_db(), &server_ids).await {
                return Ok(invalid_input_error(&err_msg, ToolGroup::Agent));
            }
        }
    }

    let config_str = match serde_json::to_string(&config) {
        Ok(s) => s,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("Failed to serialize agent config: {}", e),
                ToolGroup::Agent,
            )
            .with_guidance(vec!["Ensure config fields are valid JSON".to_string()])
            .to_mcp_result());
        }
    };

    match repo
        .create_assistant(id.clone(), normalized_name.clone(), config_str)
        .await
    {
        Ok(_) => {
            let hint = SuccessHint::new(
                build_agent_config_echo_message("created successfully", &normalized_name, &id, &config),
                vec![
                    "List agent configurations to review the new configuration".to_string(),
                    "Update the configuration if you want to refine its prompt or capabilities"
                        .to_string(),
                ],
            );

            crate::agent::tauri_events::emit_resource_updated(
                "assistant",
                "create",
                Some(id.clone()),
            );

            Ok(normalize_agent_config_result(
                hint.to_mcp_result_with_data(Some(build_agent_config_response_data(
                    &id,
                    &normalized_name,
                    &config,
                ))),
                "createAgent",
                vec![json!({
                    "toolName": "listAgents",
                    "reason": "Review the available agent configurations after creating this one.",
                    "args": {
                        "type": "configs"
                    }
                })],
            ))
        }
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Failed to create agent configuration: {}", e),
            ToolGroup::Agent,
        )
        .with_guidance(vec!["Try again".to_string()])
        .to_mcp_result()),
    }
}

/// Unified update_agent handler
pub async fn update_agent(
    server: &AgentServer,
    args: Value,
    caller_session_id: Option<String>,
) -> Result<MCPResult, String> {
    let mut mapped_args = args.clone();

    if let Some(builtins) = args.get("builtinCapabilities") {
        mapped_args["allowedBuiltInServiceAliases"] = builtins.clone();
    }
    if let Some(externals) = args.get("externalMcpServers") {
        mapped_args["mcpServerIds"] = externals.clone();
    }

    let request: UpdateAgentRequest = serde_json::from_value(mapped_args).map_err(|e| {
        log::error!("Failed to parse UpdateAgentRequest: {}", e);
        format!("Invalid request format: {}", e)
    })?;
    let requested_name = match crate::services::assistant_service::normalize_optional_assistant_name(
        request.name.clone(),
    ) {
        Ok(name) => name,
        Err(err) => return Ok(invalid_input_error(&err, ToolGroup::Agent)),
    };

    let repo = crate::repositories::SqliteAssistantRepository::new(server.get_db().clone());

    let existing_model = repo
        .get_assistant(&request.id)
        .await
        .map_err(|e| format!("Failed to fetch assistant: {}", e))?;

    let (mut name, base_config) = if let Some(model) = existing_model {
        let parsed_config = serde_json::from_str::<Value>(&model.config).unwrap_or_else(|e| {
            log::warn!("Failed to parse config for assistant {}: {}", model.id, e);
            json!({})
        });
        (model.name, parsed_config)
    } else {
        return Ok(not_found_error(
            "Agent configuration",
            &request.id,
            ToolGroup::Agent,
        ));
    };

    if let Some(ref sid) = caller_session_id {
        if let Ok(caller_assistant_id) = get_caller_assistant_id(sid).await {
            if caller_assistant_id == request.id {
                return Ok(guided_error(
                    ErrorCategory::PermissionDenied,
                    "Self-modification is not allowed: an agent cannot update the assistant configuration it is currently running as.",
                    ToolGroup::Agent,
                )
                .with_guidance(vec![
                    "This restriction prevents privilege escalation and identity drift during a session.".to_string(),
                    "If this task requires a different configuration, delegate it using another agent configuration with the required permissions."
                        .to_string(),
                    "List available agent configurations, then start a new delegated session with one that can perform the change."
                        .to_string(),
                ])
                .to_mcp_result());
            }
        }
    }

    if let Some(ref n) = requested_name {
        name = n.clone();
    }

    let config = merge_config_from_request(ConfigMergeParams {
        base_config: Some(base_config),
        system_prompt: trim_optional_text(request.system_prompt.as_deref()).as_deref(),
        description: trim_optional_text(request.description.as_deref()).as_deref(),
        allowed_builtin_service_aliases: request.allowed_builtin_service_aliases.as_ref(),
        mcp_server_ids: request.mcp_server_ids.as_ref(),
    });

    if let Some(server_ids_value) = config.get("mcpServerIds") {
        if let Some(server_ids_array) = server_ids_value.as_array() {
            let server_ids: Vec<String> = server_ids_array
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();

            if let Err(err_msg) = validate_mcp_server_ids(server.get_db(), &server_ids).await {
                return Ok(
                    guided_error(ErrorCategory::InvalidInput, err_msg, ToolGroup::Agent)
                        .with_guidance(vec![
                            "Use tool__listServers to see available servers".to_string()
                        ])
                        .to_mcp_result(),
                );
            }
        }
    }

    let config_str = match serde_json::to_string(&config) {
        Ok(s) => s,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("Failed to serialize agent config: {}", e),
                ToolGroup::Agent,
            )
            .with_guidance(vec!["Ensure config fields are valid JSON".to_string()])
            .to_mcp_result());
        }
    };

    let result = repo
        .update_assistant(&request.id, Some(name.clone()), Some(config_str))
        .await;

    match result {
        Ok(_) => {
            let hint = SuccessHint::new(
                build_agent_config_echo_message(
                    "updated successfully",
                    &name,
                    &request.id,
                    &config,
                ),
                vec![
                    "Inspect the configuration details to verify the changes".to_string(),
                    "Start a new delegated session to apply the updated configuration".to_string(),
                ],
            );

            crate::agent::tauri_events::emit_resource_updated(
                "assistant",
                "update",
                Some(request.id.clone()),
            );

            Ok(normalize_agent_config_result(
                hint.to_mcp_result_with_data(Some(build_agent_config_response_data(
                    &request.id,
                    &name,
                    &config,
                ))),
                "updateAgent",
                vec![json!({
                    "toolName": "listAgents",
                    "reason": "Review the updated agent configurations after this change.",
                    "args": {
                        "type": "configs"
                    }
                })],
            ))
        }
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Failed to update agent configuration {}: {}", request.id, e),
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            "Check database connectivity".to_string(),
            "List agent configurations to verify the configuration still exists".to_string(),
        ])
        .to_mcp_result()),
    }
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
        "sessions" => list_delegated_sessions(caller_session_id, &args).await,
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

fn extract_pagination_args(args: &Value) -> (usize, usize) {
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|value| value.max(1) as usize)
        .unwrap_or(20);
    let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

    (limit, offset)
}

fn build_pagination_note(offset: usize, page_len: usize, total: usize, limit: usize) -> String {
    if page_len == 0 {
        return String::new();
    }

    let start = offset + 1;
    let end = offset + page_len;

    if end < total {
        format!(
            "*(Showing {} to {} of {} items. Call this tool again with offset: {} to see more)*\n",
            start,
            end,
            total,
            offset + limit
        )
    } else if offset > 0 {
        format!("*(Showing {} to {} of {} items)*\n", start, end, total)
    } else {
        String::new()
    }
}

async fn list_agent_configs(server: &AgentServer, args: &Value) -> Result<MCPResult, String> {
    list_agent_configs_from_db(server.get_db(), args).await
}

pub async fn list_agent_configs_for_test(
    db: &DatabaseConnection,
    args: &Value,
) -> Result<MCPResult, String> {
    list_agent_configs_from_db(db, args).await
}

async fn list_agent_configs_from_db(
    db: &DatabaseConnection,
    args: &Value,
) -> Result<MCPResult, String> {
    let repo = crate::repositories::SqliteAssistantRepository::new(db.clone());
    let mut agents = repo.list_assistants().await.map_err(|e| e.to_string())?;

    if let Some(query) = args.get("query").and_then(|v| v.as_str()) {
        let q = query.to_lowercase();
        agents
            .retain(|a| a.name.to_lowercase().contains(&q) || a.config.to_lowercase().contains(&q));
    }

    let (limit, offset) = extract_pagination_args(args);
    let verbose = args
        .get("verbose")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    let total = agents.len();
    let paged_agents: Vec<_> = agents.into_iter().skip(offset).take(limit).collect();
    let mcp_repo = crate::state::get_mcp_server_repository();
    let external_servers = mcp_repo.list().await.map_err(|e| e.to_string())?;
    let server_name_lookup = build_server_name_lookup(&external_servers);

    let mut results = Vec::new();
    let mut text_summary = format!("Found {} agent configurations.\n\n", total);
    let mut any_truncated = false;
    if !paged_agents.is_empty() {
        text_summary.push_str("| Name | ID | Capabilities | Servers | Description |\n");
        text_summary.push_str("|---|---|---|---|---|\n");
    } else if total > 0 {
        text_summary.push_str(&format!(
            "No results for this page (offset {}, limit {}). Try a smaller offset.\n",
            offset, limit
        ));
    }

    for agent in paged_agents {
        let parsed_config = serde_json::from_str::<Value>(&agent.config).unwrap_or(Value::Null);
        let effective_builtin_capabilities =
            crate::agent::tools::runtime_allowed_builtin_service_aliases_from_value(&parsed_config);
        let external_mcp_servers = extract_string_array(&parsed_config, "mcpServerIds");

        let mut desc = parsed_config
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        if !verbose && desc.chars().count() > 80 {
            desc = desc.chars().take(77).collect::<String>() + "...";
            any_truncated = true;
        }

        let name_clean = agent.name.replace('|', "\\|");
        let id_clean = agent.id.replace('|', "\\|");
        let cap_clean = format_capability_list(&effective_builtin_capabilities).replace('|', "\\|");
        let server_clean = format_external_server_refs(&external_mcp_servers, &server_name_lookup)
            .replace('|', "\\|");
        let desc_clean = desc.replace('|', "\\|").replace('\n', " ");

        text_summary.push_str(&format!(
            "| {} | `{}` | {} | {} | {} |\n",
            name_clean, id_clean, cap_clean, server_clean, desc_clean
        ));

        let res_capabilities =
            resolve_external_server_labels(&external_mcp_servers, &server_name_lookup);

        results.push(json!({
            "id": agent.id,
            "name": agent.name,
            "description": parsed_config.get("description").and_then(Value::as_str),
            "systemPrompt": parsed_config.get("systemPrompt").and_then(Value::as_str),
            "builtinCapabilities": effective_builtin_capabilities,
            "externalMcpServers": res_capabilities,
        }));
    }

    let pagination_note = build_pagination_note(offset, results.len(), total, limit);
    if !pagination_note.is_empty() {
        text_summary.push('\n');
        text_summary.push_str(&pagination_note);
    }

    if any_truncated {
        text_summary.push_str(
            "\n*(Some descriptions were truncated. Use verbose=true to see full descriptions)*\n",
        );
    }

    let hint = SuccessHint::new(
        text_summary,
        vec![
            "Use agent__startSession to run a delegated task using one of these configurations"
                .to_string(),
        ],
    );

    let response_message = hint.message.clone();
    let mut response_data = build_agent_tool_data(
        "listAgents",
        "agentConfigCollection",
        None,
        &response_message,
        "success",
        vec![json!({
            "toolName": "startSession",
            "reason": "Spawn a new delegated agent session using one of the configurations.",
        })],
    );
    response_data.insert("type".to_string(), Value::String("configs".to_string()));
    response_data.insert("configs".to_string(), Value::Array(results));
    response_data.insert("total".to_string(), json!(total));

    Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
}

pub async fn list_delegated_sessions_for_test(
    caller_session_id: &str,
    args: &Value,
) -> Result<MCPResult, String> {
    list_delegated_sessions(caller_session_id, args).await
}

async fn list_delegated_sessions(
    caller_session_id: &str,
    args: &Value,
) -> Result<MCPResult, String> {
    let session_repo = crate::state::get_session_repository();

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

    let (limit, offset) = extract_pagination_args(args);
    let total = child_ids.len();
    let paged_child_ids: Vec<_> = child_ids.into_iter().skip(offset).take(limit).collect();

    let mut paged_results = Vec::new();
    for child_id in paged_child_ids {
        if let Ok(Some(child_data)) = session_repo.get_session(&child_id).await {
            let status = format!("{:?}", child_data.status).to_lowercase();
            paged_results.push(json!({
                "id": child_id,
                "name": child_data.name.unwrap_or_else(|| "Unnamed".to_string()),
                "status": status
            }));
        }
    }

    let mut message = format!("Found {} sub-agent sessions.\n\n", total);
    if !paged_results.is_empty() {
        message.push_str("| Name | Session ID | Status |\n");
        message.push_str("|---|---|---|\n");
        for result in &paged_results {
            let name_clean = result["name"]
                .as_str()
                .unwrap_or("")
                .replace('|', "\\|")
                .replace('\n', " ");
            let id_clean = result["id"]
                .as_str()
                .unwrap_or("")
                .replace('|', "\\|")
                .replace('\n', " ");
            let status_clean = result["status"]
                .as_str()
                .unwrap_or("")
                .replace('|', "\\|")
                .replace('\n', " ");
            message.push_str(&format!(
                "| {} | `{}` | {} |\n",
                name_clean, id_clean, status_clean
            ));
        }
    } else if total > 0 {
        message.push_str(&format!(
            "No results for this page (offset {}, limit {}). Try a smaller offset.\n",
            offset, limit
        ));
    }

    let pagination_note = build_pagination_note(offset, paged_results.len(), total, limit);
    if !pagination_note.is_empty() {
        message.push('\n');
        message.push_str(&pagination_note);
    }

    let hint = SuccessHint::new(
        message,
        vec!["Use checkSession(sessionId) to get results".to_string()],
    );
    let response_message = hint.message.clone();
    let mut response_data = build_agent_tool_data(
        "listAgents",
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
    response_data.insert("sessions".to_string(), Value::Array(paged_results));
    response_data.insert("total".to_string(), json!(total));
    Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
}
