//! Chunk loading, decoding, and formatting for workspace__readFile.

use super::super::utils::{
    format_hashline, initial_prefix_hash_state, update_prefix_hash_state, LARGE_FILE_THRESHOLD,
};
use super::range::resolve_range;
use super::types::{
    ReadFileChunk, EMPTY_FILE_OUT_OF_RANGE_PREFIX, READ_FILE_ANCHOR_HEADROOM_BYTES,
    READ_FILE_BASE_HEADROOM_BYTES, READ_FILE_MIN_VISIBLE_CONTENT_BYTES,
};

pub(super) fn format_read_chunk_summary(
    chunk: &ReadFileChunk,
    complete: bool,
    range_limited: bool,
) -> String {
    if complete {
        return format!("complete ({} lines)", chunk.total_lines);
    }

    let line_label = if chunk.displayed_line_count == 0 {
        "no lines".to_string()
    } else if chunk.displayed_start_line == chunk.displayed_end_line {
        format!("line {}", chunk.displayed_start_line)
    } else {
        format!(
            "lines {}-{}",
            chunk.displayed_start_line, chunk.displayed_end_line
        )
    };

    if chunk.next_line_too_large {
        return format!("{} of {} shown", line_label, chunk.total_lines);
    }

    if chunk.truncated {
        return format!(
            "{} of {} (truncated to stay under the inline limit)",
            line_label, chunk.total_lines
        );
    }

    if range_limited {
        return format!(
            "{} of {} (requested range; more remains)",
            line_label, chunk.total_lines
        );
    }

    // Mid-file through end: reached EOF but not a complete-file read.
    format!(
        "{} of {} (reached end; not complete file)",
        line_label, chunk.total_lines
    )
}

pub(super) async fn read_file_lines_range(
    path: &std::path::Path,
    offset_opt: Option<isize>,
    size_opt: Option<isize>,
    show_line_anchors: bool,
    visible_content_limit_bytes: usize,
) -> Result<ReadFileChunk, String> {
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let path_buf = path.to_path_buf();

    let (collected_lines, decode_note) = if file_size > LARGE_FILE_THRESHOLD {
        tokio::task::spawn_blocking(move || {
            let bytes = std::fs::read(&path_buf).map_err(|e| e.to_string())?;
            decode_file_bytes_to_lines(&bytes)
        })
        .await
        .map_err(|e| format!("Task join error: {}", e))??
    } else {
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| format!("Failed to read file: {}", e))?;
        decode_file_bytes_to_lines(&bytes)?
    };

    let total_lines = collected_lines.len();
    let (start, end) = resolve_range(total_lines, offset_opt, size_opt);

    let mut chunk = read_chunk_from_lines(
        collected_lines
            .into_iter()
            .map(Ok::<String, std::io::Error>),
        start,
        end,
        total_lines,
        show_line_anchors,
        visible_content_limit_bytes,
    )?;

    if let Some(note) = decode_note {
        if !chunk.content.is_empty() {
            chunk.content = format!("[encoding: {note}]\n{}", chunk.content);
        } else {
            chunk.content = format!("[encoding: {note}]");
        }
    }

    Ok(chunk)
}

fn decode_file_bytes_to_lines(bytes: &[u8]) -> Result<(Vec<String>, Option<&'static str>), String> {
    use crate::mcp::builtin::workspace::text_encoding::{decode_text_bytes, DecodedText};

    match decode_text_bytes(bytes) {
        DecodedText::Binary => Err(
            "Failed to read file: content appears to be binary (embedded null bytes). \
             Use a specialized tool or shell commands for binary files."
                .to_string(),
        ),
        DecodedText::Text { text, note } => {
            // Normalize newlines then split without discarding a trailing empty line oddly:
            // split_inclusive-style via lines() is fine for agent display.
            let lines: Vec<String> = text.lines().map(|line| line.to_string()).collect();
            Ok((lines, note))
        }
    }
}

