use crate::mcp::types::MCPContent;
use crate::models::chat::Message;
use crate::repositories::settings_repository::SettingsRepository;
use crate::services::WorkspaceService;
use serde_json::Value;

pub const TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES: usize = 16 * 1024;
const TOOL_RESULT_SPILLOVER_DIR: &str = ".libragent/tool-results";
const MIN_TOOL_RESULT_INLINE_LIMIT_BYTES: usize = 4 * 1024;
const MAX_TOOL_RESULT_INLINE_LIMIT_BYTES: usize = 256 * 1024;
/// Outputs at or above this size are treated as unbounded dumps even when mostly printable.
const LARGE_TOOL_RESULT_DUMP_BYTES: usize = 5 * 1024 * 1024;
/// Sample window for non-text density checks (chars, not bytes).
const NON_TEXT_SAMPLE_CHARS: usize = 8192;
/// Fraction of sampled control/replacement chars that marks output as non-text.
const NON_TEXT_CONTROL_RATIO: f64 = 0.15;

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

/// True when spilled tool output is too large or too control-dense to page as text.
///
/// Harbor traces show agents following generic readFile pagination on multi‑MB binary
/// dumps and burning the turn budget. Prefer extraction-oriented recovery instead.
fn looks_like_non_text_tool_output(text: &str, size_bytes: usize) -> bool {
    if size_bytes >= LARGE_TOOL_RESULT_DUMP_BYTES {
        return true;
    }

    let mut sample_len = 0usize;
    let mut control_like = 0usize;
    for ch in text.chars().take(NON_TEXT_SAMPLE_CHARS) {
        sample_len += 1;
        let code = ch as u32;
        let is_allowed_whitespace = ch == '\t' || ch == '\n' || ch == '\r';
        if (!is_allowed_whitespace && code < 32) || code == 127 || ch == '\u{FFFD}' {
            control_like += 1;
        }
    }

    if sample_len == 0 {
        return false;
    }

    (control_like as f64) / (sample_len as f64) >= NON_TEXT_CONTROL_RATIO
}

fn build_non_text_spillover_notice(relative_path: &str, original_size_bytes: usize) -> String {
    format!(
        "\n\n... [output truncated: {original_size_bytes} bytes of large binary/non-text data] ...\n\n\
Full output saved to workspace file: `{relative_path}`.\n\
Output appears to contain large binary or non-text data. Prefer `workspace__runShell` with \
`strings`, `head -c`, `grep -a`, or redirect the original command to a file for targeted extraction \
instead of inspecting raw bytes.\n\
Do not page through this dump with repeated `workspace__readFile` chunk reads."
    )
}

fn build_tool_result_spillover_notice(
    relative_path: &str,
    original_text: &str,
    original_size_bytes: usize,
    total_line_count: usize,
    preview_line_count: usize,
    truncate_kind: PreviewTruncateKind,
) -> String {
    if looks_like_non_text_tool_output(original_text, original_size_bytes) {
        return build_non_text_spillover_notice(relative_path, original_size_bytes);
    }

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
                                &text,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_text_detector_flags_large_dumps_by_size() {
        let text = "printable ".repeat(100);
        assert!(looks_like_non_text_tool_output(
            &text,
            LARGE_TOOL_RESULT_DUMP_BYTES
        ));
        assert!(!looks_like_non_text_tool_output(
            &text,
            TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES
        ));
    }

    #[test]
    fn non_text_detector_flags_control_dense_samples() {
        let dense: String = (0u8..32)
            .filter(|b| *b != b'\t' && *b != b'\n' && *b != b'\r')
            .map(char::from)
            .cycle()
            .take(NON_TEXT_SAMPLE_CHARS)
            .collect();
        assert!(looks_like_non_text_tool_output(
            &dense,
            TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES + 1
        ));
    }

    #[test]
    fn non_text_spillover_notice_discourages_raw_pagination() {
        let notice = build_non_text_spillover_notice(
            ".libragent/tool-results/call_example-1.txt",
            54_860_523,
        );
        assert!(notice.contains("large binary/non-text data"));
        assert!(notice.contains("strings"));
        assert!(notice.contains("grep -a"));
        assert!(notice.contains("Do not page through this dump"));
        assert!(
            !notice.contains("Read it in chunks with `readFile"),
            "binary dumps must not recommend generic readFile pagination: {notice}"
        );
    }

    #[test]
    fn text_spillover_notice_still_guides_line_pagination() {
        let text = "hello world\n".repeat(100);
        let notice = build_tool_result_spillover_notice(
            ".libragent/tool-results/call_text-1.txt",
            &text,
            text.len(),
            100,
            40,
            PreviewTruncateKind::LineBoundary,
        );
        assert!(notice.contains("To read remaining lines ("));
        assert!(!notice.contains("large binary/non-text data"));
    }
}
