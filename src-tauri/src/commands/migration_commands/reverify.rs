use sea_orm::EntityTrait;
use std::collections::HashMap;
use tauri::command;

use crate::entity::mcp_server;
use crate::state::get_database_connection;

#[command]
pub async fn reverify_mcp_servers() -> Result<HashMap<String, String>, String> {
    let db = get_database_connection();
    let repo = crate::state::get_mcp_server_repository();

    // Fetch all MCP servers from the database
    let db_mcp = mcp_server::Entity::find()
        .all(db)
        .await
        .map_err(|e| e.to_string())?;

    let mut results = HashMap::new();

    // Iterate and probe each server
    for server in db_mcp {
        let res =
            crate::services::mcp_server_service::McpServerService::probe_server(repo, &server.id)
                .await;
        match res {
            Ok(_) => {
                results.insert(server.id, "success".to_string());
            }
            Err(e) => {
                log::error!("Failed to reverify MCP server {}: {}", server.id, e);
                results.insert(server.id, "error".to_string());
            }
        }
    }

    Ok(results)
}
