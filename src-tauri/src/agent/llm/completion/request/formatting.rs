use crate::agent::references::build_default_registry;
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;

pub fn normalize_request_messages(messages: Vec<Message>) -> Vec<Message> {
    let merged_messages = merge_consecutive_user_messages(messages);
    crate::agent::llm::context_selector::remove_incomplete_tool_chains(merged_messages)
}

/// Resolve `@type:arg` references in user messages.
/// Each user message's text content is processed through the reference registry.
/// Only the returned `Vec<Message>` is modified — the session store is untouched.
pub(crate) async fn resolve_message_references(
    messages: Vec<Message>,
    session_id: &str,
    assistant_id: Option<&str>,
) -> Vec<Message> {
    let registry = build_default_registry(session_id, assistant_id).await;
    let mut result = Vec::with_capacity(messages.len());

    for mut msg in messages {
        if msg.role == "user" {
            let mut new_content: Vec<MCPContent> = Vec::with_capacity(msg.content.len());
            for part in msg.content {
                if let MCPContent::Text { text, .. } = &part {
                    // Only process parts that contain @ references
                    if text.contains('@') {
                        let resolved = registry.preprocess_message_text(text).await;
                        new_content.push(MCPContent::Text {
                            text: resolved,
                            is_error: None,
                        });
                    } else {
                        new_content.push(part);
                    }
                } else {
                    new_content.push(part);
                }
            }
            msg.content = new_content;
        }
        result.push(msg);
    }
    result
}

/// Merge consecutive `user` role messages into a single message.
///
/// This is only expected after a crash-recovery scenario where an unanswered
/// user message sits at the tail of history and the user sends another message
/// before the agent can respond. The content of subsequent user messages is
/// appended to the first with a separator. IDs and metadata from the first
/// message are preserved. This operates on the CompletionRequest payload only —
/// stored messages are never mutated.
pub fn merge_consecutive_user_messages(messages: Vec<Message>) -> Vec<Message> {
    if messages.len() < 2 {
        return messages;
    }

    let trailing_run_start = messages
        .iter()
        .rposition(|msg| msg.role != "user")
        .map(|idx| idx + 1)
        .unwrap_or(0);

    if trailing_run_start >= messages.len().saturating_sub(1) {
        return messages;
    }

    let (head, trailing_users) = messages.split_at(trailing_run_start);
    if trailing_users.iter().any(|msg| msg.role != "user") {
        return messages;
    }

    let mut result: Vec<Message> = head.to_vec();
    let mut merged = trailing_users[0].clone();

    for msg in trailing_users.iter().skip(1) {
        merged.content.push(MCPContent::Text {
            text: "\n\n---\n\n".to_string(),
            is_error: None,
        });
        merged.content.extend(msg.content.clone());
        log::info!(
            "Merged trailing consecutive user messages: base={}, appended={}",
            merged.id,
            msg.id
        );
    }

    result.push(merged);
    result
}
