use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::state::get_mcp_server_repository;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::command;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPServerDto {
    pub id: String,
    pub name: String,
    pub config: Value,           // JSON config
    pub tool_count: Option<i32>, // Cached tool count from last verification/connection
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<crate::entity::mcp_server::Model> for MCPServerDto {
    fn from(model: crate::entity::mcp_server::Model) -> Self {
        Self {
            id: model.name.clone(), // ID is name for MCP servers in current schema
            name: model.name,
            config: serde_json::from_str(&model.config).unwrap_or(Value::Null),
            tool_count: model.tool_count, // Include cached tool count
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[command]
pub async fn create_mcp_server_config(name: String, config: Value) -> Result<MCPServerDto, String> {
    let repo = get_mcp_server_repository();
    let model = repo
        .create(&name, config)
        .await
        .map_err(|e| format!("Failed to create MCP server config: {}", e))?;
    Ok(model.into())
}

#[command]
pub async fn update_mcp_server_config(
    name: String,
    config: Option<Value>,
) -> Result<MCPServerDto, String> {
    let repo = get_mcp_server_repository();

    // Since repository update expects a config value, we need to fetch existing config if only partial update logic is needed.
    // However, the repository `update` method currently takes `config: Value`.
    // The command receives `Option<Value>`. If `None`, what should happen?
    // The previous implementation updated *if* present.
    // We should probably first GET the existing model to merge, or update the repository signature.
    // Let's reuse the logic from previous implementation: fetch, patch, update.
    // The repository `update` method takes a FULL `config` value.
    // So we need to fetch, then if `config` is None, we keep existing. If `config` is Some, we use it.

    let existing = repo
        .get(&name)
        .await
        .map_err(|e| format!("Failed to find MCP server config: {}", e))?
        .ok_or_else(|| "MCP server config not found".to_string())?;

    let new_config = match config {
        Some(c) => c,
        None => serde_json::from_str(&existing.config).unwrap_or(Value::Null),
    };

    let updated = repo
        .update(&name, new_config)
        .await
        .map_err(|e| format!("Failed to update MCP server config: {}", e))?;

    Ok(updated.into())
}

#[command]
pub async fn delete_mcp_server_config(name: String) -> Result<(), String> {
    let repo = get_mcp_server_repository();
    repo.delete(&name)
        .await
        .map_err(|e| format!("Failed to delete MCP server config: {}", e))?;
    Ok(())
}

#[command]
pub async fn list_mcp_server_configs() -> Result<Vec<MCPServerDto>, String> {
    let repo = get_mcp_server_repository();
    let models = repo
        .list()
        .await
        .map_err(|e| format!("Failed to list MCP server configs: {}", e))?;
    Ok(models.into_iter().map(|s| s.into()).collect())
}
