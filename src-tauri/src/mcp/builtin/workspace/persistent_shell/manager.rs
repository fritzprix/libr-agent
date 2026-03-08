/// Persistent Shell Manager
///
/// Manages session-based persistent shell instances for state preservation
/// across multiple command executions.
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use super::session::PersistentShell;
use crate::session_isolation::types::ShellType;

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

    /// Returns the number of shells currently managed (for testing)
    #[cfg(test)]
    pub(crate) async fn shell_count(&self) -> usize {
        self.shells.lock().await.len()
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
