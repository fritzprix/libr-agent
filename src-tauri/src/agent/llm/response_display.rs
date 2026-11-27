//! `MessageDisplay` hooks (#3008): the text shown for a completed assistant
//! message. The stored message and the model context keep the original.

use crate::agent::state::AgentSession;
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;
use crate::services::hooks::lifecycle::{message_display, HookSources};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

fn message_text(message: &Message) -> String {
    message
        .content
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `message` with its text blocks replaced by one holding `text`; every other
/// block keeps its place.
pub(super) fn with_display_text(message: &Message, text: String) -> Message {
    let mut shown = message.clone();
    let position = shown
        .content
        .iter()
        .position(|content| matches!(content, MCPContent::Text { .. }));
    let Some(position) = position else {
        return shown;
    };
    shown
        .content
        .retain(|content| !matches!(content, MCPContent::Text { .. }));
    shown.content.insert(position, MCPContent::Text { text });
    shown
}

/// Run the `MessageDisplay` hooks for `message` and return the replacement text,
/// if any. A message without text does not fire the event.
pub(super) async fn display_override(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    message: &Message,
) -> Option<String> {
    let text = message_text(message);
    if text.trim().is_empty() {
        return None;
    }
    let sources = HookSources::for_session(session_id)?;
    // The turn is identified by the user message that opened it.
    let messages = active_sessions
        .read()
        .await
        .get(session_id)
        .map(|session| session.messages.clone());
    let turn_id = match messages {
        Some(messages) => messages
            .read()
            .await
            .iter()
            .rev()
            .find(|cached| cached.role == "user")
            .map(|cached| cached.id.clone()),
        None => None,
    };
    message_display(
        &sources,
        session_id,
        turn_id.as_deref().unwrap_or(&message.id),
        &message.id,
        &text,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_text_replaces_the_shown_copy_only() {
        let original: Message = serde_json::from_value(serde_json::json!({
            "id": "m1",
            "sessionId": "s1",
            "role": "assistant",
            "content": [
                { "type": "text", "text": "first" },
                { "type": "text", "text": "second" },
            ],
        }))
        .expect("message");
        assert_eq!(message_text(&original), "first\nsecond");

        let shown = with_display_text(&original, "redacted".to_string());
        assert_eq!(shown.content.len(), 1);
        assert_eq!(message_text(&shown), "redacted");
        assert_eq!(shown.id, original.id);
        assert_eq!(message_text(&original), "first\nsecond");
    }
}
