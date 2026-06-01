use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::MCPResult;
use crate::repositories::DbError;

pub(super) struct PlanningFollowUpNotice {
    pub suffix: String,
    pub hint: String,
}

pub(super) fn planning_write_error(
    action: &str,
    err: &DbError,
    guidance: Vec<String>,
) -> MCPResult {
    log_planning_error("write", action, err);

    let message = if err.is_sqlite_busy() {
        format!(
            "Planning storage was temporarily busy while trying to {}. The tool retried automatically, but the write still did not finish.",
            action
        )
    } else {
        format!(
            "Could not {} because the planning store returned an internal error.",
            action
        )
    };

    guided_error(ErrorCategory::OperationFailed, message, ToolGroup::Planning)
        .with_guidance(guidance)
        .to_mcp_result()
}

pub(super) fn planning_read_error(action: &str, err: &DbError, guidance: Vec<String>) -> MCPResult {
    log_planning_error("read", action, err);

    let message = if err.is_sqlite_busy() {
        format!(
            "Planning storage is temporarily busy, so the tool could not {} right now.",
            action
        )
    } else {
        format!(
            "Could not {} because the planning store returned an internal error.",
            action
        )
    };

    guided_error(ErrorCategory::OperationFailed, message, ToolGroup::Planning)
        .with_guidance(guidance)
        .to_mcp_result()
}

pub(super) fn planning_follow_up_read_notice(
    state_label: &str,
    err: &DbError,
) -> PlanningFollowUpNotice {
    log_planning_error("follow-up read", state_label, err);

    if err.is_sqlite_busy() {
        PlanningFollowUpNotice {
            suffix: format!(
                "\n\nNote: The write succeeded, but planning storage is still busy, so the {} could not be loaded yet.",
                state_label
            ),
            hint: format!(
                "Use getCurrentState to load the {} once the planning store settles.",
                state_label
            ),
        }
    } else {
        PlanningFollowUpNotice {
            suffix: format!(
                "\n\nNote: The write succeeded, but the {} could not be loaded because the planning store returned an internal error.",
                state_label
            ),
            hint: format!(
                "Use getCurrentState to reload the {} after the internal error clears.",
                state_label
            ),
        }
    }
}

fn log_planning_error(operation_kind: &str, action: &str, err: &DbError) {
    if err.is_sqlite_busy() {
        log::warn!(
            "Planning {} failed during {} after internal retry handling: {}",
            operation_kind,
            action,
            err
        );
    } else {
        log::error!(
            "Planning {} failed during {}: {}",
            operation_kind,
            action,
            err
        );
    }
}
