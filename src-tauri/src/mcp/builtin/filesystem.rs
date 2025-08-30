use async_trait::async_trait;
use regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::fs;
use tracing::{error, info};

use super::{utils::constants::MAX_FILE_SIZE, BuiltinMCPServer};
use crate::mcp::{utils::schema_builder::*, MCPError, MCPResponse, MCPTool};
use crate::services::SecureFileManager;

pub struct FilesystemServer {
    file_manager: std::sync::Arc<SecureFileManager>,
}

impl FilesystemServer {
    pub fn new(file_manager: std::sync::Arc<SecureFileManager>) -> Self {
        // 현재 작업 디렉터리 로그 (확인용)
        match std::env::current_dir() {
            Ok(dir) => info!("FilesystemServer starting with CWD = {:?}", dir),
            Err(e) => error!("Failed to read current_dir: {}", e),
        }

        // SecureFileManager의 base_dir 확인
        info!(
            "FilesystemServer using base_dir: {:?}",
            file_manager.base_dir()
        );

        Self { file_manager }
    }

    fn create_read_file_tool() -> MCPTool {
        let mut props = HashMap::new();
        props.insert("path".to_string(), string_prop(Some(1), Some(1000), Some("Path to the file to read")));
        props.insert("start_line".to_string(), integer_prop(Some(1), None, Some("Starting line number (1-based, optional)")));
        props.insert("end_line".to_string(), integer_prop(Some(1), None, Some("Ending line number (1-based, optional)")));

        MCPTool {
            name: "read_file".to_string(),
            title: Some("Read File".to_string()),
            description: "Read the contents of a file, optionally specifying line ranges".to_string(),
            input_schema: object_schema(props, vec!["path".to_string()]),
            output_schema: None,
            annotations: None,
        }
    }

    fn create_write_file_tool() -> MCPTool {
        let mut props = HashMap::new();
        props.insert("path".to_string(), string_prop(Some(1), Some(1000), Some("Path to the file to write")));
        props.insert("content".to_string(), string_prop(None, Some(MAX_FILE_SIZE as u32), Some("Content to write to the file")));

        MCPTool {
            name: "write_file".to_string(),
            title: Some("Write File".to_string()),
            description: "Write content to a file".to_string(),
            input_schema: object_schema(props, vec!["path".to_string(), "content".to_string()]),
            output_schema: None,
            annotations: None,
        }
    }

    fn create_list_directory_tool() -> MCPTool {
        let mut props = HashMap::new();
        props.insert("path".to_string(), string_prop(Some(1), Some(1000), Some("Path to the directory to list")));

        MCPTool {
            name: "list_directory".to_string(),
            title: Some("List Directory".to_string()),
            description: "List contents of a directory".to_string(),
            input_schema: object_schema(props, vec!["path".to_string()]),
            output_schema: None,
            annotations: None,
        }
    }

    fn create_replace_lines_in_file_tool() -> MCPTool {
        let mut item_props = HashMap::new();
        item_props.insert("line_number".to_string(), integer_prop(Some(1), None, Some("The 1-based line number to replace")));
        item_props.insert("content".to_string(), string_prop(None, None, Some("The new content for the line")));
        
        let replacement_item_schema = object_schema(item_props, vec!["line_number".to_string(), "content".to_string()]);
        
        let mut props = HashMap::new();
        props.insert("path".to_string(), string_prop(Some(1), Some(1000), Some("Path to the file to modify")));
        props.insert("replacements".to_string(), array_schema(replacement_item_schema, Some("An array of line replacement objects")));

        MCPTool {
            name: "replace_lines_in_file".to_string(),
            title: Some("Replace Lines in File".to_string()),
            description: "Replace specific lines in a file with new content".to_string(),
            input_schema: object_schema(props, vec!["path".to_string(), "replacements".to_string()]),
            output_schema: None,
            annotations: None,
        }
    }

