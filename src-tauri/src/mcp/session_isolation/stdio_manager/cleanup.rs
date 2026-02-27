use super::SessionMCPManager;
use log::{debug, info};
use std::sync::atomic::Ordering;
use std::time::Instant;

impl SessionMCPManager {
    /// Remove idle processes (called by background task).
    pub async fn cleanup_idle_processes(&self) {
        let now = Instant::now();
        let mut processes = self.active_processes.write().await;
        let activity = self.last_activity.read().await;

        let idle_servers: Vec<String> = activity
            .iter()
            .filter_map(|(name, &last_activity)| {
                // Check idle timeout
                if now.duration_since(last_activity) <= self.idle_timeout {
                    return None;
                }

                // Check if process has active calls
                if let Some(process) = processes.get(name) {
                    if process.active_calls.load(Ordering::Relaxed) > 0 {
                        debug!("Skipping cleanup of '{}' - has active calls", name);
                        return None;
                    }
                }

                Some(name.clone())
            })
            .collect();

        for server_name in idle_servers {
            info!(
                "Terminating idle MCP server '{}' for session '{}'",
                server_name, self.session_id
            );

            if let Some(process) = processes.remove(&server_name) {
                // Spawn cleanup task (don't block)
                tokio::spawn(async move {
                    process.shutdown().await;
                });
            }
        }
    }

    /// Shutdown all processes (called on session destroy).
    pub async fn shutdown_all(&self) {
        info!(
            "Shutting down all MCP processes for session '{}'",
            self.session_id
        );

        // Cancel all active calls
        let tokens = self.active_call_tokens.read().await;
        for token in tokens.values() {
            token.cancel();
        }
        drop(tokens);

        // Wait briefly for calls to abort
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Remove all processes
        let mut processes = self.active_processes.write().await;
        let process_list: Vec<_> = processes.drain().collect();

        // Shutdown in parallel
        let shutdown_tasks: Vec<_> = process_list
            .into_iter()
            .map(|(name, process)| {
                tokio::spawn(async move {
                    debug!("Killing MCP server '{}'", name);
                    process.shutdown().await;
                })
            })
            .collect();

        // Wait for all shutdowns with timeout
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            futures::future::join_all(shutdown_tasks),
        )
        .await;
    }
}
