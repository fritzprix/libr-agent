use crate::agent::types::{ToolCall, ToolCallFunction};
use crate::mcp::types::MCPContent;
use crate::models::chat::{Message, MessageSource};

/// Qualified tool name used for runtime-injected session context (matches builtin prefixing).
pub const SESSION_CONTEXT_TOOL_NAME: &str = "agent__sessionContext";

/// Local builtin tool name (before `agent__` prefix).
pub const SESSION_CONTEXT_TOOL_LOCAL_NAME: &str = "sessionContext";

#[derive(Debug, Clone)]
pub struct RequestLayout {
    pub system_prompt: Option<String>,
    pub messages: Vec<Message>,
}

/// OpenAI-compatible custom providers use the `custom:<id>` session provider id.
pub fn is_custom_openai_compatible_provider(provider: &str) -> bool {
    provider
        .strip_prefix("custom:")
        .is_some_and(|id| !id.is_empty())
}

pub fn provider_uses_synthetic_session_context(provider: &str) -> bool {
    is_custom_openai_compatible_provider(provider)
        || matches!(
            provider,
            "openai" | "openrouter" | "fireworks" | "anthropic" | "gemini" | "ollama"
        )
}

fn synthetic_session_context_id_prefix(provider: &str) -> &str {
    if is_custom_openai_compatible_provider(provider) {
        return "custom-openai-session-context";
    }
    match provider {
        "openai" => "openai-session-context",
        "openrouter" => "openrouter-session-context",
        "fireworks" => "fireworks-session-context",
        "anthropic" => "anthropic-session-context",
        "gemini" => "gemini-session-context",
        "ollama" => "ollama-session-context",
        _ => "session-context",
    }
}

/// Choose where to inject synthetic session context.
///
/// Priority:
/// 1. Keep compaction overlays last (instruction attention).
/// 2. Place context immediately before the latest external user request so that
///    real user intent retains recency over environment telemetry.
/// 3. Otherwise append at the (pre-compaction) tail — e.g. tool-result continuations.
fn synthetic_session_context_insert_index(messages: &[Message]) -> usize {
    let mut insert_idx = messages.len();
    if insert_idx > 0 && messages[insert_idx - 1].is_compaction_overlay_message() {
        insert_idx -= 1;
    }

    if insert_idx > 0 && messages[insert_idx - 1].is_external_request_message() {
        insert_idx -= 1;
    }

    insert_idx
}

/// Inject the runtime `agent__sessionContext` tool pair only when the transcript
/// prefix already contains a tool turn (tool result, or assistant tool_calls).
///
/// Fresh user turns and text-only assistant→user turns skip injection so we never
/// lead with `assistant` (strict provider gate) and never shove volatile telemetry
/// into the system prompt (prefix-cache churn).
pub fn should_inject_session_context_tool_pair(messages: &[Message], insert_idx: usize) -> bool {
    let prefix = messages.get(..insert_idx).unwrap_or(&[]);
    for message in prefix.iter().rev() {
        if message.is_compaction_overlay_message()
            || message.is_request_layout_scaffolding_message()
        {
            continue;
        }

        if message.role == "tool" {
            return true;
        }

        if message.role == "assistant" {
            let has_tool_calls = message
                .tool_calls
                .as_ref()
                .is_some_and(|calls| !calls.is_empty());
            return has_tool_calls || message.tool_use.is_some();
        }

        return false;
    }

    false
}

fn build_session_context_tool_pair(
    provider: &str,
    session_id: &str,
    reference_message_id: &str,
    session_context: &str,
) -> (Message, Message) {
    let now = chrono::Utc::now().timestamp_millis();
    let id_prefix = synthetic_session_context_id_prefix(provider);
    let tool_call_id = format!("{id_prefix}-call-{reference_message_id}");

    let assistant_msg = Message {
        id: format!("{id_prefix}-assistant-{reference_message_id}"),
        session_id: session_id.to_string(),
        role: "assistant".to_string(),
        content: vec![],
        tool_calls: Some(vec![ToolCall {
            id: tool_call_id.clone(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: SESSION_CONTEXT_TOOL_NAME.to_string(),
                arguments: "{}".to_string(),
            },
        }]),
        tool_call_id: None,
        is_streaming: None,
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: None,
        prompt_tokens: None,
        created_at: now,
        updated_at: now,
        source: Some(MessageSource::SessionContext),
        error: None,
        metadata: None,
    };

    let tool_result_msg = Message {
        id: format!("{id_prefix}-result-{reference_message_id}"),
        session_id: session_id.to_string(),
        role: "tool".to_string(),
        content: vec![MCPContent::Text {
            text: session_context.to_string(),
        }],
        tool_calls: None,
        tool_call_id: Some(tool_call_id),
        is_streaming: None,
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: None,
        prompt_tokens: None,
        created_at: now,
        updated_at: now,
        source: Some(MessageSource::SessionContext),
        error: None,
        metadata: None,
    };

    (assistant_msg, tool_result_msg)
}

pub fn build_request_layout(
    provider: &str,
    session_id: &str,
    system_prompt: Option<String>,
    session_context: Option<String>,
    mut messages: Vec<Message>,
) -> RequestLayout {
    let Some(session_context) = session_context else {
        return RequestLayout {
            system_prompt,
            messages,
        };
    };

    let insert_idx = synthetic_session_context_insert_index(&messages);
    if !should_inject_session_context_tool_pair(&messages, insert_idx) {
        // No prior tool turn: omit volatile telemetry for this request (keeps
        // system prefix cache-stable; avoids assistant-leading first turns).
        return RequestLayout {
            system_prompt,
            messages,
        };
    }

    let reference_message_id =
        if insert_idx < messages.len() && messages[insert_idx].is_external_request_message() {
            messages[insert_idx].id.as_str()
        } else if insert_idx > 0 {
            messages[insert_idx - 1].id.as_str()
        } else {
            "system"
        };

    let id_provider = if provider_uses_synthetic_session_context(provider) {
        provider
    } else {
        // Unknown providers still use the tool-pair channel when eligible —
        // never the system prompt (prefix-cache churn).
        "session-context"
    };

    let (assistant_msg, tool_result_msg) = build_session_context_tool_pair(
        id_provider,
        session_id,
        reference_message_id,
        &session_context,
    );

    messages.insert(insert_idx, assistant_msg);
    messages.insert(insert_idx + 1, tool_result_msg);

    RequestLayout {
        system_prompt,
        messages,
    }
}

pub fn select_last_submitted_input_message_id(messages: &[Message]) -> Option<String> {
    messages
        .iter()
        .rev()
        .find(|message| {
            !message.is_request_layout_scaffolding_message()
                && !message.is_internal_synthetic_user_message()
        })
        .or_else(|| messages.last())
        .map(|message| message.id.clone())
}
