use crate::agent::events;
use crate::mcp::builtin::error_guidance::{
    duplicate_error, invalid_input_error, not_found_error, operation_failed_error, SuccessHint,
    ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::repositories::{AssistantRepository, MCPServerRepository};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::AssistantServer;

/// Request structure for creating an assistant
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAssistantRequest {
    pub name: String,
    #[serde(rename = "systemPrompt")]
    pub system_prompt: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "modelProvider")]
    pub model_provider: Option<String>,
    #[serde(rename = "modelName")]
    pub model_name: Option<String>,
    pub temperature: Option<f64>,
    #[serde(rename = "maxTokens")]
    pub max_tokens: Option<i64>,
    #[serde(rename = "allowedBuiltInServiceAliases")]
    pub allowed_builtin_service_aliases: Option<Vec<String>>,
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
    #[serde(rename = "modelProvider")]
    pub model_provider: Option<String>,
    #[serde(rename = "modelName")]
    pub model_name: Option<String>,
    pub temperature: Option<f64>,
    #[serde(rename = "maxTokens")]
    pub max_tokens: Option<i64>,
    #[serde(rename = "allowedBuiltInServiceAliases")]
    pub allowed_builtin_service_aliases: Option<Vec<String>>,
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
    model_provider: Option<&'a str>,
    model_name: Option<&'a str>,
    temperature: Option<f64>,
    max_tokens: Option<i64>,
    allowed_builtin_service_aliases: Option<&'a Vec<String>>,
    mcp_server_ids: Option<&'a Vec<String>>,
    tools: Option<&'a Vec<String>>,
    mcp_servers: Option<&'a Vec<String>>,
}

/// Merge config from request fields into JSON object
/// Handles both flat fields and nested config, with legacy v2 field mapping
fn merge_config_from_request(params: ConfigMergeParams<'_>) -> Value {
    let mut config = params.base_config.unwrap_or_else(|| json!({}));

    // Map flat fields to config
    if let Some(v) = params.system_prompt {
        config["systemPrompt"] = json!(v);
    }
    if let Some(v) = params.description {
        config["description"] = json!(v);
    }
    if let Some(v) = params.model_provider {
        config["modelProvider"] = json!(v);
    }
    if let Some(v) = params.model_name {
        config["modelName"] = json!(v);
    }
    if let Some(v) = params.temperature {
        config["temperature"] = json!(v);
    }
    if let Some(v) = params.max_tokens {
        config["maxTokens"] = json!(v);
    }

    // Handle tools (v2) -> allowedBuiltInServiceAliases
    if let Some(v) = params.tools {
        config["allowedBuiltInServiceAliases"] = json!(v);
    }
    // Handle allowedBuiltInServiceAliases (v1) - takes precedence
    if let Some(v) = params.allowed_builtin_service_aliases {
        config["allowedBuiltInServiceAliases"] = json!(v);
    }

    // Handle mcpServers (v2) and mcpServerIds (v1)
    if let Some(v) = params.mcp_servers {
        config["mcpServerIds"] = json!(v);
    }
    // mcpServerIds (v1) - takes precedence
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
            "Invalid MCP server IDs: {}. Use builtin_mcp_manager__listMcpServers to see available servers with their IDs.",
            invalid_ids
                .iter()
                .map(|id| format!("'{}'", id))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    Ok(())
}

/// Create a new assistant
pub async fn create_assistant(server: &AssistantServer, args: Value) -> Result<MCPResult, String> {
    // Parse request with type safety
    let request: CreateAssistantRequest = serde_json::from_value(args).map_err(|e| {
        log::error!("Failed to parse CreateAssistantRequest: {}", e);
        format!("Invalid request format: {}", e)
    })?;

    // Always auto-generate ID
    let id = cuid2::create_id();

    // Use repository from server's db connection instead of global state
    let repo = crate::repositories::SqliteAssistantRepository::new(server.get_db().clone());

    // Check for duplicate name BEFORE attempting insert
    let exists = repo
        .check_assistant_exists(&request.name)
        .await
        .map_err(|e| format!("Failed to check for duplicate name: {}", e))?;

    if exists {
        return Ok(duplicate_error(
            "Assistant",
            &request.name,
            ToolGroup::Assistant,
        ));
    }

    // Merge config from all possible sources using helper function
    let config = merge_config_from_request(ConfigMergeParams {
        base_config: request.config,
        system_prompt: request.system_prompt.as_deref(),
        description: request.description.as_deref(),
        model_provider: request.model_provider.as_deref(),
        model_name: request.model_name.as_deref(),
        temperature: request.temperature,
        max_tokens: request.max_tokens,
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
                return Ok(invalid_input_error(&err_msg, ToolGroup::Assistant));
            }
        }
    }

    // Validate config is a valid JSON object
    let config_str =
        serde_json::to_string(&config).map_err(|e| format!("Invalid config JSON: {}", e))?;

    let result = repo
        .create_assistant(id.clone(), request.name.clone(), config_str)
        .await;

    match result {
        Ok(_) => {
            let hint = SuccessHint::new(
                format!(
                    "Assistant '{}' created successfully (ID: {})",
                    request.name, id
                ),
                vec![
                    "Use builtin_assistant__listAssistants to see all assistants".to_string(),
                    "Use builtin_assistant__updateAssistant to modify configuration".to_string(),
                ],
            );

            server.invalidate_cache().await;

            // Emit resource updated event for frontend cache revalidation
            events::emit_resource_updated("assistant", "create", Some(id.clone()));

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "success": true,
                "id": id,
                "name": request.name
            }))))
        }
        Err(e) => Ok(operation_failed_error(
            "Create assistant",
            &e.to_string(),
            vec!["Check database connection".to_string()],
            ToolGroup::Assistant,
        )),
    }
}

