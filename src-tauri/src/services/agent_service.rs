use crate::agent::AgentSessionManager;
use crate::session::get_session_manager;
use std::fs;

pub struct AgentService;

impl AgentService {
    /// Validates a workspace override path and registers it for the given session.
    ///
    /// The path must be absolute, must exist, and must be a directory.
    async fn validate_and_register_workspace_override(
        path_str: &str,
        session_id: &str,
    ) -> Result<(), String> {
        let Ok(session_manager) = get_session_manager() else {
            log::warn!("Failed to get session manager for workspace override");
            return Ok(());
        };
        let path = std::path::PathBuf::from(path_str);
        if !path.is_absolute() {
            return Err("Workspace path must be absolute".to_string());
        }
        match tokio::fs::metadata(&path).await {
            Ok(metadata) => {
                if !metadata.is_dir() {
                    return Err("Workspace path must be a directory".to_string());
                }
            }
            Err(err) => {
                return Err(format!("Workspace path is not accessible: {}", err));
            }
        }
        session_manager
            .register_session_override(session_id, path)
            .await
    }

    /// Create a new agent session
    pub async fn create_session(
        manager: &AgentSessionManager,
        request: crate::commands::agent_commands::CreateAgentSessionRequest,
    ) -> Result<crate::repositories::SessionMetadata, String> {
        use crate::repositories::in_memory_session_repository::InMemorySessionRepository;
        use crate::repositories::SessionRepository;
        use std::sync::Arc;

        // Handle workspace override if path is provided
        if let Some(path_str) = &request.workspace_path {
            Self::validate_and_register_workspace_override(path_str, &request.session_id).await?;
        }
        let session_repo: Arc<dyn SessionRepository> = if request.is_ephemeral {
            log::info!(
                "Creating ephemeral session (in-memory only): {}",
                request.session_id
            );
            Arc::new(InMemorySessionRepository::new()) as Arc<dyn SessionRepository>
        } else {
            log::info!(
                "Creating persistent session (DB-backed): {}",
                request.session_id
            );
            Arc::new(crate::state::get_session_repository().clone())
        };

        manager
            .create_session_with_repo(
                session_repo,
                request.session_id,
                request.name,
                request.model,
                request.provider,
                request.agent_config,
            )
            .await
    }

    /// Create a new session and IMMEDIATELY start the workflow with an initial message
    /// This is used for "Draft Mode" where the session is created only when the first message is sent.
    pub async fn create_session_with_initial_message(
        manager: &AgentSessionManager,
        request: crate::commands::agent_commands::CreateAgentSessionWithMessageRequest,
    ) -> Result<crate::commands::agent_commands::AgentResponse, String> {
        // Handle workspace override if path is provided
        if let Some(path_str) = &request.workspace_path {
            Self::validate_and_register_workspace_override(path_str, &request.session_id).await?;
        }

        // 1. Create the session first (persistent by default)
        // We use the default persistent repository here
        let session_repo = std::sync::Arc::new(crate::state::get_session_repository().clone());

        manager
            .create_session_with_repo(
                session_repo,
                request.session_id.clone(),
                request.name,
                request.model,
                request.provider,
                request.agent_config,
            )
            .await?;

        // 2. Start the workflow with the initial message
        manager
            .start_workflow(request.session_id.clone(), request.message)
            .await
            .map(|_| crate::commands::agent_commands::AgentResponse {
                success: true,
                message: "Session created and workflow started".to_string(),
                data: None,
            })
    }

    /// Call a builtin tool directly via proxy_manager (for testing and direct execution)
    /// Returns the unwrapped MCPResult (not the full MCPResponse wrapper)
    pub async fn call_builtin_tool(
        session_id: String,
        tool_name: String,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        use crate::mcp::types::MCPResponseResult;
        use crate::state::get_mcp_service_proxy_manager;

        let proxy_manager = get_mcp_service_proxy_manager();

        let response = proxy_manager
            .call_tool(&session_id, &tool_name, args)
            .await?;

        // Handle errors from tool execution
        if let Some(error) = response.error {
            return Err(format!("Tool execution error: {}", error.message));
        }

        // Extract result from MCPResponse
        let result = response
            .result
            .ok_or_else(|| "Tool execution returned no result or error".to_string())?;

        // Unwrap MCPResult from MCPResponseResult::ToolCall variant
        // This matches the TypeScript expectation of receiving MCPResult directly
        match result {
            MCPResponseResult::ToolCall(mcp_result) => {
                // Serialize MCPResult (with camelCase field names matching TypeScript interface)
                serde_json::to_value(mcp_result)
                    .map_err(|e| format!("Failed to serialize MCPResult: {}", e))
            }
            _ => Err(format!(
                "Unexpected response type for builtin tool '{}': expected ToolCall variant",
                tool_name
            )),
        }
    }

    /// Get service contexts for a session
    pub async fn get_service_contexts(
        session_id: String,
    ) -> Result<std::collections::HashMap<String, crate::mcp::types::ServiceContext>, String> {
        use crate::state::get_mcp_service_proxy_manager;

        let proxy_manager = get_mcp_service_proxy_manager();

        let proxy = proxy_manager
            .get_proxy(&session_id)
            .await
            .ok_or_else(|| format!("No proxy found for session: {}", session_id))?;

        Ok(proxy.get_service_contexts(None).await)
    }

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
        use crate::repositories::AssistantRepository;
        use crate::repositories::PlaybookRepository;
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
        if let Ok(session_mgr) = get_session_manager() {
            let log_dir = session_mgr.get_logs_dir();
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