fn read_chunk_from_lines<I>(
    lines: I,
    start: usize,
    end: usize,
    total_lines: usize,
    show_line_anchors: bool,
    visible_content_limit_bytes: usize,
) -> Result<ReadFileChunk, String>
where
    I: IntoIterator<Item = Result<String, std::io::Error>>,
{
    let mut result_lines = Vec::new();
    let mut prefix_state = initial_prefix_hash_state();
    let mut content_bytes = 0usize;
    let mut truncated = false;
    let mut next_start_line = None;
    let mut next_line_too_large = false;
    let mut hard_cut_chars_shown: usize = 0;

    for (current_line, line_result) in (1usize..).zip(lines) {
        let line = line_result.map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidData {
                "Failed to read file: Content appears to be binary or contains invalid UTF-8 characters. Please use a specialized tool for binary files.".to_string()
            } else {
                format!("Failed to read file: {}", e)
            }
        })?;

        if current_line >= start && current_line <= end {
            let rendered_line = if show_line_anchors {
                format_hashline(current_line, &line, &mut prefix_state)
            } else {
                line.clone()
            };

            let separator_len = usize::from(!result_lines.is_empty());
            let candidate_len = content_bytes + separator_len + rendered_line.len();

            if candidate_len <= visible_content_limit_bytes {
                content_bytes = candidate_len;
                result_lines.push(rendered_line);
            } else if result_lines.is_empty() {
                // This single line is wider than the limit.  Rather than
                // returning an empty preview (which forces the agent into a
                // dead-end "size:1 also shows nothing" loop), hard-cut the
                // line at the byte limit and emit the partial content.
                // `next_line_too_large` stays true so callers know the full
                // line was not delivered, but `hard_cut_chars_shown` lets the
                // agent continue reading from the correct character offset.
                let cut_limit = visible_content_limit_bytes;
                let mut cut_at = cut_limit.min(rendered_line.len());
                while cut_at > 0 && !rendered_line.is_char_boundary(cut_at) {
                    cut_at -= 1;
                }
                let preview = &rendered_line[..cut_at];
                let chars_shown = preview.chars().count();
                hard_cut_chars_shown = chars_shown;
                result_lines.push(format!(
                    "{} …[hard-cut at {} chars, line continues]",
                    preview, chars_shown
                ));
                truncated = true;
                next_line_too_large = true;
                next_start_line = Some(current_line);
                break;
            } else {
                truncated = true;
                next_start_line = Some(current_line);
                break;
            }
        } else if show_line_anchors {
            prefix_state = update_prefix_hash_state(prefix_state, &line);
        }

        if current_line >= end {
            break;
        }
    }

    if total_lines == 0 {
        if start > 1 {
            return Err(format!(
                "{EMPTY_FILE_OUT_OF_RANGE_PREFIX} omit offset/size or use offset: 1 (received offset: {start})"
            ));
        }

        return Ok(ReadFileChunk {
            content: String::new(),
            total_lines: 0,
            displayed_start_line: start,
            displayed_end_line: start,
            displayed_line_count: 0,
            truncated: false,
            next_start_line: None,
            suggested_end_line: None,
            next_line_too_large: false,
            hard_cut_chars_shown: 0,
        });
    }

    if result_lines.is_empty() && start > total_lines {
        return Err(format!(
            "Requested offset {} exceeds file length of {} lines",
            start, total_lines
        ));
    }

    let displayed_line_count = result_lines.len();
    let displayed_start_line = start;
    let displayed_end_line = if displayed_line_count == 0 {
        start
    } else {
        start + displayed_line_count - 1
    };
    let suggested_end_line = if displayed_line_count == 0 {
        next_start_line
    } else {
        next_start_line.map(|next_start| {
            (next_start + displayed_line_count.saturating_sub(1)).min(total_lines)
        })
    };

    Ok(ReadFileChunk {
        content: result_lines.join("\n"),
        total_lines,
        displayed_start_line,
        displayed_end_line,
        displayed_line_count,
        truncated,
        next_start_line,
        suggested_end_line,
        next_line_too_large,
        hard_cut_chars_shown,
    })
}

pub(super) fn read_file_visible_content_limit_bytes(
    inline_limit_bytes: usize,
    show_line_anchors: bool,
) -> usize {
    let preview_limit =
        crate::agent::tools::tool_result_preview_content_limit_bytes(inline_limit_bytes);
    let extra_headroom = if show_line_anchors {
        READ_FILE_BASE_HEADROOM_BYTES + READ_FILE_ANCHOR_HEADROOM_BYTES
    } else {
        READ_FILE_BASE_HEADROOM_BYTES
    };

    preview_limit
        .saturating_sub(extra_headroom)
        .max(READ_FILE_MIN_VISIBLE_CONTENT_BYTES)
}
