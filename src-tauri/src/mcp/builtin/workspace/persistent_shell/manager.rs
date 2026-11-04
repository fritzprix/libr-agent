/// Persistent Shell Manager
///
/// Manages session-based persistent shell instances for state preservation
/// across multiple command executions.
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use super::secrets::{mask_secrets, SecretMap};
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
    /// Per-session state that outlives a single shell process.
    session_states: Arc<Mutex<HashMap<String, SessionShellState>>>,
}

#[derive(Default)]
struct SessionShellState {
    /// `storeAs` values, kept in memory only and dropped when the session ends.
    secrets: SecretMap,
    /// Directory to restore when the shell is created again after a restart.
    restart_cwd: Option<String>,
    /// The next tool result tells the agent that shell state was lost.
    restart_notice: bool,
}

impl std::fmt::Debug for SessionShellState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionShellState")
            .field("secret_names", &self.secrets.keys().collect::<Vec<_>>())
            .field("restart_cwd", &self.restart_cwd)
            .field("restart_notice", &self.restart_notice)
            .finish()
    }
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
            session_states: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Stores a session secret and restarts the shell so the next call sees it as `$name`.
    pub async fn store_secret(
        &self,
        session_id: &str,
        name: &str,
        value: String,
    ) -> Result<(), String> {
        self.session_states
            .lock()
            .await
            .entry(session_id.to_string())
            .or_default()
            .secrets
            .insert(name.to_string(), value);
        info!("Stored shell secret {} for session {}", name, session_id);
        self.restart_shell(session_id).await
    }

    /// Ends the session's shell. The next call creates it again with the same
    /// secrets and working directory.
    pub async fn restart_shell(&self, session_id: &str) -> Result<(), String> {
        let cwd = self.get_shell_cwd(session_id).await;
        let result = self.terminate_shell(session_id).await;
        if let Some(cwd) = cwd {
            let mut states = self.session_states.lock().await;
            let state = states.entry(session_id.to_string()).or_default();
            state.restart_cwd = Some(cwd);
            state.restart_notice = true;
        }
        result
    }

    /// Returns whether the shell was restarted since the last call, and clears the flag.
    pub async fn take_restart_notice(&self, session_id: &str) -> bool {
        self.session_states
            .lock()
            .await
            .get_mut(session_id)
            .is_some_and(|state| std::mem::take(&mut state.restart_notice))
    }

    /// Drops the session's secrets. Call when the session ends.
    pub async fn clear_session_state(&self, session_id: &str) {
        self.session_states.lock().await.remove(session_id);
    }

    async fn session_secrets(&self, session_id: &str) -> SecretMap {
        self.session_states
            .lock()
            .await
            .get(session_id)
            .map(|state| state.secrets.clone())
            .unwrap_or_default()
    }

    async fn mask_output(
        &self,
        session_id: &str,
        (stdout, stderr, exit_code, cwd): (String, String, i32, String),
    ) -> (String, String, i32, String) {
        let secrets = self.session_secrets(session_id).await;
        if secrets.is_empty() {
            return (stdout, stderr, exit_code, cwd);
        }
        (
            mask_secrets(&stdout, &secrets),
            mask_secrets(&stderr, &secrets),
            exit_code,
            cwd,
        )
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

        let secrets = self.session_secrets(&session_id).await;
        let mut shell = if let Some(session_repo) = crate::state::try_get_session_repository() {
            match session_repo
                .get_session(&session_id)
                .await
                .map_err(|e| format!("Failed to load session isolation metadata: {e}"))?
            {
                Some(session) if session.workspace_isolation == WorkspaceIsolationMode::Docker => {
                    let spawned_shell =
                        crate::services::WorkspaceRuntimeManager::spawn_docker_persistent_shell(
                            &session, &secrets,
                        )
                        .await
                        .map_err(|error| error.to_string())?;
                    PersistentShell::from_spawned(session_id.clone(), spawned_shell)
                        .await
                        .map_err(|e| format!("Failed to create Docker shell: {e}"))?
                }
                _ => PersistentShell::new_with_env(
                    session_id.clone(),
                    workspace_path,
                    shell_type,
                    &secrets,
                )
                .await
                .map_err(|e| format!("Failed to create shell: {e}"))?,
            }
        } else {
            PersistentShell::new_with_env(session_id.clone(), workspace_path, shell_type, &secrets)
                .await
                .map_err(|e| format!("Failed to create shell: {e}"))?
        };

        let restart_cwd = self
            .session_states
            .lock()
            .await
            .get_mut(&session_id)
            .and_then(|state| state.restart_cwd.take());
        if let Some(cwd) = restart_cwd {
            if let Err(error) = shell.change_directory(&cwd).await {
                warn!(
                    "Restarted shell for session {} stays in its start directory: {}",
                    session_id, error
                );
            }
        }

        let shell_arc = Arc::new(Mutex::new(shell));
        let shell_pid = shell_arc.lock().await.pid();
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
        let result = match self
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
                self.execute_internal(session_id.clone(), workspace_path, command)
                    .await
            }
        }?;
        Ok(self.mask_output(&session_id, result).await)
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
        let result = match self
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
                    session_id.clone(),
                    workspace_path,
                    command,
                    user_input,
                    stdin_delivery,
                )
                .await
            }
        }?;
        Ok(self.mask_output(&session_id, result).await)
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

    async fn force_kill_pid(pid: u32) -> Result<(), String> {
        tokio::task::spawn_blocking(move || crate::utils::process::force_kill_process(pid))
            .await
            .map_err(crate::utils::process::describe_join_error)?
            .map_err(|error| error.to_string())
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
        let live_pid = managed.shell.try_lock().ok().and_then(|shell| shell.pid());
        let pid = live_pid.or(managed.pid);
        let Some(pid) = pid else {
            let mut shell = managed.shell.lock().await;
            shell
                .terminate()
                .await
                .map_err(|error| format!("Failed to terminate persistent shell: {error}"))?;
            return Ok(true);
        };

        Self::force_kill_pid(pid).await.map_err(|error| {
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
        let termination_generation = self.cancellation_generation(session_id).await;
        let managed = self.shells.lock().await.remove(session_id);
        let has_managed_shell = managed.is_some();
        let result = if let Some(managed) = managed {
            info!("Terminating persistent shell for session: {}", session_id);
            if let Some(pid) = managed.pid {
                Self::force_kill_pid(pid)
                    .await
                    .map_err(|error| format!("Failed to terminate shell: {error}"))
            } else {
                managed
                    .shell
                    .lock()
                    .await
                    .terminate()
                    .await
                    .map_err(|e| format!("Failed to terminate shell: {e}"))
            }
        } else {
            Ok(())
        };

        if has_managed_shell {
            let mut generations = self.cancellation_generations.lock().await;
            if generations.get(session_id).copied() == Some(termination_generation) {
                generations.remove(session_id);
            }
        }
        result
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
                if let Err(error) = Self::force_kill_pid(pid).await {
                    warn!(
                        "Failed to terminate persistent shell for session {}: {}",
                        session_id, error
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
