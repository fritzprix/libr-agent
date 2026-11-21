mod lineage;
mod messages;
mod reset;
pub mod spawn;
mod workspace;

use crate::agent::AgentSessionManager;
use crate::models::workspace_isolation::{DockerWorkspaceConfig, WorkspaceIsolationMode};
use crate::session::get_session_manager;

pub use lineage::{
    lineage_store, normalize_explicit_org, remove_lineage, resolve_child_session_model_provider,
};
pub use workspace::{is_restricted_system_path, resolve_workspace_override_path};

pub struct AgentService;

#[derive(Debug, Clone)]
pub struct SendSessionMessageResponse {
    pub message_id: String,
    pub status: String,
}

impl AgentService {
    fn validate_workspace_isolation_request(
        workspace_isolation: Option<WorkspaceIsolationMode>,
        docker_config: Option<DockerWorkspaceConfig>,
        is_ephemeral: bool,
    ) -> Result<(WorkspaceIsolationMode, Option<DockerWorkspaceConfig>), String> {
        let mode = workspace_isolation.unwrap_or_default();

        match mode {
            WorkspaceIsolationMode::Host => {
                if docker_config.is_some() {
                    return Err(
                        "dockerConfig is only valid when workspaceIsolation is 'docker'"
                            .to_string(),
                    );
                }
                Ok((WorkspaceIsolationMode::Host, None))
            }
            WorkspaceIsolationMode::Docker => {
                if is_ephemeral {
                    return Err(
                        "Docker workspace isolation is not supported for ephemeral sessions"
                            .to_string(),
                    );
                }

                let config = docker_config.unwrap_or_default().with_default_image();
                config.validate()?;
                Ok((WorkspaceIsolationMode::Docker, Some(config)))
            }
        }
    }

