use async_trait::async_trait;
use regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Write;
use std::time::Duration;
use tempfile::TempDir;
use tokio::fs;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{error, info, warn};
use zip::write::FileOptions;

use super::{
    utils::constants::{
        DEFAULT_EXECUTION_TIMEOUT, MAX_CODE_SIZE, MAX_EXECUTION_TIMEOUT, MAX_FILE_SIZE,
    },
    BuiltinMCPServer,
};
use crate::mcp::{utils::schema_builder::*, MCPResponse, MCPTool};
use crate::services::SecureFileManager;
use crate::session::SessionManager;

pub struct WorkspaceServer {
    session_manager: std::sync::Arc<SessionManager>,
}

impl WorkspaceServer {
    pub fn new(session_manager: std::sync::Arc<SessionManager>) -> Self {
        info!("WorkspaceServer using session-based workspace management");
        Self { session_manager }
    }

    /// SessionManager를 통한 현재 활성 세션의 워크스페이스 사용
    fn get_workspace_dir(&self) -> std::path::PathBuf {
        self.session_manager.get_session_workspace_dir()
    }

    /// 동적 파일 매니저 생성 (현재 세션 기반)
    fn get_file_manager(&self) -> std::sync::Arc<SecureFileManager> {
        self.session_manager.get_file_manager()
    }

    /// Generate a new request ID for MCP responses
    fn generate_request_id() -> Value {
        Value::String(cuid2::create_id())
    }

    /// Create a success response with text content
    fn success_response(request_id: Value, message: &str) -> MCPResponse {
        MCPResponse::success(
            request_id,
            json!({
                "content": [{
                    "type": "text",
                    "text": message
                }]
            }),
        )
    }

    /// Create an error response with consistent formatting
    fn error_response(request_id: Value, code: i32, message: &str) -> MCPResponse {
        MCPResponse::error(request_id, code, message)
    }

    /// 파일 경로 검증 (세션 워크스페이스 기반)
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

    // File operation tool definitions (copied from filesystem.rs)
    fn create_read_file_tool() -> MCPTool {
        let mut props = HashMap::new();
        props.insert(
            "path".to_string(),
            string_prop(Some(1), Some(1000), Some("Path to the file to read")),
        );
        props.insert(
            "start_line".to_string(),
            integer_prop(
                Some(1),
                None,
                Some("Starting line number (1-based, optional)"),
            ),
        );
        props.insert(
            "end_line".to_string(),
            integer_prop(
                Some(1),
                None,
                Some("Ending line number (1-based, optional)"),
            ),
        );

        MCPTool {
            name: "read_file".to_string(),
            title: Some("Read File".to_string()),
            description: "Read the contents of a file, optionally specifying line ranges"
                .to_string(),
            input_schema: object_schema(props, vec!["path".to_string()]),
            output_schema: None,
            annotations: None,
        }
    }

    fn create_write_file_tool() -> MCPTool {
        let mut props = HashMap::new();
        props.insert(
            "path".to_string(),
            string_prop(Some(1), Some(1000), Some("Path to the file to write")),
        );
        props.insert(
            "content".to_string(),
            string_prop(
                None,
                Some(MAX_FILE_SIZE as u32),
                Some("Content to write to the file"),
            ),
        );
        props.insert(
            "mode".to_string(),
            string_prop(
                None,
                None,
                Some("Write mode: 'w' for overwrite (default), 'a' for append"),
            ),
        );

        MCPTool {
            name: "write_file".to_string(),
            title: Some("Write File".to_string()),
            description: "Write content to a file with optional append mode".to_string(),
            input_schema: object_schema(props, vec!["path".to_string(), "content".to_string()]),
            output_schema: None,
            annotations: None,
        }
    }

    fn create_list_directory_tool() -> MCPTool {
        let mut props = HashMap::new();
        props.insert(
            "path".to_string(),
            string_prop(Some(1), Some(1000), Some("Path to the directory to list")),
        );

        MCPTool {
            name: "list_directory".to_string(),
            title: Some("List Directory".to_string()),
            description: "List contents of a directory".to_string(),
            input_schema: object_schema(props, vec!["path".to_string()]),
            output_schema: None,
            annotations: None,
        }
    }

    fn create_search_files_tool() -> MCPTool {
        let mut props = HashMap::new();
        props.insert(
            "pattern".to_string(),
            string_prop(
                Some(1),
                Some(500),
                Some("Glob pattern to match files (e.g., '*.rs', '**/*.tsx')"),
            ),
        );
        props.insert(
            "path".to_string(),
            string_prop(Some(1), Some(1000), Some("Root path to search from")),
        );
        props.insert(
            "max_depth".to_string(),
            integer_prop(
                Some(1),
                Some(50),
                Some("Maximum depth to search (optional)"),
            ),
        );
        props.insert(
            "file_type".to_string(),
            string_prop(
                None,
                None,
                Some("Filter by file type: 'file', 'dir', or 'both'"),
            ),
        );

        MCPTool {
            name: "search_files".to_string(),
            title: Some("Search Files".to_string()),
            description: "Search for files matching patterns with various filters".to_string(),
            input_schema: object_schema(props, vec!["pattern".to_string()]),
            output_schema: None,
            annotations: None,
        }
    }

