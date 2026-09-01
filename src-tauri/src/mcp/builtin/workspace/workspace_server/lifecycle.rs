use super::{clear_context_cache, WorkspaceServer};
use crate::mcp::builtin::workspace::{terminal_manager, types::PendingShellInputResolution};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::info;

pub(super) fn start_cleanup_task(
    registry: terminal_manager::ProcessRegistry,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        use std::time::Duration;
        let mut interval = tokio::time::interval(Duration::from_secs(3600));

        while !shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            interval.tick().await;
            if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            cleanup_old_processes(&registry).await;
        }
    })
}

async fn cleanup_old_processes(registry: &terminal_manager::ProcessRegistry) {
    let cutoff = chrono::Utc::now() - chrono::Duration::hours(24);
    let mut reg = registry.write().await;

    let to_remove: Vec<String> = reg
        .entries
        .values()
        .filter(|e| {
            matches!(
                e.status,
                terminal_manager::ProcessStatus::Finished
                    | terminal_manager::ProcessStatus::Failed
                    | terminal_manager::ProcessStatus::Killed
            )
        })
        .filter(|e| e.finished_at.is_some_and(|t| t < cutoff))
        .map(|e| e.id.clone())
        .collect();

    for id in to_remove {
        if let Some(entry) = reg.entries.remove(&id) {
            reg.cancellation_tokens.remove(&id);
            reg.completion_notifiers.remove(&id);
            if let Some(parent) = std::path::PathBuf::from(&entry.stdout_path).parent() {
                let _ = tokio::fs::remove_dir_all(parent).await;
            }
            tracing::info!(
                "Cleaned up old process: {} (polls: {}, poll_streak: {})",
                id,
                entry.poll_count,
                entry.poll_tracker.consecutive_identical()
            );
        }
    }
}

impl WorkspaceServer {
    /// Cancel active resources owned by a session while retaining process
    /// metadata and output files for the Process Panel.
    pub async fn kill_session_processes(&self, session_id: &str) -> Result<usize, String> {
        let mut process_ids = Vec::new();
        let mut process_pids = Vec::new();
        let mut completion_notifiers = Vec::new();

        {
            let mut registry = self.process_registry.write().await;
            let finished_at = chrono::Utc::now();
            let active_process_ids: Vec<String> = registry
                .entries
                .values()
                .filter(|entry| {
                    entry.session_id == session_id
                        && terminal_manager::is_active_process_status(&entry.status)
                })
                .map(|entry| entry.id.clone())
                .collect();

            for process_id in active_process_ids {
                if let Some(entry) = registry.entries.get_mut(&process_id) {
                    entry.status = terminal_manager::ProcessStatus::Killed;
                    entry.finished_at = Some(finished_at);
                    if let Some(pid) = entry.pid {
                        process_pids.push(pid);
                    }
                    process_ids.push(process_id.clone());
                }

                if let Some(token) = registry.cancellation_tokens.get(&process_id) {
                    token.cancel();
                }
                if let Some(notifier) = registry.completion_notifiers.get(&process_id) {
                    completion_notifiers.push(notifier.clone());
                }
            }
        }

        for notifier in completion_notifiers {
            notifier.notify_waiters();
        }

        if !process_pids.is_empty() {
            let kill_result = tokio::task::spawn_blocking(move || {
                for pid in process_pids {
                    if let Err(error) = crate::utils::process::force_kill_process_tree(pid) {
                        tracing::warn!("Failed to kill cancelled process {pid}: {error}");
                    }
                }
            })
            .await;

            if let Err(error) = kill_result {
                tracing::warn!(
                    "Process cancellation task failed: {}",
                    crate::utils::process::describe_join_error(error)
                );
            }
        }

        for pending in self.pending_executions.remove_for_session(session_id) {
            if let Some(response_tx) = pending.response_tx {
                let _ = response_tx.send(PendingShellInputResolution::Cancelled);
            }
        }

        let persistent_shell_killed = match self.shell_manager.force_kill_shell(session_id).await {
            Ok(killed) => killed,
            Err(error) => {
                tracing::warn!(
                    "Failed to terminate persistent shell for cancelled session {}: {}",
                    session_id,
                    error
                );
                false
            }
        };

        clear_context_cache(&self.context_cache).await;

        let killed_resource_count = process_ids.len() + usize::from(persistent_shell_killed);
        info!(
            "Cancelled {} active workspace resource(s) for session {}",
            killed_resource_count, session_id
        );
        Ok(killed_resource_count)
    }

    /// Session cleanup: terminate and clean up all processes for a session
    #[allow(dead_code)]
    pub async fn on_session_end(&self, session_id: &str) {
        info!("Cleaning up processes for session: {}", session_id);
        let session_entries = {
            let mut reg = self.process_registry.write().await;

            let session_process_ids: Vec<String> = reg
                .entries
                .values()
                .filter(|entry| entry.session_id == session_id)
                .map(|entry| entry.id.clone())
                .collect();

            session_process_ids
                .into_iter()
                .filter_map(|id| {
                    if let Some(token) = reg.cancellation_tokens.get(&id) {
                        token.cancel();
                    }

                    let entry = reg.entries.remove(&id)?;
                    reg.cancellation_tokens.remove(&id);
                    reg.completion_notifiers.remove(&id);
                    Some((id, entry))
                })
                .collect::<Vec<_>>()
        };

        let process_count = session_entries.len();
        for (id, entry) in session_entries {
            if let Some(pid) = entry.pid {
                if terminal_manager::is_active_process_status(&entry.status) {
                    info!("Killing process tree {} (PID {})", id, pid);
                    if let Err(error) = tokio::task::spawn_blocking(move || {
                        crate::utils::process::force_kill_process_tree(pid)
                    })
                    .await
                    .map_err(crate::utils::process::describe_join_error)
                    .and_then(|result| result.map_err(|error| error.to_string()))
                    {
                        tracing::warn!(
                            "Failed to kill process tree {} (PID {}): {}",
                            id,
                            pid,
                            error
                        );
                    }
                }
            }

            let output_dir = std::path::PathBuf::from(&entry.stdout_path)
                .parent()
                .map(|path| path.to_path_buf());
            if let Some(dir) = output_dir {
                let _ = tokio::fs::remove_dir_all(&dir).await;
                info!("Removed output directory for process: {}", id);
            }
        }

        info!(
            "Cleaned up {} processes for session {}",
            process_count, session_id
        );

        for pending in self.pending_executions.remove_for_session(session_id) {
            if let Some(response_tx) = pending.response_tx {
                let _ = response_tx.send(PendingShellInputResolution::Cancelled);
            }
        }

        if let Err(e) = self.shell_manager.terminate_shell(session_id).await {
            tracing::warn!(
                "Failed to terminate persistent shell for session {}: {}",
                session_id,
                e
            );
        }
    }
}
