use super::super::WorkspaceServer;
use super::utils::{
    detect_language, format_file_size, format_hashline, initial_prefix_hash_state,
    update_prefix_hash_state, LARGE_FILE_THRESHOLD,
};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, not_found_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};
use tokio::fs;
use tracing::{error, info};

const READ_FILE_BASE_HEADROOM_BYTES: usize = 1024;
const READ_FILE_ANCHOR_HEADROOM_BYTES: usize = 2 * 1024;
const READ_FILE_MIN_VISIBLE_CONTENT_BYTES: usize = 1024;

#[derive(Debug)]
struct ReadFileChunk {
    content: String,
    displayed_start_line: usize,
    displayed_end_line: usize,
    displayed_line_count: usize,
    truncated: bool,
    next_start_line: Option<usize>,
    suggested_end_line: Option<usize>,
    next_line_too_large: bool,
}

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
                    "Provide a file path relative to workspace root".to_string(),
                    "Example: {\"path\": \"src/main.rs\"}".to_string(),
                    "Use listDirectory to explore available paths".to_string(),
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
                "Use relative paths from workspace root".to_string(),
                "Example: 'src/main.rs' instead of '../src/main.rs'".to_string(),
                "Use listDirectory to explore available paths".to_string(),
            ])
            .to_mcp_result());
        }

        let start_line = args
            .get("startLine")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let end_line = args
            .get("endLine")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let show_line_anchors = args
            .get("showLineAnchors")
            .and_then(|v| v.as_bool())
            .unwrap_or(false); // Default OFF: reduce noise unless precise editing is needed

        // 3. Line range validation (moved before file access for efficiency)
        if let (Some(start), Some(end)) = (start_line, end_line) {
            if start > end {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("startLine ({}) must be ≤ endLine ({})", start, end),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    format!(
                        "Correct usage: {{\"startLine\": {}, \"endLine\": {}}}",
                        end, start
                    ),
                    "Or omit both parameters to read the entire file".to_string(),
                ])
                .to_mcp_result());
            }

            // Line numbers must be 1-indexed
            if start == 0 || end == 0 {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Line numbers must be ≥ 1 (1-indexed)",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Line numbering starts at 1, not 0".to_string(),
                    "Use startLine: 1 for the first line".to_string(),
                ])
                .to_mcp_result());
            }
        }

        // 4. Path security validation
        let file_manager = self.get_file_manager(session_id.clone());
        let safe_path = match file_manager
            .get_security_validator()
            .validate_path_for_read(path_str)
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
                    "Use listDirectory to see available files".to_string(),
                    "Ensure you have read permissions for the file".to_string(),
                ])
                .to_mcp_result());
            }
        };

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
                "Use listDirectory to see directory contents".to_string(),
                "To read a file inside this directory, specify the full path".to_string(),
                format!("Example: '{}/filename.ext'", path_str),
            ])
            .to_mcp_result());
        }

        // Use the file_manager initialized earlier

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
                "Use grep to find specific content instead".to_string(),
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
            start_line,
            end_line,
            show_line_anchors,
            visible_content_limit_bytes,
        )
        .await;

        match chunk {
            Ok(chunk) => {
                info!("Successfully read file: {}", path_str);

                // Get file metadata for stats
                let total_size = fs::metadata(&safe_path)
                    .await
                    .map(|m| m.len())
                    .unwrap_or(chunk.content.len() as u64);
                let size_str = format_file_size(total_size);
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
                let chunk_summary = if chunk.truncated {
                    format!(
                        "{} shown (truncated to stay under the inline limit)",
                        line_label
                    )
                } else {
                    format!("{} shown", line_label)
                };
                let mut summary_notes = Vec::new();
                if chunk.truncated {
                    if let (Some(next_start_line), Some(suggested_end_line)) =
                        (chunk.next_start_line, chunk.suggested_end_line)
                    {
                        summary_notes.push(format!(
                            "Next chunk: readFile({{\"path\": \"{}\", \"startLine\": {}, \"endLine\": {}}})",
                            path_str, next_start_line, suggested_end_line
                        ));
                    }
                }
                if chunk.next_line_too_large {
                    let target_line = chunk.next_start_line.unwrap_or(chunk.displayed_start_line);
                    let mut message = format!(
                        "The next unread line is too large to show safely as a complete line. Inspect that line directly with readFile({{\"path\": \"{}\", \"startLine\": {}, \"endLine\": {}}}).",
                        path_str, target_line, target_line
                    );
                    if show_line_anchors {
                        message.push_str(
                            " If that still truncates, rerun the same 1-line range without showLineAnchors.",
                        );
                    }
                    message.push_str(
                        " Do not rerun readFile on a broader range until you have narrowed the line range.",
                    );
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
                        "📄 **`{}`** — {} — {}{}\n\n```\n{}\n```\n\nAnchor format: `{{N}}:{{anchor}}|{{content}}` — for edit tools, pass only `{{anchor}}` (the 6 hex characters between `:` and `|`), not `{{N}}:` or `|{{content}}`",
                        path_str, size_str, chunk_summary, summary_suffix, chunk.content
                    )
                } else {
                    let language = detect_language(&safe_path);
                    format!(
                        "📄 **`{}`** — {} — {}{}\n\n```{}\n{}\n```",
                        path_str, size_str, chunk_summary, summary_suffix, language, chunk.content
                    )
                };

                let first_hint = if show_line_anchors {
                    "Use editFile with only the 6-character startAnchor; for ranges, also copy only the 6-character endAnchor from the final line".to_string()
                } else {
                    "Rerun with showLineAnchors=true to get anchors for precise line editing with editFile".to_string()
                };
                let mut next_actions = vec![
                    first_hint,
                    "Use editFile with op='insert_after', startLine, and startAnchor to insert below an existing line".to_string(),
                    "writeFile for full file replacement".to_string(),
                ];
                if let (Some(next_start_line), Some(suggested_end_line)) =
                    (chunk.next_start_line, chunk.suggested_end_line)
                {
                    next_actions.insert(
                        0,
                        format!(
                            "Read the next chunk with readFile({{\"path\": \"{}\", \"startLine\": {}, \"endLine\": {}}})",
                            path_str, next_start_line, suggested_end_line
                        ),
                    );
                }
                let hint = SuccessHint::new(text_message, next_actions);

                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "content": chunk.content,
                    "path": path_str,
                    "size": total_size,
                    "lines": chunk.displayed_line_count,
                    "startLine": chunk.displayed_start_line,
                    "endLine": chunk.displayed_end_line,
                    "truncated": chunk.truncated,
                    "nextStartLine": chunk.next_start_line,
                    "suggestedEndLine": chunk.suggested_end_line,
                    "nextLineTooLarge": chunk.next_line_too_large
                }))))
            }
            Err(e) => {
                error!("Failed to read file {}: {}", path_str, e);
                let is_not_found = e.contains("No such file") || e.contains("not found");
                if is_not_found {
                    Ok(not_found_error("File", path_str, ToolGroup::Workspace))
                } else {
                    Ok(
                        guided_error(ErrorCategory::OperationFailed, &e, ToolGroup::Workspace)
                            .guidance(vec![
                                "Verify the file exists with listDirectory".to_string(),
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

// Helper functions

async fn read_file_lines_range(
    path: &std::path::Path,
    start_line: Option<usize>,
    end_line: Option<usize>,
    show_line_anchors: bool,
    visible_content_limit_bytes: usize,
) -> Result<ReadFileChunk, String> {
    use tokio::io::{AsyncBufReadExt, BufReader};

    // ✅ ENHANCED: Use spawn_blocking for large files to prevent async runtime blocking
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let start = start_line.unwrap_or(1);
    let end = end_line.unwrap_or(usize::MAX);

    if file_size > LARGE_FILE_THRESHOLD {
        // Offload to blocking thread for large files
        let path = path.to_path_buf();

        let result = tokio::task::spawn_blocking(move || {
            // Blocking file I/O for CPU-intensive line enumeration
            use std::io::BufRead;
            let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            let reader = std::io::BufReader::new(file);
            let chunk = read_chunk_from_lines(
                reader.lines(),
                start,
                end,
                show_line_anchors,
                visible_content_limit_bytes,
            )?;
            Ok::<_, String>(chunk)
        })
        .await
        .map_err(|e| format!("Task join error: {}", e))??;

        return Ok(result);
    }

    // Small files: use async path (original implementation)
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut collected_lines = Vec::new();

    loop {
        match lines.next_line().await {
            Ok(Some(line)) => collected_lines.push(line),
            Ok(None) => break,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::InvalidData {
                    return Err("Failed to read file: Content appears to be binary or contains invalid UTF-8 characters. Please use a specialized tool for binary files.".to_string());
                }
                return Err(format!("Failed to read file: {}", e));
            }
        }
    }

    read_chunk_from_lines(
        collected_lines
            .into_iter()
            .map(Ok::<String, std::io::Error>),
        start,
        end,
        show_line_anchors,
        visible_content_limit_bytes,
    )
}

fn read_chunk_from_lines<I>(
    lines: I,
    start: usize,
    end: usize,
    show_line_anchors: bool,
    visible_content_limit_bytes: usize,
) -> Result<ReadFileChunk, String>
where
    I: IntoIterator<Item = Result<String, std::io::Error>>,
{
    let mut result_lines = Vec::new();
    let mut total_lines = 0usize;
    let mut prefix_state = initial_prefix_hash_state();
    let mut content_bytes = 0usize;
    let mut truncated = false;
    let mut next_start_line = None;
    let mut next_line_too_large = false;

    for (current_line, line_result) in (1usize..).zip(lines) {
        let line = line_result.map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidData {
                "Failed to read file: Content appears to be binary or contains invalid UTF-8 characters. Please use a specialized tool for binary files.".to_string()
            } else {
                format!("Failed to read file: {}", e)
            }
        })?;
        total_lines += 1;

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

    if result_lines.is_empty() && start > total_lines {
        return Err(format!(
            "Requested start line {} exceeds file length of {} lines",
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
        next_start_line.map(|next_start| next_start + displayed_line_count.saturating_sub(1))
    };

    Ok(ReadFileChunk {
        content: result_lines.join("\n"),
        displayed_start_line,
        displayed_end_line,
        displayed_line_count,
        truncated,
        next_start_line,
        suggested_end_line,
        next_line_too_large,
    })
}

fn read_file_visible_content_limit_bytes(
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
}
