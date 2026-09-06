#[derive(Debug, PartialEq, Eq)]
pub enum ToolResultAcceptance {
    Accept,
    Stale,
    Duplicate,
}

pub fn classify_tool_result(
    pending: &crate::agent::state::PendingToolExecution,
    tool_call_id: &str,
) -> ToolResultAcceptance {
    if !pending.expected_tool_call_ids.contains(tool_call_id) {
        return ToolResultAcceptance::Stale;
    }

    if pending.completed_tool_call_ids.contains(tool_call_id) {
        return ToolResultAcceptance::Duplicate;
    }

    ToolResultAcceptance::Accept
}
