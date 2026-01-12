use crate::entity::mcp_server;
use crate::state::get_database_connection;
use sea_orm::{ActiveModelTrait, EntityTrait, QueryOrder, Set};
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

impl From<mcp_server::Model> for MCPServerDto {
    fn from(model: mcp_server::Model) -> Self {
        Self {
            id: model.name.clone(), // ID is name for MCP servers in current schema
            name: model.name,
            config: serde_json::from_str(&model.config).unwrap_or(Value::Null),
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[command]
pub async fn create_mcp_server_config(name: String, config: Value) -> Result<MCPServerDto, String> {
    let db = get_database_connection();
    let now = chrono::Utc::now().timestamp_millis();

    // Check if exists
    let exists = mcp_server::Entity::find_by_id(&name)
        .one(db)
        .await
        .map_err(|e| format!("Failed to check existence: {}", e))?;

    if exists.is_some() {
        return Err(format!(
            "MCP server config with name '{}' already exists",
            name
        ));
    }

    let server = mcp_server::ActiveModel {
        name: Set(name.clone()),
        config: Set(config.to_string()),
        created_at: Set(now),
        updated_at: Set(now),
    };

    let result = server
        .insert(db)
        .await
        .map_err(|e| format!("Failed to create MCP server config: {}", e))?;

    Ok(result.into())
}

#[command]
pub async fn update_mcp_server_config(
    name: String,
    config: Option<Value>,
) -> Result<MCPServerDto, String> {
    let db = get_database_connection();
    let now = chrono::Utc::now().timestamp_millis();

    let mut server: mcp_server::ActiveModel = mcp_server::Entity::find_by_id(&name)
        .one(db)
        .await
        .map_err(|e| format!("Failed to find MCP server config: {}", e))?
        .ok_or_else(|| "MCP server config not found".to_string())?
        .into();

    if let Some(config) = config {
        server.config = Set(config.to_string());
    }
    server.updated_at = Set(now);

    let result = server
        .update(db)
        .await
        .map_err(|e| format!("Failed to update MCP server config: {}", e))?;

    Ok(result.into())
}

#[command]
pub async fn delete_mcp_server_config(name: String) -> Result<(), String> {
    let db = get_database_connection();
    mcp_server::Entity::delete_by_id(name)
        .exec(db)
        .await
        .map_err(|e| format!("Failed to delete MCP server config: {}", e))?;
    Ok(())
}

#[command]
pub async fn list_mcp_server_configs() -> Result<Vec<MCPServerDto>, String> {
    let db = get_database_connection();
    let servers = mcp_server::Entity::find()
        .order_by_asc(mcp_server::Column::CreatedAt)
        .all(db)
        .await
        .map_err(|e| format!("Failed to list MCP server configs: {}", e))?;

    Ok(servers.into_iter().map(|s| s.into()).collect())
}