    pub(crate) async fn prepare_workspace_setup(
        workspace_isolation: Option<WorkspaceIsolationMode>,
        docker_config: Option<DockerWorkspaceConfig>,
        is_ephemeral: bool,
        workspace_path: Option<&str>,
        session_id: &str,
    ) -> Result<(WorkspaceIsolationMode, Option<DockerWorkspaceConfig>), String> {
        let (mode, docker) = Self::validate_workspace_isolation_request(
            workspace_isolation,
            docker_config,
            is_ephemeral,
        )?;

        if matches!(mode, WorkspaceIsolationMode::Docker) {
            crate::services::WorkspaceRuntimeManager::healthcheck()
                .await
                .map_err(|error| error.to_agent_string())?;
        }

        if let Some(path_str) = workspace_path {
            Self::validate_and_register_workspace_override(path_str, session_id).await?;
        }

        // #2942 G3.3(a): the default session workspace directory
        // (`{data_dir}/workspaces/<session_id>`) is the SSOT parent of the
        // session plugin store (D1 revised), so create it at session
        // creation — with or without a workspace override. Previously it was
        // only created lazily or as a side effect of override registration.
        // #2942 G5.1: every failure layer is reported — no manager, spawn
        // (JoinError), and the inner directory-creation error.
        match crate::session::get_session_manager() {
            Ok(session_manager) => {
                let sid = session_id.to_string();
                let result = tokio::task::spawn_blocking(move || {
                    Self::ensure_default_session_workspace_dir(
                        session_manager.get_directory_service(),
                        &sid,
                    )
                })
                .await;
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        log::warn!(
                            "Failed to ensure default session workspace directory for {session_id}: {error}"
                        );
                    }
                    Err(join_error) => {
                        log::warn!(
                            "Default session workspace directory task failed for {session_id}: {join_error}"
                        );
                    }
                }
            }
            Err(error) => {
                log::warn!(
                    "Session manager unavailable; default session workspace directory not ensured for {session_id}: {error}"
                );
            }
        }

        Ok((mode, docker))
    }

    /// Create the default session workspace directory, reporting failures
    /// (#2942 G5.1). Takes the directory service directly so the success and
    /// failure paths are testable without the global session manager; the
    /// directory is keyed by session id only, so an active workspace
    /// override never changes (or skips) it.
    fn ensure_default_session_workspace_dir(
        directory_service: &crate::services::session_directory_service::SessionDirectoryService,
        session_id: &str,
    ) -> Result<(), String> {
        directory_service
            .ensure_workspace_dir(session_id)
            .map(|_| ())
    }

    /// Create a new agent session
    pub async fn create_session(
        manager: &AgentSessionManager,
        request: crate::commands::agent_commands::CreateAgentSessionRequest,
    ) -> Result<crate::repositories::SessionMetadata, String> {
        use crate::repositories::in_memory_session_repository::InMemorySessionRepository;
        use crate::repositories::SessionRepository;
        use std::sync::Arc;

        let (workspace_isolation, docker_config) = Self::prepare_workspace_setup(
            request.workspace_isolation,
            request.docker_config,
            request.is_ephemeral,
            request.workspace_path.as_deref(),
            &request.session_id,
        )
        .await?;

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
                workspace_isolation,
                docker_config,
            )
            .await
    }

    /// Create a new session and IMMEDIATELY start the workflow with an initial message
    /// This is used for "Draft Mode" where the session is created only when the first message is sent.
    pub async fn create_session_with_initial_message(
        manager: &AgentSessionManager,
        request: crate::commands::agent_commands::CreateAgentSessionWithMessageRequest,
    ) -> Result<crate::commands::agent_commands::AgentResponse, String> {
        let (workspace_isolation, docker_config) = Self::prepare_workspace_setup(
            request.workspace_isolation,
            request.docker_config,
            false,
            request.workspace_path.as_deref(),
            &request.session_id,
        )
        .await?;

        let session_repo = std::sync::Arc::new(crate::state::get_session_repository().clone());

        manager
            .create_session_with_repo(
                session_repo,
                request.session_id.clone(),
                request.name,
                request.model,
                request.provider,
                request.agent_config,
                workspace_isolation,
                docker_config,
            )
            .await?;

        // Explicit API message: a previous force-stop fence ends here (issue #2633).
        manager
            .allow_explicit_message_after_terminate(&request.session_id)
            .await;

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

        if crate::agent::tool_approvals::requests_root_execution(&tool_name, &args) {
            return Err(crate::agent::tool_approvals::ROOT_EXECUTION_NEEDS_APPROVAL.to_string());
        }

        let proxy_manager = get_mcp_service_proxy_manager();

        let response = proxy_manager
            .call_tool(&session_id, &tool_name, args)
            .await?;

        if let Some(error) = response.error {
            return Err(format!("Tool execution error: {}", error.message));
        }

        let result = response
            .result
            .ok_or_else(|| "Tool execution returned no result or error".to_string())?;

        match result {
            MCPResponseResult::ToolCall(mcp_result) => serde_json::to_value(mcp_result)
                .map_err(|e| format!("Failed to serialize MCPResult: {}", e)),
            _ => Err(format!(
                "Unexpected response type for builtin tool '{}': expected ToolCall variant",
                tool_name
            )),
        }
    }

    /// Save an attachment to the session-scoped attachment store through an internal UI-only API.
    ///
    /// Routes through the session-bound `MCPServiceProxy` so that the same
    /// `AttachmentsServer` instance used by the agent is updated — keeping
    /// `recent_uploads` tracking and the BM25 search index in sync.
    pub async fn add_attachment(
        session_id: String,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        use crate::state::get_database_connection;
        use crate::state::get_mcp_service_proxy_manager;
        use std::sync::Arc;

        let proxy_manager = get_mcp_service_proxy_manager();

        if let Some(proxy) = proxy_manager.get_proxy(&session_id).await {
            let result = proxy
                .call_builtin_internal("attachments", "addAttachment", args)
                .await?;
            return serde_json::to_value(result)
                .map_err(|e| format!("Failed to serialize MCPResult: {}", e));
        }

        log::debug!(
            "No proxy for session '{}'; falling back to direct AttachmentsServer for add_attachment",
            session_id
        );
        use crate::mcp::builtin::attachments::AttachmentsServer;

        let session_manager =
            get_session_manager().map_err(|e| format!("SessionManager not initialized: {}", e))?;
        let db = get_database_connection();
        let server = AttachmentsServer::new_with_db(
            session_id.clone(),
            Arc::new(session_manager.clone()),
            db.clone(),
        )
        .await?;
        let result = server.add_attachment_internal(args, &session_id).await?;
        serde_json::to_value(result).map_err(|e| format!("Failed to serialize MCPResult: {}", e))
    }

    /// Delete an attachment from the session-scoped attachment store through an internal UI-only API.
    ///
    /// Routes through the session-bound `MCPServiceProxy` so that the same
    /// `AttachmentsServer` instance used by the agent is updated — keeping all
    /// in-memory state consistent.
    pub async fn delete_attachment(
        session_id: String,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        use crate::state::get_database_connection;
        use crate::state::get_mcp_service_proxy_manager;
        use std::sync::Arc;

        let proxy_manager = get_mcp_service_proxy_manager();

        if let Some(proxy) = proxy_manager.get_proxy(&session_id).await {
            let result = proxy
                .call_builtin_internal("attachments", "deleteAttachment", args)
                .await?;
            return serde_json::to_value(result)
                .map_err(|e| format!("Failed to serialize MCPResult: {}", e));
        }

        log::debug!(
            "No proxy for session '{}'; falling back to direct AttachmentsServer for delete_attachment",
            session_id
        );
        use crate::mcp::builtin::attachments::AttachmentsServer;

        let session_manager =
            get_session_manager().map_err(|e| format!("SessionManager not initialized: {}", e))?;
        let db = get_database_connection();
        let server = AttachmentsServer::new_with_db(
            session_id.clone(),
            Arc::new(session_manager.clone()),
            db.clone(),
        )
        .await?;
        let result = server.delete_attachment_internal(args, &session_id).await?;
        serde_json::to_value(result).map_err(|e| format!("Failed to serialize MCPResult: {}", e))
    }

    /// Get service contexts for a session
    pub async fn get_service_contexts(
        session_id: String,
    ) -> Result<std::collections::HashMap<String, crate::mcp::types::ServiceContext>, String> {
        use crate::state::get_mcp_service_proxy_manager;

        let proxy_manager = get_mcp_service_proxy_manager();

        proxy_manager
            .ensure_configured_proxy(&session_id, crate::state::get_app_handle().cloned())
            .await?;

        let proxy = proxy_manager
            .get_proxy(&session_id)
            .await
            .ok_or_else(|| format!("No proxy found for session: {}", session_id))?;

        Ok(proxy.get_service_contexts(None).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::workspace_isolation::DEFAULT_SESSION_IMAGE;

    // #2942 G5.1: the default-session-directory step of prepare_workspace_setup
    // is tested through the extracted helper (the gatekeeper-permitted seam:
    // the global session manager is unavailable in unit tests). The helper is
    // what `prepare_workspace_setup` calls after override registration, and it
    // is keyed by session id only — with or without an override it creates the
    // same `workspaces/<session_id>/` — so these two tests cover both required
    // observations: creation succeeds, and failure is reported not dropped.
    #[test]
    fn ensure_default_session_workspace_dir_creates_the_default_dir() {
        let base = tempfile::tempdir().unwrap();
        let service = crate::services::session_directory_service::SessionDirectoryService::new(
            base.path().to_path_buf(),
        )
        .unwrap();
        let sid = "sess-ensure-ok";

        AgentService::ensure_default_session_workspace_dir(&service, sid).unwrap();

        assert!(base.path().join("workspaces").join(sid).is_dir());
    }

    #[test]
    fn ensure_default_session_workspace_dir_reports_creation_failure() {
        let base = tempfile::tempdir().unwrap();
        let service = crate::services::session_directory_service::SessionDirectoryService::new(
            base.path().to_path_buf(),
        )
        .unwrap();
        let blocked = "sess-ensure-blocked";
        // A regular file at the directory's path makes creation fail
        // deterministically; the error must be returned, not swallowed.
        std::fs::write(
            base.path().join("workspaces").join(blocked),
            "not a directory",
        )
        .unwrap();

        let error =
            AgentService::ensure_default_session_workspace_dir(&service, blocked).unwrap_err();

        assert!(
            error.contains(blocked),
            "error must name the session dir: {error}"
        );
    }

    fn docker_request(
        config: Option<DockerWorkspaceConfig>,
    ) -> Result<(WorkspaceIsolationMode, Option<DockerWorkspaceConfig>), String> {
        AgentService::validate_workspace_isolation_request(
            Some(WorkspaceIsolationMode::Docker),
            config,
            false,
        )
    }

    #[test]
    fn docker_session_without_config_uses_the_default_image() {
        let (mode, config) = docker_request(None).expect("valid request");
        assert_eq!(mode, WorkspaceIsolationMode::Docker);
        assert_eq!(
            config.as_ref().and_then(DockerWorkspaceConfig::image_ref),
            Some(DEFAULT_SESSION_IMAGE)
        );
    }

    #[test]
    fn docker_session_with_blank_image_uses_the_default_image() {
        let (_, config) = docker_request(Some(DockerWorkspaceConfig {
            image: Some("  ".to_string()),
            ..Default::default()
        }))
        .expect("valid request");
        assert_eq!(
            config.as_ref().and_then(DockerWorkspaceConfig::image_ref),
            Some(DEFAULT_SESSION_IMAGE)
        );
    }

    #[test]
    fn docker_session_keeps_the_image_it_sets() {
        let (_, config) = docker_request(Some(DockerWorkspaceConfig {
            image: Some("python:3.12-slim".to_string()),
            ..Default::default()
        }))
        .expect("valid request");
        assert_eq!(
            config.as_ref().and_then(DockerWorkspaceConfig::image_ref),
            Some("python:3.12-slim")
        );
    }

    #[test]
    fn attach_session_gets_no_image() {
        let (_, config) = docker_request(Some(DockerWorkspaceConfig {
            attach_container: Some("dev-box".to_string()),
            ..Default::default()
        }))
        .expect("valid request");
        assert_eq!(
            config.as_ref().and_then(DockerWorkspaceConfig::image_ref),
            None
        );
    }

    #[test]
    fn default_image_is_version_pinned_on_ghcr() {
        let (repository, tag) = DEFAULT_SESSION_IMAGE
            .rsplit_once(':')
            .expect("image has a tag");
        assert_eq!(repository, "ghcr.io/fritzprix/libragent-session");
        assert!(
            tag.split('.').count() == 3 && tag.split('.').all(|part| part.parse::<u32>().is_ok()),
            "tag must be a release version, got '{tag}'"
        );
    }
}
