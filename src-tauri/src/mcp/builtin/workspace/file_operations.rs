use super::WorkspaceServer;
use crate::mcp::builtin::error_guidance::{
    missing_param_error, not_found_error, operation_failed_error, permission_denied_error,
    ErrorCategory, ErrorGuidance, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use regex;
use serde_json::{json, Value};
use tokio::fs;
use tracing::{error, info};

// ✅ ENHANCED: Threshold for using spawn_blocking for CPU-intensive line enumeration
// Large files can block the async runtime during line enumeration
const LARGE_FILE_THRESHOLD: u64 = 1_048_576; // 1 MB in bytes

/// Format file size in bytes to human-readable format (B, KB, MB, GB)
fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

/// Detect language/syntax highlighting identifier from file extension
fn detect_language(path: &std::path::Path) -> &'static str {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match extension {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "md" => "markdown",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "sh" => "bash",
        "ps1" => "powershell",
        "html" => "html",
        "css" => "css",
        "go" => "go",
        "java" => "java",
        "c" => "c",
        "cpp" | "cc" | "cxx" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "sql" => "sql",
        "xml" => "xml",
        "txt" | "log" => "text",
        _ => "",
    }
}

#[allow(dead_code)]
impl WorkspaceServer {
    fn validate_path_with_error(
        &self,
        path_str: &str,
        session_id: Option<String>,
    ) -> Result<std::path::PathBuf, String> {
        let file_manager = self.get_file_manager(session_id);
        match file_manager
            .get_security_validator()
            .validate_path(path_str)
        {
            Ok(path) => Ok(path),
            Err(e) => {
                error!("Path validation failed: {}", e);
                Err(format!("Security error: {e}"))
            }
        }
    }

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
        let content = self
            .read_file_lines_range(&safe_path, start_line, end_line, show_line_numbers)
            .await;

