use crate::repositories::MCPServerRepository;
use crate::state::get_mcp_server_repository;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::command;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPServerDto {
    pub id: String,
    pub name: String,
    pub config: Value, // JSON config
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<crate::repositories::MCPServer> for MCPServerDto {
    fn from(model: crate::repositories::MCPServer) -> Self {
        Self {
            id: model.name.clone(), // ID is name for MCP servers in current schema
            name: model.name,
            config: model.config,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[command]
pub async fn create_mcp_server_config(name: String, config: Value) -> Result<MCPServerDto, String> {
    let repo = get_mcp_server_repository();
    repo.create(&name, config)
        .await
        .map(|s| s.into())
        .map_err(|e| e.to_string())
}

#[command]
pub async fn update_mcp_server_config(
    name: String,
    config: Option<Value>,
) -> Result<MCPServerDto, String> {
    let repo = get_mcp_server_repository();
    repo.update(&name, config)
        .await
        .map(|s| s.into())
        .map_err(|e| e.to_string())
}

#[command]
pub async fn delete_mcp_server_config(name: String) -> Result<(), String> {
    let repo = get_mcp_server_repository();
    repo.delete(&name).await.map_err(|e| e.to_string())
}

#[command]
pub async fn list_mcp_server_configs() -> Result<Vec<MCPServerDto>, String> {
    let repo = get_mcp_server_repository();
    repo.list()
        .await
        .map(|list| list.into_iter().map(|s| s.into()).collect())
        .map_err(|e| e.to_string())
}
