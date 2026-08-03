//! Shared helpers for merging consecutive user messages.

use crate::mcp::types::MCPContent;
use crate::models::chat::Message;

/// Separator inserted between text from consecutive user prompts when merging.
/// Used by durable pending-queue claim and crash-recovery request normalization.
pub const USER_MESSAGE_MERGE_SEPARATOR: &str = "\n\n---\n\n";

/// Merge content blocks from multiple user messages into one content list.
/// Adjacent text parts are joined with [`USER_MESSAGE_MERGE_SEPARATOR`].
pub fn merge_user_message_contents(messages: &[Message]) -> Vec<MCPContent> {
    let mut merged_contents: Vec<MCPContent> = Vec::new();
    let mut pending_text_parts: Vec<String> = Vec::new();

    for msg in messages {
        for content_item in &msg.content {
            match content_item {
                MCPContent::Text { text, .. } => {
                    if !text.trim().is_empty() {
                        pending_text_parts.push(text.clone());
                    }
                }
                other => {
                    flush_pending_text(&mut merged_contents, &mut pending_text_parts);
                    merged_contents.push(other.clone());
                }
            }
        }
    }

    flush_pending_text(&mut merged_contents, &mut pending_text_parts);
    merged_contents
}

fn flush_pending_text(merged_contents: &mut Vec<MCPContent>, pending_text_parts: &mut Vec<String>) {
    if pending_text_parts.is_empty() {
        return;
    }
    let combined_text = pending_text_parts.join(USER_MESSAGE_MERGE_SEPARATOR);
    merged_contents.push(MCPContent::Text {
        text: combined_text,
    });
    pending_text_parts.clear();
}

/// Concatenate attachment arrays from multiple user messages.
pub fn merge_user_message_attachments(messages: &[Message]) -> Option<serde_json::Value> {
    let mut combined: Vec<serde_json::Value> = Vec::new();
    for msg in messages {
        if let Some(serde_json::Value::Array(arr)) = &msg.attachments {
            combined.extend(arr.clone());
        }
    }
    if combined.is_empty() {
        None
    } else {
        Some(serde_json::Value::Array(combined))
    }
}
