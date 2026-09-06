//! workspace__readFile handler and helpers.

use super::super::edit_mode::{read_file_anchor_output_suffix, read_file_anchor_prefix_note};
use super::super::WorkspaceServer;
use super::utils::{detect_language, format_file_size};
#[cfg(test)]
use super::utils::{format_hashline, initial_prefix_hash_state};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, not_found_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};
use tokio::fs;
use tracing::{error, info};

mod args;
mod chunk;
mod range;
mod types;

pub use types::ReadMode;

use args::{parse_offset_parameter, parse_show_line_anchors, parse_size_parameter};
use chunk::{
    format_read_chunk_summary, read_file_lines_range, read_file_visible_content_limit_bytes,
};
use range::{is_empty_file_out_of_range_error, parse_offset_exceeds_error};

impl WorkspaceServer {
    pub async fn handle_read_file(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // ✅ ENHANCED: Add proactive parameter validation before file operations

        // 1. Parameter existence and non-empty check
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) if !path.trim().is_empty() => path.trim(),
            Some(_) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Path parameter cannot be empty",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Provide a file path (relative paths resolve from the workspace)".to_string(),
                    "Examples: {\"path\": \"src/main.rs\"} or {\"path\": \"/tmp/file.txt\"}"
                        .to_string(),
                    "Use workspace__listDirectory to explore available paths".to_string(),
                ])
                .to_mcp_result());
            }
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };

        // 2. Path pattern validation (reject dangerous patterns)
        if path_str.contains("..") {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Path traversal patterns (..) are not allowed",
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use a normal file path without '..' traversal segments".to_string(),
                "Example: 'src/main.rs' instead of '../src/main.rs'".to_string(),
                "Use workspace__listDirectory to explore available paths".to_string(),
            ])
            .to_mcp_result());
        }

        let offset_opt = match parse_offset_parameter(&args) {
            Ok(off) => off,
            Err(result) => return Ok(result),
        };
        let size_opt = match parse_size_parameter(&args) {
            Ok(size) => size,
            Err(result) => return Ok(result),
        };
        let show_line_anchors = parse_show_line_anchors(&args);

        if let Some(sz) = size_opt {
            if sz == 0 {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "size must be non-zero",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "To read a specific number of lines, specify a positive size (e.g. 50)"
                        .to_string(),
                    "To read the end of the file (tail mode), specify a negative size (e.g. -20)"
                        .to_string(),
                ])
                .to_mcp_result());
            }
        }

        // 4. Path security validation
        let file_manager = self.get_file_manager(session_id.clone());
        let safe_path = match self
            .validate_read_path_with_skill_access(path_str, session_id.clone())
            .await
        {
            Ok(path) => path,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::PermissionDenied,
                    format!("Path validation failed: {}", e),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Verify the file path is correct".to_string(),
                    "Use workspace__listDirectory to see available files".to_string(),
                    "Ensure you have read permissions for the file".to_string(),
                ])
                .to_mcp_result());
            }
        };

        if let Err(sync_error) = self
            .sync_attach_before_host_read(&safe_path, session_id.as_deref())
            .await
        {
            return Ok(guided_error(
                ErrorCategory::OperationFailed,
                format!("Failed to sync attached container file before read: {sync_error}"),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Verify the Harbor/Docker container is still running".to_string(),
                "Retry readFile after confirming docker exec works".to_string(),
            ])
            .to_mcp_result());
        }

        // 5. File existence check
        if !safe_path.exists() {
            return Ok(not_found_error("File", path_str, ToolGroup::Workspace));
        }

        // 6. File type check (must be file, not directory)
        if safe_path.is_dir() {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("'{}' is a directory, not a file", path_str),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use workspace__listDirectory to see directory contents".to_string(),
                "To read a file inside this directory, specify the full path".to_string(),
                format!("Example: '{}/filename.ext'", path_str),
            ])
            .to_mcp_result());
        }

        // Security check: validate file size before reading
        if let Err(e) = file_manager
            .get_security_validator()
            .validate_file_size(&safe_path, crate::config::max_file_size())
        {
            error!("File size validation failed: {}", e);
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("File size error: {}", e),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "The file is too large to read entirely".to_string(),
                "Try reading specific line ranges if possible".to_string(),
                "Use workspace__grepFiles to find specific content instead".to_string(),
            ])
            .to_mcp_result());
        }

        // Use read_file_lines_range for all file reading to ensure consistent
        // handling of large files (spawn_blocking) and formatting.
        let inline_limit_bytes = crate::agent::tools::tool_result_inline_limit_bytes().await;
        let visible_content_limit_bytes =
            read_file_visible_content_limit_bytes(inline_limit_bytes, show_line_anchors);
        let chunk = read_file_lines_range(
            &safe_path,
            offset_opt,
            size_opt,
            show_line_anchors,
            visible_content_limit_bytes,
        )
        .await;

        match chunk {
            Ok(mut chunk) => {
                info!("Successfully read file: {}", path_str);

                // Get file metadata for stats
                let total_size = fs::metadata(&safe_path)
                    .await
                    .map(|m| m.len())
                    .unwrap_or(chunk.content.len() as u64);
                let size_str = format_file_size(total_size);

                let complete = !chunk.truncated
                    && !chunk.next_line_too_large
                    && (chunk.total_lines == 0
                        || (chunk.displayed_start_line == 1
                            && chunk.displayed_end_line == chunk.total_lines));
                let range_limited = chunk.total_lines > 0
                    && chunk.displayed_end_line < chunk.total_lines
                    && !chunk.truncated
                    && !chunk.next_line_too_large;

                // Request-range remainder: same next-chunk contract as inline truncation.
                if range_limited && chunk.next_start_line.is_none() {
                    let next_start = chunk.displayed_end_line + 1;
                    let next_size = size_opt
                        .filter(|size| *size > 0)
                        .map(|size| size as usize)
                        .unwrap_or(chunk.displayed_line_count)
                        .max(1);
                    chunk.next_start_line = Some(next_start);
                    chunk.suggested_end_line =
                        Some((next_start + next_size - 1).min(chunk.total_lines));
                }

                let chunk_summary = format_read_chunk_summary(&chunk, complete, range_limited);
                let mut summary_notes = Vec::new();
                if (chunk.truncated || range_limited) && !chunk.next_line_too_large {
                    if let Some(next_start_line) = chunk.next_start_line {
                        let next_size = if range_limited {
                            size_opt
                                .filter(|size| *size > 0)
                                .map(|size| size as usize)
                                .unwrap_or(chunk.displayed_line_count)
                                .max(1)
                        } else {
                            chunk.displayed_line_count.max(1)
                        };
                        summary_notes.push(format!(
                            "Next chunk: workspace__readFile({{\"path\": \"{}\", \"offset\": {}, \"size\": {}}})",
                            path_str, next_start_line, next_size
                        ));
                    }
                }
                if chunk.next_line_too_large {
                    let target_line = chunk.next_start_line.unwrap_or(chunk.displayed_start_line);
                    let already_shown = chunk.hard_cut_chars_shown;
                    let mut message = if already_shown > 0 {
                        // A hard-cut preview was shown; guide character-range continuation.
                        format!(
                            "Line {} is too large to fit in one response ({} characters already shown as a hard-cut preview). \
                             To read more of that line, run a shell command to extract a character slice, e.g.: \
                             `(Get-Content -Path \"{}\" -Raw -Encoding UTF8).Substring({})` on Windows (char offset), \
                             or `cut -c{}-{} \"{}\"` on Unix (char offset).",
                            target_line,
                            already_shown,
                            path_str,
                            already_shown,
                            already_shown + 1,
                            already_shown + visible_content_limit_bytes / 3,
                            path_str
                        )
                    } else {
                        format!(
                            "The next unread line is too large to show safely as a complete line. Inspect that line directly with workspace__readFile({{\"path\": \"{}\", \"offset\": {}, \"size\": 1}}).",
                            path_str, target_line
                        )
                    };
                    if show_line_anchors {
                        message.push_str(
                            " If that still truncates, rerun the same 1-line range without showLineAnchors.",
                        );
                    }
                    if already_shown == 0 {
                        message.push_str(
                            " Do not rerun workspace__readFile on a broader range until you have narrowed the line range.",
                        );
                    }
                    summary_notes.push(message);
                }
                let summary_suffix = if summary_notes.is_empty() {
                    String::new()
                } else {
                    format!("\n\n{}", summary_notes.join("\n"))
                };

                // Format response for clean markdown rendering
                let text_message = if show_line_anchors {
                    format!(
                        "📄 **`{}`** — {} — {}{}\n\n```\n{}\n```\n\nLine format: `{{lineNumber}}:{{anchor}}|{{content}}`\n- `{{lineNumber}}`: 1-based line number\n- `{{anchor}}`: 6-character hex code (example: `792c6f`)\n- `{{content}}`: line content\n\n{}{}",
                        path_str, size_str, chunk_summary, summary_suffix, chunk.content, read_file_anchor_prefix_note(), read_file_anchor_output_suffix()
                    )
                } else {
                    let language = detect_language(&safe_path);
                    format!(
                        "📄 **`{}`** — {} — {}{}\n\n```{}\n{}\n```",
                        path_str, size_str, chunk_summary, summary_suffix, language, chunk.content
                    )
                };

                // Omit edit-promotion next-action hints on successful reads.
                // Truncation / next-chunk coaching stays in the message body above.
                let hint = SuccessHint::new(text_message, vec![]);

                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "content": chunk.content,
                    "path": path_str,
                    "size": total_size,
                    "totalLines": chunk.total_lines,
                    "lines": chunk.displayed_line_count,
                    "startLine": chunk.displayed_start_line,
                    "endLine": chunk.displayed_end_line,
                    "complete": complete,
                    "rangeLimited": range_limited,
                    "truncated": chunk.truncated,
                    "nextStartLine": chunk.next_start_line,
                    "suggestedEndLine": chunk.suggested_end_line,
                    "nextLineTooLarge": chunk.next_line_too_large,
                    "hardCutCharsShown": if chunk.hard_cut_chars_shown > 0 { Some(chunk.hard_cut_chars_shown) } else { None }
                }))))
            }
            Err(e) => {
                error!("Failed to read file {}: {}", path_str, e);
                let is_not_found = e.contains("No such file") || e.contains("not found");
                if is_not_found {
                    Ok(not_found_error("File", path_str, ToolGroup::Workspace))
                } else if is_empty_file_out_of_range_error(&e) {
                    Ok(
                        guided_error(ErrorCategory::InvalidInput, &e, ToolGroup::Workspace)
                            .guidance(vec![
                                "Empty files have no readable line range".to_string(),
                                "Omit offset/size to read the empty-file summary".to_string(),
                            ])
                            .to_mcp_result(),
                    )
                } else if let Some((_requested_line, total_lines)) = parse_offset_exceeds_error(&e)
                {
                    Ok(
                        guided_error(ErrorCategory::InvalidInput, &e, ToolGroup::Workspace)
                            .guidance(vec![
                                format!(
                                    "Choose offset between 1 and {} for this file",
                                    total_lines
                                ),
                                "Omit offset/size to read the entire file".to_string(),
                            ])
                            .to_mcp_result(),
                    )
                } else {
                    Ok(
                        guided_error(ErrorCategory::OperationFailed, &e, ToolGroup::Workspace)
                            .guidance(vec![
                                "Verify the file exists with workspace__listDirectory".to_string(),
                                "Check file permissions".to_string(),
                                "Ensure the path is correct".to_string(),
                            ])
                            .to_mcp_result(),
                    )
                }
            }
        }
    }
}

