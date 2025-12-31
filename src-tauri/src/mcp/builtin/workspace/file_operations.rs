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

#[allow(dead_code)]
impl WorkspaceServer {
    fn validate_path_with_error(&self, path_str: &str) -> Result<std::path::PathBuf, String> {
        let file_manager = self.get_file_manager();
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

    pub async fn handle_read_file(&self, args: Value) -> Result<MCPResult, String> {
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) => path,
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };

        let start_line = args
            .get("startLine")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let end_line = args
            .get("endLine")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);

        if let (Some(start), Some(end)) = (start_line, end_line) {
            if start > end {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    format!(
                        "start_line ({}) must be less than or equal to end_line ({})",
                        start, end
                    ),
                    vec![
                        "Swap the values: make startLine smaller".to_string(),
                        format!(
                            "Example: {{\"startLine\": {}, \"endLine\": {}}}",
                            end, start
                        ),
                        "Omit both parameters to read the entire file".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        }

        let safe_path = match self.validate_path_with_error(path_str) {
            Ok(path) => path,
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
        };

        let file_manager = self.get_file_manager();
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
                        "Use searchFiles to find specific content instead".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }

            self.read_file_lines_range(&safe_path, start_line, end_line)
                .await
        } else {
            file_manager
                .read_file_as_string(path_str)
                .await
                .map_err(|e| e.to_string())
        };

        match content {
            Ok(content) => {
                info!("Successfully read file: {}", path_str);

                // Include actual content in text for AI agent visibility
                let text_message = format!(
                    "File read successfully: {}\n\nContent:\n{}\n\n💡 Next: Use writeFile to modify or replaceLines to make targeted edits",
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
    ) -> Result<String, String> {
        use tokio::io::{AsyncBufReadExt, BufReader};

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
                result_lines.push(line);
            }

            if current_line > end {
                break;
            }

            current_line += 1;
        }

        Ok(result_lines.join("\n"))
    }

    pub async fn handle_write_file(&self, args: Value) -> Result<MCPResult, String> {
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

        let file_manager = self.get_file_manager();
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

    pub async fn handle_list_directory(&self, args: Value) -> Result<MCPResult, String> {
        let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let safe_path = match self.validate_path_with_error(path_str) {
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

                let item_lines: Vec<String> = items
                    .iter()
                    .map(|item| {
                        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                        let type_ = item.get("type").and_then(|v| v.as_str()).unwrap_or("?");
                        let size = item.get("size").and_then(|v| v.as_u64());

                        let prefix = if type_ == "directory" {
                            "[DIR]"
                        } else {
                            "[FILE]"
                        };
                        let size_str = if let Some(s) = size {
                            format!(" ({} bytes)", s)
                        } else {
                            "".to_string()
                        };

                        format!("{} {}{}", prefix, name, size_str)
                    })
                    .collect();

                let listing_str = item_lines.join("\n");

                info!(
                    "Successfully listed directory: {:?} ({} items)",
                    safe_path,
                    items.len()
                );
                let hint = SuccessHint::new(
                    format!(
                        "Directory listing for {} ({} items):\n{}",
                        path_str,
                        items.len(),
                        listing_str
                    ),
                    SuccessHint::for_tool("listDirectory", ToolGroup::Workspace),
                );
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

    pub async fn handle_search_files(&self, args: Value) -> Result<MCPResult, String> {
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

        let safe_path = self.validate_path_with_error(search_path)?;

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

    pub async fn handle_replace_lines_in_file(&self, args: Value) -> Result<MCPResult, String> {
        // Layer 1: Parameter existence validation
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) => path,
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };

        let replacements_val = match args.get("replacements") {
            Some(val) => val,
            None => {
                return Ok(missing_param_error("replacements", ToolGroup::Workspace));
            }
        };

        // Layer 2: Format validation
        let replacements: Vec<HashMap<String, Value>> = match serde_json::from_value(
            replacements_val.clone(),
        ) {
            Ok(r) => r,
            Err(e) => {
                return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::InvalidFormat,
                        format!("Invalid replacements format: {}", e),
                        vec![
                            "Replacements must be an array of objects".to_string(),
                            "Each object needs startLine/lineNumber and newContent".to_string(),
                            "Example: [{\"startLine\": 1, \"endLine\": 2, \"newContent\": \"new text\"}]".to_string(),
                        ],
                        ToolGroup::Workspace,
                    ).to_mcp_result());
            }
        };

        // Layer 3: Business logic - path validation and file reading
        let safe_path = self.validate_path_with_error(path_str)?;

        let lines = match self.read_file_lines(&safe_path).await {
            Ok(lines) => lines,
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

        let mut new_lines = lines.clone();
        let mut replacements_map: HashMap<String, String> = HashMap::new();

        // Layer 2 (continued): Validate each replacement object
        for rep in replacements {
            let start_line = match rep.get("startLine").and_then(|v| v.as_u64()) {
                Some(num) => num as usize,
                Option::None => match rep.get("lineNumber").and_then(|v| v.as_u64()) {
                    Some(num) => num as usize,
                    Option::None => {
                        return Ok(ErrorGuidance::with_guidance(
                            ErrorCategory::InvalidInput,
                            "Missing startLine or lineNumber in replacement".to_string(),
                            vec![
                                "Each replacement must have either 'startLine' or 'lineNumber'".to_string(),
                                "Use 'startLine' and optional 'endLine' for ranges".to_string(),
                                "Example: {\"startLine\": 5, \"endLine\": 7, \"newContent\": \"text\"}".to_string(),
                            ],
                            ToolGroup::Workspace,
                        ).to_mcp_result());
                    }
                },
            };

            let end_line = rep
                .get("endLine")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize)
                .unwrap_or(start_line);

            if start_line > end_line {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    format!(
                        "startLine ({}) must be <= endLine ({})",
                        start_line, end_line
                    ),
                    vec![
                        "Swap the values if you meant to specify a range".to_string(),
                        format!(
                            "Correct range: {{\"startLine\": {}, \"endLine\": {}}}",
                            end_line, start_line
                        ),
                        "Or use a single line replacement".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }

            if start_line == 0 || end_line > new_lines.len() {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Line range {}-{} is out of bounds (file has {} lines)",
                        start_line,
                        end_line,
                        new_lines.len()
                    ),
                    vec![
                        format!(
                            "File has {} lines, use line numbers 1-{}",
                            new_lines.len(),
                            new_lines.len()
                        ),
                        "Use readFile to see the file content and line count".to_string(),
                        "Line numbers start at 1, not 0".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }

            let content = match rep.get("newContent") {
                Some(Value::String(s)) => s.to_string(), // Handle string values including empty strings
                Some(Value::Null) => String::new(), // Handle explicit null as empty string for deletion
                Some(_) => {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::InvalidInput,
                        "newContent must be a string".to_string(),
                        vec![
                            "Use a string value for newContent".to_string(),
                            "Use empty string \"\" or null to delete lines".to_string(),
                            "Example: {\"startLine\": 1, \"newContent\": \"new line text\"}"
                                .to_string(),
                        ],
                        ToolGroup::Workspace,
                    )
                    .to_mcp_result());
                }
                None => String::new(), // Missing newContent means delete lines
            };

            let range_key = format!("{start_line}-{end_line}");
            replacements_map.insert(range_key, content);
        }

        for (range_key, content) in replacements_map {
            let parts: Vec<&str> = range_key.split('-').collect();
            let start_line: usize = parts[0].parse().unwrap();
            let end_line: usize = parts[1].parse().unwrap();

            if start_line == end_line {
                if content.is_empty() {
                    // Delete single line
                    new_lines.remove(start_line - 1);
                } else {
                    // Replace single line
                    new_lines[start_line - 1] = content;
                }
            } else if content.is_empty() {
                // Delete line range
                new_lines.splice((start_line - 1)..end_line, vec![]);
            } else {
                // Replace line range with single line
                new_lines.splice((start_line - 1)..end_line, vec![content]);
            }
        }

        // Layer 4: Apply replacements and write
        let new_content = new_lines.join("\n");
        let file_manager = self.get_file_manager();
        match file_manager.write_file_string(path_str, &new_content).await {
            Ok(_) => {
                let hint = SuccessHint::new(
                    format!("Successfully replaced lines in file {}", path_str),
                    SuccessHint::for_tool("replaceLinesInFile", ToolGroup::Workspace),
                );
                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "path": path_str,
                    "lines_count": new_lines.len()
                }))))
            }
            Err(e) => Ok(operation_failed_error(
                "Write file after replacement",
                &e.to_string(),
                vec![
                    "Check file permissions".to_string(),
                    "Verify the file is not locked by another process".to_string(),
                    "Ensure sufficient disk space".to_string(),
                ],
                ToolGroup::Workspace,
            )),
        }
    }

    pub async fn handle_grep(&self, args: Value) -> Result<MCPResult, String> {
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
            let file_manager = self.get_file_manager();
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

    pub async fn handle_import_file(&self, args: Value) -> Result<MCPResult, String> {
        let src_path_str = match args
            .get("srcAbsPath")
            .or_else(|| args.get("src_abs_path"))
            .and_then(|v| v.as_str())
        {
            Some(path) => path,
            None => {
                return Ok(MCPResult::error("Missing required parameter: srcAbsPath"));
            }
        };

        let dest_rel_path = match args
            .get("destRelPath")
            .or_else(|| args.get("dest_rel_path"))
            .and_then(|v| v.as_str())
        {
            Some(path) => path,
            None => {
                return Ok(MCPResult::error("Missing required parameter: destRelPath"));
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
                return Ok(MCPResult::error(&format!(
                    "Invalid source path: '{src_path_str}'. {e}. \
                     Please ensure the file exists and the path is correct. \
                     On Windows, use absolute paths like 'C:\\Users\\...'"
                )));
            }
        };

        // Ensure source is a file, not a directory
        if !src_path.is_file() {
            return Ok(MCPResult::error(
                "Source path must be a file, not a directory",
            ));
        }

        // Use file manager to handle destination path validation and copying
        let file_manager = self.get_file_manager();
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

                Ok(MCPResult::success(&format!(
                    "Successfully imported {} ({} bytes) to {}",
                    src_path.display(),
                    file_size,
                    dest_rel_path
                )))
            }
            Err(e) => {
                error!(
                    "Failed to import file from {} to {}: {}",
                    src_path.display(),
                    dest_rel_path,
                    e
                );
                Ok(MCPResult::error(&format!("Failed to import file: {e}")))
            }
        }
    }
}
