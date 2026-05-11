use crate::mcp::builtin::error_guidance::{
    duplicate_error, guided_error, invalid_input_error, not_found_error, ErrorCategory,
    SuccessHint, ToolGroup,
};
use crate::mcp::builtin::service_id::BuiltinServiceId;
use crate::mcp::types::MCPResult;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::{AssistantRepository, MCPServerRepository};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::AssistantServer;

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
        "temperature": config.get("temperature").and_then(Value::as_f64),
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
    let temperature = config
        .get("temperature")
        .and_then(Value::as_f64)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "provider default".to_string());

    format!(
        "Agent configuration '{}' {} (ID: {})\n\nDescription: {}\nTemperature: {}\nConfigured builtin capabilities: {}\nEffective builtin capabilities: {}\nExternal MCP servers: {}",
        name,
        action,
        id,
        description,
        temperature,
        format_summary_list(&configured_builtin_capabilities),
        format_summary_list(&effective_builtin_capabilities),
        format_summary_list(&external_mcp_servers),
    )
}

/// Request structure for creating an assistant
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAssistantRequest {
    pub name: String,
    #[serde(rename = "systemPrompt")]
    pub system_prompt: Option<String>,
    pub description: Option<String>,
    pub temperature: Option<f32>,
    #[serde(rename = "allowedBuiltInServiceAliases")]
    pub allowed_builtin_service_aliases: Option<Vec<BuiltinServiceId>>,
    #[serde(rename = "mcpServerIds")]
    pub mcp_server_ids: Option<Vec<String>>,
    // Legacy v2 fields
    pub tools: Option<Vec<String>>,
    #[serde(rename = "mcpServers")]
    pub mcp_servers: Option<Vec<String>>,
    // Nested config object (deprecated pattern)
    pub config: Option<Value>,
}

/// Request structure for updating an assistant
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAssistantRequest {
    pub id: String,
    pub name: Option<String>,
    #[serde(rename = "systemPrompt")]
    pub system_prompt: Option<String>,
    pub description: Option<String>,
    pub temperature: Option<f32>,
    #[serde(rename = "allowedBuiltInServiceAliases")]
    pub allowed_builtin_service_aliases: Option<Vec<BuiltinServiceId>>,
    #[serde(rename = "mcpServerIds")]
    pub mcp_server_ids: Option<Vec<String>>,
    // Legacy v2 fields
    pub tools: Option<Vec<String>>,
    #[serde(rename = "mcpServers")]
    pub mcp_servers: Option<Vec<String>>,
    // Nested config object (deprecated pattern)
    pub config: Option<Value>,
}

/// Request structure for deleting an assistant
#[derive(Debug, Deserialize, Serialize)]
pub struct DeleteAssistantRequest {
    pub id: String,
}

/// Parameters for merging config fields from request
/// Using struct to avoid Clippy warning about too many arguments
struct ConfigMergeParams<'a> {
    base_config: Option<Value>,
    system_prompt: Option<&'a str>,
    description: Option<&'a str>,
    temperature: Option<f32>,
    allowed_builtin_service_aliases: Option<&'a Vec<BuiltinServiceId>>,
    mcp_server_ids: Option<&'a Vec<String>>,
    tools: Option<&'a Vec<String>>,
    mcp_servers: Option<&'a Vec<String>>,
}

/// Merge config from request fields into JSON object.
/// Handles both flat fields and nested config, with legacy v2 field mapping.
///
/// Note: `allowedBuiltInServiceAliases` IS merged here. The self-modification
/// guard in `update_assistant` / `delete_assistant` is the actual privilege
/// boundary — an agent can set this field on OTHER assistants (creation /
/// therapy), just not on itself.
fn merge_config_from_request(params: ConfigMergeParams<'_>) -> Value {
    let mut config = params.base_config.unwrap_or_else(|| json!({}));

    if let Some(v) = params.system_prompt {
        config["systemPrompt"] = json!(v);
    }
    if let Some(v) = params.description {
        config["description"] = json!(v);
    }
    if let Some(v) = params.temperature {
        config["temperature"] = json!(v);
    }

    // Handle tools (v2 legacy) -> allowedBuiltInServiceAliases
    if let Some(v) = params.tools {
        config["allowedBuiltInServiceAliases"] = json!(v);
    }
    // allowedBuiltInServiceAliases (v1) takes precedence
    if let Some(v) = params.allowed_builtin_service_aliases {
        config["allowedBuiltInServiceAliases"] = json!(v);
    }

    // Handle mcpServers (v2 legacy) and mcpServerIds (v1)
    if let Some(v) = params.mcp_servers {
        config["mcpServerIds"] = json!(v);
    }
    // mcpServerIds (v1) takes precedence
    if let Some(v) = params.mcp_server_ids {
        config["mcpServerIds"] = json!(v);
    }

    config
}

