use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::mcp::builtin::error_guidance::{
    duplicate_error, guided_error, invalid_input_error, not_found_error, ErrorCategory,
    SuccessHint, ToolGroup,
};
use crate::mcp::builtin::service_id::BuiltinServiceId;
use crate::mcp::types::MCPResult;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::AssistantRepository;

use super::super::super::AgentServer;
use super::super::shared::normalize_agent_config_result;
use super::response::{
    build_agent_config_echo_message, build_agent_config_response_data,
    config_needs_tool_configuration, create_agent_follow_ups, create_agent_next_actions,
};

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
            let needs_tools = config_needs_tool_configuration(&config);
            let hint = SuccessHint::new(
                build_agent_config_echo_message(
                    "created successfully",
                    &normalized_name,
                    &id,
                    &config,
                ),
                create_agent_follow_ups(&id, needs_tools),
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
                "agent__createAgent",
                create_agent_next_actions(&id, needs_tools),
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
                    "Active session tool access is fixed at session start; updating a template cannot inject tools into this running session.".to_string(),
                    "If this task requires a different configuration, delegate it using another agent configuration with the required permissions.".to_string(),
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
                    "This updates the agent template only — it does not change tools in any currently running session.".to_string(),
                    "Inspect the configuration details to verify the changes.".to_string(),
                    "Spawn a new session with agent__spawnSession(configId=...) to apply the updated tool access.".to_string(),
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
                "agent__updateAgent",
                vec![json!({
                    "toolName": "agent__listAgents",
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
