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
use crate::models::workspace_isolation::WorkspaceIsolationMode;
use crate::repositories::SessionRepository;
use crate::session_isolation::types::ShellType;

/// Manager for persistent shell sessions
///
/// Maintains a pool of shell instances mapped by session ID,
/// handling lifecycle management and cleanup.
#[derive(Debug)]
pub struct PersistentShellManager {
    /// session_id -> PersistentShell mapping
    shells: Arc<Mutex<HashMap<String, ManagedShell>>>,
    /// Monotonically increasing cancellation generation per session.
    ///
    /// An in-flight command captures the generation before execution. If a
    /// session is cancelled while that command is running, the generation
    /// changes and the normal crash-recovery retry is suppressed.
    cancellation_generations: Arc<Mutex<HashMap<String, u64>>>,
}

#[derive(Debug)]
struct ManagedShell {
    shell: Arc<Mutex<PersistentShell>>,
    pid: Option<u32>,
}

impl PersistentShellManager {
    /// Create a new persistent shell manager
    pub fn new() -> Self {
        Self {
            shells: Arc::new(Mutex::new(HashMap::new())),
            cancellation_generations: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Returns the number of shells currently managed.
    pub async fn shell_count(&self) -> usize {
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
        if let Some(shell) = self
            .shells
            .lock()
            .await
            .get(&session_id)
            .map(|managed| managed.shell.clone())
        {
            let shell_guard = shell.lock().await;
            if shell_guard.pid().is_some() {
                debug!("Reusing existing shell for session: {}", session_id);
                drop(shell_guard);
                return Ok(shell.clone());
            }

            debug!("Removing dead shell for session: {}", session_id);
            drop(shell_guard);
            self.remove_shell(&session_id).await;
        }

        // Create new shell
        info!("Creating new persistent shell for session: {}", session_id);
        #[cfg(unix)]
        let shell_type = ShellType::Bash;
        #[cfg(windows)]
        let shell_type = ShellType::PowerShell; // Default to PowerShell on Windows

        let shell = if let Some(session_repo) = crate::state::try_get_session_repository() {
            match session_repo
                .get_session(&session_id)
                .await
                .map_err(|e| format!("Failed to load session isolation metadata: {e}"))?
            {
                Some(session) if session.workspace_isolation == WorkspaceIsolationMode::Docker => {
                    let spawned_shell =
                        crate::services::WorkspaceRuntimeManager::spawn_docker_persistent_shell(
                            &session,
                        )
                        .await
                        .map_err(|error| error.to_string())?;
                    PersistentShell::from_spawned(session_id.clone(), spawned_shell)
                        .await
                        .map_err(|e| format!("Failed to create Docker shell: {e}"))?
                }
                _ => PersistentShell::new(session_id.clone(), workspace_path, shell_type)
                    .await
                    .map_err(|e| format!("Failed to create shell: {e}"))?,
            }
        } else {
            PersistentShell::new(session_id.clone(), workspace_path, shell_type)
                .await
                .map_err(|e| format!("Failed to create shell: {e}"))?
        };

        let shell_arc = Arc::new(Mutex::new(shell));
        let shell_pid = shell_arc.lock().await.process_id();
        let mut shells = self.shells.lock().await;
        if let Some(existing) = shells.get(&session_id) {
            let existing_shell = existing.shell.clone();
            drop(shells);
            debug!(
                "Discarding concurrently created duplicate shell for session: {}",
                session_id
            );
            return Ok(existing_shell);
        }
        shells.insert(
            session_id.clone(),
            ManagedShell {
                shell: shell_arc.clone(),
                pid: shell_pid,
            },
        );

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
        let cancellation_generation = self.cancellation_generation(&session_id).await;

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

                if self.cancellation_generation(&session_id).await != cancellation_generation {
                    return Err(format!(
                        "Shell execution cancelled for session {session_id}"
                    ));
                }

                // Remove dead shell
                self.remove_shell(&session_id).await;

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
        stdin_delivery: crate::mcp::builtin::workspace::StdinDelivery,
    ) -> Result<(String, String, i32, String), String> {
        let cancellation_generation = self.cancellation_generation(&session_id).await;

        // Try with retry on failure
        match self
            .execute_with_input_internal(
                session_id.clone(),
                workspace_path.clone(),
                command,
                user_input,
                stdin_delivery,
            )
            .await
        {
            Ok(result) => Ok(result),
            Err(e) => {
                warn!(
                    "Shell execution with input failed for session {}: {}. Attempting recovery...",
                    session_id, e
                );

                if self.cancellation_generation(&session_id).await != cancellation_generation {
                    return Err(format!(
                        "Shell execution with input cancelled for session {session_id}"
                    ));
                }

                // Remove dead shell
                self.remove_shell(&session_id).await;

                // Retry once with new shell
                self.execute_with_input_internal(
                    session_id,
                    workspace_path,
                    command,
                    user_input,
                    stdin_delivery,
                )
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
        stdin_delivery: crate::mcp::builtin::workspace::StdinDelivery,
    ) -> Result<(String, String, i32, String), String> {
        let shell = self.get_or_create_shell(session_id, workspace_path).await?;
        let mut shell_guard = shell.lock().await;

        shell_guard
            .execute_with_input(command, user_input, stdin_delivery)
            .await
            .map_err(|e| format!("Shell execution with input failed: {e}"))
    }

    /// Get the current working directory for a session's persistent shell
    pub async fn get_shell_cwd(&self, session_id: &str) -> Option<String> {
        let shell = self
            .shells
            .lock()
            .await
            .get(session_id)
            .map(|managed| managed.shell.clone())?;
        let shell = shell.lock().await;
        Some(shell.get_cwd().to_string())
    }

    pub(crate) async fn cancellation_generation(&self, session_id: &str) -> u64 {
        self.cancellation_generations
            .lock()
            .await
            .get(session_id)
            .copied()
            .unwrap_or(0)
    }

    async fn remove_shell(&self, session_id: &str) -> Option<ManagedShell> {
        self.shells.lock().await.remove(session_id)
    }

    /// Force-kill and remove a session's persistent shell without waiting for
    /// its command mutex. This is used by workflow cancellation.
    pub async fn force_kill_shell(&self, session_id: &str) -> Result<bool, String> {
        let generation = {
            let mut generations = self.cancellation_generations.lock().await;
            let generation = generations.entry(session_id.to_string()).or_insert(0);
            *generation = generation.saturating_add(1);
            *generation
        };

        let managed = self.shells.lock().await.remove(session_id);
        let Some(managed) = managed else {
            debug!(
                "No persistent shell to kill for session {} (generation {})",
                session_id, generation
            );
            return Ok(false);
        };

        // Prefer the live child handle. If an in-flight command owns the mutex,
        // use the PID captured at registration rather than waiting for it.
        let live_pid = managed
            .shell
            .try_lock()
            .ok()
            .and_then(|shell| shell.process_id());
        let pid = live_pid.or(managed.pid);
        let Some(pid) = pid else {
            let mut shell = managed.shell.lock().await;
            shell
                .terminate()
                .await
                .map_err(|error| format!("Failed to terminate persistent shell: {error}"))?;
            return Ok(true);
        };

        tokio::task::spawn_blocking(move || crate::utils::process::force_kill_process_tree(pid))
            .await
            .map_err(crate::utils::process::describe_join_error)?
            .map_err(|error| {
                format!("Failed to force-kill persistent shell for session {session_id}: {error}")
            })?;

        info!(
            "Force-killed persistent shell for session {} (PID {})",
            session_id, pid
        );
        Ok(true)
    }

    /// Terminate shell for session
    ///
    /// Gracefully terminates and removes the shell instance.
    pub async fn terminate_shell(&self, session_id: &str) -> Result<(), String> {
        let managed = self.shells.lock().await.remove(session_id);
        if let Some(managed) = managed {
            info!("Terminating persistent shell for session: {}", session_id);
            if let Some(pid) = managed.pid {
                tokio::task::spawn_blocking(move || {
                    crate::utils::process::force_kill_process_tree(pid)
                })
                .await
                .map_err(crate::utils::process::describe_join_error)?
                .map_err(|error| format!("Failed to terminate shell: {error}"))?;
            } else {
                managed
                    .shell
                    .lock()
                    .await
                    .terminate()
                    .await
                    .map_err(|e| format!("Failed to terminate shell: {e}"))?;
            }
        }
        Ok(())
    }

    /// Terminates all active shells. Used during shutdown and integration tests.
    pub async fn cleanup_all(&self) -> Result<(), String> {
        let (count, shells) = {
            let mut managed_shells = self.shells.lock().await;
            let count = managed_shells.len();
            let shells = managed_shells.drain().collect::<Vec<_>>();
            (count, shells)
        };

        info!("Cleaning up {} persistent shell(s)", count);

        for (session_id, managed) in shells {
            debug!("Terminating shell for session: {}", session_id);
            if let Some(pid) = managed.pid {
                if let Err(error) = tokio::task::spawn_blocking(move || {
                    crate::utils::process::force_kill_process_tree(pid)
                })
                .await
                {
                    warn!(
                        "Failed to terminate persistent shell for session {}: {}",
                        session_id,
                        crate::utils::process::describe_join_error(error)
                    );
                }
            } else {
                let _ = managed.shell.lock().await.terminate().await;
            }
        }

        Ok(())
    }
}

impl Default for PersistentShellManager {
    fn default() -> Self {
        Self::new()
    }
}