/// Validate that all mcpServerIds exist in the mcp_servers table
async fn validate_mcp_server_ids(
    db: &sea_orm::DatabaseConnection,
    server_ids: &[String],
) -> Result<(), String> {
    if server_ids.is_empty() {
        return Ok(()); // Empty list is valid
    }

    // Query database to check which IDs exist
    let repo = crate::repositories::SqliteMCPServerRepository::new(db.clone());
    let all_servers = repo
        .list()
        .await
        .map_err(|e| format!("Failed to validate MCP server IDs: {}", e))?;

    // Build set of valid server IDs
    let existing_ids: std::collections::HashSet<_> =
        all_servers.iter().map(|s| s.id.as_str()).collect();

    // Find invalid IDs
    let invalid_ids: Vec<_> = server_ids
        .iter()
        .filter(|id| !existing_ids.contains(id.as_str()))
        .collect();

    if !invalid_ids.is_empty() {
        return Err(format!(
            "Invalid MCP server IDs: {}. Use tool__list to see available servers with their IDs.",
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

/// Create a new assistant
pub async fn create_assistant(server: &AssistantServer, args: Value) -> Result<MCPResult, String> {
    // Parse request with type safety
    let request: CreateAssistantRequest = serde_json::from_value(args).map_err(|e| {
        log::error!("Failed to parse CreateAssistantRequest: {}", e);
        format!("Invalid request format: {}", e)
    })?;
    let normalized_name =
        match crate::services::assistant_service::normalize_assistant_name(&request.name) {
            Ok(name) => name,
            Err(err) => return Ok(invalid_input_error(&err, ToolGroup::Agent)),
        };

    // Always auto-generate ID
    let id = uuid::Uuid::new_v4().to_string();

    // Use repository from server's db connection instead of global state
    let repo = crate::repositories::SqliteAssistantRepository::new(server.get_db().clone());

    // Check for duplicate name BEFORE attempting insert
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

    // Merge config from all possible sources using helper function
    let config = merge_config_from_request(ConfigMergeParams {
        base_config: request.config,
        system_prompt: trim_optional_text(request.system_prompt.as_deref()).as_deref(),
        description: trim_optional_text(request.description.as_deref()).as_deref(),
        temperature: request.temperature,
        allowed_builtin_service_aliases: request.allowed_builtin_service_aliases.as_ref(),
        mcp_server_ids: request.mcp_server_ids.as_ref(),
        tools: request.tools.as_ref(),
        mcp_servers: request.mcp_servers.as_ref(),
    });

    // Validate mcpServerIds if provided
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

    // Validate config is a valid JSON object
    let config_str = match serde_json::to_string(&config) {
        Ok(s) => s,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("Failed to serialize assistant config: {}", e),
                ToolGroup::Agent,
            )
            .with_guidance(vec!["Ensure config fields are valid JSON".to_string()])
            .to_mcp_result());
        }
    };

    // Use common logic for creation (using repo)
    match repo
        .create_assistant(id.clone(), normalized_name.clone(), config_str)
        .await
    {
        Ok(_) => {
            let hint = SuccessHint::new(
                build_agent_config_echo_message("created successfully", &normalized_name, &id, &config),
                vec![
                    "List agent configurations to review the new configuration".to_string(),
                    "Update the configuration if you want to refine its prompt, temperature, or capabilities"
                        .to_string(),
                ],
            );

            server.invalidate_cache().await;

            // Emit resource updated event for frontend cache revalidation
            crate::agent::tauri_events::emit_resource_updated(
                "assistant",
                "create",
                Some(id.clone()),
            );

            Ok(
                hint.to_mcp_result_with_data(Some(build_agent_config_response_data(
                    &id,
                    &normalized_name,
                    &config,
                ))),
            )
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

/// Update an existing assistant
pub async fn update_assistant(
    server: &AssistantServer,
    args: Value,
    caller_session_id: Option<String>,
) -> Result<MCPResult, String> {
    // Parse request with type safety
    let request: UpdateAssistantRequest = serde_json::from_value(args).map_err(|e| {
        log::error!("Failed to parse UpdateAssistantRequest: {}", e);
        format!("Invalid request format: {}", e)
    })?;
    let requested_name = match crate::services::assistant_service::normalize_optional_assistant_name(
        request.name.clone(),
    ) {
        Ok(name) => name,
        Err(err) => return Ok(invalid_input_error(&err, ToolGroup::Agent)),
    };

    // Use repository from server's db connection
    let repo = crate::repositories::SqliteAssistantRepository::new(server.get_db().clone());

    // Fetch existing assistant to merge config
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

    // Self-modification guard: an AI agent must not modify the assistant it is
    // currently running as. That would let it rewrite its own identity and
    // constraints mid-session.
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

    // Update name if provided
    if let Some(ref n) = requested_name {
        name = n.clone();
    }

    // Merge config from all sources, starting with base config
    let mut config = merge_config_from_request(ConfigMergeParams {
        base_config: Some(base_config),
        system_prompt: trim_optional_text(request.system_prompt.as_deref()).as_deref(),
        description: trim_optional_text(request.description.as_deref()).as_deref(),
        temperature: request.temperature,
        allowed_builtin_service_aliases: request.allowed_builtin_service_aliases.as_ref(),
        mcp_server_ids: request.mcp_server_ids.as_ref(),
        tools: request.tools.as_ref(),
        mcp_servers: request.mcp_servers.as_ref(),
    });

    // Merge nested config object if provided (deprecated pattern)
    if let Some(c) = request.config.as_ref().and_then(|v| v.as_object()) {
        for (k, v) in c {
            config[k] = v.clone();
        }
    }

    // Validate mcpServerIds if provided
    if let Some(server_ids_value) = config.get("mcpServerIds") {
        if let Some(server_ids_array) = server_ids_value.as_array() {
            let server_ids: Vec<String> = server_ids_array
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();

            if let Err(err_msg) = validate_mcp_server_ids(server.get_db(), &server_ids).await {
                return Ok(
                    guided_error(ErrorCategory::InvalidInput, err_msg, ToolGroup::Agent)
                        .with_guidance(vec!["Use tool__list to see available servers".to_string()])
                        .to_mcp_result(),
                );
            }
        }
    }

    // Validate config is a valid JSON object
    let config_str = match serde_json::to_string(&config) {
        Ok(s) => s,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("Failed to serialize assistant config: {}", e),
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

            server.invalidate_cache().await;

            // Emit resource updated event for frontend cache revalidation
            crate::agent::tauri_events::emit_resource_updated(
                "assistant",
                "update",
                Some(request.id.clone()),
            );

            Ok(
                hint.to_mcp_result_with_data(Some(build_agent_config_response_data(
                    &request.id,
                    &name,
                    &config,
                ))),
            )
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

/// Look up which assistant ID is associated with the given session.
///
/// Used by the self-modification and self-deletion guards. Returns an error
/// if the session doesn't exist or has no assistant binding — callers treat
/// that as "no guard needed" (fail-open is intentional: a session without a
/// known assistant_id should not be blocked).
async fn get_caller_assistant_id(session_id: &str) -> Result<String, String> {
    let session = crate::get_session_repository()
        .get_session(session_id)
        .await
        .map_err(|e| format!("DB error: {}", e))?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    let config_str = session
        .agent_config
        .ok_or_else(|| "Session has no agent_config".to_string())?;

    let config: serde_json::Value =
        serde_json::from_str(&config_str).map_err(|e| format!("Invalid config JSON: {}", e))?;

    config
        .get("assistant_id")
        .or_else(|| config.get("assistantId"))
        .or_else(|| config.get("id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "No assistant_id in session config".to_string())
}
/// Delete an assistant
pub async fn delete_assistant(
    server: &AssistantServer,
    args: Value,
    caller_session_id: Option<String>,
) -> Result<MCPResult, String> {
    // Parse request with type safety
    let request: DeleteAssistantRequest = serde_json::from_value(args).map_err(|e| {
        log::error!("Failed to parse DeleteAssistantRequest: {}", e);
        format!("Invalid request format: {}", e)
    })?;

    // Use repository from server's db connection
    let repo = crate::repositories::SqliteAssistantRepository::new(server.get_db().clone());

    // Self-deletion guard: an AI agent must not delete the assistant it is running as.
    if let Some(ref sid) = caller_session_id {
        if let Ok(caller_assistant_id) = get_caller_assistant_id(sid).await {
            if caller_assistant_id == request.id {
                return Ok(guided_error(
                    ErrorCategory::PermissionDenied,
                    "Self-deletion is not allowed: an agent cannot delete the assistant configuration it is currently running as.",
                    ToolGroup::Agent,
                )
                .with_guidance(vec![
                    "This restriction prevents an agent from removing its own identity during an active session.".to_string(),
                    "If this task genuinely requires deletion, delegate it using a different agent configuration with the required permissions."
                        .to_string(),
                    "List available agent configurations, then start a delegated session with one that can perform the deletion."
                        .to_string(),
                ])
                .to_mcp_result());
            }
        }
    }

    // Hallucination Firewall: Check existence first
    let exists = repo
        .get_assistant(&request.id)
        .await
        .unwrap_or(None)
        .is_some();

    if !exists {
        return Ok(guided_error(
            ErrorCategory::ResourceNotFound,
            format!("Agent configuration '{}' not found", request.id),
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            "List agent configurations to find the correct ID".to_string()
        ])
        .to_mcp_result());
    }

    let result = repo.delete_assistant(&request.id).await;

    match result {
        Ok(_) => {
            server.invalidate_cache().await;

            // Emit resource updated event for frontend cache revalidation
            crate::agent::tauri_events::emit_resource_updated(
                "assistant",
                "delete",
                Some(request.id.clone()),
            );

            let hint = SuccessHint::new(
                format!("Agent configuration '{}' deleted successfully", request.id),
                vec!["List agent configurations to review the remaining entries".to_string()],
            );

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "success": true,
                "id": request.id
            }))))
        }
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Failed to delete agent configuration {}: {}", request.id, e),
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            "Check database connectivity".to_string(),
            "Ensure the assistant is not in use".to_string(),
        ])
        .to_mcp_result()),
    }
}
