use crate::commands::agent_commands::ToolExecutionResult;
use crate::mcp::types::MCPContent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopPreventionKind {
    RepeatedErrorOutcome,
    /// Last soft intervention before hard break: escalate to strategy reset via reflect.
    RepeatedErrorEscalate,
    RepeatedSuccessOutcome,
    /// Same (name, args) appeared earlier in the current assistant tool_calls batch.
    DuplicateInBatch,
    /// The whole tool_calls batch fingerprint repeated across consecutive turns.
    RepeatedBatchSequence,
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
            This call was blocked. This environment strictly limits repeating the same tool with the same arguments.\n\n\
            If you are waiting for background work to finish, do not poll with snapshot/status-only calls. \
            Prefer a single blocking wait on that tool (for example wait=true, or a non-zero timeout) \
            so one call covers the wait window. Only re-check after a wait returns incomplete — \
            and change approach if the state is not progressing. \
            Otherwise use different parameters or a different tool.",
            short_circuit.tool_name, short_circuit.count
        ),
        LoopPreventionKind::DuplicateInBatch => format!(
            "Loop prevention: '{}' appears more than once in this tool-call batch with identical parameters.\n\n\
            Duplicate calls in the same turn were blocked. Keep a single call per unique (tool, arguments) pair, \
            then continue with a different tool or updated arguments.",
            short_circuit.tool_name
        ),
        LoopPreventionKind::RepeatedBatchSequence => format!(
            "Loop prevention: the same tool-call batch (including '{}') was repeated {} times across consecutive turns.\n\n\
            This call was blocked. Repeating the identical mixed batch will not change the outcome. \
            Change at least one tool or its arguments, or choose a different approach based on results you already have.",
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
        assert!(guidance.contains("blocking wait"));
        assert!(guidance.contains("wait=true"));
        assert!(!guidance.contains("sleep"));
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
