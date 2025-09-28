use super::WorkspaceServer;
use crate::mcp::MCPResponse;
use regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::fs;
use tracing::{error, info};

impl WorkspaceServer {
    fn validate_path_with_error(
        &self,
        path_str: &str,
        request_id: &Value,
    ) -> Result<std::path::PathBuf, Box<MCPResponse>> {
        let file_manager = self.get_file_manager();
        match file_manager
            .get_security_validator()
            .validate_path(path_str)
        {
            Ok(path) => Ok(path),
            Err(e) => {
                error!("Path validation failed: {}", e);
                Err(Box::new(Self::error_response(
                    request_id.clone(),
                    -32603,
                    &format!("Security error: {e}"),
                )))
            }
        }
    }

    pub async fn handle_read_file(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) => path,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: path",
                );
            }
        };

        let start_line = args
            .get("start_line")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let end_line = args
            .get("end_line")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);

        if let (Some(start), Some(end)) = (start_line, end_line) {
            if start > end {
                return Self::error_response(
                    request_id,
                    -32602,
                    "start_line must be less than or equal to end_line",
                );
            }
        }

        let safe_path = match self.validate_path_with_error(path_str, &request_id) {
            Ok(path) => path,
            Err(error_response) => return *error_response,
        };

        let file_manager = self.get_file_manager();
        let content = if start_line.is_some() || end_line.is_some() {
            if let Err(e) = file_manager
                .get_security_validator()
                .validate_file_size(&safe_path, super::utils::constants::MAX_FILE_SIZE)
            {
                error!("File size validation failed: {}", e);
                return Self::error_response(request_id, -32603, &format!("File size error: {e}"));
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
                Self::success_response(request_id, &content)
            }
            Err(e) => {
                error!("Failed to read file {}: {}", path_str, e);
                Self::error_response(request_id, -32603, &format!("Failed to read file: {e}"))
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

    pub async fn handle_write_file(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) => path,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: path",
                );
            }
        };

        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(content) => content,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: content",
                );
            }
        };

        let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("w");

        let file_manager = self.get_file_manager();
        let result = match mode {
            "w" => file_manager.write_file_string(path_str, content).await,
            "a" => file_manager.append_file_string(path_str, content).await,
            _ => {
                return Self::error_response(request_id, -32602, "Invalid mode. Use 'w' or 'a'");
            }
        };

        match result {
            Ok(()) => {
                info!("Successfully wrote file: {}", path_str);
                Self::success_response(
                    request_id,
                    &format!(
                        "Successfully wrote {} bytes to {} (mode: {})",
                        content.len(),
                        path_str,
                        mode
                    ),
                )
            }
            Err(e) => {
                error!("Failed to write file {}: {}", path_str, e);
                Self::error_response(request_id, -32603, &format!("Failed to write file: {e}"))
            }
        }
    }

    pub async fn handle_list_directory(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let safe_path = match self.validate_path_with_error(path_str, &request_id) {
            Ok(path) => path,
            Err(error_response) => return *error_response,
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

                info!(
                    "Successfully listed directory: {:?} ({} items)",
                    safe_path,
                    items.len()
                );
                Self::success_response(
                    request_id,
                    &format!(
                        "Directory listing for {}:\n{}",
                        path_str,
                        serde_json::to_string_pretty(&items).unwrap_or_default()
                    ),
                )
            }
            Err(e) => {
                error!("Failed to list directory {:?}: {}", safe_path, e);
                Self::error_response(
                    request_id,
                    -32603,
                    &format!("Failed to list directory: {e}"),
                )
            }
        }
    }

    pub async fn handle_search_files(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(pattern) => pattern,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: pattern",
                );
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

        let safe_path = match self.validate_path_with_error(search_path, &request_id) {
            Ok(path) => path,
            Err(error_response) => return *error_response,
        };

        match self
            .search_files_by_pattern(&safe_path, pattern, max_depth, file_type)
            .await
        {
            Ok(results) => {
                let result_text = if results.is_empty() {
                    format!("No files found matching pattern '{pattern}' in '{search_path}'")
                } else {
                    format!(
                        "Found {} files matching pattern '{}':\n{}",
                        results.len(),
                        pattern,
                        serde_json::to_string_pretty(&results).unwrap_or_default()
                    )
                };

                Self::success_response(request_id, &result_text)
            }
            Err(e) => {
                error!("File search failed: {}", e);
                Self::error_response(request_id, -32603, &format!("Search failed: {e}"))
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

    pub async fn handle_replace_lines_in_file(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) => path,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: path",
                );
            }
        };

        let replacements_val = match args.get("replacements") {
            Some(val) => val,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: replacements",
                );
            }
        };

        let replacements: Vec<HashMap<String, Value>> =
            match serde_json::from_value(replacements_val.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return Self::error_response(
                        request_id,
                        -32602,
                        &format!("Invalid replacements format: {e}"),
                    );
                }
            };

        let safe_path = match self.validate_path_with_error(path_str, &request_id) {
            Ok(path) => path,
            Err(error_response) => return *error_response,
        };

        let lines = match self.read_file_lines(&safe_path).await {
            Ok(lines) => lines,
            Err(e) => {
                return Self::error_response(
                    request_id,
                    -32603,
                    &format!("Failed to read file: {e}"),
                );
            }
        };

        let mut new_lines = lines.clone();
        let mut replacements_map: HashMap<String, String> = HashMap::new();

        for rep in replacements {
            let start_line = match rep.get("start_line").and_then(|v| v.as_u64()) {
                Some(num) => num as usize,
                None => match rep.get("line_number").and_then(|v| v.as_u64()) {
                    Some(num) => num as usize,
                    None => {
                        return Self::error_response(
                            request_id,
                            -32602,
                            "Missing start_line or line_number",
                        );
                    }
                },
            };

            let end_line = rep
                .get("end_line")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize)
                .unwrap_or(start_line);

            if start_line > end_line {
                return Self::error_response(request_id, -32602, "start_line must be <= end_line");
            }

            if start_line == 0 || end_line > new_lines.len() {
                return Self::error_response(
                    request_id,
                    -32602,
                    &format!(
                        "Line range {}-{} is out of bounds (file has {} lines)",
                        start_line,
                        end_line,
                        new_lines.len()
                    ),
                );
            }

            let content = match rep.get("new_content") {
                Some(Value::String(s)) => s.to_string(), // Handle string values including empty strings
                Some(Value::Null) => String::new(), // Handle explicit null as empty string for deletion
                Some(_) => {
                    return Self::error_response(request_id, -32602, "new_content must be a string");
                }
                None => String::new(), // Missing new_content means delete lines
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
            } else {
                if content.is_empty() {
                    // Delete line range
                    new_lines.splice((start_line - 1)..end_line, vec![]);
                } else {
                    // Replace line range with single line
                    new_lines.splice((start_line - 1)..end_line, vec![content]);
                }
            }
        }

        let new_content = new_lines.join("\n");
        let file_manager = self.get_file_manager();
        match file_manager.write_file_string(path_str, &new_content).await {
            Ok(_) => Self::success_response(
                request_id,
                &format!("Successfully replaced lines in file {path_str}"),
            ),
            Err(e) => {
                Self::error_response(request_id, -32603, &format!("Failed to write file: {e}"))
            }
        }
    }

    pub async fn handle_grep(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return Self::error_response(request_id, -32602, "missing 'pattern' argument"),
        };

        let ignore_case = args
            .get("ignore_case")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let line_numbers = args
            .get("line_numbers")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let input_text = if let Some(path_str) = args.get("path").and_then(|v| v.as_str()) {
            let file_manager = self.get_file_manager();
            match file_manager
                .get_security_validator()
                .validate_path(path_str)
            {
                Ok(safe_path) => match tokio::fs::read_to_string(safe_path).await {
                    Ok(s) => s,
                    Err(e) => {
                        return Self::error_response(
                            request_id,
                            -32603,
                            &format!("failed to read file {path_str}: {e}"),
                        );
                    }
                },
                Err(e) => {
                    return Self::error_response(
                        request_id,
                        -32603,
                        &format!("Security error: {e}"),
                    );
                }
            }
        } else if let Some(s) = args.get("input").and_then(|v| v.as_str()) {
            s.to_string()
        } else {
            return Self::error_response(
                request_id,
                -32602,
                "either 'path' or 'input' must be provided",
            );
        };

        let regex = match regex::RegexBuilder::new(pattern)
            .case_insensitive(ignore_case)
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                return Self::error_response(request_id, -32602, &format!("invalid pattern: {e}"))
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

        Self::success_response(
            request_id,
            &format!(
                "Found {} matches:\n{}",
                matches.len(),
                serde_json::to_string_pretty(&matches).unwrap_or_default()
            ),
        )
    }

    pub async fn handle_import_file(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let src_path_str = match args.get("src_abs_path").and_then(|v| v.as_str()) {
            Some(path) => path,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: src_abs_path",
                );
            }
        };

        let dest_rel_path = match args.get("dest_rel_path").and_then(|v| v.as_str()) {
            Some(path) => path,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: dest_rel_path",
                );
            }
        };

        // Validate source path exists and is readable
        let src_path = match std::path::Path::new(src_path_str).canonicalize() {
            Ok(path) => path,
            Err(e) => {
                error!("Invalid source path {}: {}", src_path_str, e);
                return Self::error_response(
                    request_id,
                    -32603,
                    &format!("Invalid source path: {e}"),
                );
            }
        };

        // Ensure source is a file, not a directory
        if !src_path.is_file() {
            return Self::error_response(
                request_id,
                -32602,
                "Source path must be a file, not a directory",
            );
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

                Self::success_response(
                    request_id,
                    &format!(
                        "Successfully imported {} ({} bytes) to {}",
                        src_path.display(),
                        file_size,
                        dest_rel_path
                    ),
                )
            }
            Err(e) => {
                error!(
                    "Failed to import file from {} to {}: {}",
                    src_path.display(),
                    dest_rel_path,
                    e
                );
                Self::error_response(request_id, -32603, &format!("Failed to import file: {e}"))
            }
        }
    }
}
