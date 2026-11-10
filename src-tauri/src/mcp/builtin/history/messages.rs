use serde_json::{json, Value};

use super::args::ReadMessageArgs;
use super::constants::*;
use super::helpers::render_message_content;
use super::types::HistoryMessageReadResponse;
use super::HistoryServer;
use crate::mcp::builtin::error_guidance::{operation_failed_error, SuccessHint, ToolGroup};
use crate::mcp::types::MCPResult;
use crate::repositories::MessageRepository;
use crate::state::get_message_repository;

/// Read one message object with paginated content when previews are too large.
pub async fn read_message(_server: &HistoryServer, args: Value) -> Result<MCPResult, String> {
    let args: ReadMessageArgs =
        serde_json::from_value(args).map_err(|e| format!("Invalid readMessage arguments: {e}"))?;
    let offset_chars = args.offset_chars.unwrap_or(0);
    let max_chars = args
        .max_chars
        .unwrap_or(DEFAULT_MESSAGE_CHARS)
        .clamp(1, MAX_MESSAGE_CHARS);

    let message_repo = get_message_repository();
    let message = match message_repo
        .get_by_id(&args.message_id)
        .await
        .map_err(|e| e.to_string())?
    {
        Some(message) => message,
        None => {
            return Ok(operation_failed_error(
                "Read Message",
                &format!("Message '{}' not found", args.message_id),
                vec![
                    "Use history__readSession(sessionId=\"...\") to list message IDs".to_string(),
                    "Retry readMessage with a copied messageId from readSession()".to_string(),
                ],
                ToolGroup::Agent,
            ));
        }
    };

    let rendered = render_message_content(&message.content);
    let total_chars = rendered.chars().count();
    let (content_chunk, chunk_length, has_more, next_offset) =
        slice_text_chunk(&rendered, total_chars, offset_chars, max_chars);

    let response = HistoryMessageReadResponse {
        message_id: message.id.clone(),
        session_id: message.session_id.clone(),
        role: message.role.clone(),
        created_at: message.created_at,
        total_chars,
        chunk_offset: offset_chars.min(total_chars),
        chunk_length,
        has_more,
        next_offset,
        content_chunk: content_chunk.clone(),
    };

    let text = super::render::render_read_message_text(&response);

    Ok(SuccessHint::new(
        text,
        next_offset
            .map(|offset| {
                vec![format!(
                    "Use history__readMessage(messageId=\"{}\", offsetChars={offset}) for the next chunk",
                    message.id
                )]
            })
            .unwrap_or_default(),
    )
    .to_mcp_result_with_data(Some(json!(response))))
}

/// Slice a text string into a character-bounded chunk with byte-accurate slicing.
pub fn slice_text_chunk(
    text: &str,
    total_chars: usize,
    offset_chars: usize,
    max_chars: usize,
) -> (String, usize, bool, Option<usize>) {
    let safe_offset = offset_chars.min(total_chars);

    if max_chars == 0 {
        let has_more = safe_offset < total_chars;
        let next_offset = has_more.then_some(safe_offset);
        return (String::new(), 0, has_more, next_offset);
    }

    if safe_offset == total_chars {
        return (String::new(), 0, false, None);
    }

    let mut start_byte = text.len();
    let mut end_byte = text.len();
    let mut chunk_length = 0usize;
    let mut started = false;

    for (char_index, (byte_index, ch)) in text.char_indices().enumerate() {
        if char_index == safe_offset {
            start_byte = byte_index;
            end_byte = byte_index;
            started = true;
        }

        if started && chunk_length < max_chars {
            end_byte = byte_index + ch.len_utf8();
            chunk_length += 1;

            if chunk_length == max_chars {
                break;
            }
        }
    }

    let chunk = text[start_byte..end_byte].to_string();
    let next_char_offset = safe_offset.saturating_add(chunk_length);
    let has_more = next_char_offset < total_chars;
    let next_offset = has_more.then_some(next_char_offset);
    (chunk, chunk_length, has_more, next_offset)
}
