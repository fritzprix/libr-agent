use crate::commands::agent_commands::ToolExecutionResult;
use crate::mcp::types::MCPContent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopPreventionKind {
    RepeatedErrorOutcome,
    /// Last soft intervention before hard break: escalate to strategy reset via reflect.
    RepeatedErrorEscalate,
    RepeatedSuccessOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopPreventionShortCircuit {
    pub kind: LoopPreventionKind,
    pub tool_name: String,
    pub count: usize,
}

pub fn build_loop_prevention_guidance(short_circuit: &LoopPreventionShortCircuit) -> String {
    match short_circuit.kind {
        LoopPreventionKind::RepeatedErrorOutcome => format!(
            "Loop prevention: '{}' was called {} times with identical parameters and the same error outcome.\n\n\
            This call was blocked. Do not retry with the same arguments. If retrying, use different \
            parameters or a different tool. Review what you have already tried, \
            re-read the user's request, and choose a different tool or approach. If you are completely blocked, \
            report your current progress and the specific blocker to the user.",
            short_circuit.tool_name, short_circuit.count
        ),
        LoopPreventionKind::RepeatedErrorEscalate => format!(
            "Loop prevention: '{}' was called {} times with identical parameters and the same error outcome.\n\n\
            This call was blocked. Do not retry with the same arguments — another identical attempt will trigger a hard circuit break.\n\n\
            Before continuing, call planning__reflect: critique why this loop failed, reflect on what you learned, \
            and set one concrete nextAction that uses a different approach. Then proceed from that nextAction.",
            short_circuit.tool_name, short_circuit.count
        ),
        LoopPreventionKind::RepeatedSuccessOutcome => format!(
            "Loop prevention: '{}' was called {} times with identical parameters and the same successful result.\n\n\
            This call was blocked. Repeating it with the same arguments will not change the outcome. If retrying, use different \
            parameters or a different tool. If you are waiting for an external \
            state change, use a delay first (for example `workspace__runShell` with `sleep 5`, or \
            `workspace__waitForProcess` for background processes), then retry with updated parameters if needed. \
            Otherwise choose a different tool or approach.",
            short_circuit.tool_name, short_circuit.count
        ),
    }
}

pub fn loop_prevention_tool_result(guidance: &str) -> ToolExecutionResult {
    ToolExecutionResult {
        success: false,
        content: String::new(),
        structured_content: Some(serde_json::json!({
            "loopPrevention": true,
        })),
        error: Some(guidance.to_string()),
        is_error: true,
        mcp_content: Some(vec![MCPContent::Text {
            text: guidance.to_string(),
            is_error: Some(true),
        }]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_track_guidance_is_blocked_as_error() {
        let guidance = build_loop_prevention_guidance(&LoopPreventionShortCircuit {
            kind: LoopPreventionKind::RepeatedSuccessOutcome,
            tool_name: "workspace__readFile".to_string(),
            count: 3,
        });

        assert!(guidance.contains("blocked"));
        assert!(guidance.contains("sleep"));
    }

    #[test]
    fn escalate_guidance_recommends_reflect() {
        let guidance = build_loop_prevention_guidance(&LoopPreventionShortCircuit {
            kind: LoopPreventionKind::RepeatedErrorEscalate,
            tool_name: "workspace__readFile".to_string(),
            count: 3,
        });

        assert!(guidance.contains("blocked"));
        assert!(guidance.contains("planning__reflect"));
        assert!(guidance.contains("nextAction"));
        assert!(!guidance.contains("sleep"));
    }

    #[test]
    fn soft_error_guidance_does_not_require_reflect() {
        let guidance = build_loop_prevention_guidance(&LoopPreventionShortCircuit {
            kind: LoopPreventionKind::RepeatedErrorOutcome,
            tool_name: "workspace__readFile".to_string(),
            count: 3,
        });

        assert!(guidance.contains("blocked"));
        assert!(!guidance.contains("planning__reflect"));
    }

    #[test]
    fn loop_prevention_result_marks_error() {
        let result = loop_prevention_tool_result("blocked");
        assert!(!result.success);
        assert_eq!(result.is_error, true);
    }
}
