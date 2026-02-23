use super::MCPServiceProxyManager;
use std::sync::atomic::Ordering;
use std::time::Duration;

impl MCPServiceProxyManager {
    /// Start the background cleanup task for idle process management
    ///
    /// This task runs periodically to clean up idle MCP server processes
    /// across all active sessions.
    pub(super) fn start_cleanup_task(&self) {
        let managers = self.session_stdio_managers.clone();
        let shutdown = self.cleanup_shutdown.clone();
        let interval_secs = self.config.cleanup_interval_minutes * 60;

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                // Check shutdown signal
                if shutdown.load(Ordering::Relaxed) {
                    log::info!("MCP cleanup task shutting down");
                    break;
                }

                // Cleanup idle processes for all sessions
                let managers_read = managers.read().await;
                for (session_id, manager) in managers_read.iter() {
                    log::debug!("Checking idle processes for session '{}'", session_id);
                    manager.cleanup_idle_processes().await;
                }
            }
        });

        if let Ok(mut task) = self.cleanup_task.try_lock() {
            *task = Some(handle);
        }
    }
}
