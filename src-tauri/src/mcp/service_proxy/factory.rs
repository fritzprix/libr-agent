use crate::mcp::builtin::service_id::BuiltinServiceId;
use crate::mcp::builtin::BuiltinMCPServer;
use crate::repositories::session_repository::SessionRepository;
use crate::session::SessionManager;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

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
    // Resolve string → stable enum.
    // Unknown strings return Ok(None) — not an error, just not a builtin service.
    let Some(service_id) = BuiltinServiceId::from_alias(tool_id) else {
        return Ok(None);
    };

    match service_id {
        BuiltinServiceId::SetupWizard => Ok(Some(Box::new(
            crate::mcp::builtin::setup_wizard::SetupWizardServer::new(),
        ))),
        BuiltinServiceId::Knowledge => {
            let session = crate::get_session_repository()
                .get_session(&_session_id)
                .await
                .map_err(|e| format!("Database error fetching session: {}", e))?
                .ok_or_else(|| format!("Session not found: {}", _session_id))?;
            let assistant_id = crate::agent::extract_assistant_id_from_session(&session)
                .ok_or_else(|| "Session has no assistant configuration".to_string())?;
            Ok(Some(Box::new(
                crate::mcp::builtin::knowledge::KnowledgeServer::new(assistant_id, _db).await?,
            )))
        }
        BuiltinServiceId::History => Ok(Some(Box::new(
            crate::mcp::builtin::history::HistoryServer::new(_session_id, _db).await?,
        ))),
        BuiltinServiceId::Planning => Ok(Some(Box::new(
            crate::mcp::builtin::planning::PlanningServer::new(_session_id, _db).await?,
        ))),
        BuiltinServiceId::Agent => {
            let agent_manager = app_handle.as_ref().map(|h| {
                h.state::<crate::agent::AgentSessionManager>()
                    .inner()
                    .clone()
            });

            Ok(Some(Box::new(
                crate::mcp::builtin::agent::AgentServer::new(
                    _session_id,
                    _db.clone(),
                    agent_manager,
                )
                .await?,
            )))
        }
        BuiltinServiceId::Scratchpad => Ok(Some(Box::new(
            crate::mcp::builtin::scratchpad::ScratchpadServer::new(_session_id, _db).await?,
        ))),
        BuiltinServiceId::Playbook => Ok(Some(Box::new(
            crate::mcp::builtin::playbook::PlaybookServer::new(_session_id, _db).await?,
        ))),
        BuiltinServiceId::Workspace => {
            let isolation = match crate::get_session_repository()
                .get_session(&_session_id)
                .await
            {
                Ok(Some(session)) => session.workspace_isolation,
                Ok(None) => {
                    log::warn!(
                        "Session '{}' not found when creating WorkspaceServer; defaulting to host isolation",
                        _session_id
                    );
                    crate::models::workspace_isolation::WorkspaceIsolationMode::Host
                }
                Err(error) => {
                    log::warn!(
                        "Failed to load session '{}' for WorkspaceServer isolation ({}); defaulting to host",
                        _session_id,
                        error
                    );
                    crate::models::workspace_isolation::WorkspaceIsolationMode::Host
                }
            };
            Ok(Some(Box::new(
                crate::mcp::builtin::workspace::WorkspaceServer::with_isolation(
                    _session_id,
                    _session_manager,
                    isolation,
                ),
            )))
        }
        BuiltinServiceId::Attachments => Ok(Some(Box::new(
            crate::mcp::builtin::attachments::AttachmentsServer::new(_session_id, _session_manager),
        ))),
        BuiltinServiceId::Ui => Ok(Some(Box::new(crate::mcp::builtin::ui::UiServer::new()))),
        BuiltinServiceId::Browser => {
            if let Some(handle) = app_handle {
                Ok(Some(Box::new(
                    crate::mcp::builtin::browser::BrowserServer::new(handle, _session_id),
                )))
            } else {
                log::warn!("Browser tool requested but no AppHandle provided (skipping)");
                Ok(None)
            }
        }
        BuiltinServiceId::ScheduledTask => Ok(Some(Box::new(
            crate::mcp::builtin::scheduled_task::ScheduledTaskServer::new(_session_id, _db).await?,
        ))),
        BuiltinServiceId::Tool => Ok(Some(Box::new(crate::mcp::builtin::tool::ToolServer::new()))),
        BuiltinServiceId::Skills => Ok(Some(Box::new(
            crate::mcp::builtin::skills::SkillsServer::new(_session_id),
        ))),
        BuiltinServiceId::Media => Ok(Some(Box::new(
            crate::mcp::builtin::media::MediaServer::new(_session_id, _session_manager),
        ))),
    }
}
