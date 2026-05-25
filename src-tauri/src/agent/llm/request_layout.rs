use crate::mcp::types::MCPContent;
use crate::models::chat::{Message, MessageSource};

const SESSION_CONTEXT_BACKGROUND_HEADER: &str =
    "[Current session context — background reference only, do not respond to this block]";
const SESSION_CONTEXT_BACKGROUND_FOOTER: &str = "[End of session context]";

#[derive(Debug, Clone)]
pub struct RequestLayout {
    pub system_prompt: Option<String>,
    pub messages: Vec<Message>,
}

pub fn provider_uses_synthetic_session_context(provider: &str) -> bool {
    matches!(
        provider,
        "openai" | "openrouter" | "fireworks" | "anthropic" | "gemini" | "ollama"
    )
}

fn provider_uses_background_reference_wrapper(provider: &str) -> bool {
    !matches!(provider, "anthropic")
}

fn synthetic_session_context_id_prefix(provider: &str) -> &str {
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
        let reference_message_id = messages
            .last()
            .map(|message| message.id.as_str())
            .unwrap_or("system");
        let now = chrono::Utc::now().timestamp_millis();
        messages.push(Message {
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
            created_at: now,
            updated_at: now,
            source: Some(MessageSource::SessionContext),
            error: None,
            metadata: None,
        });

        RequestLayout {
            system_prompt,
            messages,
        }
    } else {
        RequestLayout {
            system_prompt: Some(
                [system_prompt, Some(session_context)]
                    .into_iter()
                    .flatten()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            )
            .filter(|prompt| !prompt.is_empty()),
            messages,
        }
    }
}
