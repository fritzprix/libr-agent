use serde::Deserialize;
use serde_json::Value;

use crate::mcp::builtin::workspace::{
    PendingExecutionLookupError, PendingShellInputResolution, WorkspaceServer,
};
use crate::mcp::builtin::{
    error_guidance::{guided_error, ErrorCategory, ToolGroup},
    workspace::utils::sanitize_command_for_logging,
};
use crate::mcp::types::MCPResult;

#[derive(Debug, Deserialize)]
struct CancelPendingExecutionArgs {
    execution_id: String,
}

impl WorkspaceServer {
    pub(crate) async fn handle_cancel_pending_execution(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        let args: CancelPendingExecutionArgs =
            serde_json::from_value(args).map_err(|e| format!("Invalid arguments: {}", e))?;

        let pending = match self
            .pending_executions
            .remove_if_session_matches(&args.execution_id, session_id)
        {
            Ok(Some(pending)) => pending,
            Err(PendingExecutionLookupError::SessionMismatch) => {
                return Ok(guided_error(
                    ErrorCategory::PermissionDenied,
                    "Interactive execution belongs to a different session".to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Cancel the prompt from the same agent session that created it".to_string(),
                    "If needed, run the command again in the active session".to_string(),
                ])
                .to_mcp_result());
            }
            Ok(None) => {
                return Ok(guided_error(
                    ErrorCategory::ResourceNotFound,
                    format!("Interactive execution '{}' not found", args.execution_id),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "The prompt may have expired or already been cancelled".to_string(),
                    "Run the original command again to request a new prompt".to_string(),
                ])
                .to_mcp_result());
            }
        };

        let Some(response_tx) = pending.response_tx else {
            self.pending_executions.insert(pending);
            return Ok(guided_error(
                ErrorCategory::InvalidState,
                format!(
                    "Interactive execution '{}' is already being resolved",
                    args.execution_id
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Wait for the current resolution to finish before retrying".to_string(),
                "Run the original command again if the prompt appears stuck".to_string(),
            ])
            .to_mcp_result());
        };

        if response_tx
            .send(PendingShellInputResolution::Cancelled)
            .is_err()
        {
            return Ok(MCPResult::informational(
                "Interactive shell prompt already expired or resolved.",
            ));
        }

        let _ = self.emit_interactive_shell_resolution(session_id, &args.execution_id, "cancelled");

        Ok(MCPResult::informational(&format!(
            "Interactive command cancelled.\n\nCommand: {}",
            sanitize_command_for_logging(&pending.display_command)
        )))
    }
}