/// Format test lines as either anchor lines or raw file content.
///
/// When `show_anchors` is true the output uses the `{N}:{anchor}|{content}`
/// format. Otherwise it returns raw content without synthetic line numbers.
/// Empty lines are preserved in both modes.
#[cfg(test)]
fn format_lines_with_numbers(lines: &[(usize, String)], show_anchors: bool) -> String {
    if lines.is_empty() {
        return String::new();
    }

    if show_anchors {
        // Anchor format: "{N}:{anchor}|{content}"
        let mut prefix_state = initial_prefix_hash_state();
        return lines
            .iter()
            .map(|(line_num, content)| format_hashline(*line_num, content, &mut prefix_state))
            .collect::<Vec<_>>()
            .join("\n");
    }

    // Return raw content without line numbers
    lines
        .iter()
        .map(|(_, content)| content.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::range::resolve_range;
    use super::*;

    #[test]
    fn test_format_lines_with_numbers_basic() {
        let lines = vec![
            (1, "#include <stdio.h>".to_string()),
            (2, "".to_string()),
            (3, "int main() {".to_string()),
        ];

        let result = format_lines_with_numbers(&lines, false);

        assert!(result.contains("#include <stdio.h>"));
        assert!(result.contains("int main() {"));
    }

    #[test]
    fn test_format_lines_preserves_all_empty_lines() {
        let lines = vec![
            (1, "#include <stdio.h>".to_string()),
            (2, "".to_string()),
            (3, "".to_string()),
            (4, "".to_string()),
            (5, "int main() {".to_string()),
            (6, "".to_string()),
            (7, "".to_string()),
            (8, "    printf(\"Hello\");".to_string()),
            (9, "".to_string()),
            (10, "    return 0;".to_string()),
            (11, "}".to_string()),
        ];

        let result = format_lines_with_numbers(&lines, false);

        assert_eq!(
            result,
            "#include <stdio.h>\n\n\n\nint main() {\n\n\n    printf(\"Hello\");\n\n    return 0;\n}"
        );
    }

    #[test]
    fn test_format_lines_returns_raw_content_without_wrappers() {
        let lines = vec![(1, "int main() {}".to_string()), (2, "".to_string())];

        let result = format_lines_with_numbers(&lines, false);

        assert_eq!(result, "int main() {}\n");
    }

    #[test]
    fn test_format_lines_anchor_format() {
        let lines = vec![
            (11, "function hello() {".to_string()),
            (22, "  return \"world\";".to_string()),
            (33, "}".to_string()),
        ];

        let result = format_lines_with_numbers(&lines, true);

        // Each line must be {N}:{6-char-anchor}|{content}
        let result_lines: Vec<&str> = result.lines().collect();
        assert_eq!(result_lines.len(), 3);

        // Verify format: starts with line number, colon, 6 hex chars, pipe
        for line in &result_lines {
            let parts: Vec<&str> = line.splitn(2, '|').collect();
            assert_eq!(parts.len(), 2, "Anchored line must contain '|' separator");
            let prefix = parts[0];
            let colon_pos = prefix
                .find(':')
                .expect("Anchored line prefix must contain ':'");
            let anchor = &prefix[colon_pos + 1..];
            assert_eq!(anchor.len(), 6, "Anchor must be 6 hex chars");
            assert!(
                anchor.chars().all(|c| c.is_ascii_hexdigit()),
                "Anchor must be hex digits"
            );
        }

        // Verify content is preserved after '|'
        assert!(result_lines[0].ends_with("function hello() {"));
        assert!(result_lines[1].ends_with("  return \"world\";"));
        assert!(result_lines[2].ends_with('}'));

        // Verify determinism: same content → same hash
        let result2 = format_lines_with_numbers(&lines, true);
        assert_eq!(result, result2);
    }

    #[test]
    fn test_resolve_range_simple() {
        // Range mode forward
        assert_eq!(resolve_range(100, Some(10), Some(5)), (10, 14));
        assert_eq!(resolve_range(100, Some(0), Some(5)), (1, 5));

        // Tail mode (size is negative)
        assert_eq!(resolve_range(100, None, Some(-10)), (91, 100));
        assert_eq!(resolve_range(100, Some(-5), Some(-10)), (86, 95));
        assert_eq!(resolve_range(100, Some(50), Some(-10)), (41, 50));

        // Size positive, offset negative
        assert_eq!(resolve_range(100, Some(-5), Some(10)), (96, 105));
    }
}
