use crate::mcp::types::MCPContent;
use crate::models::chat::Message;
use crate::repositories::settings_repository::SettingsRepository;
use crate::services::WorkspaceService;
use serde_json::Value;

pub const TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES: usize = 16 * 1024;
const TOOL_RESULT_SPILLOVER_DIR: &str = ".libragent/tool-results";
const MIN_TOOL_RESULT_INLINE_LIMIT_BYTES: usize = 4 * 1024;
const MAX_TOOL_RESULT_INLINE_LIMIT_BYTES: usize = 256 * 1024;

fn sanitize_spillover_identifier(raw: &str) -> String {
    let sanitized: String = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();

    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "tool-result".to_string()
    } else {
        trimmed.to_string()
    }
}

fn clamp_tool_result_inline_limit_bytes(limit_bytes: usize) -> usize {
    limit_bytes.clamp(
        MIN_TOOL_RESULT_INLINE_LIMIT_BYTES,
        MAX_TOOL_RESULT_INLINE_LIMIT_BYTES,
    )
}

pub fn tool_result_preview_headroom_bytes(limit_bytes: usize) -> usize {
    if limit_bytes <= 4 * 1024 {
        return 512.min(limit_bytes.saturating_sub(1));
    }

    (limit_bytes / 8).clamp(1024, 8 * 1024)
}

pub fn tool_result_preview_content_limit_bytes(limit_bytes: usize) -> usize {
    limit_bytes.saturating_sub(tool_result_preview_headroom_bytes(limit_bytes))
}

pub async fn tool_result_inline_limit_bytes() -> usize {
    let Some(settings_repo) = crate::state::try_get_settings_repository() else {
        return TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES;
    };

    match settings_repo.get("advancedSettings").await {
        Ok(Some(model)) => match serde_json::from_str::<Value>(&model.value) {
            Ok(json) => json
                .get("toolResultInlineLimitBytes")
                .and_then(|value| value.as_u64())
                .map(|value| clamp_tool_result_inline_limit_bytes(value as usize))
                .unwrap_or(TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES),
            Err(_) => TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES,
        },
        _ => TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES,
    }
}

/// How the inline spillover preview was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewTruncateKind {
    /// Cut at a newline boundary (complete lines only).
    LineBoundary,
    /// Hard byte cut (single giant line, or newline too close to the start).
    HardByteCut,
}

/// Truncate `text` to at most `limit` bytes for inline preview.
///
/// Normally we cut at the last newline boundary so we don't expose a partial
/// line. However, if the output is a single giant line (e.g. compact JSON),
/// `rfind('\n')` only finds the tiny header before the payload, yielding an
/// almost-empty preview. In that case we fall back to a **hard byte cut** so
/// the agent sees real content instead of nothing.
///
/// Returns `(preview_slice, truncate_kind)`.
fn truncate_to_complete_lines(text: &str, limit: usize) -> (&str, PreviewTruncateKind) {
    if text.len() <= limit {
        // Caller only truncates when over the inline limit; treat as line-safe.
        return (text, PreviewTruncateKind::LineBoundary);
    }

    let mut boundary = limit.min(text.len());
    // Walk back to a valid UTF-8 char boundary.
    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }

    let truncated = &text[..boundary];

    // Only snap to the last newline if it gives us a meaningful amount of
    // content (at least 5 % of the limit). Otherwise fall back to the hard
    // byte cut so single-line payloads still show real content.
    let min_useful = (limit / 20).max(1);
    match truncated.rfind('\n') {
        Some(pos) if pos >= min_useful => (&truncated[..pos], PreviewTruncateKind::LineBoundary),
        // No newline, or newline is too close to the start → hard cut.
        _ => (truncated, PreviewTruncateKind::HardByteCut),
    }
}

