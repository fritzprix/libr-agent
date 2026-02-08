pub mod handlers;
pub mod security;
pub mod ui;

use serde_json::Value;
use crate::mcp::types::MCPResult;
use crate::mcp::builtin::workspace::WorkspaceServer;

impl WorkspaceServer {
    /// Handle interactive shell execution (1st tool call)
    /// Returns UIResource with execution_id for user input
    pub(crate) async fn handle_interactive_shell(
        &self,
        command: &str,
        args: &Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        handlers::handle_interactive_shell(self, command, args, session_id).await
    }

    /// Handle execute_pending_shell tool call (2nd tool call)
    /// Executes pending command with user input via stdin
    pub async fn handle_execute_pending_shell(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        handlers::handle_execute_pending_shell(self, args, session_id).await
    }

    /// Cancel a pending shell execution
    /// Removes the pending execution from state without executing it
    pub async fn handle_cancel_pending_execution(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        handlers::handle_cancel_pending_execution(self, args, session_id).await
    }
}
