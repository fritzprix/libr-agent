use super::super::WorkspaceServer;
use super::utils::{detect_language, format_file_size, LARGE_FILE_THRESHOLD};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, not_found_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};
use tokio::fs;
use tracing::{error, info};

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
        let show_line_numbers = args
            .get("showLineNumbers")
            .and_then(|v| v.as_bool())
            .unwrap_or(false); // Default to false for cleaner raw content

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

        let file_manager = self.get_file_manager(session_id);

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
        let content =
            read_file_lines_range(&safe_path, start_line, end_line, show_line_numbers).await;

        match content {
            Ok(content) => {
                info!("Successfully read file: {}", path_str);

                // Get file metadata for stats
                let total_size = fs::metadata(&safe_path)
                    .await
                    .map(|m| m.len())
                    .unwrap_or(content.len() as u64);
                let size_str = format_file_size(total_size);
                let line_count = content.lines().count();

                // Format response for clean markdown rendering
                let text_message = if show_line_numbers {
                    // Line numbers mode: use plain code block
                    format!(
                        "📄 **File: `{}`**\n**Size:** {}\n**Lines:** {}\n\n```\n{}\n```",
                        path_str, size_str, line_count, content
                    )
                } else {
                    // Auto-detect language from file extension for syntax highlighting
                    let language = detect_language(&safe_path);

                    format!(
                        "📄 **File: `{}`**\n**Size:** {}\n**Lines:** {}\n\n```{}\n{}\n```",
                        path_str, size_str, line_count, language, content
                    )
                };

                let hint = SuccessHint::new(
                    text_message,
                    vec![
                        "Use editFile for targeted changes".to_string(),
                        "Use writeFile for full file updates".to_string(),
                        "Use searchLineInFile to find specific text".to_string(),
                    ],
                );

                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "content": content,
                    "path": path_str,
                    "size": total_size,
                    "lines": line_count
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
    show_line_numbers: bool,
) -> Result<String, String> {
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
            let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            let reader = std::io::BufReader::new(file);
            let mut result_lines = Vec::new();
            let mut current_line = 1;
            let mut total_lines = 0;

            use std::io::BufRead;
            for line_result in reader.lines() {
                let line = line_result.map_err(|e| {
                    if e.kind() == std::io::ErrorKind::InvalidData {
                        "Failed to read file: Content appears to be binary or contains invalid UTF-8 characters. Please use a specialized tool for binary files.".to_string()
                    } else {
                        format!("Failed to read file: {}", e)
                    }
                })?;
                total_lines += 1;

                if current_line >= start && current_line <= end {
                    result_lines.push((current_line, line));
                }

                if current_line > end {
                    // Continue counting total lines if checking bounds is critical,
                    // but for performance we might stop if we have what we need.
                    // However, to strictly validate start > total, we need to know total
                    // OR we know if we never reached start.
                    if result_lines.is_empty() {
                        // We haven't found any lines yet, so we must continue
                    } else {
                        break;
                    }
                }

                current_line += 1;
            }

            if result_lines.is_empty() && start > total_lines && total_lines > 0 {
                return Err(format!(
                    "Requested start line {} exceeds file length of {} lines",
                    start, total_lines
                ));
            }

            Ok::<_, String>(format_lines_with_numbers(&result_lines, show_line_numbers))
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
    let mut result_lines = Vec::new();
    let mut current_line = 1;
    let mut total_lines = 0;

    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                total_lines += 1;
                if current_line >= start && current_line <= end {
                    result_lines.push((current_line, line));
                }

                if current_line > end {
                    break;
                }

                current_line += 1;
            }
            Ok(None) => break,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::InvalidData {
                    return Err("Failed to read file: Content appears to be binary or contains invalid UTF-8 characters. Please use a specialized tool for binary files.".to_string());
                }
                return Err(format!("Failed to read file: {}", e));
            }
        }
    }

    // Check if start line was out of bounds
    if result_lines.is_empty() && start > total_lines {
        return Err(format!(
            "Requested start line {} exceeds file length of {} lines",
            start, total_lines
        ));
    }

    Ok(format_lines_with_numbers(&result_lines, show_line_numbers))
}

/// Format lines with pipe-separated line numbers (LLM-friendly format)
///
/// Uses visual separation to prevent confusion between metadata and code:
/// ```text
/// 10 | def calculate_sum(a, b):
/// 11 |     return a + b
/// 12 |
/// ```
///
/// Note: Preserves ALL empty lines for accurate indentation/structure visibility
fn format_lines_with_numbers(lines: &[(usize, String)], show_line_numbers: bool) -> String {
    if lines.is_empty() {
        return String::new();
    }

    if !show_line_numbers {
        // Return raw content without line numbers
        return lines
            .iter()
            .map(|(_, content)| content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
    }

    // Add header for clarity
    let mut result = vec![
        "[File Content - Line numbers are for reference only]".to_string(),
        "─────────────────────────────────────────────────────".to_string(),
    ];

    // Format each line with pipe separator
    for (line_num, content) in lines {
        result.push(format!("{:4} | {}", line_num, content));
    }

    result.push("─────────────────────────────────────────────────────".to_string());
    result.push("(Note: Line numbers and '|' symbols are NOT part of the code)".to_string());

    result.join("\n")
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

        let result = format_lines_with_numbers(&lines, true);

        assert!(result.contains("   1 | #include <stdio.h>"));
        assert!(result.contains("   2 | "));
        assert!(result.contains("   3 | int main() {"));
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

        let result = format_lines_with_numbers(&lines, true);

        // Should have pipe-separated format
        assert!(result.contains("   1 | #include <stdio.h>"));
        assert!(result.contains("   5 | int main() {"));
        assert!(result.contains("   8 |     printf(\"Hello\");"));

        // All empty lines should be preserved (not collapsed)
        assert!(result.contains("   2 | "));
        assert!(result.contains("   3 | "));
        assert!(result.contains("   4 | "));
        assert!(result.contains("   6 | "));
        assert!(result.contains("   7 | "));
        assert!(result.contains("   9 | "));
    }

    #[test]
    fn test_format_lines_includes_header_and_footer() {
        let lines = vec![(1, "int main() {}".to_string()), (2, "".to_string())];

        let result = format_lines_with_numbers(&lines, true);

        assert!(result.contains("[File Content"));
        assert!(result.contains("NOT part of the code"));
        assert!(result.contains("   1 | int main() {}"));
        assert!(result.contains("   2 | "));
    }
}