fn build_tool_result_spillover_notice(
    relative_path: &str,
    original_size_bytes: usize,
    total_line_count: usize,
    preview_line_count: usize,
    truncate_kind: PreviewTruncateKind,
) -> String {
    let truncation_summary = match truncate_kind {
        PreviewTruncateKind::LineBoundary if preview_line_count > 0 => format!(
            "total {} lines / {} bytes (showing lines 1-{})",
            total_line_count, original_size_bytes, preview_line_count
        ),
        PreviewTruncateKind::LineBoundary => format!(
            "total {} lines / {} bytes",
            total_line_count, original_size_bytes
        ),
        PreviewTruncateKind::HardByteCut => format!(
            "total {} lines / {} bytes (byte preview; not line-aligned)",
            total_line_count, original_size_bytes
        ),
    };

    let mut notice = format!(
        "\n\n... [output truncated: {}] ...\n\nFull output saved to workspace file: `{}`\nRead it in chunks with `readFile({{\"path\": \"{}\", \"offset\": 1, \"size\": 200}})`.\nDo not call `readFile({{\"path\": \"{}\"}})` on the saved file without `offset` and `size`; that will just truncate again.",
        truncation_summary, relative_path, relative_path, relative_path
    );

    match truncate_kind {
        PreviewTruncateKind::LineBoundary
            if preview_line_count > 0 && preview_line_count < total_line_count =>
        {
            let next_start_line = preview_line_count + 1;
            notice.push_str(&format!(
                "\nTo read remaining lines ({} to {}), call `readFile({{\"path\": \"{}\", \"offset\": {}, \"size\": 200}})`.",
                next_start_line, total_line_count, relative_path, next_start_line
            ));
        }
        PreviewTruncateKind::LineBoundary if preview_line_count > 0 => {
            // All counted lines appeared in the preview (e.g. trailing partial
            // content was excluded at a newline). Still give an explicit
            // follow-up offset past the preview.
            notice.push_str(&format!(
                "\nTo continue after the inline preview, call `readFile({{\"path\": \"{}\", \"offset\": {}, \"size\": 200}})`.",
                relative_path,
                preview_line_count + 1
            ));
        }
        PreviewTruncateKind::HardByteCut => {
            // Preview is a raw byte cut — line offsets for the truncated
            // region are unreliable. Have the agent re-read from line 1.
            notice.push_str(&format!(
                "\nThe inline preview above is a raw byte cut (not line-aligned). \
 Read the saved file from the start in chunks: `readFile({{\"path\": \"{}\", \"offset\": 1, \"size\": 200}})` \
 and increment offset by size each time (file has {} lines / {} bytes).",
                relative_path, total_line_count, original_size_bytes
            ));
        }
        PreviewTruncateKind::LineBoundary => {
            notice.push_str(&format!(
                "\nRead the full file in chunks: `readFile({{\"path\": \"{}\", \"offset\": 1, \"size\": 200}})`.",
                relative_path
            ));
        }
    }

    notice
}

pub async fn spill_oversized_tool_result_messages(
    session_id: &str,
    messages: Vec<Message>,
) -> Result<Vec<Message>, String> {
    let inline_limit_bytes = tool_result_inline_limit_bytes().await;
    let preview_limit_bytes = tool_result_preview_content_limit_bytes(inline_limit_bytes);
    let mut processed_messages = Vec::with_capacity(messages.len());

    for mut message in messages {
        if message.role != "tool" {
            processed_messages.push(message);
            continue;
        }

        let tool_call_id =
            sanitize_spillover_identifier(message.tool_call_id.as_deref().unwrap_or("tool-result"));
        let message_id = sanitize_spillover_identifier(&message.id);
        let mut next_content = Vec::with_capacity(message.content.len());
        for (content_index, content) in message.content.into_iter().enumerate() {
            match content {
                MCPContent::Text { text } if text.len() > inline_limit_bytes => {
                    let relative_path = format!(
                        "{}/{}-{}-{}.txt",
                        TOOL_RESULT_SPILLOVER_DIR,
                        tool_call_id,
                        message_id,
                        content_index + 1
                    );
                    WorkspaceService::workspace_write_file(
                        &relative_path,
                        text.as_bytes(),
                        Some(session_id.to_string()),
                    )
                    .await
                    .map_err(|error| {
                        format!(
                            "Failed to spill oversized tool output to '{}': {}",
                            relative_path, error
                        )
                    })?;

                    log::info!(
                        "Spilled oversized tool output for session {} to workspace file '{}' ({} bytes)",
                        session_id,
                        relative_path,
                        text.len()
                    );

                    let (preview, truncate_kind) =
                        truncate_to_complete_lines(&text, preview_limit_bytes);
                    let total_line_count = text.lines().count();
                    let preview_line_count = preview.lines().count();

                    next_content.push(MCPContent::Text {
                        text: format!(
                            "{}{}",
                            preview,
                            build_tool_result_spillover_notice(
                                &relative_path,
                                text.len(),
                                total_line_count,
                                preview_line_count,
                                truncate_kind,
                            )
                        ),
                        // Error semantics live on Message.metadata.toolError, not content items.
                    });
                }
                other => next_content.push(other),
            }
        }

        message.content = next_content;
        processed_messages.push(message);
    }

    Ok(processed_messages)
}
