use super::super::WorkspaceServer;
use super::utils::{detect_language, format_file_size};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, not_found_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};
use tokio::fs;
use tracing::{error, info};

impl WorkspaceServer {
    pub async fn handle_list_directory(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");

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
                    "Verify the directory path is correct".to_string(),
                    "Use listDirectory to see available files".to_string(),
                    "Ensure you have read permissions for the directory".to_string(),
                ])
                .to_mcp_result());
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
                    Ok(guided_error(
                        ErrorCategory::OperationFailed,
                        e.to_string(),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Verify the directory exists".to_string(),
                        "Check directory permissions".to_string(),
                        "Try using '.' to list the current directory".to_string(),
                    ])
                    .to_mcp_result())
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

        let file_manager = self.get_file_manager(session_id.clone());
        let safe_path = match file_manager
            .get_security_validator()
            .validate_path_for_read(search_path)
        {
            Ok(path) => path,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::PermissionDenied,
                    format!("Path validation failed: {}", e),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Verify the directory path is correct".to_string(),
                    "Use listDirectory to see available files".to_string(),
                ])
                .to_mcp_result());
            }
        };

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
                Ok(guided_error(
                    ErrorCategory::OperationFailed,
                    e.to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Verify the pattern syntax is correct (use glob format like '*.txt' or '**/*.rs')".to_string(),
                    "Check if the directory path exists with listDirectory".to_string(),
                    "Try a simpler pattern to narrow down the issue".to_string(),
                ])
                .to_mcp_result())
            }
        }
    }

    pub async fn handle_search_line_in_file(
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
            .unwrap_or(true); // Default to true as per tool definition

        let input_text = if let Some(path_str) = args.get("path").and_then(|v| v.as_str()) {
            let file_manager = self.get_file_manager(session_id);
            match file_manager
                .get_security_validator()
                .validate_path_for_read(path_str)  // Use validate_path_for_read for read operations
            {
                Ok(safe_path) => match tokio::fs::read_to_string(safe_path).await {
                    Ok(s) => s,
                    Err(e) => {
                        let error_msg = if e.kind() == std::io::ErrorKind::InvalidData {
                            "Failed to read file: Content appears to be binary or contains invalid UTF-8 characters. Please use a specialized tool for binary files.".to_string()
                        } else {
                            e.to_string()
                        };

                        return Ok(guided_error(
                            ErrorCategory::OperationFailed,
                            &error_msg,
                            ToolGroup::Workspace,
                        )
                        .guidance(vec![
                            "Verify the file exists with listDirectory".to_string(),
                            "Check file permissions".to_string(),
                            "Ensure the path is correct".to_string(),
                        ])
                        .to_mcp_result());
                    }
                },
                Err(e) => {
                    return Ok(guided_error(
                        ErrorCategory::PermissionDenied,
                        format!("Path validation failed: {}", e),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Verify the file path is within allowed directories".to_string(),
                        "Use listDirectory to see available files".to_string(),
                        "Check that the path doesn't contain '..' or absolute paths outside workspace".to_string(),
                    ])
                    .to_mcp_result());
                }
            }
        } else if let Some(s) = args.get("input").and_then(|v| v.as_str()) {
            s.to_string()
        } else {
            return Ok(guided_error(
                ErrorCategory::MissingRequiredParam,
                "Either 'path' or 'input' parameter must be provided".to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use 'path' to search within a file".to_string(),
                "Use 'input' to search within provided text".to_string(),
                "Example: {\"pattern\": \"error\", \"path\": \"logs.txt\"}".to_string(),
            ])
            .to_mcp_result());
        };

        let regex = match regex::RegexBuilder::new(pattern)
            .case_insensitive(ignore_case)
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("Invalid regex pattern: {}", e),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Check regex syntax - use basic patterns like 'error|warning'".to_string(),
                    "Escape special characters with backslash: \\. \\* \\+ \\?".to_string(),
                    "Test pattern with a simpler string first".to_string(),
                ])
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
                "**🔍 Search Results: No matches found**\n\n\
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
            let mut s = format!(
                "**🔍 Search Results: {} match(es) found**\n\n",
                matches.len()
            );

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
                        let line_content = obj.get("text").and_then(|t| t.as_str()).unwrap_or("");

                        // ✅ ENHANCED: Explicitly format line number in text output
                        // Format: "Line 123: content"
                        s.push_str(&format!("Line {}: {}\n", line_num, line_content));
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
                - Use editFile to modify matched content\n\
                - Refine search pattern for more specific results",
            );

            s
        };

        Ok(MCPResult::success_with_data(
            &text_output,
            json!({ "matches": matches }),
        ))
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
}
