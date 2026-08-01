use crate::mcp::types::MCPContent;
use crate::models::chat::{Message, MessageSource};

/// XML tag wrapper to isolate volatile session context (planning, workspace state, etc.)
/// within a synthetic user message for LLMs. This is cleaner than HTML comments for parsing,
/// prevents cache churn in system prompts, and clearly scopes background reference context.
const SESSION_CONTEXT_BACKGROUND_HEADER: &str = "<session-context>";
const SESSION_CONTEXT_BACKGROUND_FOOTER: &str = "</session-context>";

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

fn provider_uses_background_reference_wrapper(provider: &str) -> bool {
    !matches!(provider, "anthropic")
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

fn format_session_context_text(provider: &str, session_context: &str) -> String {
    if provider_uses_background_reference_wrapper(provider) {
        format!(
            "{}\n\n{}\n\n{}",
            SESSION_CONTEXT_BACKGROUND_HEADER, session_context, SESSION_CONTEXT_BACKGROUND_FOOTER
        )
    } else {
        session_context.to_string()
    }
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

    if provider_uses_synthetic_session_context(provider) {
        // If the last message is a compaction overlay (e.g. instruction), we want to
        // insert the session context message before it to preserve LLM instruction attention.
        let mut insert_idx = messages.len();
        if insert_idx > 0 && messages[insert_idx - 1].is_compaction_overlay_message() {
            insert_idx -= 1;
        }

        let reference_message_id = if insert_idx > 0 {
            messages[insert_idx - 1].id.as_str()
        } else {
            "system"
        };
        let now = chrono::Utc::now().timestamp_millis();
        let session_context_msg = Message {
            id: format!(
                "{}-{}",
                synthetic_session_context_id_prefix(provider),
                reference_message_id
            ),
            session_id: session_id.to_string(),
            role: "user".to_string(),
            content: vec![MCPContent::Text {
                text: format_session_context_text(provider, &session_context),
                is_error: None,
            }],
            tool_calls: None,
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

        messages.insert(insert_idx, session_context_msg);

        RequestLayout {
            system_prompt,
            messages,
        }
    } else {
        let mut merged = false;
        for message in messages.iter_mut().rev() {
            if message.role == "user" && !message.is_internal_synthetic_user_message() {
                let mut new_content = vec![MCPContent::Text {
                    text: format_session_context_text(provider, &session_context),
                    is_error: None,
                }];
                new_content.append(&mut message.content);
                message.content = new_content;
                merged = true;
                break;
            }
        }

        let system_prompt = if !merged {
            Some(
                [system_prompt, Some(session_context)]
                    .into_iter()
                    .flatten()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            )
            .filter(|prompt| !prompt.is_empty())
        } else {
            system_prompt
        };

        RequestLayout {
            system_prompt,
            messages,
        }
    }
}

pub fn select_last_submitted_input_message_id(messages: &[Message]) -> Option<String> {
    messages
        .iter()
        .rev()
        .find(|message| !message.is_internal_synthetic_user_message())
        .or_else(|| messages.last())
        .map(|message| message.id.clone())
}