    fn create_replace_lines_in_file_tool() -> MCPTool {
        let mut item_props = HashMap::new();
        item_props.insert(
            "start_line".to_string(),
            integer_prop(Some(1), None, Some("Starting line number (1-based)")),
        );
        item_props.insert(
            "end_line".to_string(),
            integer_prop(
                Some(1),
                None,
                Some("Ending line number (1-based, optional). If not provided, equals start_line"),
            ),
        );
        item_props.insert(
            "content".to_string(),
            string_prop(None, None, Some("The new content for the line range")),
        );

        // 기존 line_number 지원을 위한 backward compatibility
        item_props.insert(
            "line_number".to_string(),
            integer_prop(
                Some(1),
                None,
                Some("The 1-based line number to replace (deprecated, use start_line)"),
            ),
        );

        let replacement_item_schema = object_schema(
            item_props,
            vec!["start_line".to_string(), "content".to_string()],
        );

        let mut props = HashMap::new();
        props.insert(
            "path".to_string(),
            string_prop(Some(1), Some(1000), Some("Path to the file to modify")),
        );
        props.insert(
            "replacements".to_string(),
            array_schema(
                replacement_item_schema,
                Some("An array of line replacement objects"),
            ),
        );

        MCPTool {
            name: "replace_lines_in_file".to_string(),
            title: Some("Replace Lines in File".to_string()),
            description: "Replace specific lines or line ranges in a file with new content"
                .to_string(),
            input_schema: object_schema(
                props,
                vec!["path".to_string(), "replacements".to_string()],
            ),
            output_schema: None,
            annotations: None,
        }
    }

    fn create_grep_tool() -> MCPTool {
        let mut props = HashMap::new();
        props.insert(
            "pattern".to_string(),
            string_prop(Some(1), None, Some("Regex pattern to search for")),
        );
        props.insert(
            "path".to_string(),
            string_prop(
                Some(1),
                Some(1000),
                Some("Path to the file to search (exclusive with 'input')"),
            ),
        );
        props.insert(
            "input".to_string(),
            string_prop(
                Some(1),
                None,
                Some("Input string to search (exclusive with 'path')"),
            ),
        );
        props.insert(
            "ignore_case".to_string(),
            boolean_prop(Some("Perform case-insensitive matching")),
        );
        props.insert(
            "line_numbers".to_string(),
            boolean_prop(Some("Include line numbers in the output")),
        );

        MCPTool {
            name: "grep".to_string(),
            title: Some("Grep".to_string()),
            description: "Search for a pattern in a file or input string.".to_string(),
            input_schema: object_schema(props, vec!["pattern".to_string()]),
            output_schema: None,
            annotations: None,
        }
    }

    // Code execution tool definitions (copied from sandbox.rs)
    fn create_execute_python_tool() -> MCPTool {
        let mut props = HashMap::new();
        props.insert(
            "code".to_string(),
            string_prop_with_examples(
                Some(1),
                Some(MAX_CODE_SIZE as u32),
                Some("Python code to execute"),
                vec![json!("print('Hello, World!')")],
            ),
        );
        props.insert(
            "timeout".to_string(),
            integer_prop_with_default(
                Some(1),
                Some(MAX_EXECUTION_TIMEOUT as i64),
                DEFAULT_EXECUTION_TIMEOUT as i64,
                Some("Timeout in seconds (default: 30)"),
            ),
        );

        MCPTool {
            name: "execute_python".to_string(),
            title: Some("Execute Python Code".to_string()),
            description: "Execute Python code in a sandboxed environment".to_string(),
            input_schema: object_schema(props, vec!["code".to_string()]),
            output_schema: None,
            annotations: None,
        }
    }

    fn create_execute_typescript_tool() -> MCPTool {
        let mut props = HashMap::new();
        props.insert(
            "code".to_string(),
            string_prop_with_examples(
                Some(1),
                Some(MAX_CODE_SIZE as u32),
                Some("TypeScript code to execute"),
                vec![json!("console.log('Hello, World!');")],
            ),
        );
        props.insert(
            "timeout".to_string(),
            integer_prop_with_default(
                Some(1),
                Some(MAX_EXECUTION_TIMEOUT as i64),
                DEFAULT_EXECUTION_TIMEOUT as i64,
                Some("Timeout in seconds (default: 30)"),
            ),
        );

        MCPTool {
            name: "execute_typescript".to_string(),
            title: Some("Execute TypeScript Code".to_string()),
            description: "Execute TypeScript code in a sandboxed environment using ts-node"
                .to_string(),
            input_schema: object_schema(props, vec!["code".to_string()]),
            output_schema: None,
            annotations: None,
        }
    }

