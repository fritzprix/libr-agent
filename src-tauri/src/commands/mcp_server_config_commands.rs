use crate::services::McpServerService;
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
    pub verification_status: Option<String>,
    pub last_verification_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<crate::entity::mcp_server::Model> for MCPServerDto {
    fn from(model: crate::entity::mcp_server::Model) -> Self {
        let config = serde_json::from_str(&model.config).unwrap_or_else(|err| {
            log::warn!(
                "Failed to parse MCP server config JSON for server id='{}' name='{}': {}",
                model.id,
                model.name,
                err
            );
            Value::Null
        });
        Self {
            id: model.id,
            name: model.name,
            config,
            tool_count: model.tool_count, // Include cached tool count
            verification_status: model.verification_status,
            last_verification_error: model.last_verification_error,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[command]
pub async fn create_mcp_server_config(name: String, config: Value) -> Result<MCPServerDto, String> {
    let model =
        McpServerService::create_server_config(get_mcp_server_repository(), name, config).await?;
    McpServerService::schedule_background_probe(model.id.clone());
    Ok(model.into())
}

#[command]
pub async fn update_mcp_server_config(
    id: String,
    name: Option<String>,
    config: Option<Value>,
) -> Result<MCPServerDto, String> {
    let updated =
        McpServerService::update_server_config(get_mcp_server_repository(), id, name, config)
            .await?;
    if updated.verification_status.as_deref() == Some("pending") {
        McpServerService::schedule_background_probe(updated.id.clone());
    }
    Ok(updated.into())
}

#[command]
pub async fn delete_mcp_server_config(id: String) -> Result<(), String> {
    McpServerService::delete_server_config(get_mcp_server_repository(), &id).await
}

#[command]
pub async fn list_mcp_server_configs() -> Result<Vec<MCPServerDto>, String> {
    let models = McpServerService::list_server_configs(get_mcp_server_repository()).await?;
    Ok(models.into_iter().map(|s| s.into()).collect())
}

#[command]
pub async fn list_mcp_server_presets() -> Result<Vec<crate::mcp::presets::MCPServerPreset>, String>
{
    Ok(crate::mcp::presets::get_recommended_servers())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_dto_from_valid_json() {
        let model = crate::entity::mcp_server::Model {
            id: "mcp-1".to_string(),
            name: "test-server".to_string(),
            config: r#"{"command":"node","args":["server.js"]}"#.to_string(),
            tool_count: Some(5),
            cached_tools: None,
            verification_status: Some("success".to_string()),
            last_verification_error: None,
            created_at: 1000,
            updated_at: 2000,
        };

        let dto: MCPServerDto = model.into();
        assert_eq!(dto.id, "mcp-1");
        assert_eq!(dto.name, "test-server");
        assert_eq!(dto.config["command"], "node");
        assert_eq!(dto.tool_count, Some(5));
    }

    #[test]
    fn test_mcp_server_dto_from_malformed_json_falls_back_gracefully() {
        let model = crate::entity::mcp_server::Model {
            id: "mcp-2".to_string(),
            name: "corrupted-server".to_string(),
            config: "{invalid json".to_string(),
            tool_count: None,
            cached_tools: None,
            verification_status: Some("error".to_string()),
            last_verification_error: Some("Parse error".to_string()),
            created_at: 1000,
            updated_at: 2000,
        };

        let dto: MCPServerDto = model.into();
        assert_eq!(dto.id, "mcp-2");
        assert_eq!(dto.name, "corrupted-server");
        assert_eq!(dto.config, Value::Null);
    }
}
