use super::super::WorkspaceServer;
use super::utils::{detect_language, format_file_size, LARGE_FILE_THRESHOLD};
use crate::mcp::builtin::error_guidance::{
    missing_param_error, not_found_error, operation_failed_error, permission_denied_error,
    ErrorCategory, ErrorGuidance, SuccessHint, ToolGroup,
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
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Path parameter cannot be empty",
                    vec![
                        "Provide a file path relative to workspace root".to_string(),
                        "Example: {\"path\": \"src/main.rs\"}".to_string(),
                        "Use listDirectory to explore available paths".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };

        // 2. Path pattern validation (reject dangerous patterns)
        if path_str.contains("..") {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                "Path traversal patterns (..) are not allowed",
                vec![
                    "Use relative paths from workspace root".to_string(),
                    "Example: 'src/main.rs' instead of '../src/main.rs'".to_string(),
                    "Use listDirectory to explore available paths".to_string(),
                ],
                ToolGroup::Workspace,
            )
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
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    format!("startLine ({}) must be ≤ endLine ({})", start, end),
                    vec![
                        format!(
                            "Correct usage: {{\"startLine\": {}, \"endLine\": {}}}",
                            end, start
                        ),
                        "Or omit both parameters to read the entire file".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }

            // Line numbers must be 1-indexed
            if start == 0 || end == 0 {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Line numbers must be ≥ 1 (1-indexed)",
                    vec![
                        "Line numbering starts at 1, not 0".to_string(),
                        "Use startLine: 1 for the first line".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        }

        // 4. Path security validation
        let safe_path = match self.validate_path_with_error(path_str, session_id.clone()) {
            Ok(path) => path,
            Err(e) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::PermissionDenied,
                    format!("Path validation failed: {}", e),
                    vec![
                        "Verify the file path is within workspace boundaries".to_string(),
                        "Use listDirectory to see available files".to_string(),
                        "Avoid absolute paths outside workspace".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        };

        // 5. File existence check
        if !safe_path.exists() {
            return Ok(not_found_error("File", path_str, ToolGroup::Workspace));
        }

        // 6. File type check (must be file, not directory)
        if safe_path.is_dir() {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                format!("'{}' is a directory, not a file", path_str),
                vec![
                    "Use listDirectory to see directory contents".to_string(),
                    "To read a file inside this directory, specify the full path".to_string(),
                    format!("Example: '{}/filename.ext'", path_str),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        let file_manager = self.get_file_manager(session_id);

        // Security check: validate file size before reading
        if let Err(e) = file_manager
            .get_security_validator()
            .validate_file_size(&safe_path, crate::config::max_file_size())
        {
            error!("File size validation failed: {}", e);
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                format!("File size error: {}", e),
                vec![
                    "The file is too large to read entirely".to_string(),
                    "Try reading specific line ranges if possible".to_string(),
                    "Use grep to find specific content instead".to_string(),
                ],
                ToolGroup::Workspace,
            )
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
                let total_size = tokio::fs::metadata(&safe_path)
                    .await
                    .map(|m| m.len())
                    .unwrap_or(content.len() as u64);
                let size_str = format_file_size(total_size);
                let line_count = content.lines().count();

                // Format response for clean markdown rendering
                let text_message = if show_line_numbers {
                    // Line numbers mode: use plain code block
                    format!(
                        "📄 **File: `{}`**\n**Size:** {}\n**Lines:** {}\n\n```\n{}\n```\n\n💡 **Next Steps:**\n- Use `createFile` to create or overwrite the file\n- Use `editFile` to make targeted edits",
                        path_str,
                        size_str,
                        line_count,
                        content
                    )
                } else {
                    // Auto-detect language from file extension for syntax highlighting
                    let language = detect_language(&safe_path);

                    format!(
                        "📄 **File: `{}`**\n**Size:** {}\n**Lines:** {}\n\n```{}\n{}\n```\n\n💡 **Next Steps:**\n- Use `createFile` to create or overwrite the file\n- Use `editFile` to make targeted edits",
                        path_str,
                        size_str,
                        line_count,
                        language,
                        content
                    )
                };

                Ok(MCPResult::success_with_data(
                    &text_message,
                    json!({
                        "content": content,
                        "path": path_str,
                        "size": total_size,
                        "lines": line_count
                    }),
                ))
            }
            Err(e) => {
                error!("Failed to read file {}: {}", path_str, e);
                let is_not_found = e.contains("No such file") || e.contains("not found");
                if is_not_found {
                    Ok(not_found_error("File", path_str, ToolGroup::Workspace))
                } else {
                    Ok(operation_failed_error(
                        "Read file",
                        &e,
                        vec![
                            "Verify the file exists with listDirectory".to_string(),
                            "Check file permissions".to_string(),
                            "Ensure the path is correct".to_string(),
                        ],
                        ToolGroup::Workspace,
                    ))
                }
            }
        }
    }

    pub async fn handle_create_file(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // Layer 1: Proactive Parameter Validation

        // 1. Path parameter existence and non-empty check
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) if !path.trim().is_empty() => path.trim(),
            Some(_) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Path parameter cannot be empty",
                    vec![
                        "Provide a file path relative to workspace root".to_string(),
                        "Example: {\"path\": \"src/main.rs\"}".to_string(),
                        "Use listDirectory('.') to explore available paths".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };

        // 2. Path traversal validation
        if path_str.contains("..") {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                "Path traversal patterns (..) are not allowed",
                vec![
                    "Use relative paths from workspace root".to_string(),
                    "Example: 'src/main.rs' instead of '../src/main.rs'".to_string(),
                    "Use listDirectory to explore available paths".to_string(),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        // 3. Content parameter validation
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(content) => content,
            None => {
                return Ok(missing_param_error("content", ToolGroup::Workspace));
            }
        };

        // Validate path security
        let safe_path = match self.validate_path_with_error(path_str, session_id.clone()) {
            Ok(path) => path,
            Err(e) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::PermissionDenied,
                    format!("Path validation failed: {}", e),
                    vec![
                        "Verify the file path is within workspace boundaries".to_string(),
                        "Use listDirectory to see available paths".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        };

        // Check if file already exists - PREVENT OVERWRITE
        if safe_path.exists() {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                format!(
                    "File '{}' already exists - createFile cannot overwrite",
                    path_str
                ),
                vec![
                    "✅ RECOMMENDED: For incremental changes (safer)".to_string(),
                    format!(
                        "   → First: readFile(\"{}\") to see current content",
                        path_str
                    ),
                    format!("   → Then: editFile(\"{}\", oldText, newText)", path_str),
                    "   → Why: Preserves existing content, only changes specific sections"
                        .to_string(),
                    "".to_string(),
                    "⚠️ ALTERNATIVE: Complete file replacement (destructive)".to_string(),
                    format!("   → First: deleteFile(\"{}\")", path_str),
                    format!("   → Then: createFile(\"{}\", newContent)", path_str),
                    "   → Why: Use when rewriting entire file structure".to_string(),
                    "".to_string(),
                    "💡 DECISION GUIDE:".to_string(),
                    "   • Small edits → Use editFile".to_string(),
                    "   • Add/remove sections → Use editFile".to_string(),
                    "   • Complete rewrite → Use deleteFile + createFile".to_string(),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        let file_manager = self.get_file_manager(session_id.clone());
        let result = file_manager.write_file_string(path_str, content).await;

        match result {
            Ok(()) => {
                info!("Successfully created new file: {}", path_str);

                // Invalidate service context cache
                self.invalidate_context_cache().await;

                let lines = content.lines().count();
                let size_str = format_file_size(content.len() as u64);
                let language = detect_language(std::path::Path::new(path_str));

                // Truncate content for display - show only first 100 lines as preview
                let max_display_lines = 100;
                let max_display_bytes = 51200; // 50KB
                let content_lines: Vec<&str> = content.lines().collect();
                let is_truncated =
                    content_lines.len() > max_display_lines || content.len() > max_display_bytes;

                let display_content = if is_truncated {
                    let truncated_lines: Vec<&str> = if content.len() > max_display_bytes {
                        // Truncate by bytes first
                        let truncated = &content[..max_display_bytes.min(content.len())];
                        truncated.lines().take(max_display_lines).collect()
                    } else {
                        content_lines
                            .iter()
                            .take(max_display_lines)
                            .copied()
                            .collect()
                    };
                    format!(
                        "{}\n\n... ⚠️ TRUNCATED: Showing first {} of {} lines ({}% shown)",
                        truncated_lines.join("\n"),
                        truncated_lines.len(),
                        content_lines.len(),
                        (truncated_lines.len() * 100) / content_lines.len()
                    )
                } else {
                    content.to_string()
                };

                let mut message = format!(
                    "**✅ File Created**\n\n\
                    **File:** `{}`\n\
                    **Size:** {}\n\
                    **Lines:** {}\n\n\
                    **Content:**\n```{}",
                    path_str, size_str, lines, language
                );

                message.push('\n');
                message.push_str(&display_content);
                message.push_str("\n```\n\n");

                if is_truncated {
                    message.push_str(
                        "⚠️ **CONTENT TRUNCATED**: Only showing first 100 lines as preview\n\n",
                    );
                }

                // Context-aware next steps
                let mut next_steps = vec!["- Content verified above (preview only)".to_string()];

                if is_truncated {
                    next_steps.push(format!(
                        "- 📖 Use `readFile(\"{}\")` to see full content",
                        path_str
                    ));
                }

                // File type specific suggestions
                if path_str.ends_with(".md")
                    || path_str.ends_with(".txt")
                    || path_str.ends_with(".rs")
                    || path_str.ends_with(".js")
                    || path_str.ends_with(".ts")
                {
                    next_steps.push(format!(
                        "- ✏️ Use `editFile(\"{}\", oldText, newText)` for edits",
                        path_str
                    ));
                } else if path_str.ends_with(".json") || path_str.ends_with(".yaml") {
                    next_steps.push(format!(
                        "- 🔍 Use `grep(\"{}\", pattern)` to validate structure",
                        path_str
                    ));
                    next_steps.push(format!(
                        "- ✏️ Use `editFile(\"{}\", oldText, newText)` for edits",
                        path_str
                    ));
                }

                next_steps.push(format!(
                    "- 🗑️ Use `deleteFile(\"{}\")` to remove if needed",
                    path_str
                ));

                message.push_str(&format!("**Next Steps:**\n{}", next_steps.join("\n")));

                Ok(MCPResult::success_with_data(
                    &message,
                    json!({
                        "path": path_str,
                        "bytes_written": content.len(),
                        "lines": lines,
                        "truncated": is_truncated
                    }),
                ))
            }
            Err(e) => {
                error!("Failed to write file {}: {}", path_str, e);
                let is_permission = e.to_string().contains("Permission denied")
                    || e.to_string().contains("permission");
                if is_permission {
                    Ok(permission_denied_error(path_str, ToolGroup::Workspace))
                } else {
                    Ok(operation_failed_error(
                        "Write file",
                        &e.to_string(),
                        vec![
                            "Check that the directory exists with listDirectory".to_string(),
                            "Verify you have write permissions".to_string(),
                            "Ensure the path is valid and within allowed directories".to_string(),
                        ],
                        ToolGroup::Workspace,
                    ))
                }
            }
        }
    }

    pub async fn handle_import_file(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // ✅ ENHANCED: Replace legacy MCPResult::error() with ErrorGuidance for better context

        // Parameter validation 1: srcAbsPath
        let src_path_str = match args
            .get("srcAbsPath")
            .or_else(|| args.get("src_abs_path"))
            .and_then(|v| v.as_str())
        {
            Some(path) => path,
            None => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Missing required parameter: srcAbsPath",
                    vec![
                        "Provide the absolute path to the file you want to import".to_string(),
                        "Example: {\"srcAbsPath\": \"/home/user/file.txt\", \"destRelPath\": \"imports/file.txt\"}".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        };

        // Parameter validation 2: destRelPath
        let dest_rel_path = match args
            .get("destRelPath")
            .or_else(|| args.get("dest_rel_path"))
            .and_then(|v| v.as_str())
        {
            Some(path) => path,
            None => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Missing required parameter: destRelPath",
                    vec![
                        "Provide the destination path relative to workspace root".to_string(),
                        "Example: \"imports/filename.ext\" or \"src/data/file.txt\"".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        };

        // Log import attempt for debugging
        info!(
            "importFile called: src='{}', dest='{}'",
            src_path_str, dest_rel_path
        );

        // Validate source path exists and is readable
        let src_path = match std::path::Path::new(src_path_str).canonicalize() {
            Ok(path) => path,
            Err(e) => {
                error!(
                    "Failed to canonicalize source path '{}': {}",
                    src_path_str, e
                );
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::ResourceNotFound,
                    format!("Source file not found or cannot be accessed: {}", src_path_str),
                    vec![
                        "Verify the file path is correct and the file exists".to_string(),
                        "Check file permissions and ensure you have read access".to_string(),
                        format!("On Windows, use absolute paths like 'C:\\Users\\...', on Unix like '/home/user/...'"),
                        "Use an absolute path, not a relative path".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        };

        // Ensure source is a file, not a directory
        if !src_path.is_file() {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                format!("Source path is a directory, not a file: {}", src_path_str),
                vec![
                    "Provide the path to a specific file, not a directory".to_string(),
                    "To import multiple files, call importFile multiple times".to_string(),
                    "To import directory contents, use shell commands (e.g., runShell('cp -r src dest'))".to_string(),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        // Use file manager to handle destination path validation and copying
        let file_manager = self.get_file_manager(session_id);
        match file_manager
            .copy_file_from_external(&src_path, dest_rel_path)
            .await
        {
            Ok(dest_path) => {
                info!(
                    "Successfully imported file from {} to {}",
                    src_path.display(),
                    dest_path.display()
                );

                // Get file size for reporting
                let file_size = match fs::metadata(&dest_path).await {
                    Ok(metadata) => metadata.len(),
                    Err(_) => 0,
                };

                let hint = SuccessHint::new(
                    format!(
                        "✅ Successfully imported {} ({} bytes) to {}",
                        src_path.display(),
                        file_size,
                        dest_rel_path
                    ),
                    vec![
                        format!(
                            "Use readFile(\"{}\") to view imported content",
                            dest_rel_path
                        ),
                        "Use createFile to modify the imported file".to_string(),
                    ],
                );

                Ok(hint.to_mcp_result())
            }
            Err(e) => {
                error!(
                    "Failed to import file from {} to {}: {}",
                    src_path.display(),
                    dest_rel_path,
                    e
                );

                // Provide context-specific error guidance
                let (category, guidance) = if e.contains("already exists")
                    || e.contains("duplicate")
                {
                    (
                        ErrorCategory::InvalidInput,
                        vec![
                            format!("File already exists at: {}", dest_rel_path),
                            "Use createFile to overwrite the existing file".to_string(),
                            "Or specify a different destination path with a unique name"
                                .to_string(),
                        ],
                    )
                } else if e.contains("permission") || e.contains("denied") {
                    (
                        ErrorCategory::PermissionDenied,
                        vec![
                            "Insufficient permissions to write to destination".to_string(),
                            "Check workspace permissions and destination directory access"
                                .to_string(),
                            "Ensure you have write access to the destination directory".to_string(),
                        ],
                    )
                } else if e.contains("space") {
                    (
                        ErrorCategory::InvalidInput,
                        vec![
                            "Insufficient disk space to import file".to_string(),
                            "Free up disk space and try again".to_string(),
                        ],
                    )
                } else {
                    (
                        ErrorCategory::InvalidInput,
                        vec![
                            "Verify source file is accessible and destination path is valid"
                                .to_string(),
                            "Check workspace configuration and file manager settings".to_string(),
                        ],
                    )
                };

                Ok(ErrorGuidance::with_guidance(
                    category,
                    format!("Failed to import file: {}", e),
                    guidance,
                    ToolGroup::Workspace,
                )
                .to_mcp_result())
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
                let line = line_result.map_err(|e| e.to_string())?;
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

    while let Ok(Some(line)) = lines.next_line().await {
        total_lines += 1;
        if current_line >= start && current_line <= end {
            result_lines.push((current_line, line));
        }

        if current_line > end {
            break;
        }

        current_line += 1;
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
