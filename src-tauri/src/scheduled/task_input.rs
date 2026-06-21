//! Validation helpers for scheduled task create/update payloads.

use crate::mcp::builtin::error_guidance::{invalid_input_error, ToolGroup};
use crate::mcp::types::MCPResult;
use serde_json::Value;

/// Reject removed MCP parameters with explicit guidance (mirrors Tauri `deny_unknown_fields`).
pub fn reject_legacy_scheduled_task_fields(args: &Value) -> Result<(), MCPResult> {
    for legacy_field in ["yoloMode", "unsafeMode"] {
        if args.get(legacy_field).is_some() {
            return Err(invalid_input_error(
                &format!(
                    "Parameter '{legacy_field}' was removed. Use executionMode (normal|yolo|unsafe) instead."
                ),
                ToolGroup::ScheduledTask,
            ));
        }
    }

    for removed_field in ["groupId", "groupName", "clearGroup"] {
        if args.get(removed_field).is_some() {
            return Err(invalid_input_error(
                &format!("Parameter '{removed_field}' was removed."),
                ToolGroup::ScheduledTask,
            ));
        }
    }

    Ok(())
}
