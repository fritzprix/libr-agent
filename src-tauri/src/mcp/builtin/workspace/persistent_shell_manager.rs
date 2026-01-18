/// Persistent Shell Manager
///
/// Manages session-based persistent shell instances for state preservation
/// across multiple command executions.
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use super::persistent_shell::PersistentShell;
use super::ShellType;

/// Manager for persistent shell sessions
///
/// Maintains a pool of shell instances mapped by session ID,
/// handling lifecycle management and cleanup.
#[derive(Debug)]
pub struct PersistentShellManager {
    /// session_id -> PersistentShell mapping
    shells: Arc<Mutex<HashMap<String, Arc<Mutex<PersistentShell>>>>>,

    /// Maximum shells per session (resource limit)
    #[allow(dead_code)]
    max_shells_per_session: usize,
}

impl PersistentShellManager {
    /// Create a new persistent shell manager
    pub fn new() -> Self {
        Self {
            shells: Arc::new(Mutex::new(HashMap::new())),
            max_shells_per_session: 3,
        }
    }

    /// Get or create persistent shell for session
    ///
    /// Returns existing shell if alive, otherwise creates new one.
    /// Dead shells are automatically cleaned up.
    pub async fn get_or_create_shell(
        &self,
        session_id: String,
        workspace_path: std::path::PathBuf,
    ) -> Result<Arc<Mutex<PersistentShell>>, String> {
        let mut shells = self.shells.lock().await;

        // Check if shell exists and is still alive
        if let Some(shell) = shells.get(&session_id) {
            let shell_guard = shell.lock().await;
            if shell_guard.pid().is_some() {
                debug!("Reusing existing shell for session: {}", session_id);
                drop(shell_guard); // Release lock before returning
                return Ok(shell.clone());
            } else {
                // Dead shell, remove it
                debug!("Removing dead shell for session: {}", session_id);
                drop(shell_guard);
                shells.remove(&session_id);
            }
        }

        // Create new shell
        info!("Creating new persistent shell for session: {}", session_id);
        #[cfg(unix)]
        let shell_type = ShellType::Bash;
        #[cfg(windows)]
        let shell_type = ShellType::PowerShell; // Default to PowerShell on Windows

        let shell = PersistentShell::new(session_id.clone(), workspace_path, shell_type)
            .await
            .map_err(|e| format!("Failed to create shell: {e}"))?;

        let shell_arc = Arc::new(Mutex::new(shell));
        shells.insert(session_id.clone(), shell_arc.clone());

        Ok(shell_arc)
    }

    /// Execute command in persistent shell
    ///
    /// Automatically handles shell creation and retries on crash.
    /// Returns (stdout, stderr, exit_code, cwd).
    pub async fn execute(
        &self,
        session_id: String,
        workspace_path: std::path::PathBuf,
        command: &str,
    ) -> Result<(String, String, i32, String), String> {
        // Try to execute with retry on failure
        match self
            .execute_internal(session_id.clone(), workspace_path.clone(), command)
            .await
        {
            Ok(result) => Ok(result),
            Err(e) => {
                warn!(
                    "Shell execution failed for session {}: {}. Attempting recovery...",
                    session_id, e
                );

                // Remove dead shell
                let mut shells = self.shells.lock().await;
                shells.remove(&session_id);
                drop(shells);

                // Retry once with new shell
                self.execute_internal(session_id, workspace_path, command)
                    .await
            }
        }
    }

    /// Internal execute helper (no retry logic)
    async fn execute_internal(
        &self,
        session_id: String,
        workspace_path: std::path::PathBuf,
        command: &str,
    ) -> Result<(String, String, i32, String), String> {
        let shell = self.get_or_create_shell(session_id, workspace_path).await?;
        let mut shell_guard = shell.lock().await;

        shell_guard
            .execute(command)
            .await
            .map_err(|e| format!("Shell execution failed: {e}"))
    }

    /// Execute command with user input (Two-Tool Pattern)
    ///
    /// Injects user input via stdin before executing command.
    /// Used for interactive commands like sudo.
    pub async fn execute_with_input(
        &self,
        session_id: String,
        workspace_path: std::path::PathBuf,
        command: &str,
        user_input: &str,
    ) -> Result<(String, String, i32, String), String> {
        // Try with retry on failure
        match self
            .execute_with_input_internal(
                session_id.clone(),
                workspace_path.clone(),
                command,
                user_input,
            )
            .await
        {
            Ok(result) => Ok(result),
            Err(e) => {
                warn!(
                    "Shell execution with input failed for session {}: {}. Attempting recovery...",
                    session_id, e
                );

                // Remove dead shell
                let mut shells = self.shells.lock().await;
                shells.remove(&session_id);
                drop(shells);

                // Retry once with new shell
                self.execute_with_input_internal(session_id, workspace_path, command, user_input)
                    .await
            }
        }
    }

    /// Internal execute with input helper (no retry logic)
    async fn execute_with_input_internal(
        &self,
        session_id: String,
        workspace_path: std::path::PathBuf,
        command: &str,
        user_input: &str,
    ) -> Result<(String, String, i32, String), String> {
        let shell = self.get_or_create_shell(session_id, workspace_path).await?;
        let mut shell_guard = shell.lock().await;

        shell_guard
            .execute_with_input(command, user_input)
            .await
            .map_err(|e| format!("Shell execution with input failed: {e}"))
    }

    /// Get the current working directory for a session's persistent shell
    pub async fn get_shell_cwd(&self, session_id: &str) -> Option<String> {
        let shells = self.shells.lock().await;
        if let Some(shell) = shells.get(session_id) {
            let shell = shell.lock().await;
            Some(shell.get_cwd().to_string())
        } else {
            None
        }
    }

