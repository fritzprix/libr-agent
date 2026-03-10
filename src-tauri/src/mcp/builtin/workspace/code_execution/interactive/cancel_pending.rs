use serde_json::Value;

use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;

use crate::mcp::builtin::workspace::WorkspaceServer;

impl WorkspaceServer {
    /// Cancel a pending shell execution
    /// Removes the pending execution from state without executing it
    pub async fn handle_cancel_pending_execution(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        // Extract execution_id (support both camelCase and snake_case)
        let execution_id = match args
            .get("executionId")
            .or_else(|| args.get("execution_id"))
            .and_then(|v| v.as_str())
        {
            Some(id) => id,
            None => {
                return Ok(missing_param_error("executionId", ToolGroup::Workspace));
            }
        };

        // Remove pending execution
        match self.pending_executions.remove(execution_id) {
            Some(pending) => {
                // Validate session ownership
                if pending.session_id != session_id {
                    // Restore it if session mismatch
                    self.pending_executions.insert(pending);

                    return Ok(guided_error(
                        ErrorCategory::PermissionDenied,
                        format!(
                            "Pending execution '{}' belongs to a different session",
                            execution_id
                        ),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Ensure you are executing the command in the correct session".to_string(),
                        "Executions are isolated per session".to_string(),
                    ])
                    .to_mcp_result());
                }

                let hint = SuccessHint::new(
                    format!("Cancelled pending execution: {}", pending.display_command),
                    vec!["Execute the command again if needed".to_string()],
                );

                let response_data = serde_json::json!({
                    "execution_id": execution_id,
                    "command": pending.display_command,
                    "cancelled": true
                });

                Ok(hint.to_mcp_result_with_data(Some(response_data)))
            }
            None => Ok(guided_error(
                ErrorCategory::ResourceNotFound,
                format!("Pending execution '{}' not found", execution_id),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "The execution may have already been completed or cancelled".to_string(),
                "Verify the execution_id is correct".to_string(),
                format!("Executions expire after {} minutes", 5),
            ])
            .to_mcp_result()),
        }
    }
}
