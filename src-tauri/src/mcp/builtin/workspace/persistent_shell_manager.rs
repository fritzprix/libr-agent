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
        let shell = PersistentShell::new(session_id.clone(), workspace_path)
            .await
            .map_err(|e| format!("Failed to create shell: {e}"))?;

        let shell_arc = Arc::new(Mutex::new(shell));
        shells.insert(session_id.clone(), shell_arc.clone());

        Ok(shell_arc)
    }

    /// Execute command in persistent shell (Atomic Mode)
    pub async fn execute(
        &self,
        session_id: String,
        workspace_path: std::path::PathBuf,
        command: &str,
    ) -> Result<(String, String, i32, String), String> {
        let shell = self.get_or_create_shell(session_id.clone(), workspace_path.clone()).await?;
        let mut shell_guard = shell.lock().await;

        // Try execution, retry once on failure (e.g., if shell died unexpectedly)
        match shell_guard.execute(command).await {
            Ok(res) => Ok(res),
            Err(e) => {
                warn!("Shell execution failed for session {}: {}. Retrying...", session_id, e);
                drop(shell_guard);

                // Remove dead shell
                {
                    let mut shells = self.shells.lock().await;
                    shells.remove(&session_id);
                }

                // Create new shell and retry
                let shell = self.get_or_create_shell(session_id, workspace_path).await?;
                let mut shell_guard = shell.lock().await;
                shell_guard.execute(command).await.map_err(|e| e.to_string())
            }
        }
    }

    /// Execute command with user input (Two-Tool Pattern) - Atomic Mode
    pub async fn execute_with_input(
        &self,
        session_id: String,
        workspace_path: std::path::PathBuf,
        command: &str,
        user_input: &str,
    ) -> Result<(String, String, i32, String), String> {
        let shell = self.get_or_create_shell(session_id.clone(), workspace_path.clone()).await?;
        let mut shell_guard = shell.lock().await;

        match shell_guard.execute_with_input(command, user_input).await {
            Ok(res) => Ok(res),
            Err(e) => {
                warn!("Shell execution with input failed: {}. Retrying...", e);
                drop(shell_guard);

                {
                    let mut shells = self.shells.lock().await;
                    shells.remove(&session_id);
                }

                let shell = self.get_or_create_shell(session_id, workspace_path).await?;
                let mut shell_guard = shell.lock().await;
                shell_guard.execute_with_input(command, user_input).await.map_err(|e| e.to_string())
            }
        }
    }

    // --- Interactive Mode Methods ---

    /// Write input to the persistent shell
    pub async fn write_interactive(
        &self,
        session_id: String,
        workspace_path: std::path::PathBuf,
        data: &str,
    ) -> Result<(), String> {
        let shell = self.get_or_create_shell(session_id, workspace_path).await?;
        let mut shell_guard = shell.lock().await;

        shell_guard.write_input(data).await.map_err(|e| e.to_string())
    }

    /// Read output from the persistent shell
    pub async fn read_interactive(
        &self,
        session_id: String,
    ) -> Result<String, String> {
        let shells = self.shells.lock().await;
        if let Some(shell) = shells.get(&session_id) {
            let shell_guard = shell.lock().await;
            Ok(shell_guard.read_output().await)
        } else {
             Err(format!("No active shell for session {}", session_id))
        }
    }

    /// Create a new interactive shell explicitly
    pub async fn create_interactive(
        &self,
        session_id: String,
        workspace_path: std::path::PathBuf,
    ) -> Result<String, String> {
        let _ = self.get_or_create_shell(session_id.clone(), workspace_path).await?;
        Ok(format!("Interactive shell created for session {}", session_id))
    }

    /// Kill the interactive shell
    pub async fn kill_interactive(&self, session_id: &str) -> Result<String, String> {
        self.terminate_shell(session_id).await?;
        Ok(format!("Interactive shell killed for session {}", session_id))
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