    fn create_grep_tool() -> MCPTool {
        let mut props = HashMap::new();
        props.insert("pattern".to_string(), string_prop(Some(1), None, Some("Regex pattern to search for")));
        props.insert("path".to_string(), string_prop(Some(1), Some(1000), Some("Path to the file to search (exclusive with 'input')")));
        props.insert("input".to_string(), string_prop(Some(1), None, Some("Input string to search (exclusive with 'path')")));
        props.insert("ignore_case".to_string(), boolean_prop(Some("Perform case-insensitive matching")));
        props.insert("line_numbers".to_string(), boolean_prop(Some("Include line numbers in the output")));

        MCPTool {
            name: "grep".to_string(),
            title: Some("Grep".to_string()),
            description: "Search for a pattern in a file or input string.".to_string(),
            input_schema: object_schema(props, vec!["pattern".to_string()]),
            output_schema: None,
            annotations: None,
        }
    }

    fn create_search_files_tool() -> MCPTool {
        let mut props = HashMap::new();
        props.insert("pattern".to_string(), string_prop(Some(1), Some(500), Some("Glob pattern to match files (e.g., '*.rs', '**/*.tsx')")));
        props.insert("path".to_string(), string_prop(Some(1), Some(1000), Some("Root path to search from")));
        props.insert("max_depth".to_string(), integer_prop(Some(1), Some(50), Some("Maximum depth to search (optional)")));
        props.insert("file_type".to_string(), string_prop(None, None, Some("Filter by file type: 'file', 'dir', or 'both'")));

        MCPTool {
            name: "search_files".to_string(),
            title: Some("Search Files".to_string()),
            description: "Search for files matching patterns with various filters".to_string(),
            input_schema: object_schema(props, vec!["pattern".to_string()]),
            output_schema: None,
            annotations: None,
        }
    }