    fn create_execute_shell_tool() -> MCPTool {
        let mut props = HashMap::new();
        props.insert(
            "command".to_string(),
            string_prop_with_examples(
                Some(1),
                Some(1000),
                Some("Shell command to execute"),
                vec![json!("ls -la"), json!("grep -r 'pattern' .")],
            ),
        );
        props.insert(
            "timeout".to_string(),
            integer_prop_with_default(
                Some(1),
                Some(300), // 5분 최대
                30,
                Some("Timeout in seconds (default: 30)"),
            ),
        );
        props.insert(
            "working_dir".to_string(),
            string_prop(
                Some(1),
                Some(1000),
                Some("Working directory for command execution (optional)"),
            ),
        );

        MCPTool {
            name: "execute_shell".to_string(),
            title: Some("Execute Shell Command".to_string()),
            description: "Execute a shell command in the current environment".to_string(),
            input_schema: object_schema(props, vec!["command".to_string()]),
            output_schema: None,
            annotations: None,
        }
    }

    // File operation handlers (copied from filesystem.rs)
    async fn handle_read_file(&self, args: Value) -> MCPResponse {
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

        // Validate line range
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

        // Read file with line range support using dynamic session-based file manager
        let file_manager = self.get_file_manager();
        let content = if start_line.is_some() || end_line.is_some() {
            // Check file size
            if let Err(e) = file_manager
                .get_security_validator()
                .validate_file_size(&safe_path, MAX_FILE_SIZE)
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

    async fn handle_write_file(&self, args: Value) -> MCPResponse {
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

        // 동적으로 현재 세션의 파일 매니저 사용
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

    async fn handle_list_directory(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        // Validate path security using SecureFileManager
        let safe_path = match self.validate_path_with_error(path_str, &request_id) {
            Ok(path) => path,
            Err(error_response) => return *error_response,
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

    async fn handle_search_files(&self, args: Value) -> MCPResponse {
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

        // Validate search path security using dynamic file manager
        let safe_path = match self.validate_path_with_error(search_path, &request_id) {
            Ok(path) => path,
            Err(error_response) => return *error_response,
        };

        // Search files
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

    async fn handle_replace_lines_in_file(&self, args: Value) -> MCPResponse {
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

        // 경로 유효성 검사
        let safe_path = match self.validate_path_with_error(path_str, &request_id) {
            Ok(path) => path,
            Err(error_response) => return *error_response,
        };

        // 파일 읽기
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
                None => {
                    // backward compatibility: line_number fallback
                    match rep.get("line_number").and_then(|v| v.as_u64()) {
                        Some(num) => num as usize,
                        None => {
                            return Self::error_response(
                                request_id,
                                -32602,
                                "Missing start_line or line_number",
                            );
                        }
                    }
                }
            };

            let end_line = rep
                .get("end_line")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize)
                .unwrap_or(start_line); // 기본값: start_line과 동일

            // 범위 검증
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

            let content = match rep.get("content").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => {
                    return Self::error_response(request_id, -32602, "Invalid content format");
                }
            };

            // 범위 교체를 위한 키 생성 (start_line-end_line 형식)
            let range_key = format!("{start_line}-{end_line}");
            replacements_map.insert(range_key, content);
        }

        // 범위 교체 처리
        for (range_key, content) in replacements_map {
            let parts: Vec<&str> = range_key.split('-').collect();
            let start_line: usize = parts[0].parse().unwrap();
            let end_line: usize = parts[1].parse().unwrap();

            if start_line == end_line {
                // 단일 줄 교체
                new_lines[start_line - 1] = content;
            } else {
                // 범위 교체: start_line부터 end_line까지를 content로 교체
                new_lines.splice((start_line - 1)..end_line, vec![content]);
            }
        }

        // 파일 쓰기
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

    async fn handle_grep(&self, args: Value) -> MCPResponse {
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

    // Code execution handlers (adapted from sandbox.rs)
    async fn execute_code_in_sandbox(
        &self,
        command: &str,
        args: &[&str],
        code: &str,
        file_extension: &str,
        timeout_secs: u64,
    ) -> MCPResponse {
        let request_id = Self::generate_request_id();

        // Validate code size
        if code.len() > MAX_CODE_SIZE {
            return Self::error_response(
                request_id,
                -32602,
                &format!(
                    "Code size {} exceeds maximum allowed size {}",
                    code.len(),
                    MAX_CODE_SIZE
                ),
            );
        }

        // Create temporary directory for sandboxed execution
        let temp_dir = match TempDir::new() {
            Ok(dir) => dir,
            Err(e) => {
                return Self::error_response(
                    request_id,
                    -32603,
                    &format!("Failed to create temporary directory: {}", e),
                )
            }
        };

        // Write code to temporary file
        let script_path = temp_dir.path().join(format!("script{}", file_extension));
        if let Err(e) = fs::write(&script_path, code).await {
            return Self::error_response(
                request_id,
                -32603,
                &format!("Failed to write script file: {}", e),
            );
        }

        // Prepare command with arguments
        let mut cmd = Command::new(command);
        for arg in args {
            cmd.arg(arg);
        }
        cmd.arg(&script_path);

        // 핵심 변경: SessionManager의 workspace 디렉토리 사용
        let work_dir = self.get_workspace_dir();
        info!("Code execution in workspace: {:?}", work_dir);
        cmd.current_dir(&work_dir);

        // Clear environment variables for isolation
        cmd.env_clear();
        cmd.env("PATH", std::env::var("PATH").unwrap_or_default());

        // HOME은 workspace 디렉토리로 설정
        if let Some(workspace_str) = work_dir.to_str() {
            cmd.env("HOME", workspace_str);
            cmd.env("PWD", workspace_str);
        }

        // 임시 관련 변수는 temp_dir로 설정
        if let Some(tmp_str) = temp_dir.path().to_str() {
            cmd.env("TMPDIR", tmp_str);
            cmd.env("TMP", tmp_str);
            cmd.env("TEMP", tmp_str);
        }

        // Execute command with timeout
        let timeout_duration = Duration::from_secs(timeout_secs.min(MAX_EXECUTION_TIMEOUT));
        let execution_result = timeout(timeout_duration, cmd.output()).await;

        // 실행 결과 처리
        match execution_result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                let result = if output.status.success() {
                    json!({
                        "success": true,
                        "stdout": stdout,
                        "stderr": stderr,
                        "exit_code": output.status.code()
                    })
                } else {
                    json!({
                        "success": false,
                        "stdout": stdout,
                        "stderr": stderr,
                        "exit_code": output.status.code()
                    })
                };

                MCPResponse::success(request_id, result)
            }
            Ok(Err(e)) => Self::error_response(
                request_id,
                -32603,
                &format!("Failed to execute command: {}", e),
            ),
            Err(_) => Self::error_response(
                request_id,
                -32603,
                &format!("Command execution timed out after {} seconds", timeout_secs),
            ),
        }
    }

    async fn handle_execute_python(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let code = match args.get("code").and_then(|v| v.as_str()) {
            Some(code) => code,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: code",
                );
            }
        };

        let timeout_secs = args
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_EXECUTION_TIMEOUT)
            .min(MAX_EXECUTION_TIMEOUT);

