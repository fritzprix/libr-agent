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
            id: model.id.clone(), // Use actual database ID (cuid2)
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
    id: String,
    name: Option<String>,
    config: Option<Value>,
) -> Result<MCPServerDto, String> {
    let repo = get_mcp_server_repository();

    let updated = repo
        .update(&id, name.as_deref(), config)
        .await
        .map_err(|e| format!("Failed to update MCP server config: {}", e))?;

    Ok(updated.into())
}

#[command]
pub async fn delete_mcp_server_config(id: String) -> Result<(), String> {
    let repo = get_mcp_server_repository();
    repo.delete(&id)
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

#[command]
pub async fn list_mcp_server_presets() -> Result<Vec<crate::mcp::presets::MCPServerPreset>, String>
{
    Ok(crate::mcp::presets::get_recommended_servers())
}
