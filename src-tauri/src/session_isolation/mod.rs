use tokio::process::Command as AsyncCommand;
use tracing::info;

use crate::repositories::session_repository::SessionRepository;

pub(crate) mod common;
pub mod path_mapper;
pub mod platforms;
pub mod runtime;
pub mod types;

pub use path_mapper::PathMappingLayer;
pub use runtime::{ShellDialect, SpawnedShell};
pub use types::*;

/// Cross-platform session isolation manager
#[derive(Debug, Clone)]
pub struct SessionIsolationManager;

impl Default for SessionIsolationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionIsolationManager {
    pub fn new() -> Self {
        Self
    }

    /// Create an isolated command based on the current platform
    pub async fn create_isolated_command(
        &self,
        config: IsolatedProcessConfig,
    ) -> Result<AsyncCommand, String> {
        info!(
            "Creating isolated command for session: {}",
            config.session_id
        );

        if let Some(session_repo) = crate::state::try_get_session_repository() {
            if let Some(session) = session_repo
                .get_session(&config.session_id)
                .await
                .map_err(|e| format!("Failed to load session isolation metadata: {e}"))?
            {
                if session.workspace_isolation
                    == crate::models::workspace_isolation::WorkspaceIsolationMode::Docker
                {
                    return crate::services::WorkspaceRuntimeManager::create_docker_exec_command(
                        &session,
                        &config.command,
                        &config.env_vars,
                    )
                    .await
                    .map_err(|error| error.to_string());
                }
            }
        }

        match config.isolation_level {
            IsolationLevel::Basic => self.create_basic_isolated_command(config).await,
            IsolationLevel::Medium => self.create_medium_isolated_command(config).await,
            IsolationLevel::High => self.create_high_isolated_command(config).await,
        }
    }

    /// Basic isolation: environment variables and working directory
    async fn create_basic_isolated_command(
        &self,
        config: IsolatedProcessConfig,
    ) -> Result<AsyncCommand, String> {
        platforms::create_basic_isolated_command(config).await
    }

    /// Medium isolation: process groups + resource limits
    async fn create_medium_isolated_command(
        &self,
        config: IsolatedProcessConfig,
    ) -> Result<AsyncCommand, String> {
        platforms::create_medium_isolated_command(config).await
    }

    /// High isolation: platform-specific sandboxing
    async fn create_high_isolated_command(
        &self,
        config: IsolatedProcessConfig,
    ) -> Result<AsyncCommand, String> {
        // Platform modules expose create_high_isolated_command
        platforms::create_high_isolated_command(config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolation_manager_creation() {
        let _manager = SessionIsolationManager::new();
    }
}