        self.execute_code_in_sandbox("python3", &[], code, ".py", timeout_secs)
            .await
    }

    async fn handle_execute_typescript(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let code = match args.get("code").and_then(|v| v.as_str()) {
            Some(code) => code,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: code",
                );
            }
        };

        let timeout_secs = args
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_EXECUTION_TIMEOUT)
            .min(MAX_EXECUTION_TIMEOUT);

        // 코드 전달 검증 강화 (직렬화 문제 방지)
        if let Err(e) = std::str::from_utf8(code.as_bytes()) {
            error!("Invalid UTF-8 in TypeScript code: {}", e);
            return Self::error_response(request_id, -32603, "Invalid UTF-8 encoding in code");
        }

        // 코드 크기 검증 (기존과 동일)
        if code.len() > MAX_CODE_SIZE {
            return Self::error_response(
                request_id,
                -32603,
                &format!(
                    "Code too large: {} bytes (max: {} bytes)",
                    code.len(),
                    MAX_CODE_SIZE
                ),
            );
        }

        // Deno 설치 확인
        let deno_check = Command::new("which").arg("deno").output().await;
        if deno_check.is_err() || !deno_check.unwrap().status.success() {
            error!("Deno not found on system");
            return Self::error_response(
                request_id,
                -32603,
                "Deno is required for TypeScript execution.\n\n\
                    To install Deno automatically, run:\n\
                    curl -fsSL https://deno.land/install.sh | sh\n\n\
                    Or using package managers:\n\
                    - macOS: brew install deno\n\
                    - Windows: winget install deno\n\
                    - Linux: curl -fsSL https://deno.land/install.sh | sh\n\n\
                    After installation, restart the application.",
            );
        }

        info!("Using Deno for TypeScript execution");

        // 임시 디렉터리 생성 (파일 기반 실행으로 안정성 향상)
        let temp_dir = match TempDir::new() {
            Ok(dir) => dir,
            Err(e) => {
                error!(
                    "Failed to create temporary directory for Deno execution: {}",
                    e
                );
                return Self::error_response(
                    request_id,
                    -32603,
                    &format!("Failed to create temp directory: {e}"),
                );
            }
        };

        let ts_file = temp_dir.path().join("script.ts");

        // 코드 파일 쓰기 (직렬화 검증 후)
        if let Err(e) = fs::write(&ts_file, code).await {
            error!("Failed to write TypeScript file: {}", e);
            return Self::error_response(
                request_id,
                -32603,
                &format!("Failed to write TypeScript file: {e}"),
            );
        }

        // Deno 실행 명령 준비
        let mut deno_cmd = Command::new("deno");
        deno_cmd
            .arg("run")
            .arg("--allow-read") // 파일 읽기 허용
            .arg("--allow-write") // 파일 쓰기 허용
            .arg("--allow-net") // 네트워크 접근 허용 (필요시)
            .arg("--quiet") // Deno 출력 최소화
            .arg(&ts_file);

        // 작업 디렉터리 설정
        let work_dir = self.get_workspace_dir();
        deno_cmd.current_dir(&work_dir);

        // 환경 변수 설정 (보안 및 격리)
        deno_cmd.env_clear();
        deno_cmd.env("PATH", std::env::var("PATH").unwrap_or_default());
        if let Some(home_str) = work_dir.to_str() {
            deno_cmd.env("HOME", home_str);
        }

        // 타임아웃 설정
        let timeout_duration = Duration::from_secs(timeout_secs.min(MAX_EXECUTION_TIMEOUT));

        // 실행 및 결과 처리
        match timeout(timeout_duration, deno_cmd.output()).await {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let success = output.status.success();

                let result_text = if success {
                    if stdout.trim().is_empty() && stderr.trim().is_empty() {
                        "TypeScript executed successfully (no output)".to_string()
                    } else if stderr.trim().is_empty() {
                        format!("Output:\n{}", stdout.trim())
                    } else {
                        format!(
                            "Output:\n{}\n\nWarnings/Errors:\n{}",
                            stdout.trim(),
                            stderr.trim()
                        )
                    }
                } else {
                    format!(
                        "Execution failed (exit code: {}):\n{}",
                        output.status.code().unwrap_or(-1),
                        if stderr.trim().is_empty() {
                            stdout.trim()
                        } else {
                            stderr.trim()
                        }
                    )
                };

                info!(
                    "TypeScript execution completed via Deno. Success: {}, Output length: {}",
                    success,
                    result_text.len()
                );

                Self::success_response(request_id, &result_text)
            }
            Ok(Err(e)) => {
                error!("Failed to execute with Deno: {}", e);
                Self::error_response(request_id, -32603, &format!("Deno execution error: {e}"))
            }
            Err(_) => {
                warn!(
                    "TypeScript execution timed out after {} seconds",
                    timeout_secs
                );
                Self::error_response(
                    request_id,
                    -32603,
                    &format!("Execution timed out after {timeout_secs} seconds"),
                )
            }
        }
    }

    async fn handle_execute_shell(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let command_str = match args.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: command",
                );
            }
        };

        let timeout_secs = args
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(30)
            .min(300); // 최대 5분

        let working_dir = args.get("working_dir").and_then(|v| v.as_str());

        // Determine working directory
        let work_dir = if let Some(dir) = working_dir {
            std::path::PathBuf::from(dir)
        } else {
            self.get_workspace_dir().to_path_buf()
        };

        // Execute shell command
        let mut cmd = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", command_str]);
            cmd
        } else {
            let mut cmd = Command::new("sh");
            cmd.args(["-c", command_str]);
            cmd
        };

        cmd.current_dir(&work_dir);

        let timeout_duration = Duration::from_secs(timeout_secs);

        match timeout(timeout_duration, cmd.output()).await {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let success = output.status.success();
                let exit_code = output.status.code().unwrap_or(-1);

                let result_text = if success {
                    if stdout.trim().is_empty() && stderr.trim().is_empty() {
                        "Command executed successfully (no output)".to_string()
                    } else if stderr.trim().is_empty() {
                        format!("Command executed successfully:\n{}", stdout.trim())
                    } else {
                        format!(
                            "Command executed successfully:\nSTDOUT:\n{}\n\nSTDERR:\n{}",
                            stdout.trim(),
                            stderr.trim()
                        )
                    }
                } else {
                    format!(
                        "Command failed with exit code {}:\nSTDOUT:\n{}\n\nSTDERR:\n{}",
                        exit_code,
                        stdout.trim(),
                        stderr.trim()
                    )
                };

                info!(
                    "Shell command executed: {} (exit: {})",
                    command_str, exit_code
                );

                Self::success_response(request_id, &result_text)
            }
            Ok(Err(e)) => {
                error!("Failed to execute shell command '{}': {}", command_str, e);
                Self::error_response(request_id, -32603, &format!("Execution error: {e}"))
            }
            Err(_) => {
                error!(
                    "Shell command '{}' timed out after {} seconds",
                    command_str, timeout_secs
                );
                Self::error_response(
                    request_id,
                    -32603,
                    &format!("Command timed out after {timeout_secs} seconds"),
                )
            }
        }
    }

    /// Create a success response with both text description and UIResource content
    fn success_response_with_text_and_resource(
        request_id: Value,
        message: &str,
        ui_resource: Value,
    ) -> MCPResponse {
        MCPResponse::success(
            request_id,
            json!({
                "content": [
                    {
                        "type": "text",
                        "text": message
                    },
                    ui_resource
                ]
            }),
        )
    }

    /// UIResource 생성 (MCP-UI 표준 준수)
    fn create_export_ui_resource(
        &self,
        request_id: u64,
        title: &str,
        files: &[String],
        export_type: &str,
        download_path: &str,
        content: String,
    ) -> Value {
        // MCP-UI 표준 UIResource 형식
        json!({
            "type": "resource",
            "resource": {
                "uri": format!("ui://export/{}/{}", export_type.to_lowercase(), request_id),
                "mimeType": "text/html",
                "text": content,
                "title": title,
                "annotations": {
                    "export_type": export_type,
                    "file_count": files.len(),
                    "download_path": download_path,
                    "created_at": chrono::Utc::now().to_rfc3339()
                }
            }
        })
    }

    /// HTML UIResource 생성 (postMessage 통신 사용)
    fn create_html_export_ui(
        &self,
        title: &str,
        files: &[String],
        export_type: &str,
        download_path: &str,
        _display_name: &str,
    ) -> String {
        let files_list = files
            .iter()
            .map(|f| format!("<li class='file-item'>{}</li>", html_escape::encode_text(f)))
            .collect::<Vec<_>>()
            .join("");

        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            max-width: 600px;
            margin: 0 auto;
            padding: 20px;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            min-height: 100vh;
        }}
        .container {{
            background: rgba(255, 255, 255, 0.1);
            border-radius: 15px;
            padding: 30px;
            backdrop-filter: blur(10px);
            box-shadow: 0 8px 32px 0 rgba(31, 38, 135, 0.37);
        }}
        h1 {{
            text-align: center;
            margin-bottom: 30px;
            font-size: 2em;
            text-shadow: 2px 2px 4px rgba(0,0,0,0.3);
        }}
        .export-info {{
            background: rgba(255, 255, 255, 0.1);
            border-radius: 10px;
            padding: 20px;
            margin-bottom: 30px;
        }}
        .download-btn {{
            background: linear-gradient(45deg, #2196F3, #21CBF3);
            color: white;
            border: none;
            padding: 15px 30px;
            font-size: 18px;
            border-radius: 25px;
            cursor: pointer;
            transition: all 0.3s ease;
            box-shadow: 0 4px 15px 0 rgba(33, 150, 243, 0.3);
        }}
        .download-btn:hover {{
            transform: translateY(-2px);
            box-shadow: 0 6px 20px 0 rgba(33, 150, 243, 0.5);
        }}
        .download-btn:disabled {{
            opacity: 0.6;
            cursor: not-allowed;
            transform: none;
        }}
        .status-message {{
            margin-top: 15px;
            padding: 10px;
            border-radius: 5px;
            text-align: center;
        }}
        .success {{ background-color: rgba(76, 175, 80, 0.3); }}
        .error {{ background-color: rgba(244, 67, 54, 0.3); }}
        .loading {{ background-color: rgba(255, 193, 7, 0.3); }}
        ul {{ padding: 0; list-style: none; }}
        .file-item {{
            background: rgba(255, 255, 255, 0.1);
            margin: 5px 0;
            padding: 8px 12px;
            border-radius: 5px;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>🎉 {}</h1>
        <div class="export-info">
            <h3>📦 Export Type: {}</h3>
            <p>📅 Created: {}</p>
            <p>📁 Files: {} items</p>
            <h4>📋 Included Files:</h4>
            <ul>{}</ul>
        </div>
        <div style="text-align: center;">
            <button id="downloadBtn" onclick="downloadFile()" class="download-btn">
                ⬇️ Download Now
            </button>
            <div id="statusMessage" class="status-message" style="display: none;"></div>
        </div>
    </div>

    <script>
        const downloadBtn = document.getElementById('downloadBtn');
        const statusMessage = document.getElementById('statusMessage');

        async function downloadFile() {{
            downloadBtn.disabled = true;
            downloadBtn.textContent = '⏳ Downloading...';
            showStatus('Preparing download...', 'loading');

            try {{
                // MCP-UI 표준 Tool Call Action으로 다운로드 요청
                window.parent.postMessage({{
                    type: 'tool',
                    payload: {{
                        toolName: 'download_workspace_file',
                        params: {{
                            filePath: '{}'
                        }}
                    }}
                }}, '*');

                showStatus('Download request sent!', 'success');
            }} catch (error) {{
                console.error('Download failed:', error);
                showStatus('Download failed: ' + error.message, 'error');
                resetButton();
            }}
        }}

        function showStatus(message, type) {{
            statusMessage.textContent = message;
            statusMessage.className = 'status-message ' + type;
            statusMessage.style.display = 'block';
        }}

        function resetButton() {{
            downloadBtn.disabled = false;
            downloadBtn.textContent = '⬇️ Download Now';
        }}

        // Listen for download completion from parent
        window.addEventListener('message', function(event) {{
            // Security: Only accept messages from the parent window
            if (event.source !== window.parent) {{
                return;
            }}

            if (event.data.type === 'download_complete') {{
                if (event.data.success) {{
                    showStatus('✅ Download completed successfully!', 'success');
                }} else {{
                    showStatus('❌ Download failed: ' + event.data.error, 'error');
                }}
                resetButton();
            }}
        }});
    </script>
</body>
</html>"#,
            html_escape::encode_text(title),
            html_escape::encode_text(title),
            html_escape::encode_text(export_type),
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            files.len(),
            files_list,
            html_escape::encode_text(download_path)
        )
    }

    /// Export 디렉토리 초기화
    fn ensure_exports_directory(&self) -> Result<std::path::PathBuf, String> {
        let exports_dir = self.get_workspace_dir().join("exports");

        // files 및 packages 하위 디렉토리 생성
        let files_dir = exports_dir.join("files");
        let packages_dir = exports_dir.join("packages");

        for dir in [&exports_dir, &files_dir, &packages_dir] {
            if !dir.exists() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| format!("Failed to create directory {:?}: {}", dir, e))?;
            }
        }

        Ok(exports_dir)
    }

    async fn handle_export_file(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        // 파라미터 파싱
        let path = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) => path,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: path",
                );
            }
        };
        let display_name = args
            .get("display_name")
            .and_then(|v| v.as_str())
            .unwrap_or(path)
            .to_string();

        // 소스 파일 경로 검증
        let source_path = self.get_workspace_dir().join(path);
        if !source_path.exists() || !source_path.is_file() {
            return Self::error_response(
                request_id,
                -32603,
                "File not found or is not a regular file",
            );
        }

        // exports 디렉토리 준비
        let exports_dir = match self.ensure_exports_directory() {
            Ok(dir) => dir,
            Err(e) => return Self::error_response(request_id, -32603, &e),
        };

        // 고유한 export 파일명 생성
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let file_stem = source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let file_ext = source_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let export_filename = if file_ext.is_empty() {
            format!("{}_{}", file_stem, timestamp)
        } else {
            format!("{}_{}.{}", file_stem, timestamp, file_ext)
        };

        // 파일 복사
        let export_path = exports_dir.join("files").join(&export_filename);
        if let Err(e) = std::fs::copy(&source_path, &export_path) {
            return Self::error_response(
                request_id,
                -32603,
                &format!("Failed to copy file: {}", e),
            );
        }

        let relative_path = format!("exports/files/{}", export_filename);
        let source_path_str = path;

        // Generate unique request_id as u64 for UIResource
        let uid = cuid2::create_id();
        let ui_request_id: u64 = uid
            .chars()
            .filter_map(|c| c.to_digit(36))
            .fold(0u64, |acc, d| acc.wrapping_mul(36).wrapping_add(d as u64));

        // HTML UIResource 생성
        let html_content = self.create_html_export_ui(
            &format!("File Export: {}", display_name),
            &[source_path_str.to_string()],
            "Single File",
            &relative_path,
            &display_name,
        );

        let ui_resource = self.create_export_ui_resource(
            ui_request_id,
            &format!("File Export: {}", display_name),
            &[source_path_str.to_string()],
            "Single File",
            &relative_path,
            html_content,
        );

        let success_message = format!(
            "파일 '{}'이(가) 성공적으로 export되었습니다. 아래 링크에서 다운로드할 수 있습니다.",
            display_name
        );

        Self::success_response_with_text_and_resource(request_id, &success_message, ui_resource)
    }

    async fn handle_export_zip(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        // 파라미터 파싱
        let files_array = match args.get("files").and_then(|v| v.as_array()) {
            Some(files) => files,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: files (array)",
                );
            }
        };
        let package_name = args
            .get("package_name")
            .and_then(|v| v.as_str())
            .unwrap_or("workspace_export")
            .to_string();

        if files_array.is_empty() {
            return Self::error_response(request_id, -32602, "Files array cannot be empty");
        }

        // exports 디렉토리 준비
        let exports_dir = match self.ensure_exports_directory() {
            Ok(dir) => dir,
            Err(e) => return Self::error_response(request_id, -32603, &e),
        };

        // 고유한 ZIP 파일명 생성
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let zip_filename = format!("{}_{}.zip", package_name, timestamp);
        let zip_path = exports_dir.join("packages").join(&zip_filename);

        // ZIP 파일 생성
        let zip_file = match std::fs::File::create(&zip_path) {
            Ok(file) => file,
            Err(e) => {
                return Self::error_response(
                    request_id,
                    -32603,
                    &format!("Failed to create ZIP file: {}", e),
                )
            }
        };

        let mut zip = zip::ZipWriter::new(zip_file);
        let options = FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        // 파일들을 ZIP에 추가
        let mut processed_files = Vec::new();
        for file_value in files_array {
            let file_path = match file_value.as_str() {
                Some(path) => path,
                None => continue,
            };

            let source_path = self.get_workspace_dir().join(file_path);
            if !source_path.exists() || !source_path.is_file() {
                continue; // 존재하지 않는 파일은 건너뛰기
            }

            // ZIP 내부 경로 설정 (디렉토리 구조 유지)
            let archive_path = file_path.replace("\\", "/");

            match zip.start_file(&archive_path, options) {
                Ok(_) => {} // 성공적으로 파일 시작
                Err(e) => {
                    error!("Failed to start file in ZIP: {}", e);
                    continue;
                }
            }

            match std::fs::read(&source_path) {
                Ok(content) => {
                    if let Err(e) = zip.write_all(&content) {
                        error!("Failed to write file content to ZIP: {}", e);
                        continue;
                    }
                    processed_files.push(file_path.to_string());
                }
                Err(e) => {
                    error!("Failed to read file {}: {}", file_path, e);
                    continue;
                }
            }
        }

        // ZIP 파일 완료
        if let Err(e) = zip.finish() {
            return Self::error_response(
                request_id,
                -32603,
                &format!("Failed to finalize ZIP: {}", e),
            );
        }

        if processed_files.is_empty() {
            return Self::error_response(
                request_id,
                -32603,
                "No files were successfully added to ZIP",
            );
        }

        let relative_path = format!("exports/packages/{}", zip_filename);

        // Generate unique request_id as u64 for UIResource
        let uid = cuid2::create_id();
        let ui_request_id: u64 = uid
            .chars()
            .filter_map(|c| c.to_digit(36))
            .fold(0u64, |acc, d| acc.wrapping_mul(36).wrapping_add(d as u64));

        // HTML UIResource 생성
        let html_content = self.create_html_export_ui(
            &format!("ZIP Package: {}", package_name),
            &processed_files,
            "ZIP Package",
            &relative_path,
            &zip_filename,
        );

        let ui_resource = self.create_export_ui_resource(
            ui_request_id,
            &format!("ZIP Package: {}", package_name),
            &processed_files,
            "ZIP Package",
            &relative_path,
            html_content,
        );

        let success_message = format!(
            "ZIP 패키지 '{}'이(가) 성공적으로 생성되었습니다. {}개 파일이 포함되어 있으며, 아래 링크에서 다운로드할 수 있습니다.",
            package_name,
            processed_files.len()
        );

        Self::success_response_with_text_and_resource(request_id, &success_message, ui_resource)
    }

    fn create_export_file_tool() -> MCPTool {
        let mut props = HashMap::new();
        props.insert(
            "path".to_string(),
            string_prop(Some(1), Some(1000), Some("Workspace 내 export할 파일 경로")),
        );
        props.insert(
            "display_name".to_string(),
            string_prop(None, None, Some("다운로드시 표시할 파일명 (선택적)")),
        );
        props.insert(
            "description".to_string(),
            string_prop(None, None, Some("파일 설명 (선택적)")),
        );

        MCPTool {
            name: "export_file".to_string(),
            title: Some("Export Single File".to_string()),
            description: "Export a single file from workspace for download with interactive UI"
                .to_string(),
            input_schema: object_schema(props, vec!["path".to_string()]),
            output_schema: None,
            annotations: None,
        }
    }

    fn create_export_zip_tool() -> MCPTool {
        let mut props = HashMap::new();
        props.insert(
            "files".to_string(),
            array_schema(
                string_prop(Some(1), Some(1000), None),
                Some("Export할 파일 경로들의 배열"),
            ),
        );
        props.insert(
            "package_name".to_string(),
            string_prop(
                None,
                Some(50),
                Some("ZIP 패키지명 (선택적, 기본값: workspace_export)"),
            ),
        );
        props.insert(
            "description".to_string(),
            string_prop(None, None, Some("패키지 설명 (선택적)")),
        );

        MCPTool {
            name: "export_zip".to_string(),
            title: Some("Export ZIP Package".to_string()),
            description: "Export multiple files as a ZIP package for download with interactive UI"
                .to_string(),
            input_schema: object_schema(props, vec!["files".to_string()]),
            output_schema: None,
            annotations: None,
        }
    }
}

