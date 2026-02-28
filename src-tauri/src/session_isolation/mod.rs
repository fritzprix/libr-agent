use tokio::process::Command as AsyncCommand;
use tracing::info;

pub(crate) mod common;
pub mod platforms;
pub mod types;

pub use types::*;

/// Cross-platform session isolation manager
#[derive(Debug, Clone)]
pub struct SessionIsolationManager {
    isolation_config: IsolationConfig,
}

impl Default for SessionIsolationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionIsolationManager {
    pub fn new() -> Self {
        Self {
            isolation_config: IsolationConfig::default(),
        }
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
        platforms::create_medium_isolated_command(config, &self.isolation_config).await
    }

    /// High isolation: platform-specific sandboxing
    async fn create_high_isolated_command(
        &self,
        config: IsolatedProcessConfig,
    ) -> Result<AsyncCommand, String> {
        // Platform modules expose create_high_isolated_command
        platforms::create_high_isolated_command(config, &self.isolation_config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolation_manager_creation() {
        let manager = SessionIsolationManager::new();
        assert!(manager
            .isolation_config
            .resource_limits
            .max_memory_mb
            .is_some());
    }
}