/// Update an existing assistant
pub async fn update_assistant(server: &AssistantServer, args: Value) -> Result<MCPResult, String> {
    // Parse request with type safety
    let request: UpdateAssistantRequest = serde_json::from_value(args).map_err(|e| {
        log::error!("Failed to parse UpdateAssistantRequest: {}", e);
        format!("Invalid request format: {}", e)
    })?;

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
            "Assistant",
            &request.id,
            ToolGroup::Assistant,
        ));
    };

    // Update name if provided
    if let Some(ref n) = request.name {
        name = n.clone();
    }

    // Merge config from all sources, starting with base config
    let mut config = merge_config_from_request(ConfigMergeParams {
        base_config: Some(base_config),
        system_prompt: request.system_prompt.as_deref(),
        description: request.description.as_deref(),
        model_provider: request.model_provider.as_deref(),
        model_name: request.model_name.as_deref(),
        temperature: request.temperature,
        max_tokens: request.max_tokens,
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
                return Ok(invalid_input_error(&err_msg, ToolGroup::Assistant));
            }
        }
    }

    let config_str =
        serde_json::to_string(&config).map_err(|e| format!("Invalid config JSON: {}", e))?;

    let result = repo
        .update_assistant(&request.id, Some(name.clone()), Some(config_str))
        .await;

    match result {
        Ok(_) => {
            let hint = SuccessHint::new(
                format!("Assistant '{}' updated successfully", request.id),
                vec!["Use builtin_assistant__getAssistant to verify changes".to_string()],
            );

            server.invalidate_cache().await;

            // Emit resource updated event for frontend cache revalidation
            events::emit_resource_updated("assistant", "update", Some(request.id.clone()));

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "success": true,
                "id": request.id,
                "name": name,
                "config": config
            }))))
        }
        Err(e) => Ok(operation_failed_error(
            "Update assistant",
            &e.to_string(),
            vec![
                "Verify the config JSON is valid".to_string(),
                "Check database connectivity".to_string(),
                "Use builtin_assistant__getAssistant to verify the assistant exists".to_string(),
            ],
            ToolGroup::Assistant,
        )),
    }
}

/// Delete an assistant
pub async fn delete_assistant(server: &AssistantServer, args: Value) -> Result<MCPResult, String> {
    // Parse request with type safety
    let request: DeleteAssistantRequest = serde_json::from_value(args).map_err(|e| {
        log::error!("Failed to parse DeleteAssistantRequest: {}", e);
        format!("Invalid request format: {}", e)
    })?;

    // Use repository from server's db connection
    let repo = crate::repositories::SqliteAssistantRepository::new(server.get_db().clone());

    // Hallucination Firewall: Check existence first
    let exists = repo
        .get_assistant(&request.id)
        .await
        .unwrap_or(None)
        .is_some();

    if !exists {
        return Ok(not_found_error(
            "Assistant",
            &request.id,
            ToolGroup::Assistant,
        ));
    }

    let result = repo.delete_assistant(&request.id).await;

    match result {
        Ok(_) => {
            server.invalidate_cache().await;

            // Emit resource updated event for frontend cache revalidation
            events::emit_resource_updated("assistant", "delete", Some(request.id.clone()));

            let hint = SuccessHint::new(
                format!("Assistant '{}' deleted successfully", request.id),
                vec![
                    "Use builtin_assistant__listAssistants to see remaining assistants".to_string(),
                ],
            );

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "success": true,
                "id": request.id
            }))))
        }
        Err(e) => Ok(operation_failed_error(
            "Delete assistant",
            &e.to_string(),
            vec![
                "Verify the assistant ID is correct".to_string(),
                "Use builtin_assistant__listAssistants to see existing assistants".to_string(),
                "Check database connectivity".to_string(),
            ],
            ToolGroup::Assistant,
        )),
    }
}