#[async_trait]
impl BuiltinMCPServer for WorkspaceServer {
    fn name(&self) -> &str {
        "workspace"
    }

    fn description(&self) -> &str {
        "Integrated workspace for file operations and code execution"
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            // 파일 작업 도구들
            Self::create_read_file_tool(),
            Self::create_write_file_tool(),
            Self::create_list_directory_tool(),
            Self::create_search_files_tool(),
            Self::create_replace_lines_in_file_tool(),
            Self::create_grep_tool(),
            // 코드 실행 도구들
            Self::create_execute_python_tool(),
            Self::create_execute_typescript_tool(),
            Self::create_execute_shell_tool(),
            // Export 도구들
            Self::create_export_file_tool(),
            Self::create_export_zip_tool(),
        ]
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse {
        match tool_name {
            // 파일 작업 도구들
            "read_file" => self.handle_read_file(args).await,
            "write_file" => self.handle_write_file(args).await,
            "list_directory" => self.handle_list_directory(args).await,
            "search_files" => self.handle_search_files(args).await,
            "replace_lines_in_file" => self.handle_replace_lines_in_file(args).await,
            "grep" => self.handle_grep(args).await,
            // 코드 실행 도구들
            "execute_python" => self.handle_execute_python(args).await,
            "execute_typescript" => self.handle_execute_typescript(args).await,
            "execute_shell" => self.handle_execute_shell(args).await,
            // Export 도구들
            "export_file" => self.handle_export_file(args).await,
            "export_zip" => self.handle_export_zip(args).await,
            _ => {
                let request_id = Self::generate_request_id();
                Self::error_response(
                    request_id,
                    -32601,
                    &format!("Tool '{}' not found", tool_name),
                )
            }
        }
    }
}
