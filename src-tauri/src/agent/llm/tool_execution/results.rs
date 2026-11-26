use super::approval::ToolExecutionContext;
use crate::agent::llm::tool_args_validation::{args_preview, ArgsParseFailureKind};
use crate::agent::state::AgentSession;
use crate::agent::types::ToolCall;
use crate::commands::agent_commands::{ToolCancellation, ToolExecutionResult};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// Build a guided tool-result for malformed / truncated tool-call arguments so the
/// agent can recover on the next turn instead of retrying the same broken payload.
///
/// This intentionally closes the tool call with an error result (via
/// `continue_after_tool`). The existing loop-prevention circuit then treats
/// repeated identical (tool, args) + error outcomes as a streak (default soft
/// block at 3).
pub(super) fn args_parse_error_result(
    tool_name: &str,
    args_str: &str,
    kind: ArgsParseFailureKind,
    parse_error: &str,
    max_output_tokens: Option<u32>,
) -> ToolExecutionResult {
    let args_bytes = crate::agent::llm::tool_args_validation::parse_redacted_byte_length(args_str)
        .unwrap_or(args_str.len());

    let budget_info = match max_output_tokens {
        Some(tokens) => format!(" (max output tokens budget: ~{})", tokens),
        None => String::new(),
    };

    let (headline, mut guidance) = if kind.is_truncated() {
        (
            format!(
                "Failed to parse arguments for `{tool_name}`: output budget exceeded{budget_info}, arguments truncated ({parse_error}). The tool call was not executed."
            ),
            vec![
                "Output budget exceeded: arguments were truncated before JSON was complete"
                    .to_string(),
                "The tool call was not executed".to_string(),
                "Split the work into smaller tool calls with smaller payloads".to_string(),
                "Do NOT retry the identical oversized payload — it will truncate again".to_string(),
            ],
        )
    } else {
        (
            format!(
                "Failed to parse arguments for `{tool_name}`: invalid JSON ({parse_error}). The tool call was not executed."
            ),
            vec![
                "Fix the JSON syntax (quotes, commas, braces, escapes) and call the tool again"
                    .to_string(),
                "The tool call was not executed".to_string(),
                "If argument payloads are large, split the work into smaller tool calls"
                    .to_string(),
            ],
        )
    };

    if let Some(tokens) = max_output_tokens {
        guidance.push(format!(
            "Session output budget is ~{} tokens; keep each tool call JSON well below this budget.",
            tokens
        ));
    }

    guidance.push(
        "Repeating the same broken arguments will eventually be blocked by loop prevention"
            .to_string(),
    );

    let guidance_text = guidance
        .iter()
        .enumerate()
        .map(|(i, step)| format!("{}. {}", i + 1, step))
        .collect::<Vec<_>>()
        .join("\n");
    let message = format!(
        "✗ {headline}\n\n{}\n{guidance_text}",
        crate::mcp::builtin::error_guidance::hint_headers::ERROR_RECOVERY
    );

    let structured = serde_json::json!({
        "errorKind": kind.as_error_kind(),
        "toolName": tool_name,
        "parseError": parse_error,
        "argsByteLength": args_bytes,
        "likelyTruncated": kind.is_truncated(),
        "maxOutputTokens": max_output_tokens,
        "argsPreview": args_preview(args_str, 240),
        "nextActions": guidance,
        "recoverable": true,
    });

    ToolExecutionResult {
        success: false,
        content: message.clone(),
        mcp_content: Some(vec![crate::mcp::types::MCPContent::Text {
            text: message.clone(),
        }]),
        structured_content: Some(structured),
        error: Some(message),
        is_error: true,
        cancellation: None,
        execution_duration_ms: None,
    }
}

pub(super) fn tool_result_from_mcp_result(
    result: crate::mcp::types::MCPResult,
) -> ToolExecutionResult {
    let is_error = result.indicates_error();
    let structured_content = result.structured_content.clone();
    ToolExecutionResult {
        success: !is_error,
        content: String::new(),
        structured_content,
        error: None,
        is_error,
        mcp_content: crate::agent::tools::convert_mcp_response_content(Some(
            crate::mcp::types::MCPResponseResult::ToolCall(result),
        )),
        cancellation: None,
        execution_duration_ms: None,
    }
}

pub(super) fn error_result(message: impl Into<String>) -> ToolExecutionResult {
    let message = message.into();
    ToolExecutionResult {
        success: false,
        content: message.clone(),
        structured_content: None,
        error: Some(message),
        is_error: true,
        mcp_content: None,
        cancellation: None,
        execution_duration_ms: None,
    }
}

pub(super) fn cancelled_tool_result() -> ToolExecutionResult {
    let mut result = error_result("Tool call cancelled by user.");
    result.cancellation = Some(ToolCancellation::user());
    result
}

pub(super) async fn session_cancel_requested(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    cancellation_token: &CancellationToken,
) -> bool {
    if cancellation_token.is_cancelled() {
        return true;
    }
    let active = active_sessions.read().await;
    active
        .get(session_id)
        .map(|session| session.cancel_pending.load(Ordering::SeqCst))
        .unwrap_or(false)
}

pub(super) async fn inject_cancel_tombstones_for_remaining(
    context: &ToolExecutionContext<'_>,
    remaining: &[ToolCall],
) {
    for tool_call in remaining {
        log::info!(
            "Injecting cancel tombstone for tool call {} in session {}",
            tool_call.id,
            context.session_id
        );
        context
            .continue_after_tool(
                &tool_call.id,
                cancelled_tool_result(),
                "cancelled tool call",
            )
            .await;
    }
}
