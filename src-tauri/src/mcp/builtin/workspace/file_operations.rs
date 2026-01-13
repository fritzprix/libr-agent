use super::WorkspaceServer;
use crate::mcp::builtin::error_guidance::{
    missing_param_error, not_found_error, operation_failed_error, permission_denied_error,
    ErrorCategory, ErrorGuidance, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::fs;
use tracing::{error, info};

// ✅ ENHANCED: Threshold for using spawn_blocking for CPU-intensive line enumeration
// Large files can block the async runtime during line enumeration
const LARGE_FILE_THRESHOLD: u64 = 1_048_576; // 1 MB in bytes

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
        let content = if start_line.is_some() || end_line.is_some() {
            if let Err(e) = file_manager
                .get_security_validator()
                .validate_file_size(&safe_path, crate::config::max_file_size())
            {
                error!("File size validation failed: {}", e);
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    format!("File size error: {}", e),
                    vec![
                        "The file is too large to read with line ranges".to_string(),
                        "Try reading the entire file without startLine/endLine".to_string(),
                        "Use grep to find specific content instead".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }

            self.read_file_lines_range(&safe_path, start_line, end_line, show_line_numbers)
                .await
        } else {
            // Read full file and format with line numbers
            let raw_content = file_manager
                .read_file_as_string(path_str)
                .await
                .map_err(|e| e.to_string())?;

            let lines_with_numbers: Vec<(usize, String)> = raw_content
                .lines()
                .enumerate()
                .map(|(idx, line)| (idx + 1, line.to_string()))
                .collect();

            Ok(Self::format_lines_with_numbers(
                &lines_with_numbers,
                show_line_numbers,
            ))
        };

        match content {
            Ok(content) => {
                info!("Successfully read file: {}", path_str);

                // Include actual content in text for AI agent visibility
                let text_message = format!(
                    "File read successfully: {}\n\nContent:\n{}\n\n💡 Next: Use writeFile to modify or replaceStringInFile to make targeted edits",
                    path_str,
                    content
                );

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
                let action = if mode == "a" { "appended" } else { "written" };
                let hint = SuccessHint::new(
                    format!(
                        "Successfully {} {} bytes to {} (mode: {})",
                        action,
                        content.len(),
                        path_str,
                        mode
                    ),
                    SuccessHint::for_tool("writeFile", ToolGroup::Workspace),
                );
                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "path": path_str,
                    "bytes_written": content.len(),
                    "mode": mode
                }))))
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
                    format!("No files found matching pattern '{pattern}' in '{search_path}'")
                } else {
                    let mut text = format!(
                        "Found {} files matching pattern '{}':\n",
                        results.len(),
                        pattern
                    );
                    for item in results.iter().take(50) {
                        let path = item.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                        let type_ = item.get("type").and_then(|v| v.as_str()).unwrap_or("?");
                        text.push_str(&format!("- [{}] {}\n", type_, path));
                    }
                    if results.len() > 50 {
                        text.push_str(&format!("... and {} more items.", results.len() - 50));
                    }
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
                        "Provide a valid file path: replaceStringInFile('src/file.rs', ...)"
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

        let replacements_val = match args.get("replacements") {
            Some(val) if val.is_array() => val,
            Some(_) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "'replacements' must be an array",
                    vec![
                        "Format: {\"replacements\": [{\"oldString\": \"...\", \"newString\": \"...\"}]}".to_string(),
                        "Use readFile first to see exact content to replace".to_string(),
                    ],
                    ToolGroup::Workspace,
                ).to_mcp_result());
            }
            None => return Ok(missing_param_error("replacements", ToolGroup::Workspace)),
        };

        // Empty replacements check
        if replacements_val.as_array().unwrap().is_empty() {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                "Replacements array is empty",
                vec![
                    "Provide at least one replacement: {\"oldString\": \"...\", \"newString\": \"...\"}".to_string(),
                    "Use readFile to identify content to replace".to_string(),
                ],
                ToolGroup::Workspace,
            ).to_mcp_result());
        }

        // Layer 2: Format validation
        let replacements: Vec<HashMap<String, Value>> =
            match serde_json::from_value(replacements_val.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::InvalidFormat,
                        format!("Invalid replacements format: {}", e),
                        vec![
                            "Replacements must be an array of objects".to_string(),
                            "Each object needs oldString and newString".to_string(),
                            "Example: [{\"oldString\": \"old text\", \"newString\": \"new text\"}]"
                                .to_string(),
                        ],
                        ToolGroup::Workspace,
                    )
                    .to_mcp_result());
                }
            };

        // Layer 3: Business logic - path validation and file reading
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

        let mut new_content = original_content.clone();
        let mut successful_replacements = Vec::new();
        let mut failed_replacements = Vec::new();

        // Process each replacement
        for (idx, rep) in replacements.iter().enumerate() {
            let old_string = match rep.get("oldString").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    failed_replacements.push(format!(
                        "Replacement #{}: missing 'oldString' parameter",
                        idx + 1
                    ));
                    continue;
                }
            };

            let new_string = match rep.get("newString").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    failed_replacements.push(format!(
                        "Replacement #{}: missing 'newString' parameter",
                        idx + 1
                    ));
                    continue;
                }
            };

            // Count occurrences
            let occurrences = new_content.matches(old_string).count();

            if occurrences == 0 {
                // Calculate similarity for suggestions
                let lines: Vec<&str> = new_content.lines().collect();
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
                        "Similar content found at line {} ({}% match). Use readFile('{}', {}, {}) to verify",
                        line_num,
                        (similarity * 100.0) as u32,
                        path_str,
                        line_num,
                        line_num + search_size.saturating_sub(1)
                    )
                } else {
                    "Use readFile to see current content and verify the string exists".to_string()
                };

                failed_replacements.push(format!(
                    "Replacement #{}: pattern not found. {}",
                    idx + 1,
                    suggestion
                ));
                continue;
            }

            if occurrences > 1 {
                failed_replacements.push(format!(
                    "Replacement #{}: pattern found {} times. Pattern must be unique. Include more context to make it unique.",
                    idx + 1, occurrences
                ));
                continue;
            }

            // Perform replacement (exactly one match)
            new_content = new_content.replacen(old_string, new_string, 1);
            successful_replacements.push((old_string.to_string(), new_string.to_string()));
        }

        // Check if any replacements failed
        if !failed_replacements.is_empty() && successful_replacements.is_empty() {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                format!("All {} replacement(s) failed", failed_replacements.len()),
                failed_replacements.iter().take(3).cloned().collect(),
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        // Write the modified content
        let file_manager = self.get_file_manager(session_id);
        match file_manager.write_file_string(path_str, &new_content).await {
            Ok(_) => {
                // Generate diff output
                let diff_output = self.format_string_diff(&successful_replacements);

                let summary = if failed_replacements.is_empty() {
                    format!(
                        "✓ Successfully replaced {} pattern(s) in '{}'",
                        successful_replacements.len(),
                        path_str
                    )
                } else {
                    format!(
                        "⚠ Partially successful: {} of {} replacement(s) succeeded in '{}'\n\nFailed:\n{}",
                        successful_replacements.len(),
                        replacements.len(),
                        path_str,
                        failed_replacements.join("\n")
                    )
                };

                let hint = SuccessHint::new(
                    format!("{}\n\n{}", summary, diff_output),
                    vec![
                        format!("Use readFile('{}') to verify all changes", path_str),
                        "Use grep(path, pattern) to search for specific content".to_string(),
                    ],
                );

                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "path": path_str,
                    "successful_replacements": successful_replacements.len(),
                    "failed_replacements": failed_replacements.len(),
                    "diff": diff_output,
                }))))
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

    // Helper: Format diff output (Git-style)
    fn format_string_diff(&self, replacements: &[(String, String)]) -> String {
        let mut diff_lines = vec!["=== Changes Made ===\n".to_string()];

        for (old_str, new_str) in replacements {
            // Find context around change
            let old_lines: Vec<&str> = old_str.lines().collect();
            let new_lines: Vec<&str> = new_str.lines().collect();

            diff_lines.push(format!(
                "@@ Replaced {} line(s) with {} line(s) @@",
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

            diff_lines.push(String::new()); // Blank line
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
        for (idx, line) in input_text.lines().enumerate() {
            if regex.is_match(line) {
                if line_numbers {
                    matches.push(json!({ "line": idx + 1, "text": line }));
                } else {
                    matches.push(json!(line));
                }
            }
        }

        let text_output = if matches.is_empty() {
            "No matches found".to_string()
        } else {
            let mut s = format!("Found {} matches:\n", matches.len());
            for match_item in matches.iter().take(20) {
                if let Some(obj) = match_item.as_object() {
                    if let Some(line_num) = obj.get("line") {
                        s.push_str(&format!(
                            "Line {}: {}\n",
                            line_num,
                            obj.get("text").and_then(|t| t.as_str()).unwrap_or("")
                        ));
                    } else {
                        s.push_str(&format!("{}\n", match_item.as_str().unwrap_or("")));
                    }
                } else if let Some(str_val) = match_item.as_str() {
                    s.push_str(&format!("{}\n", str_val));
                }
            }
            if matches.len() > 20 {
                s.push_str(&format!("... and {} more matches.", matches.len() - 20));
            }
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
