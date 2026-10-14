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
    /// Last soft batch intervention before hard break: nudge strategy reset via reflect.
    RepeatedBatchSequenceEscalate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopPreventionShortCircuit {
    pub kind: LoopPreventionKind,
    pub tool_name: String,
    /// Canonicalized tool args string used to detect loop signatures and
    /// (when needed) parameterize the circuit breaker UI.
    pub args: String,
    pub count: usize,
    /// When `kind == DuplicateInBatch`, the earlier tool_call_id in this batch
    /// that already used the same (tool, arguments) signature.
    pub duplicate_of_tool_call_id: Option<String>,
}

/// Soft-cap args shown in loop-prevention guidance so long payloads stay readable.
const LOOP_PREVENTION_ARGS_PREVIEW_CHARS: usize = 160;

fn format_args_preview(args: &str) -> String {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return "(empty)".to_string();
    }
    let mut chars = trimmed.chars();
    let preview: String = chars
        .by_ref()
        .take(LOOP_PREVENTION_ARGS_PREVIEW_CHARS)
        .collect();
    if chars.next().is_some() {
        format!("{}…", preview)
    } else {
        preview
    }
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
        LoopPreventionKind::DuplicateInBatch => {
            let origin_desc = match short_circuit.duplicate_of_tool_call_id.as_deref() {
                Some(id) => format!("earlier tool_call_id '{}'", id),
                None => "an earlier call".to_string(),
            };
            let args_preview = format_args_preview(&short_circuit.args);
            format!(
                "Loop prevention: '{}' is a duplicate of {} in this batch \
                (identical parameters — other calls of this tool with different arguments are not blocked by this rule).\n\n\
                Matched arguments: {}\n\n\
                Duplicate calls in the same turn were blocked. Keep a single call per unique (tool, arguments) pair, \
                then continue with a different tool or updated arguments.",
                short_circuit.tool_name, origin_desc, args_preview
            )
        }
        LoopPreventionKind::RepeatedBatchSequence => format!(
            "Loop prevention: the same tool-call batch (including '{}') was repeated {} times across consecutive turns.\n\n\
            This call was blocked. Repeating the identical mixed batch will not change the outcome. \
            Change at least one tool or its arguments, or choose a different approach based on results you already have.",
            short_circuit.tool_name, short_circuit.count
        ),
        LoopPreventionKind::RepeatedBatchSequenceEscalate => format!(
            "Loop prevention: the same tool-call batch (including '{}') was repeated {} times across consecutive turns.\n\n\
            This call was blocked. Another identical batch will trigger a hard circuit break.\n\n\
            Before continuing, call planning__reflect: critique why this loop failed, reflect on what you learned, \
            and set one concrete nextAction that uses a different approach. Then proceed from that nextAction.",
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
        cancellation: None,
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
            args: "{}".to_string(),
            count: 3,
            duplicate_of_tool_call_id: None,
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
            args: "{}".to_string(),
            count: 3,
            duplicate_of_tool_call_id: None,
        });

        assert!(guidance.contains("blocked"));
        assert!(guidance.contains("planning__reflect"));
        assert!(guidance.contains("nextAction"));
        assert!(!guidance.contains("sleep"));
    }

    #[test]
    fn batch_escalate_guidance_recommends_reflect() {
        let guidance = build_loop_prevention_guidance(&LoopPreventionShortCircuit {
            kind: LoopPreventionKind::RepeatedBatchSequenceEscalate,
            tool_name: "workspace__readFile".to_string(),
            args: r#"{"path":"a.ts"}"#.to_string(),
            count: 4,
            duplicate_of_tool_call_id: None,
        });

        assert!(guidance.contains("blocked"));
        assert!(guidance.contains("planning__reflect"));
        assert!(guidance.contains("hard circuit break"));
    }

    #[test]
    fn soft_error_guidance_does_not_require_reflect() {
        let guidance = build_loop_prevention_guidance(&LoopPreventionShortCircuit {
            kind: LoopPreventionKind::RepeatedErrorOutcome,
            tool_name: "workspace__readFile".to_string(),
            args: "{}".to_string(),
            count: 3,
            duplicate_of_tool_call_id: None,
        });

        assert!(guidance.contains("blocked"));
        assert!(!guidance.contains("planning__reflect"));
    }

    #[test]
    fn duplicate_in_batch_guidance_cites_original_call_and_args() {
        let guidance = build_loop_prevention_guidance(&LoopPreventionShortCircuit {
            kind: LoopPreventionKind::DuplicateInBatch,
            tool_name: "planning__addTodo".to_string(),
            args: r#"{"description":"IMF PPP 추정"}"#.to_string(),
            count: 2,
            duplicate_of_tool_call_id: Some("earlier-call-id".to_string()),
        });

        assert!(guidance.contains("earlier tool_call_id 'earlier-call-id'"));
        assert!(guidance.contains("IMF PPP 추정"));
        assert!(guidance.contains("different arguments are not blocked"));
        assert!(guidance.contains("Matched arguments:"));
    }

    #[test]
    fn duplicate_in_batch_guidance_falls_back_without_original_id() {
        let guidance = build_loop_prevention_guidance(&LoopPreventionShortCircuit {
            kind: LoopPreventionKind::DuplicateInBatch,
            tool_name: "planning__addTodo".to_string(),
            args: r#"{"description":"x"}"#.to_string(),
            count: 2,
            duplicate_of_tool_call_id: None,
        });

        assert!(guidance.contains("duplicate of an earlier call"));
        assert!(!guidance.contains("tool_call_id '<earlier call>'"));
        assert!(!guidance.contains("tool_call_id 'an earlier call'"));
    }

    #[test]
    fn format_args_preview_truncates_long_payloads_with_ellipsis() {
        let long: String = "한".repeat(LOOP_PREVENTION_ARGS_PREVIEW_CHARS + 20);
        let preview = format_args_preview(&long);
        assert!(preview.ends_with('…'));
        assert_eq!(
            preview.chars().count(),
            LOOP_PREVENTION_ARGS_PREVIEW_CHARS + 1,
            "preview should be soft-cap chars plus ellipsis"
        );
    }

    #[test]
    fn format_args_preview_keeps_short_payloads_intact() {
        assert_eq!(format_args_preview("  {\"a\":1}  "), "{\"a\":1}");
        assert_eq!(format_args_preview("   "), "(empty)");
    }

    #[test]
    fn loop_prevention_result_marks_error() {
        let result = loop_prevention_tool_result("blocked");
        assert!(!result.success);
        assert_eq!(result.is_error, true);
    }
}
