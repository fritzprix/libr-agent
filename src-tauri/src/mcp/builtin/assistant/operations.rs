use crate::entity::{
    assistant, assistant::Entity as AssistantEntity, mcp_server::Entity as McpServerEntity,
};
use crate::mcp::builtin::error_guidance::{
    duplicate_error, invalid_input_error, missing_param_error, not_found_error,
    operation_failed_error, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use sea_orm::*;
use serde_json::{json, Value};

use super::AssistantServer;

/// Validate that all mcpServerIds exist in the mcp_servers table
async fn validate_mcp_server_ids(
    db: &DatabaseConnection,
    server_ids: &[String],
) -> Result<(), String> {
    if server_ids.is_empty() {
        return Ok(()); // Empty list is valid
    }

    // Query database to check which IDs exist
    let existing_servers = McpServerEntity::find()
        .filter(
            sea_orm::sea_query::Expr::col(crate::entity::mcp_server::Column::Name)
                .is_in(server_ids.to_vec()),
        )
        .all(db)
        .await
        .map_err(|e| format!("Failed to validate MCP server IDs: {}", e))?;

    let existing_ids: std::collections::HashSet<_> =
        existing_servers.iter().map(|s| s.name.as_str()).collect();

    // Find invalid IDs
    let invalid_ids: Vec<_> = server_ids
        .iter()
        .filter(|id| !existing_ids.contains(id.as_str()))
        .collect();

    if !invalid_ids.is_empty() {
        return Err(format!(
            "Invalid MCP server IDs: {}. Use builtin_mcp_manager__listMcpServers to see available servers.",
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
    let db = server.get_db();
    // Always auto-generate ID
    let id = cuid2::create_id();

    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(v) => v,
        Option::None => return Ok(missing_param_error("name", ToolGroup::Assistant)),
    };

    // Check for duplicate name BEFORE attempting insert
    let existing = AssistantEntity::find()
        .filter(assistant::Column::Name.eq(name))
        .one(db)
        .await
        .map_err(|e| format!("Failed to check for duplicate name: {}", e))?;

    if existing.is_some() {
        return Ok(duplicate_error("Assistant", name, ToolGroup::Assistant));
    }

    // Extract config fields
    let mut config = args.get("config").cloned().unwrap_or(json!({}));

    // Map legacy/flat fields to config
    if let Some(v) = args.get("systemPrompt") {
        config["systemPrompt"] = v.clone();
    }
    if let Some(v) = args.get("description") {
        config["description"] = v.clone();
    }
    if let Some(v) = args.get("modelProvider") {
        config["modelProvider"] = v.clone();
    }
    if let Some(v) = args.get("modelName") {
        config["modelName"] = v.clone();
    }
    if let Some(v) = args.get("temperature") {
        config["temperature"] = v.clone();
    }
    if let Some(v) = args.get("maxTokens") {
        config["maxTokens"] = v.clone();
    }

    // Handle tools (v2) -> allowedBuiltInServiceAliases
    if let Some(v) = args.get("tools") {
        config["allowedBuiltInServiceAliases"] = v.clone();
    }
    // Handle allowedBuiltInServiceAliases (v1)
    if let Some(v) = args.get("allowedBuiltInServiceAliases") {
        config["allowedBuiltInServiceAliases"] = v.clone();
    }

    // Handle mcpServers (v2) and mcpServerIds (v1)
    if let Some(v) = args.get("mcpServers") {
        config["mcpServerIds"] = v.clone();
    } else if let Some(v) = args.get("mcpServerIds") {
        config["mcpServerIds"] = v.clone();
    }

    // Validate mcpServerIds if provided
    if let Some(server_ids_value) = config.get("mcpServerIds") {
        if let Some(server_ids_array) = server_ids_value.as_array() {
            let server_ids: Vec<String> = server_ids_array
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();

            if let Err(err_msg) = validate_mcp_server_ids(db, &server_ids).await {
                return Ok(invalid_input_error(&err_msg, ToolGroup::Assistant));
            }
        }
    }

    // Validate config is a valid JSON object
    let config_str =
        serde_json::to_string(&config).map_err(|e| format!("Invalid config JSON: {}", e))?;

    let now = chrono::Utc::now().timestamp_millis();

    let model = assistant::ActiveModel {
        id: Set(id.clone()),
        name: Set(name.to_string()),
        config: Set(config_str),
        created_at: Set(now),
        updated_at: Set(now),
    };

    let result = AssistantEntity::insert(model).exec(db).await;

    match result {
        Ok(_) => {
            let hint = SuccessHint::new(
                format!("Assistant '{}' created successfully (ID: {})", name, id),
                vec![
                    "Use builtin_assistant__listAssistants to see all assistants".to_string(),
                    "Use builtin_assistant__updateAssistant to modify configuration".to_string(),
                ],
            );

            server.invalidate_cache().await;

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "success": true,
                "id": id,
                "name": name
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
    let db = server.get_db();
    let id = match args.get("id").and_then(|v| v.as_str()) {
        Some(v) => v,
        Option::None => return Ok(missing_param_error("id", ToolGroup::Assistant)),
    };

    // Fetch existing assistant to merge config
    let existing_model = AssistantEntity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| format!("Failed to fetch assistant: {}", e))?;

    let (mut name, mut config) = if let Some(model) = existing_model {
        (
            model.name,
            serde_json::from_str::<Value>(&model.config).unwrap_or(json!({})),
        )
    } else {
        return Ok(not_found_error("Assistant", id, ToolGroup::Assistant));
    };

    // Update name if provided
    if let Some(n) = args.get("name").and_then(|v| v.as_str()) {
        name = n.to_string();
    }

    // Update config from 'config' object if provided
    if let Some(c) = args.get("config").and_then(|v| v.as_object()) {
        for (k, v) in c {
            config[k] = v.clone();
        }
    }

    // Update config fields (individual overrides)
    if let Some(v) = args.get("systemPrompt") {
        config["systemPrompt"] = v.clone();
    }
    if let Some(v) = args.get("modelProvider") {
        config["modelProvider"] = v.clone();
    }
    if let Some(v) = args.get("modelName") {
        config["modelName"] = v.clone();
    }
    if let Some(v) = args.get("temperature") {
        config["temperature"] = v.clone();
    }
    if let Some(v) = args.get("maxTokens") {
        config["maxTokens"] = v.clone();
    }
    // Handle tools (v2) -> allowedBuiltInServiceAliases
    if let Some(v) = args.get("tools") {
        config["allowedBuiltInServiceAliases"] = v.clone();
    }
    // Handle allowedBuiltInServiceAliases (v1)
    if let Some(v) = args.get("allowedBuiltInServiceAliases") {
        config["allowedBuiltInServiceAliases"] = v.clone();
    }

    // Handle mcpServers (v2) and mcpServerIds (v1)
    if let Some(v) = args.get("mcpServers") {
        config["mcpServerIds"] = v.clone();
    }
    if let Some(v) = args.get("mcpServerIds") {
        config["mcpServerIds"] = v.clone();
    }

    // Validate mcpServerIds if provided
    if let Some(server_ids_value) = config.get("mcpServerIds") {
        if let Some(server_ids_array) = server_ids_value.as_array() {
            let server_ids: Vec<String> = server_ids_array
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();

            if let Err(err_msg) = validate_mcp_server_ids(db, &server_ids).await {
                return Ok(invalid_input_error(&err_msg, ToolGroup::Assistant));
            }
        }
    }

    let config_str =
        serde_json::to_string(&config).map_err(|e| format!("Invalid config JSON: {}", e))?;

    let now = chrono::Utc::now().timestamp_millis();

    let model = assistant::ActiveModel {
        id: Set(id.to_string()),
        name: Set(name.to_string()),
        config: Set(config_str),
        created_at: NotSet,
        updated_at: Set(now),
    };

    let result = AssistantEntity::update(model).exec(db).await;

    match result {
        Ok(_) => {
            let hint = SuccessHint::new(
                format!("Assistant '{}' updated successfully", id),
                vec!["Use builtin_assistant__getAssistant to verify changes".to_string()],
            );

            server.invalidate_cache().await;

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "success": true,
                "id": id,
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
    let db = server.get_db();
    let id = match args.get("id").and_then(|v| v.as_str()) {
        Some(v) => v,
        Option::None => return Ok(missing_param_error("id", ToolGroup::Assistant)),
    };

    // Hallucination Firewall: Check existence first
    if AssistantEntity::find_by_id(id)
        .one(db)
        .await
        .unwrap_or(None)
        .is_none()
    {
        return Ok(not_found_error("Assistant", id, ToolGroup::Assistant));
    }

    let result = AssistantEntity::delete_by_id(id.to_string()).exec(db).await;

    match result {
        Ok(_) => {
            server.invalidate_cache().await;

            let hint = SuccessHint::new(
                format!("Assistant '{}' deleted successfully", id),
                vec![
                    "Use builtin_assistant__listAssistants to see remaining assistants".to_string(),
                ],
            );

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "success": true,
                "id": id
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