    async fn handle_replace_lines_in_file(&self, args: Value) -> MCPResponse {
        let request_id = Value::String(uuid::Uuid::new_v4().to_string());

        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) => path,
            None => {
                return MCPResponse::error(request_id, -32602, "Missing required parameter: path");
            }
        };

        let replacements_val = match args.get("replacements") {
            Some(val) => val,
            None => {
                return MCPResponse::error(
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
                    return MCPResponse::error(
                        request_id,
                        -32602,
                        &format!("Invalid replacements format: {e}"),
                    );
                }
            };

        // 경로 유효성 검사
        let safe_path = match self
            .file_manager
            .get_security_validator()
            .validate_path(path_str)
        {
            Ok(path) => path,
            Err(e) => {
                return MCPResponse::error(request_id, -32603, &format!("Security error: {e}"));
            }
        };

        // 파일 읽기
        let lines = match self.read_file_lines(&safe_path).await {
            Ok(lines) => lines,
            Err(e) => {
                return MCPResponse::error(
                    request_id,
                    -32603,
                    &format!("Failed to read file: {e}"),
                );
            }
        };

        let mut new_lines = lines.clone();
        let mut replacements_map: HashMap<usize, String> = HashMap::new();

        for rep in replacements {
            let line_number = match rep.get("line_number").and_then(|v| v.as_u64()) {
                Some(num) => num as usize,
                None => {
                    return MCPResponse::error(request_id, -32602, "Invalid line_number format");
                }
            };
            let content = match rep.get("content").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => {
                    return MCPResponse::error(request_id, -32602, "Invalid content format");
                }
            };

            if line_number == 0 || line_number > new_lines.len() {
                return MCPResponse::error(
                    request_id,
                    -32602,
                    &format!("Line number {line_number} is out of bounds"),
                );
            }
            replacements_map.insert(line_number, content);
        }

        // 라인 교체 (인덱스 기반)
        for (line_number, content) in replacements_map {
            new_lines[line_number - 1] = content;
        }

        // 파일 쓰기
        let new_content = new_lines.join("\n");
        match self
            .file_manager
            .write_file_string(path_str, &new_content)
            .await
        {
            Ok(_) => MCPResponse::success(
                request_id,
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Successfully replaced lines in file {}", path_str)
                    }]
                }),
            ),
            Err(e) => {
                MCPResponse::error(request_id, -32603, &format!("Failed to write file: {e}"))
            }
        }
    }

    async fn handle_grep(&self, args: Value) -> MCPResponse {
        let request_id = Value::String(uuid::Uuid::new_v4().to_string());

        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return MCPResponse::error(request_id, -32602, "missing 'pattern' argument"),
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
            match self
                .file_manager
                .get_security_validator()
                .validate_path(path_str)
            {
                Ok(safe_path) => match tokio::fs::read_to_string(safe_path).await {
                    Ok(s) => s,
                    Err(e) => {
                        return MCPResponse::error(
                            request_id,
                            -32603,
                            &format!("failed to read file {path_str}: {e}"),
                        )
                    }
                },
                Err(e) => {
                    return MCPResponse::error(
                        request_id,
                        -32603,
                        &format!("Security error: {e}"),
                    );
                }
            }
        } else if let Some(s) = args.get("input").and_then(|v| v.as_str()) {
            s.to_string()
        } else {
            return MCPResponse::error(
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
                return MCPResponse::error(request_id, -32602, &format!("invalid pattern: {e}"))
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

        MCPResponse::success(
            request_id,
            json!({
                "content": [{
                    "type": "text",
                    "text": format!("Found {} matches:\n{}", matches.len(), serde_json::to_string_pretty(&matches).unwrap_or_default())
                }]
            }),
        )
    }

    async fn handle_read_file(&self, args: Value) -> MCPResponse {
        let request_id = Value::String(uuid::Uuid::new_v4().to_string());

        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) => path,
            None => {
                return MCPResponse::error(request_id, -32602, "Missing required parameter: path");
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

        // Validate line range
        if let (Some(start), Some(end)) = (start_line, end_line) {
            if start > end {
                return MCPResponse::error(
                    request_id,
                    -32602,
                    "start_line must be less than or equal to end_line",
                );
            }
        }

        let safe_path = match self
            .file_manager
            .get_security_validator()
            .validate_path(path_str)
        {
            Ok(path) => path,
            Err(e) => {
                error!("Path validation failed: {}", e);
                return MCPResponse::error(request_id, -32603, &format!("Security error: {e}"));
            }
        };

        // Read file with line range support using SecureFileManager
        let content = if start_line.is_some() || end_line.is_some() {
            // Check file size
            if let Err(e) = self
                .file_manager
                .get_security_validator()
                .validate_file_size(&safe_path, MAX_FILE_SIZE)
            {
                error!("File size validation failed: {}", e);
                return MCPResponse::error(request_id, -32603, &format!("File size error: {e}"));
            }

            self.read_file_lines_range(&safe_path, start_line, end_line)
                .await
        } else {
            self.file_manager
                .read_file_as_string(path_str)
                .await
                .map_err(|e| e.to_string())
        };

        match content {
            Ok(content) => {
                info!("Successfully read file: {}", path_str);
                MCPResponse::success(
                    request_id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": content
                        }]
                    }),
                )
            }
            Err(e) => {
                error!("Failed to read file {}: {}", path_str, e);
                MCPResponse::error(request_id, -32603, &format!("Failed to read file: {e}"))
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

    async fn handle_search_files(&self, args: Value) -> MCPResponse {
        let request_id = Value::String(uuid::Uuid::new_v4().to_string());

        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(pattern) => pattern,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter: pattern".to_string(),
                        data: None,
                    }),
                };
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

        // Validate search path security using SecureFileManager
        let safe_path = match self
            .file_manager
            .get_security_validator()
            .validate_path(search_path)
        {
            Ok(path) => path,
            Err(e) => {
                error!("Path validation failed: {}", e);
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Security error: {e}"),
                        data: None,
                    }),
                };
            }
        };

        // Search files
        match self
            .search_files_by_pattern(&safe_path, pattern, max_depth, file_type)
            .await
        {
            Ok(results) => {
                let result_text = if results.is_empty() {
                    format!(
                        "No files found matching pattern '{pattern}' in '{search_path}'"
                    )
                } else {
                    format!(
                        "Found {} files matching pattern '{}':\n{}",
                        results.len(),
                        pattern,
                        serde_json::to_string_pretty(&results).unwrap_or_default()
                    )
                };

                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: Some(json!({
                        "content": [{
                            "type": "text",
                            "text": result_text
                        }]
                    })),
                    error: None,
                }
            }
            Err(e) => {
                error!("File search failed: {}", e);
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Search failed: {e}"),
                        data: None,
                    }),
                }
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

            // Check file type filter
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

            // Check pattern match
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

    async fn handle_write_file(&self, args: Value) -> MCPResponse {
        let request_id = Value::String(uuid::Uuid::new_v4().to_string());

        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) => path,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter: path".to_string(),
                        data: None,
                    }),
                };
            }
        };

        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(content) => content,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter: content".to_string(),
                        data: None,
                    }),
                };
            }
        };

        // Use SecureFileManager to write file
        match self.file_manager.write_file_string(path_str, content).await {
            Ok(()) => {
                info!("Successfully wrote file: {}", path_str);
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: Some(json!({
                        "content": [{
                            "type": "text",
                            "text": format!("Successfully wrote {} bytes to {}", content.len(), path_str)
                        }]
                    })),
                    error: None,
                }
            }
            Err(e) => {
                error!("Failed to write file {}: {}", path_str, e);
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Failed to write file: {e}"),
                        data: None,
                    }),
                }
            }
        }
    }

    async fn handle_list_directory(&self, args: Value) -> MCPResponse {
        let request_id = Value::String(uuid::Uuid::new_v4().to_string());

        let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        // Validate path security using SecureFileManager
        let safe_path = match self
            .file_manager
            .get_security_validator()
            .validate_path(path_str)
        {
            Ok(path) => path,
            Err(e) => {
                error!("Path validation failed: {}", e);
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Security error: {e}"),
                        data: None,
                    }),
                };
            }
        };

        // List directory contents
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

                    // Directories first, then files, then by name
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
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: Some(json!({
                        "content": [{
                            "type": "text",
                            "text": format!("Directory listing for {}:\n{}", path_str,
                                serde_json::to_string_pretty(&items).unwrap_or_default())
                        }]
                    })),
                    error: None,
                }
            }
            Err(e) => {
                error!("Failed to list directory {:?}: {}", safe_path, e);
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Failed to list directory: {e}"),
                        data: None,
                    }),
                }
            }
        }
    }
}

#[async_trait]
impl BuiltinMCPServer for FilesystemServer {
    fn name(&self) -> &str {
        "filesystem"
    }

    fn description(&self) -> &str {
        "Built-in filesystem operations server"
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            Self::create_read_file_tool(),
            Self::create_write_file_tool(),
            Self::create_list_directory_tool(),
            Self::create_search_files_tool(),
            Self::create_replace_lines_in_file_tool(),
            Self::create_grep_tool(),
        ]
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse {
        match tool_name {
            "read_file" => self.handle_read_file(args).await,
            "write_file" => self.handle_write_file(args).await,
            "list_directory" => self.handle_list_directory(args).await,
            "search_files" => self.handle_search_files(args).await,
            "replace_lines_in_file" => self.handle_replace_lines_in_file(args).await,
            "grep" => self.handle_grep(args).await,
            _ => {
                let request_id = Value::String(uuid::Uuid::new_v4().to_string());
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32601,
                        message: format!("Tool '{tool_name}' not found in filesystem server"),
                        data: None,
                    }),
                }
            }
        }
    }
}

impl Default for FilesystemServer {
    fn default() -> Self {
        Self::new(std::sync::Arc::new(SecureFileManager::new()))
    }
}
