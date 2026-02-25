use crate::mcp::builtin::BuiltinMCPServer;
use crate::session::SessionManager;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tauri::AppHandle;

/// Factory function to create session-bound builtin server instances
///
/// This function is called during proxy initialization to create dedicated
/// server instances for the session.
///
/// # Arguments
/// * `tool_id` - The builtin tool identifier (e.g., "knowledge", "planning")
/// * `session_id` - The session to bind the server to
/// * `db` - Shared SeaORM database connection
///
/// # Returns
/// * `Ok(Some(Box<dyn BuiltinMCPServer>))` - Server instance
/// * `Ok(None)` - Unknown tool ID, skip
/// * `Err(String)` - Server initialization failed
pub(crate) async fn create_builtin_server(
    tool_id: &str,
    _session_id: String,
    _db: Arc<DatabaseConnection>,
    _session_manager: Arc<SessionManager>,
    app_handle: Option<AppHandle>,
) -> Result<Option<Box<dyn BuiltinMCPServer>>, String> {
    match tool_id {
        "bootstrap" => Ok(Some(Box::new(
            crate::mcp::builtin::bootstrap::BootstrapServer::new(),
        ))),
        "knowledge" => {
            let assistant_id = get_assistant_id_from_session(&_session_id).await?;
            Ok(Some(Box::new(
                crate::mcp::builtin::knowledge::KnowledgeServer::new(assistant_id, _db).await?,
            )))
        }
        "planning" => Ok(Some(Box::new(
            crate::mcp::builtin::planning::PlanningServer::new(_session_id, _db).await?,
        ))),
        "playbook" => Ok(Some(Box::new(
            crate::mcp::builtin::playbook::PlaybookServer::new(_session_id, _db).await?,
        ))),
        "assistant" => Ok(Some(Box::new(
            crate::mcp::builtin::assistant::AssistantServer::new(_db).await?,
        ))),
        "workspace" => Ok(Some(Box::new(
            crate::mcp::builtin::workspace::WorkspaceServer::new(_session_id, _session_manager),
        ))),
        "content_store" | "contentstore" => Ok(Some(Box::new(
            crate::mcp::builtin::content_store::ContentStoreServer::new(
                _session_id,
                _session_manager,
            ),
        ))),
        "ui" => Ok(Some(Box::new(crate::mcp::builtin::ui::UiServer::new()))),
        "browser" => {
            if let Some(handle) = app_handle {
                Ok(Some(Box::new(
                    crate::mcp::builtin::browser::BrowserServer::new(handle, _session_id),
                )))
            } else {
                log::warn!("Browser tool requested but no AppHandle provided (skipping)");
                Ok(None)
            }
        }
        "mcp_manager" => Ok(Some(Box::new(
            crate::mcp::builtin::mcp_manager::MCPManagerServer::new(),
        ))),
        "swarm" => Ok(Some(Box::new(
            crate::mcp::builtin::session_api::SessionApiServer::new(),
        ))),
        "skills" => Ok(Some(Box::new(
            crate::mcp::builtin::skills::SkillsServer::new(_session_id),
        ))),
        _ => Ok(None), // Unknown tool, skip
    }
}

async fn get_assistant_id_from_session(session_id: &str) -> Result<String, String> {
    let session = crate::get_session_repository()
        .get_session(session_id)
        .await
        .map_err(|e| format!("Database error fetching session: {}", e))?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    let config_str = session
        .agent_config
        .clone()
        .ok_or_else(|| "Session has no config".to_string())?;

    let config: serde_json::Value = serde_json::from_str(&config_str)
        .map_err(|e| format!("Invalid session config JSON: {}", e))?;

    config
        .get("assistant_id")
        .or_else(|| config.get("assistantId"))
        .or_else(|| config.get("id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "No assistant ID in session config".to_string())
}
