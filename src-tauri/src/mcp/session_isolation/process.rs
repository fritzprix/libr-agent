use log::debug;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Represents a running MCP server process with its associated client and metadata.
///
/// Note: The actual child process is managed internally by rmcp's `TokioChildProcess`.
/// We only store the client handle and metadata here.
#[allow(dead_code)]
#[derive(Debug)]
pub struct MCPProcess {
    /// The rmcp client instance for communicating with the server.
    pub client: rmcp::service::RunningService<rmcp::service::RoleClient, ()>,

    /// Timestamp when the process was created.
    pub created_at: Instant,

    /// Number of times this process has been restarted.
    pub restart_count: u32,

    /// Number of active tool calls currently in progress.
    /// Used to prevent idle cleanup while calls are active.
    pub active_calls: Arc<AtomicU32>,
}

impl MCPProcess {
    /// Creates a new `MCPProcess` instance.
    pub fn new(client: rmcp::service::RunningService<rmcp::service::RoleClient, ()>) -> Self {
        Self {
            client,
            created_at: Instant::now(),
            restart_count: 0,
            active_calls: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Gracefully shutdown the process.
    ///
    /// This will cancel the rmcp client, which will also terminate the underlying
    /// child process managed by `TokioChildProcess`.
    pub async fn shutdown(self) {
        debug!("Shutting down MCP process");

        // Cancel rmcp client with timeout
        // The client cancel will also terminate the child process
        match tokio::time::timeout(Duration::from_secs(3), self.client.cancel()).await {
            Ok(_) => {
                debug!("Process cancelled gracefully");
            }
            Err(_) => {
                debug!("Process cancel timed out after 3 seconds");
            }
        }
    }
}
