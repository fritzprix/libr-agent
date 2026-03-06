use crate::services::McpServerService;
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
    let model = McpServerService::create_server_config(name, config).await?;
    Ok(model.into())
}

#[command]
pub async fn update_mcp_server_config(
    id: String,
    name: Option<String>,
    config: Option<Value>,
) -> Result<MCPServerDto, String> {
    let updated = McpServerService::update_server_config(id, name, config).await?;
    Ok(updated.into())
}

#[command]
pub async fn delete_mcp_server_config(id: String) -> Result<(), String> {
    McpServerService::delete_server_config(&id).await
}

#[command]
pub async fn list_mcp_server_configs() -> Result<Vec<MCPServerDto>, String> {
    let models = McpServerService::list_server_configs().await?;
    Ok(models.into_iter().map(|s| s.into()).collect())
}

#[command]
pub async fn list_mcp_server_presets() -> Result<Vec<crate::mcp::presets::MCPServerPreset>, String>
{
    Ok(crate::mcp::presets::get_recommended_servers())
}
