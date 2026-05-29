use super::lineage::remove_lineage;
use super::AgentService;
use crate::agent::AgentSessionManager;
use crate::session::get_session_manager;
use crate::repositories::settings_repository::SettingsRepository;
use std::fs;

impl AgentService {
    /// Clear all agent sessions (used for "Clear All Sessions" feature)
    pub async fn clear_all_sessions(manager: &AgentSessionManager) -> Result<usize, String> {
        let sessions = manager.get_all_sessions().await?;
        let count = sessions.len();

        for session in sessions {
            match manager.delete_session(session.id.clone()).await {
                Ok(deleted_ids) => {
                    for deleted_id in deleted_ids {
                        remove_lineage(&deleted_id).await;
                    }
                }
                Err(error) => {
                    log::error!(
                        "Failed to delete session {} during clear all: {}",
                        session.id,
                        error
                    );
                }
            }
        }

        if let Ok(session_manager) = get_session_manager() {
            if let Ok(fs_sessions) = session_manager.list_sessions() {
                let mut dangled_count = 0;
                for session_id in fs_sessions {
                    if session_id != "default" {
                        let _ = session_manager.get_session_workspace_dir_by_id(&session_id);
                        if let Err(error) = session_manager.remove_session(&session_id).await {
                            log::debug!(
                                "Attempted to remove potential dangled session {}: {}",
                                session_id,
                                error
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
        use crate::repositories::AssistantRepository;
        use crate::repositories::PlaybookRepository;
        use crate::state::get_mcp_server_repository;

        Self::clear_all_sessions(manager).await?;

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

        if let Some(settings_repo) = crate::state::try_get_settings_repository() {
            if let Ok(settings) = settings_repo.list().await {
                for setting in settings {
                    let _ = settings_repo.delete(&setting.key).await;
                }
            }
        }

        if let Err(error) = crate::services::assistant_init::ensure_default_assistants().await {
            return Err(format!(
                "Factory reset failed to restore default assistants: {}",
                error
            ));
        }

        if let Ok(session_mgr) = get_session_manager() {
            let log_dir = session_mgr.get_logs_dir();
            if log_dir.exists() {
                if let Ok(entries) = fs::read_dir(&log_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            if let Some(filename) = path.file_name().and_then(|name| name.to_str())
                            {
                                if filename.ends_with(".log") || filename.ends_with(".log.bak") {
                                    if let Err(error) = fs::remove_file(&path) {
                                        log::warn!(
                                            "Failed to delete log file {:?}: {}",
                                            path,
                                            error
                                        );
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
