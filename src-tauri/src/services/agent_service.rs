use crate::agent::AgentSessionManager;
use crate::commands::workspace_commands::get_app_logs_dir;
use std::fs;

pub struct AgentService;

impl AgentService {
    /// Clear all agent sessions (used for "Clear All Sessions" feature)
    pub async fn clear_all_sessions(manager: &AgentSessionManager) -> Result<usize, String> {
        // 1. Get all sessions
        let sessions = manager.get_all_sessions().await?;
        let count = sessions.len();

        // 2. Delete each session
        for session in sessions {
            if let Err(e) = manager.delete_session(session.id.clone()).await {
                log::error!(
                    "Failed to delete session {} during clear all: {}",
                    session.id,
                    e
                );
            }
        }

        // 3. Cleanup dangled workspaces (FS only)
        // Clean up dangling workspace directories (exist on disk but no longer have DB sessions)
        if let Ok(session_manager) = crate::session::get_session_manager() {
            if let Ok(fs_sessions) = session_manager.list_sessions() {
                let mut dangled_count = 0;
                for session_id in fs_sessions {
                    // Skip 'default' workspace to preserve the fallback environment
                    if session_id != "default" {
                        // Lazy load workspace into pool, then attempt removal
                        let _ = session_manager.get_session_workspace_dir_by_id(&session_id);
                        if let Err(e) = session_manager.remove_session(&session_id).await {
                            log::debug!(
                                "Attempted to remove potential dangled session {}: {}",
                                session_id,
                                e
                            );
                        } else {
                            dangled_count += 1;
                        }
                    }
                }
                if dangled_count > 0 {
                    log::info!(
                        "Cleaned up {} dangled/residual workspace directories",
                        dangled_count
                    );
                }
            }
        }

        Ok(count)
    }

    /// Factory reset the agent system (used for "Reset All Data & Settings" feature)
    /// Deletes all sessions, assistants, playbooks, mcp servers, and logs.
    pub async fn factory_reset(manager: &AgentSessionManager) -> Result<(), String> {
        use crate::repositories::mcp_server_repository::MCPServerRepository;
        use crate::repositories::PlaybookRepository;
        use crate::repositories::AssistantRepository;
        use crate::state::get_mcp_server_repository;

        // 1. Clear all sessions first
        Self::clear_all_sessions(manager).await?;

        // 2. Delete all Playbooks (must happen before assistants due to foreign key)
        let playbook_repo = crate::state::get_playbook_repository();
        let all_playbooks = playbook_repo
            .list_playbooks(
                None,
                crate::repositories::PaginationParams {
                    page: 1,
                    page_size: 100000,
                },
            )
            .await
            .map_err(|e| format!("Failed to list playbooks: {}", e))?;

        for playbook in all_playbooks.items {
            playbook_repo
                .delete_playbook(&playbook.id, &playbook.assistant_id)
                .await
                .map_err(|e| format!("Failed to delete playbook {}: {}", playbook.id, e))?;
        }

        // 3. Delete all Assistants
        let assistant_repo = crate::state::get_assistant_repository();
        let all_assistants = assistant_repo
            .list_assistants()
            .await
            .map_err(|e| format!("Failed to list assistants: {}", e))?;

        for assistant in all_assistants {
            assistant_repo
                .delete_assistant(&assistant.id)
                .await
                .map_err(|e| format!("Failed to delete assistant {}: {}", assistant.id, e))?;
        }

        // 4. Delete all MCP Servers
        let mcp_repo = get_mcp_server_repository();
        let servers = mcp_repo
            .list()
            .await
            .map_err(|e| format!("Failed to list MCP servers: {}", e))?;
        for server in servers {
            mcp_repo
                .delete(&server.name)
                .await
                .map_err(|e| format!("Failed to delete MCP server {}: {}", server.name, e))?;
        }

        // 5. Restore default assistants so the app is not empty
        if let Err(e) = crate::services::assistant_init::ensure_default_assistants().await {
            return Err(format!(
                "Factory reset failed to restore default assistants: {}",
                e
            ));
        }

        // 6. Clear application logs
        // We do this last to preserve logging of the reset process as much as possible
        if let Ok(log_dir_str) = get_app_logs_dir().await {
            let log_dir = std::path::PathBuf::from(log_dir_str);
            if log_dir.exists() {
                if let Ok(entries) = fs::read_dir(&log_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                                if filename.ends_with(".log") || filename.ends_with(".log.bak") {
                                    if let Err(e) = fs::remove_file(&path) {
                                        log::warn!("Failed to delete log file {:?}: {}", path, e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
