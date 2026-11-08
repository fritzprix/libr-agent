use super::AgentSessionManager;
use crate::agent::tauri_events::TauriEventDispatcher;
use crate::repositories::{SessionListCursor, SessionListPage, SessionMetadata};
use std::sync::Arc;

impl AgentSessionManager {
    /// Create a new session
    pub async fn create_session(
        &self,
        session_id: String,
        name: Option<String>,
        model: Option<String>,
        provider: Option<String>,
        agent_config: crate::agent::AgentConfig,
    ) -> Result<SessionMetadata, String> {
        self.create_session_with_repo(
            self.session_repo.clone(),
            session_id,
            name,
            model,
            provider,
            agent_config,
            crate::models::workspace_isolation::WorkspaceIsolationMode::Host,
            None,
        )
        .await
    }

    /// Create or update a session with a specific repository
    #[allow(clippy::too_many_arguments)]
    pub async fn create_session_with_repo(
        &self,
        session_repo: Arc<dyn crate::repositories::SessionRepository>,
        session_id: String,
        name: Option<String>,
        model: Option<String>,
        provider: Option<String>,
        agent_config: crate::agent::AgentConfig,
        workspace_isolation: crate::models::workspace_isolation::WorkspaceIsolationMode,
        docker_config: Option<crate::models::workspace_isolation::DockerWorkspaceConfig>,
    ) -> Result<SessionMetadata, String> {
        crate::agent::lifecycle::create_session(crate::agent::lifecycle::CreateSessionParams {
            session_repo,
            active_sessions: self.active_sessions.clone(),
            proxy_manager: self.proxy_manager.clone(),
            app_handle: self.app_handle.clone(),
            context_registry: self.context_registry.clone(),
            session_id,
            name,
            model,
            provider,
            agent_config,
            workspace_isolation,
            docker_config,
        })
        .await
    }

    /// Update session configuration (model, provider, and/or assistant binding)
    pub async fn update_session_config(
        &self,
        session_id: String,
        model: Option<String>,
        provider: Option<String>,
        assistant_id: Option<String>,
        recursive: Option<bool>,
    ) -> Result<(), String> {
        crate::agent::lifecycle::update_session_config(
            &self.session_repo,
            &self.active_sessions,
            &self.app_handle,
            &session_id,
            model,
            provider,
            assistant_id,
            recursive,
        )
        .await
    }

    /// Resume an existing session by loading it into active sessions
    pub async fn resume_session(&self, session_id: &str) -> Result<SessionMetadata, String> {
        let result = crate::agent::lifecycle::resume_session(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            self.context_registry.clone(),
            session_id,
        )
        .await?;

        super::approvals::reemit_pending_approvals_on_resume(self, session_id).await;

        Ok(result)
    }

    /// Get session metadata
    pub async fn get_session(&self, session_id: &str) -> Result<Option<SessionMetadata>, String> {
        crate::agent::lifecycle::get_session(&self.session_repo, session_id).await
    }

    /// Get all sessions
    pub async fn get_all_sessions(&self) -> Result<Vec<SessionMetadata>, String> {
        crate::agent::lifecycle::get_all_sessions(&self.session_repo).await
    }

    pub async fn list_sessions(
        &self,
        cursor: Option<SessionListCursor>,
        limit: u64,
        search: Option<&str>,
        bookmarked_only: bool,
        status: Option<&str>,
    ) -> Result<SessionListPage, String> {
        crate::agent::lifecycle::list_sessions(
            &self.session_repo,
            cursor,
            limit,
            search,
            bookmarked_only,
            status,
        )
        .await
    }

    pub async fn list_attention_sessions(&self) -> Result<Vec<SessionMetadata>, String> {
        crate::agent::lifecycle::list_attention_sessions(&self.session_repo).await
    }

    /// Recover sessions stuck in BUSY state after app crash/restart
    pub async fn recover_sessions(&self) -> Result<(), String> {
        let dispatcher = TauriEventDispatcher::new(self.app_handle.clone());
        let message_repo = self.message_repo();
        crate::agent::lifecycle::recover_sessions_with_dispatcher_and_message_repo(
            &self.session_repo,
            &self.active_sessions,
            &dispatcher,
            self.context_registry.clone(),
            message_repo.as_ref(),
        )
        .await?;

        crate::services::docker_provisioning::recover_provisioning_sessions(
            &crate::services::docker_provisioning::DockerProvisioningDeps {
                session_repo: Arc::clone(&self.session_repo),
                active_sessions: Arc::clone(&self.active_sessions),
                proxy_manager: Arc::clone(&self.proxy_manager),
                app_handle: self.app_handle.clone(),
            },
        )
        .await
    }

    /// Load messages from DB into in-memory cache
    pub async fn init_session_with_messages(&self, session_id: &str) -> Result<(), String> {
        if let Some(repo) = self.message_repo() {
            crate::agent::lifecycle::init_session_with_messages_with_repo(
                &self.active_sessions,
                repo.as_ref(),
                session_id,
            )
            .await
        } else {
            crate::agent::lifecycle::init_session_with_messages(&self.active_sessions, session_id)
                .await
        }
    }

    pub async fn ensure_session_active(&self, session_id: &str) -> Result<(), String> {
        let is_active = {
            let active = self.active_sessions.read().await;
            active.contains_key(session_id)
        };

        if is_active {
            return Ok(());
        }

        self.resume_session(session_id).await?;
        self.init_session_with_messages(session_id).await?;
        Ok(())
    }

    pub async fn get_session_display_name(&self, session_id: &str) -> Option<String> {
        let active = self.active_sessions.read().await;
        active.get(session_id).map(|session| {
            session
                .metadata
                .name
                .clone()
                .unwrap_or_else(|| session_id.chars().take(8).collect::<String>())
        })
    }

    /// Delete an agent session and cascade through its descendants.
    ///
    /// This removes the target session together with child sessions and their
    /// associated state, messages, workspace data, and search index entries.
    /// Use `delete_session_only` when descendants should remain as top-level
    /// sessions instead of being deleted.
    pub async fn delete_session(&self, session_id: String) -> Result<Vec<String>, String> {
        let deleted_ids = crate::agent::lifecycle::delete_session(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
        )
        .await?;
        Ok(deleted_ids)
    }

    /// Delete only this session, leaving children as orphaned top-level sessions.
    pub async fn delete_session_only(
        &self,
        session_id: String,
    ) -> Result<(String, Vec<String>), String> {
        crate::agent::lifecycle::delete_session_only(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
        )
        .await
    }
}