        match content {
            Ok(content) => {
                info!("Successfully read file: {}", path_str);

                // Format response for clean markdown rendering
                let text_message = if show_line_numbers {
                    // Line numbers mode: use plain code block
                    format!(
                        "📄 **File: `{}`**\n\n```\n{}\n```\n\n💡 **Next Steps:**\n- Use `writeFile` to modify the entire file\n- Use `replaceStringInFile` to make targeted edits",
                        path_str,
                        content
                    )
                } else {
                    // Auto-detect language from file extension for syntax highlighting
                    let language = safe_path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| match ext {
                            "rs" => "rust",
                            "ts" | "tsx" => "typescript",
                            "js" | "jsx" => "javascript",
                            "py" => "python",
                            "md" => "markdown",
                            "json" => "json",
                            "yaml" | "yml" => "yaml",
                            "toml" => "toml",
                            "sh" => "bash",
                            "ps1" => "powershell",
                            "html" => "html",
                            "css" => "css",
                            "go" => "go",
                            "java" => "java",
                            "c" | "h" => "c",
                            "cpp" | "hpp" | "cc" => "cpp",
                            "cs" => "csharp",
                            "rb" => "ruby",
                            "php" => "php",
                            "swift" => "swift",
                            "kt" => "kotlin",
                            "sql" => "sql",
                            "xml" => "xml",
                            _ => ext,
                        })
                        .unwrap_or("");

                    format!(
                        "📄 **File: `{}`**\n\n```{}\n{}\n```\n\n💡 **Next Steps:**\n- Use `writeFile` to modify the entire file\n- Use `replaceStringInFile` to make targeted edits",
                        path_str,
                        language,
                        content
                    )
                };

                Ok(MCPResult::success_with_data(
                    &text_message,
                    json!({
                        "content": content,
                        "path": path_str,
                        "size": content.len()
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

    async fn read_file_lines(&self, path: &std::path::Path) -> Result<Vec<String>, String> {
        use tokio::io::{AsyncBufReadExt, BufReader};

        let file = tokio::fs::File::open(path)
            .await
            .map_err(|e| e.to_string())?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut result_lines = Vec::new();

        while let Ok(Some(line)) = lines.next_line().await {
            result_lines.push(line);
        }

        Ok(result_lines)
    }

    async fn read_file_lines_range(
        &self,
        path: &std::path::Path,
        start_line: Option<usize>,
        end_line: Option<usize>,
        show_line_numbers: bool,
    ) -> Result<String, String> {
        use tokio::io::{AsyncBufReadExt, BufReader};

        // ✅ ENHANCED: Use spawn_blocking for large files to prevent async runtime blocking
        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        if file_size > LARGE_FILE_THRESHOLD {
            // Offload to blocking thread for large files
            let path = path.to_path_buf();
            let start = start_line.unwrap_or(1);
            let end = end_line.unwrap_or(usize::MAX);

            let result = tokio::task::spawn_blocking(move || {
                // Blocking file I/O for CPU-intensive line enumeration
                let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
                let reader = std::io::BufReader::new(file);
                let mut result_lines = Vec::new();
                let mut current_line = 1;

                use std::io::BufRead;
                for line_result in reader.lines() {
                    let line = line_result.map_err(|e| e.to_string())?;

                    if current_line >= start && current_line <= end {
                        result_lines.push((current_line, line));
                    }

                    if current_line > end {
                        break;
                    }

                    current_line += 1;
                }

                Ok::<_, String>(Self::format_lines_with_numbers(
                    &result_lines,
                    show_line_numbers,
                ))
            })
            .await
            .map_err(|e| format!("Task join error: {}", e))?;

            return result;
        }

        // Small files: use async path (original implementation)
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|e| e.to_string())?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut result_lines = Vec::new();
        let mut current_line = 1;

        let start = start_line.unwrap_or(1);
        let end = end_line.unwrap_or(usize::MAX);

        while let Ok(Some(line)) = lines.next_line().await {
            if current_line >= start && current_line <= end {
                result_lines.push((current_line, line));
            }

            if current_line > end {
                break;
            }

            current_line += 1;
        }

        Ok(Self::format_lines_with_numbers(
            &result_lines,
            show_line_numbers,
        ))
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

    pub async fn handle_write_file(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) => path,
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };

        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(content) => content,
            None => {
                return Ok(missing_param_error("content", ToolGroup::Workspace));
            }
        };

        let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("w");

        let file_manager = self.get_file_manager(session_id.clone());

        // Read original content for diff generation (overwrite mode only)
        let original_content = if mode == "w" {
            let path = std::path::Path::new(path_str);
            if path.exists() {
                tokio::fs::read_to_string(path).await.ok()
            } else {
                None
            }
        } else {
            None
        };

        let result = match mode {
            "w" => file_manager.write_file_string(path_str, content).await,
            "a" => file_manager.append_file_string(path_str, content).await,
            _ => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    format!("Invalid mode '{}'. Must be 'w' or 'a'", mode),
                    vec![
                        "Use 'w' to overwrite the file (default)".to_string(),
                        "Use 'a' to append to the file".to_string(),
                        "Example: {\"mode\": \"w\"}".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        };

        match result {
            Ok(()) => {
                info!("Successfully wrote file: {}", path_str);

                // Invalidate service context cache
                self.invalidate_context_cache().await;

                let action = if mode == "a" { "Appended" } else { "Written" };
                let action_emoji = if mode == "a" { "➕" } else { "✅" };

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

                // Generate diff for overwrite mode if original content exists
                let diff_section =
                    if let (true, Some(old_content)) = (mode == "w", &original_content) {
                        let diff = self.format_file_diff(old_content, content, path_str);
                        format!("\n{}\n\n", diff)
                    } else {
                        String::new()
                    };

                let mut message = if mode == "w" && original_content.is_some() {
                    // Overwrite mode with existing file - show diff
                    format!(
                        "**{} File Overwritten**\n\n\
                        **File:** `{}`\n\
                        **Mode:** {} ({})\n\
                        **Size:** {}\n\
                        **Lines:** {}\n\
                        {}",
                        action_emoji, path_str, mode, "overwrite", size_str, lines, diff_section
                    )
                } else {
                    // Append mode or new file - show content
                    let mut msg = format!(
                        "**{} File {}**\n\n\
                        **File:** `{}`\n\
                        **Mode:** {} ({})\n\
                        **Size:** {}\n\
                        **Lines:** {}\n\n\
                        **Content Written:**\n```{}",
                        action_emoji,
                        action,
                        path_str,
                        mode,
                        if mode == "a" { "append" } else { "create new" },
                        size_str,
                        lines,
                        language
                    );

                    msg.push('\n');
                    msg.push_str(&display_content);
                    msg.push_str("\n```\n\n");

                    if is_truncated {
                        msg.push_str(
                            "⚠️ **CONTENT TRUNCATED**: Only showing first 100 lines as preview\n\n",
                        );
                    }

                    msg
                };

                message.push_str(
                    "**Next Steps:**\n\
                    - Content verified above (preview only)\n\
                    - 📖 Use `readFile` to see full content if truncated\n\
                    - 🔍 Use `grep` to search within the file",
                );

                Ok(MCPResult::success_with_data(
                    &message,
                    json!({
                        "path": path_str,
                        "bytes_written": content.len(),
                        "lines": lines,
                        "mode": mode,
                        "truncated": is_truncated,
                        "had_original": original_content.is_some()
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

    pub async fn handle_list_directory(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let safe_path = match self.validate_path_with_error(path_str, session_id.clone()) {
            Ok(path) => path,
            Err(e) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::PermissionDenied,
                    format!("Path validation failed: {}", e),
                    vec![
                        "Verify the directory path is within allowed directories".to_string(),
                        "Check that the path doesn't contain '..' or absolute paths outside workspace".to_string(),
                        "Try using '.' to list the current directory".to_string(),
                    ],
                    ToolGroup::Workspace,
                ).to_mcp_result());
            }
        };

        match fs::read_dir(&safe_path).await {
            Ok(mut entries) => {
                let mut items = Vec::new();

                while let Ok(Some(entry)) = entries.next_entry().await {
                    if let Ok(metadata) = entry.metadata().await {
                        let file_type = if metadata.is_dir() {
                            "directory"
                        } else if metadata.is_file() {
                            "file"
                        } else {
                            "other"
                        };

                        let name = entry.file_name().to_string_lossy().to_string();
                        let size = if metadata.is_file() {
                            Some(metadata.len())
                        } else {
                            None
                        };

                        items.push(json!({
                            "name": name,
                            "type": file_type,
                            "size": size
                        }));
                    }
                }

                items.sort_by(|a, b| {
                    let a_type = a.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let b_type = b.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let a_name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let b_name = b.get("name").and_then(|v| v.as_str()).unwrap_or("");

                    match (a_type, b_type) {
                        ("directory", "file") => std::cmp::Ordering::Less,
                        ("file", "directory") => std::cmp::Ordering::Greater,
                        _ => a_name.cmp(b_name),
                    }
                });

                // ✅ ENHANCED: Format listing with emojis, types, and sizes for AI visibility
                const MAX_ITEMS_DISPLAY: usize = 100;

                let item_lines: Vec<String> = items
                    .iter()
                    .take(MAX_ITEMS_DISPLAY)
                    .map(|item| {
                        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                        let type_ = item.get("type").and_then(|v| v.as_str()).unwrap_or("?");
                        let size = item.get("size").and_then(|v| v.as_u64());

                        // Use emoji icons for visual clarity
                        let icon = match type_ {
                            "directory" => "📁",
                            "file" => "📄",
                            "symlink" => "🔗",
                            _ => "❓",
                        };

                        // Format size in human-readable way
                        let size_str = if let Some(s) = size {
                            if s < 1024 {
                                format!(" ({}B)", s)
                            } else if s < 1024 * 1024 {
                                format!(" ({:.1}KB)", s as f64 / 1024.0)
                            } else {
                                format!(" ({:.1}MB)", s as f64 / 1024.0 / 1024.0)
                            }
                        } else {
                            "".to_string()
                        };

                        format!("{} [{}] {}{}", icon, type_, name, size_str)
                    })
                    .collect();

                // Add truncation note if needed
                let truncation_note = if items.len() > MAX_ITEMS_DISPLAY {
                    format!("\n\n... and {} more items", items.len() - MAX_ITEMS_DISPLAY)
                } else {
                    String::new()
                };

                info!(
                    "Successfully listed directory: {:?} ({} items)",
                    safe_path,
                    items.len()
                );

                // ✅ ENHANCED: Clear messaging for empty directories
                let hint = if items.is_empty() {
                    SuccessHint::new(
                        format!(
                            "Directory listing for '{}':

(This directory is empty)

💡 Next Steps:
- Use writeFile('{}/filename.txt', content) to create a file
- Use listDirectory('{}') to verify the directory exists
- This is a valid empty directory",
                            path_str, path_str, path_str
                        ),
                        vec![],
                    )
                } else {
                    let listing_str = item_lines.join("\n");
                    SuccessHint::new(
                        format!(
                            "Directory listing for '{}':

{}{}

💡 Next Steps:
- Use readFile('{}/filename') to read a file
- Use listDirectory('{}/subdir') to explore subdirectories
- Use grep to search for content in files",
                            path_str, listing_str, truncation_note, path_str, path_str
                        ),
                        vec![],
                    )
                };

                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "items": items,
                    "path": path_str,
                    "count": items.len()
                }))))
            }
            Err(e) => {
                error!("Failed to list directory {:?}: {}", safe_path, e);
                let is_not_found =
                    e.to_string().contains("No such file") || e.to_string().contains("not found");
                if is_not_found {
                    Ok(not_found_error("Directory", path_str, ToolGroup::Workspace))
                } else {
                    Ok(operation_failed_error(
                        "List directory",
                        &e.to_string(),
                        vec![
                            "Verify the directory exists".to_string(),
                            "Check directory permissions".to_string(),
                            "Try using '.' to list the current directory".to_string(),
                        ],
                        ToolGroup::Workspace,
                    ))
                }
            }
        }
    }

    pub async fn handle_search_files(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(pattern) => pattern,
            None => {
                return Ok(missing_param_error("pattern", ToolGroup::Workspace));
            }
        };

        let search_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let max_depth = args
            .get("max_depth")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let file_type = args
            .get("file_type")
            .and_then(|v| v.as_str())
            .unwrap_or("both");

        let safe_path = self.validate_path_with_error(search_path, session_id)?;

        match self
            .search_files_by_pattern(&safe_path, pattern, max_depth, file_type)
            .await
        {
            Ok(results) => {
                let result_text = if results.is_empty() {
                    format!(
                        "**🔍 File Search: No matches found**\n\n\
                        Pattern: `{}`\n\
                        Search Path: `{}`\n\
                        File Type: {}\n\n\
                        **Next Steps:**\n\
                        - Verify the pattern syntax (use glob format like `*.txt` or `**/*.rs`)\n\
                        - Use listDirectory to explore available files\n\
                        - Try a broader pattern or different search path",
                        pattern, search_path, file_type
                    )
                } else {
                    let mut text = format!(
                        "**🔍 File Search: {} file(s) found**\n\n\
                        Pattern: `{}`\n\
                        Search Path: `{}`\n\
                        File Type: {}\n\n",
                        results.len(),
                        pattern,
                        search_path,
                        file_type
                    );

                    // Get metadata for file sizes
                    let mut enriched_results = Vec::new();
                    for item in results.iter().take(50) {
                        let path_str = item.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                        let type_ = item.get("type").and_then(|v| v.as_str()).unwrap_or("?");

                        let size_str = if type_ == "file" {
                            let full_path = safe_path.join(path_str);
                            match tokio::fs::metadata(&full_path).await {
                                Ok(metadata) => format!(" ({})", format_file_size(metadata.len())),
                                Err(_) => String::new(),
                            }
                        } else {
                            String::new()
                        };

                        let icon = if type_ == "file" { "📄" } else { "📁" };
                        enriched_results.push((icon, path_str, size_str));
                    }

                    // Display results
                    text.push_str("**Matches:**\n");
                    for (icon, path, size) in &enriched_results {
                        text.push_str(&format!("- {} `{}`{}\n", icon, path, size));
                    }

                    if results.len() > 50 {
                        text.push_str(&format!(
                            "\n*Showing first 50 of {} total matches*\n",
                            results.len()
                        ));
                    }

                    text.push_str(
                        "\n**Next Steps:**\n\
                        - Use readFile to view file contents\n\
                        - Use grep to search within matching files\n\
                        - Refine pattern for more specific results",
                    );

                    text
                };

                Ok(MCPResult::success_with_data(
                    &result_text,
                    json!({ "matches": results }),
                ))
            }
            Err(e) => {
                error!("File search failed: {}", e);
                Ok(operation_failed_error(
                    "Search files",
                    &e.to_string(),
                    vec![
                        "Verify the pattern syntax is correct (use glob format like '*.txt' or '**/*.rs')".to_string(),
                        "Check if the directory path exists with listDirectory".to_string(),
                        "Try a simpler pattern to narrow down the issue".to_string(),
                    ],
                    ToolGroup::Workspace,
                ))
            }
        }
    }

    async fn search_files_by_pattern(
        &self,
        root_path: &std::path::Path,
        pattern: &str,
        max_depth: Option<usize>,
        file_type: &str,
    ) -> Result<Vec<serde_json::Value>, String> {
        use glob::Pattern;
        use walkdir::WalkDir;

        let glob_pattern = Pattern::new(pattern).map_err(|e| format!("Invalid pattern: {e}"))?;
        let mut results = Vec::new();

        let walker = if let Some(depth) = max_depth {
            WalkDir::new(root_path).max_depth(depth)
        } else {
            WalkDir::new(root_path)
        };

        for entry in walker {
            let entry = entry.map_err(|e| format!("Walk error: {e}"))?;
            let path = entry.path();

            let is_dir = path.is_dir();
            let is_file = path.is_file();

            let should_include = match file_type {
                "file" => is_file,
                "dir" => is_dir,
                "both" => is_file || is_dir,
                _ => is_file || is_dir,
            };

            if !should_include {
                continue;
            }

            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if glob_pattern.matches(file_name) || glob_pattern.matches(&path.to_string_lossy())
                {
                    let metadata = entry
                        .metadata()
                        .map_err(|e| format!("Metadata error: {e}"))?;

                    results.push(json!({
                        "path": path.to_string_lossy(),
                        "name": file_name,
                        "type": if is_dir { "directory" } else { "file" },
                        "size": if is_file { Some(metadata.len()) } else { None }
                    }));
                }
            }
        }

        Ok(results)
    }