    /// Terminate shell for session
    ///
    /// Gracefully terminates and removes the shell instance.
    pub async fn terminate_shell(&self, session_id: &str) -> Result<(), String> {
        let mut shells = self.shells.lock().await;

        if let Some(shell) = shells.remove(session_id) {
            info!("Terminating persistent shell for session: {}", session_id);
            shell
                .lock()
                .await
                .terminate()
                .await
                .map_err(|e| format!("Failed to terminate shell: {e}"))?;
        }
        Ok(())
    }

    /// Cleanup all shells
    ///
    /// Terminates all active shells. Used during shutdown.
    #[allow(dead_code)]
    pub async fn cleanup_all(&self) -> Result<(), String> {
        let mut shells = self.shells.lock().await;
        let count = shells.len();

        info!("Cleaning up {} persistent shell(s)", count);

        for (session_id, shell) in shells.drain() {
            debug!("Terminating shell for session: {}", session_id);
            let _ = shell.lock().await.terminate().await;
        }

        Ok(())
    }
}

impl Default for PersistentShellManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shell_creation_and_reuse() -> Result<()> {
        let manager = PersistentShellManager::new();
        let session_id = "test-session".to_string();
        let workspace_path = std::env::temp_dir().join("test_shell_reuse");
        std::fs::create_dir_all(&workspace_path)?;

        // First call should create new shell
        let shell1 = manager
            .get_or_create_shell(session_id.clone(), workspace_path.clone())
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        let pid1 = shell1.lock().await.pid();

        // Second call should reuse same shell
        let shell2 = manager
            .get_or_create_shell(session_id.clone(), workspace_path.clone())
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        let pid2 = shell2.lock().await.pid();

        assert_eq!(pid1, pid2, "Should reuse same shell instance");

        manager
            .terminate_shell(&session_id)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        let _ = std::fs::remove_dir_all(&workspace_path);
        Ok(())
    }

    #[tokio::test]
    async fn test_execute_basic_command() -> Result<()> {
        let manager = PersistentShellManager::new();
        let session_id = "test-exec".to_string();
        let workspace_path = std::env::temp_dir().join("test_execute_basic");
        std::fs::create_dir_all(&workspace_path)?;

        #[cfg(unix)]
        let (stdout, _, exit_code, _cwd) = manager
            .execute(
                session_id.clone(),
                workspace_path.clone(),
                "echo 'Hello World'",
            )
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        #[cfg(windows)]
        let (stdout, _, exit_code, _cwd) = manager
            .execute(
                session_id.clone(),
                workspace_path.clone(),
                "Write-Output 'Hello World'",
            )
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        assert_eq!(exit_code, 0);
        assert!(stdout.contains("Hello World"));

        manager
            .terminate_shell(&session_id)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        let _ = std::fs::remove_dir_all(&workspace_path);
        Ok(())
    }

    #[tokio::test]
    async fn test_state_persistence_across_commands() -> Result<()> {
        let manager = PersistentShellManager::new();
        let session_id = "test-state".to_string();
        let workspace_path = std::env::temp_dir().join("test_state_persistence");
        std::fs::create_dir_all(&workspace_path)?;

        #[cfg(unix)]
        {
            // Set environment variable
            manager
                .execute(
                    session_id.clone(),
                    workspace_path.clone(),
                    "export TEST_VAR=TestValue",
                )
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

            // Verify it persists
            let (stdout, _, exit_code, _cwd) = manager
                .execute(session_id.clone(), workspace_path.clone(), "echo $TEST_VAR")
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

            assert_eq!(exit_code, 0);
            assert!(stdout.contains("TestValue"));
        }

        #[cfg(windows)]
        {
            // Set environment variable
            manager
                .execute(
                    session_id.clone(),
                    workspace_path.clone(),
                    "$env:TEST_VAR='TestValue'",
                )
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

            // Verify it persists
            let (stdout, _, exit_code, _cwd) = manager
                .execute(
                    session_id.clone(),
                    workspace_path.clone(),
                    "echo $env:TEST_VAR",
                )
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

            assert_eq!(exit_code, 0);
            assert!(stdout.contains("TestValue"));
        }

        manager
            .terminate_shell(&session_id)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        let _ = std::fs::remove_dir_all(&workspace_path);
        Ok(())
    }

    #[tokio::test]
    async fn test_cleanup_all() -> Result<()> {
        let manager = PersistentShellManager::new();
        let ws1 = std::env::temp_dir().join("test_cleanup_1");
        let ws2 = std::env::temp_dir().join("test_cleanup_2");
        let ws3 = std::env::temp_dir().join("test_cleanup_3");
        std::fs::create_dir_all(&ws1)?;
        std::fs::create_dir_all(&ws2)?;
        std::fs::create_dir_all(&ws3)?;

        // Create multiple shells
        manager
            .get_or_create_shell("session1".to_string(), ws1.clone())
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        manager
            .get_or_create_shell("session2".to_string(), ws2.clone())
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        manager
            .get_or_create_shell("session3".to_string(), ws3.clone())
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        // Cleanup all
        manager
            .cleanup_all()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        // Verify all shells are removed
        let shells = manager.shells.lock().await;
        assert_eq!(shells.len(), 0, "All shells should be cleaned up");

        let _ = std::fs::remove_dir_all(&ws1);
        let _ = std::fs::remove_dir_all(&ws2);
        let _ = std::fs::remove_dir_all(&ws3);
        Ok(())
    }
}