    /// Preview replacement without modifying file
    pub async fn handle_preview_replacement(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        use crate::mcp::builtin::error_guidance::ErrorCategory;

        // Parameter validation
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) if !path.trim().is_empty() => path.trim(),
            Some(_) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Parameter 'path' cannot be empty",
                    vec!["Provide a valid file path".to_string()],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };

        let old_string = match args.get("oldString").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s,
            Some(_) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Parameter 'oldString' cannot be empty",
                    vec![
                        "Extract exact text from readFile response".to_string(),
                        "Include surrounding context for uniqueness".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
            None => return Ok(missing_param_error("oldString", ToolGroup::Workspace)),
        };

        let new_string = match args.get("newString").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return Ok(missing_param_error("newString", ToolGroup::Workspace)),
        };

        // Validate path and read file
        let safe_path = self.validate_path_with_error(path_str, session_id)?;
        let original_content = match self.read_file_as_string(&safe_path).await {
            Ok(content) => content,
            Err(e) => {
                return Ok(operation_failed_error(
                    "Read file for preview",
                    &e,
                    vec![
                        "Verify file exists with listDirectory".to_string(),
                        format!("Use readFile('{}') to check content", path_str),
                    ],
                    ToolGroup::Workspace,
                ));
            }
        };

        // Find matches and generate preview
        let occurrences = original_content.matches(old_string).count();

        if occurrences == 0 {
            // Similar content search
            let lines: Vec<&str> = original_content.lines().collect();
            let old_lines: Vec<&str> = old_string.lines().collect();
            let search_size = old_lines.len();

            let mut best_match: Option<(usize, f32)> = None;
            for (line_idx, window) in lines.windows(search_size.max(1)).enumerate() {
                let window_text = window.join("\n");
                let similarity = self.calculate_similarity(&window_text, old_string);
                if similarity > 0.3 && (best_match.is_none() || similarity > best_match.unwrap().1)
                {
                    best_match = Some((line_idx + 1, similarity));
                }
            }

            let suggestion = if let Some((line_num, similarity)) = best_match {
                format!(
                    "❌ Pattern NOT FOUND (but {}% similar at line {})\n\n\
                    💡 NEXT: Use readFile('{}', {}, {}) to see actual content",
                    (similarity * 100.0) as u32,
                    line_num,
                    path_str,
                    line_num,
                    line_num + search_size.saturating_sub(1)
                )
            } else {
                format!(
                    "❌ Pattern NOT FOUND in file\n\n\
                    💡 NEXT: Use readFile('{}') to see full content",
                    path_str
                )
            };

            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                "Pattern not found in preview",
                vec![suggestion],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        if occurrences > 1 {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                format!("Pattern found {} times (not unique)", occurrences),
                vec![
                    "Include more surrounding context to make the pattern unique".to_string(),
                    format!("Use readFile('{}') to see full content", path_str),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        // Exactly 1 match - generate context preview
        let preview_diff =
            self.generate_replacement_context(&original_content, old_string, new_string);

        let output = format!(
            "**🔍 Preview Replacement**\n\n\
            **File:** `{}`\n\
            **Status:** ✅ EXACT MATCH FOUND\n\n\
            **Changes Preview:**\n\
            ```diff\n\
            {}\n\
            ```\n\n\
            **Next Steps:**\n\
            - ✅ Preview looks correct? Call replaceStringInFile with SAME parameters\n\
            - 📖 Use readFile to see full file context",
            path_str, preview_diff
        );

        Ok(MCPResult::success_with_data(
            &output,
            json!({
                "path": path_str,
                "occurrences": 1,
                "status": "ready"
            }),
        ))
    }

    /// Generate contextual diff preview (shows surrounding lines)
    fn generate_replacement_context(
        &self,
        content: &str,
        old_string: &str,
        new_string: &str,
    ) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let search_lines: Vec<&str> = old_string.lines().collect();

        // Find the match location
        for (line_idx, window) in lines.windows(search_lines.len()).enumerate() {
            if window.join("\n") == old_string {
                // Show context: 2 lines before, matched section, 2 lines after
                let context_start = line_idx.saturating_sub(2);
                let context_end = (line_idx + search_lines.len() + 2).min(lines.len());

                let mut diff_lines = Vec::new();
                diff_lines.push(format!(
                    "@@ Lines {}-{} (showing context) @@",
                    line_idx + 1,
                    line_idx + search_lines.len()
                ));

                for (i, line) in lines[context_start..context_end].iter().enumerate() {
                    let absolute_line = context_start + i + 1;
                    let relative_to_match = (context_start + i) as isize - line_idx as isize;

                    if relative_to_match < 0 || relative_to_match >= search_lines.len() as isize {
                        // Context lines (unchanged)
                        diff_lines.push(format!("  {:4} | {}", absolute_line, line));
                    } else {
                        // Matched lines (will be replaced)
                        diff_lines.push(format!("- {:4} | {}", absolute_line, line));
                    }
                }

                // Show new content
                for (i, new_line) in new_string.lines().enumerate() {
                    let target_line = line_idx + i + 1;
                    diff_lines.push(format!("+ {:4} | {}", target_line, new_line));
                }

                return diff_lines.join("\n");
            }
        }

        "ERROR: Match location not found (should not happen)".to_string()
    }

    pub async fn handle_replace_string_in_file(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // Layer 1: Parameter existence validation
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) if !path.trim().is_empty() => path.trim(),
            Some(_) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Parameter 'path' cannot be empty",
                    vec![
                        "Provide a valid file path: replaceStringInFile({path, oldString, newString})"
                            .to_string(),
                        "Use listDirectory('.') to find files".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };

        // Get oldString parameter
        let old_string = match args.get("oldString").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s,
            Some(_) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Parameter 'oldString' cannot be empty",
                    vec![
                        "⚠️ CRITICAL: Call readFile FIRST to get exact content".to_string(),
                        "Extract text exactly as shown in readFile response".to_string(),
                        "Include surrounding context (3-5 lines) for uniqueness".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
            None => return Ok(missing_param_error("oldString", ToolGroup::Workspace)),
        };

        // Get newString parameter (can be empty for deletion)
        let new_string = match args.get("newString").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return Ok(missing_param_error("newString", ToolGroup::Workspace)),
        };

        // Layer 2: Business logic - path validation and file reading
        let safe_path = self.validate_path_with_error(path_str, session_id.clone())?;

        let original_content = match self.read_file_as_string(&safe_path).await {
            Ok(content) => content,
            Err(e) => {
                return Ok(operation_failed_error(
                    "Read file for replacement",
                    &e,
                    vec![
                        "Verify the file exists with listDirectory".to_string(),
                        "Check file permissions".to_string(),
                        "Use readFile to see the current content".to_string(),
                    ],
                    ToolGroup::Workspace,
                ));
            }
        };

        // Count occurrences
        let occurrences = original_content.matches(old_string).count();

        if occurrences == 0 {
            // Calculate similarity for suggestions
            let lines: Vec<&str> = original_content.lines().collect();
            let old_lines: Vec<&str> = old_string.lines().collect();
            let search_size = old_lines.len();

            let mut best_match: Option<(usize, f32)> = None; // (line_num, similarity)

            // Search for similar content
            for (idx, window) in lines.windows(search_size.max(1)).enumerate() {
                let window_text = window.join("\n");
                let similarity = self.calculate_similarity(&window_text, old_string);

                if similarity > 0.3 {
                    // 30% threshold
                    if best_match.is_none() || similarity > best_match.unwrap().1 {
                        best_match = Some((idx + 1, similarity));
                    }
                }
            }

            let suggestion = if let Some((line_num, similarity)) = best_match {
                format!(
                    "Similar content found at line {} ({}% match).

⚠️ MANDATORY STEPS:
1. Call readFile('{}', {}, {}) to see the ACTUAL content
2. Extract the exact text from readFile response (including whitespace)
3. Use the extracted text as oldString in your next attempt

💡 RECOMMENDED: Use previewReplacement BEFORE replaceStringInFile
   → previewReplacement(path, oldString, newString) shows exact diffs
   → Catches mismatches early and shows line numbers

❌ DO NOT retry with the same oldString
❌ DO NOT reconstruct the text from previous attempts",
                    line_num,
                    (similarity * 100.0) as u32,
                    path_str,
                    line_num,
                    line_num + search_size.saturating_sub(1)
                )
            } else {
                format!(
                    "Pattern not found in file.

⚠️ MANDATORY STEPS:
1. Call readFile('{}') to see current file content
2. Extract the exact text you want to replace from readFile response
3. Use the extracted text as oldString (must match EXACTLY including whitespace)

💡 RECOMMENDED: Use previewReplacement BEFORE replaceStringInFile
   → previewReplacement(path, oldString, newString) verifies without modification
   → Shows exact line numbers and context for better accuracy

❌ DO NOT retry without reading the file first
❌ DO NOT use oldString reconstructed from previous attempts or assumptions",
                    path_str
                )
            };

            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                "Pattern not found",
                vec![suggestion],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        if occurrences > 1 {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                format!("Pattern found {} times (must be unique)", occurrences),
                vec![
                    "Include more surrounding context (5-10 lines) to make the pattern unique"
                        .to_string(),
                    format!("Use readFile('{}') to see the full content", path_str),
                    "Use previewReplacement to verify before actual replacement".to_string(),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        // Perform replacement (exactly one match)
        let new_content = original_content.replacen(old_string, new_string, 1);

        // Write the modified content
        let file_manager = self.get_file_manager(session_id);
        match file_manager.write_file_string(path_str, &new_content).await {
            Ok(_) => {
                // Invalidate service context cache
                self.invalidate_context_cache().await;

                // Generate diff output
                let diff_output = self.format_string_diff(
                    &[(old_string.to_string(), new_string.to_string())],
                    path_str,
                );

                let message = format!(
                    "**✅ String Replacement Successful**\n\n\
                    **File:** `{}`\n\n\
                    {}\n\n\
                    **Next Steps:**\n\
                    - Use readFile to verify the changes\n\
                    - For multiple changes, call replaceStringInFile again\n\
                    - Each replacement is atomic and independent",
                    path_str, diff_output
                );

                Ok(MCPResult::success_with_data(
                    &message,
                    json!({
                        "path": path_str,
                        "old_string_length": old_string.len(),
                        "new_string_length": new_string.len(),
                        "diff": diff_output,
                    }),
                ))
            }
            Err(e) if e.contains("Permission denied") => Ok(ErrorGuidance::with_guidance(
                ErrorCategory::PermissionDenied,
                format!("Permission denied writing to '{}'", path_str),
                vec![
                    "File may be read-only or locked by another process".to_string(),
                    "Use listDirectory to check file permissions".to_string(),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result()),
            Err(e) => Ok(operation_failed_error(
                "Write file",
                &e,
                vec![
                    "File may be locked or inaccessible".to_string(),
                    format!("Use readFile('{}') to verify file still exists", path_str),
                ],
                ToolGroup::Workspace,
            )),
        }
    }

    // Helper: Read file as string
    async fn read_file_as_string(&self, path: &std::path::Path) -> Result<String, String> {
        tokio::fs::read_to_string(path)
            .await
            .map_err(|e| e.to_string())
    }

    // Helper: Calculate text similarity (Levenshtein-based)
    fn calculate_similarity(&self, text1: &str, text2: &str) -> f32 {
        let len1 = text1.len();
        let len2 = text2.len();

        if len1 == 0 && len2 == 0 {
            return 1.0;
        }
        if len1 == 0 || len2 == 0 {
            return 0.0;
        }

        // Simplified similarity: count matching characters
        let matching_chars = text1
            .chars()
            .zip(text2.chars())
            .filter(|(a, b)| a == b)
            .count();

        matching_chars as f32 / len1.max(len2) as f32
    }

    // Helper: Format full file diff output (Git-style unified diff)
    fn format_file_diff(&self, old_content: &str, new_content: &str, _file_path: &str) -> String {
        let old_lines: Vec<&str> = old_content.lines().collect();
        let new_lines: Vec<&str> = new_content.lines().collect();

        let added = new_lines.len().saturating_sub(old_lines.len());
        let removed = old_lines.len().saturating_sub(new_lines.len());

        let mut diff_lines = Vec::new();

        diff_lines.push(format!(
            "**Changes:** {} line(s) added, {} line(s) removed\n",
            added, removed
        ));
        diff_lines.push("**Diff:**".to_string());
        diff_lines.push("```diff".to_string());
        diff_lines.push(format!(
            "@@ -{},{} +{},{} @@",
            1,
            old_lines.len(),
            1,
            new_lines.len()
        ));

        // Show removed lines (limited to first 50 for display)
        let max_diff_lines = 50;
        let mut shown_lines = 0;

        for (idx, line) in old_lines.iter().enumerate() {
            if shown_lines >= max_diff_lines {
                diff_lines.push(format!(
                    "... ({} more old lines not shown)",
                    old_lines.len() - idx
                ));
                break;
            }
            diff_lines.push(format!("- {}", line));
            shown_lines += 1;
        }

        shown_lines = 0;
        // Show added lines (limited to first 50 for display)
        for (idx, line) in new_lines.iter().enumerate() {
            if shown_lines >= max_diff_lines {
                diff_lines.push(format!(
                    "... ({} more new lines not shown)",
                    new_lines.len() - idx
                ));
                break;
            }
            diff_lines.push(format!("+ {}", line));
            shown_lines += 1;
        }

        diff_lines.push("```".to_string());

        diff_lines.join("\n")
    }

    // Helper: Format diff output (Git-style)
    fn format_string_diff(&self, replacements: &[(String, String)], file_path: &str) -> String {
        let language = detect_language(std::path::Path::new(file_path));
        let mut diff_lines = Vec::new();

        diff_lines.push("**Changes Made:**\n".to_string());
        diff_lines.push("```diff".to_string());

        for (idx, (old_str, new_str)) in replacements.iter().enumerate() {
            let old_lines: Vec<&str> = old_str.lines().collect();
            let new_lines: Vec<&str> = new_str.lines().collect();

            if idx > 0 {
                diff_lines.push(String::new()); // Separator between replacements
            }

            diff_lines.push(format!(
                "@@ Replacement #{}: {} line(s) → {} line(s) @@",
                idx + 1,
                old_lines.len(),
                new_lines.len()
            ));

            // Show removed lines
            for line in old_lines {
                diff_lines.push(format!("- {}", line));
            }

            // Show added lines
            for line in new_lines {
                diff_lines.push(format!("+ {}", line));
            }
        }

        diff_lines.push("```".to_string());

        if !language.is_empty() {
            diff_lines.push(format!("\n*Language: {}*", language));
        }

        diff_lines.join("\n")
    }

    pub async fn handle_grep(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return Ok(missing_param_error("pattern", ToolGroup::Workspace)),
        };

        let ignore_case = args
            .get("ignoreCase")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let line_numbers = args
            .get("lineNumbers")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let input_text = if let Some(path_str) = args.get("path").and_then(|v| v.as_str()) {
            let file_manager = self.get_file_manager(session_id);
            match file_manager
                .get_security_validator()
                .validate_path_for_read(path_str)  // Use validate_path_for_read for read operations
            {
                Ok(safe_path) => match tokio::fs::read_to_string(safe_path).await {
                    Ok(s) => s,
                    Err(e) => {
                        return Ok(operation_failed_error(
                            "Read file for grep",
                            &e.to_string(),
                            vec![
                                "Verify the file exists with listDirectory".to_string(),
                                "Check file permissions".to_string(),
                                "Ensure the path is correct".to_string(),
                            ],
                            ToolGroup::Workspace,
                        ));
                    }
                },
                Err(e) => {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::PermissionDenied,
                        format!("Path validation failed: {}", e),
                        vec![
                            "Verify the file path is within allowed directories".to_string(),
                            "Use listDirectory to see available files".to_string(),
                            "Check that the path doesn't contain '..' or absolute paths outside workspace".to_string(),
                        ],
                        ToolGroup::Workspace,
                    ).to_mcp_result());
                }
            }
        } else if let Some(s) = args.get("input").and_then(|v| v.as_str()) {
            s.to_string()
        } else {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::MissingRequiredParam,
                "Either 'path' or 'input' parameter must be provided".to_string(),
                vec![
                    "Use 'path' to search within a file".to_string(),
                    "Use 'input' to search within provided text".to_string(),
                    "Example: {\"pattern\": \"error\", \"path\": \"logs.txt\"}".to_string(),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        };

        let regex = match regex::RegexBuilder::new(pattern)
            .case_insensitive(ignore_case)
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    format!("Invalid regex pattern: {}", e),
                    vec![
                        "Check regex syntax - use basic patterns like 'error|warning'".to_string(),
                        "Escape special characters with backslash: \\. \\* \\+ \\?".to_string(),
                        "Test pattern with a simpler string first".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        };

        let mut matches = Vec::new();
        let lines: Vec<&str> = input_text.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            if regex.is_match(line) {
                if line_numbers {
                    matches.push(json!({ "line": idx + 1, "text": line }));
                } else {
                    matches.push(json!(line));
                }
            }
        }

        let file_path = args.get("path").and_then(|v| v.as_str());
        let language = file_path
            .map(|p| detect_language(std::path::Path::new(p)))
            .unwrap_or("");

        let text_output = if matches.is_empty() {
            format!(
                "**🔍 Grep Results: No matches found**\n\n\
                Pattern: `{}`\n\
                Options: {}\n\n\
                **Next Steps:**\n\
                - Try a different search pattern\n\
                - Use ignoreCase: true for case-insensitive search\n\
                - Check if the file contains the expected content with readFile",
                pattern,
                if ignore_case {
                    "case-insensitive"
                } else {
                    "case-sensitive"
                }
            )
        } else {
            let mut s = format!("**🔍 Grep Results: {} match(es) found**\n\n", matches.len());

            if let Some(path) = file_path {
                s.push_str(&format!("File: `{}`\n", path));
            }
            s.push_str(&format!("Pattern: `{}`\n", pattern));
            s.push_str(&format!(
                "Options: {}\n\n",
                if ignore_case {
                    "case-insensitive"
                } else {
                    "case-sensitive"
                }
            ));

            // Show up to 20 matches with context
            let matches_to_show = matches.len().min(20);
            s.push_str("```");
            if !language.is_empty() {
                s.push_str(language);
            }
            s.push('\n');

            for match_item in matches.iter().take(matches_to_show) {
                if let Some(obj) = match_item.as_object() {
                    if let Some(line_num) = obj.get("line").and_then(|v| v.as_u64()) {
                        let line_idx = (line_num as usize).saturating_sub(1);

                        // Show context: ±2 lines
                        let context_start = line_idx.saturating_sub(2);
                        let context_end = (line_idx + 3).min(lines.len());

                        for (i, line) in lines
                            .iter()
                            .enumerate()
                            .skip(context_start)
                            .take(context_end - context_start)
                        {
                            let prefix = if i == line_idx { ">" } else { " " };
                            let line_number = format!("{:4}", i + 1);
                            s.push_str(&format!("{} {} | {}\n", prefix, line_number, line));
                        }
                        s.push('\n');
                    } else if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                        s.push_str(&format!("{}\n", text));
                    }
                } else if let Some(str_val) = match_item.as_str() {
                    s.push_str(&format!("{}\n", str_val));
                }
            }

            s.push_str("```\n\n");

            if matches.len() > 20 {
                s.push_str(&format!(
                    "*Showing first 20 of {} total matches*\n\n",
                    matches.len()
                ));
            }

            s.push_str(
                "**Next Steps:**\n\
                - Use readFile to see full file context\n\
                - Use replaceStringInFile to modify matched content\n\
                - Refine search pattern for more specific results",
            );

            s
        };

        Ok(MCPResult::success_with_data(
            &text_output,
            json!({ "matches": matches }),
        ))
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
                    "To import directory contents, use shell copy commands instead".to_string(),
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
                        "Use writeFile to modify the imported file".to_string(),
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
                            "Use writeFile to overwrite the existing file".to_string(),
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

        let result = WorkspaceServer::format_lines_with_numbers(&lines, true);

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

        let result = WorkspaceServer::format_lines_with_numbers(&lines, true);

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

        let result = WorkspaceServer::format_lines_with_numbers(&lines, true);

        assert!(result.contains("[File Content"));
        assert!(result.contains("NOT part of the code"));
        assert!(result.contains("   1 | int main() {}"));
        assert!(result.contains("   2 | "));
    }
}
