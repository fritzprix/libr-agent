use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::fs;
use tracing::{error, info};

use super::{
    utils::{constants::MAX_FILE_SIZE, SecurityValidator},
    BuiltinMCPServer,
};
use crate::mcp::{JSONSchema, JSONSchemaType, MCPError, MCPResponse, MCPTool};

pub struct FilesystemServer {
    security: SecurityValidator,
}

impl FilesystemServer {
    pub fn new() -> Self {
        // 현재 작업 디렉터리 로그 (확인용)
        match std::env::current_dir() {
            Ok(dir) => info!("FilesystemServer starting with CWD = {:?}", dir),
            Err(e) => error!("Failed to read current_dir: {}", e),
        }

        let security_validator = SecurityValidator::new();

        // SecurityValidator의 base_dir 확인
        info!(
            "FilesystemServer using base_dir: {:?}",
            security_validator.base_dir()
        );

        Self {
            security: security_validator,
        }
    }

    fn create_read_file_tool() -> MCPTool {
        MCPTool {
            name: "read_file".to_string(),
            title: Some("Read File".to_string()),
            description: "Read the contents of a file, optionally specifying line ranges"
                .to_string(),
            input_schema: JSONSchema {
                schema_type: JSONSchemaType::Object {
                    properties: Some({
                        let mut props = HashMap::new();
                        props.insert(
                            "path".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::String {
                                    min_length: Some(1),
                                    max_length: Some(1000),
                                    pattern: None,
                                    format: None,
                                },
                                title: None,
                                description: Some("Path to the file to read".to_string()),
                                default: None,
                                examples: None,
                                enum_values: None,
                                const_value: None,
                            },
                        );
                        props.insert(
                            "start_line".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::Integer {
                                    minimum: Some(1),
                                    maximum: None,
                                    exclusive_minimum: None,
                                    exclusive_maximum: None,
                                    multiple_of: None,
                                },
                                title: None,
                                description: Some(
                                    "Starting line number (1-based, optional)".to_string(),
                                ),
                                default: None,
                                examples: None,
                                enum_values: None,
                                const_value: None,
                            },
                        );
                        props.insert(
                            "end_line".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::Integer {
                                    minimum: Some(1),
                                    maximum: None,
                                    exclusive_minimum: None,
                                    exclusive_maximum: None,
                                    multiple_of: None,
                                },
                                title: None,
                                description: Some(
                                    "Ending line number (1-based, optional)".to_string(),
                                ),
                                default: None,
                                examples: None,
                                enum_values: None,
                                const_value: None,
                            },
                        );
                        props
                    }),
                    required: Some(vec!["path".to_string()]),
                    additional_properties: Some(false),
                    min_properties: None,
                    max_properties: None,
                },
                title: None,
                description: None,
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
            output_schema: None,
            annotations: None,
        }
    }

    fn create_write_file_tool() -> MCPTool {
        MCPTool {
            name: "write_file".to_string(),
            title: Some("Write File".to_string()),
            description: "Write content to a file".to_string(),
            input_schema: JSONSchema {
                schema_type: JSONSchemaType::Object {
                    properties: Some({
                        let mut props = HashMap::new();
                        props.insert(
                            "path".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::String {
                                    min_length: Some(1),
                                    max_length: Some(1000),
                                    pattern: None,
                                    format: None,
                                },
                                title: None,
                                description: Some("Path to the file to write".to_string()),
                                default: None,
                                examples: None,
                                enum_values: None,
                                const_value: None,
                            },
                        );
                        props.insert(
                            "content".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::String {
                                    min_length: None,
                                    max_length: Some(MAX_FILE_SIZE as u32),
                                    pattern: None,
                                    format: None,
                                },
                                title: None,
                                description: Some("Content to write to the file".to_string()),
                                default: None,
                                examples: None,
                                enum_values: None,
                                const_value: None,
                            },
                        );
                        props
                    }),
                    required: Some(vec!["path".to_string(), "content".to_string()]),
                    additional_properties: Some(false),
                    min_properties: None,
                    max_properties: None,
                },
                title: None,
                description: None,
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
            output_schema: None,
            annotations: None,
        }
    }

    fn create_list_directory_tool() -> MCPTool {
        MCPTool {
            name: "list_directory".to_string(),
            title: Some("List Directory".to_string()),
            description: "List contents of a directory".to_string(),
            input_schema: JSONSchema {
                schema_type: JSONSchemaType::Object {
                    properties: Some({
                        let mut props = HashMap::new();
                        props.insert(
                            "path".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::String {
                                    min_length: Some(1),
                                    max_length: Some(1000),
                                    pattern: None,
                                    format: None,
                                },
                                title: None,
                                description: Some("Path to the directory to list".to_string()),
                                default: Some(json!(".")),
                                examples: None,
                                enum_values: None,
                                const_value: None,
                            },
                        );
                        props
                    }),
                    required: Some(vec!["path".to_string()]),
                    additional_properties: Some(false),
                    min_properties: None,
                    max_properties: None,
                },
                title: None,
                description: None,
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
            output_schema: None,
            annotations: None,
        }
    }

    fn create_search_files_tool() -> MCPTool {
        MCPTool {
            name: "search_files".to_string(),
            title: Some("Search Files".to_string()),
            description: "Search for files matching patterns with various filters".to_string(),
            input_schema: JSONSchema {
                schema_type: JSONSchemaType::Object {
                    properties: Some({
                        let mut props = HashMap::new();
                        props.insert(
                            "pattern".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::String {
                                    min_length: Some(1),
                                    max_length: Some(500),
                                    pattern: None,
                                    format: None,
                                },
                                title: None,
                                description: Some(
                                    "Glob pattern to match files (e.g., '*.rs', '**/*.tsx')"
                                        .to_string(),
                                ),
                                default: None,
                                examples: Some(vec![
                                    json!("*.rs"),
                                    json!("**/*.tsx"),
                                    json!("src/**/*.ts"),
                                ]),
                                enum_values: None,
                                const_value: None,
                            },
                        );
                        props.insert(
                            "path".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::String {
                                    min_length: Some(1),
                                    max_length: Some(1000),
                                    pattern: None,
                                    format: None,
                                },
                                title: None,
                                description: Some("Root path to search from".to_string()),
                                default: Some(json!(".")),
                                examples: None,
                                enum_values: None,
                                const_value: None,
                            },
                        );
                        props.insert(
                            "max_depth".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::Integer {
                                    minimum: Some(1),
                                    maximum: Some(50),
                                    exclusive_minimum: None,
                                    exclusive_maximum: None,
                                    multiple_of: None,
                                },
                                title: None,
                                description: Some("Maximum depth to search (optional)".to_string()),
                                default: None,
                                examples: None,
                                enum_values: None,
                                const_value: None,
                            },
                        );
                        props.insert(
                            "file_type".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::String {
                                    min_length: None,
                                    max_length: None,
                                    pattern: None,
                                    format: None,
                                },
                                title: None,
                                description: Some(
                                    "Filter by file type: 'file', 'dir', or 'both'".to_string(),
                                ),
                                default: Some(json!("both")),
                                examples: None,
                                enum_values: Some(vec![json!("file"), json!("dir"), json!("both")]),
                                const_value: None,
                            },
                        );
                        props
                    }),
                    required: Some(vec!["pattern".to_string()]),
                    additional_properties: Some(false),
                    min_properties: None,
                    max_properties: None,
                },
                title: None,
                description: None,
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
            output_schema: None,
            annotations: None,
        }
    }

    async fn handle_read_file(&self, args: Value) -> MCPResponse {
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
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "start_line must be less than or equal to end_line".to_string(),
                        data: None,
                    }),
                };
            }
        }

        // Validate path security
        let safe_path = match self.security.validate_path(path_str) {
            Ok(path) => path,
            Err(e) => {
                error!("Path validation failed: {}", e);
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Security error: {}", e),
                        data: None,
                    }),
                };
            }
        };

        // Check file size
        if let Err(e) = self.security.validate_file_size(&safe_path, MAX_FILE_SIZE) {
            error!("File size validation failed: {}", e);
            return MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32603,
                    message: format!("File size error: {}", e),
                    data: None,
                }),
            };
        }

        // Read file with line range support
        let content = if start_line.is_some() || end_line.is_some() {
            self.read_file_lines(&safe_path, start_line, end_line).await
        } else {
            fs::read_to_string(&safe_path)
                .await
                .map_err(|e| e.to_string())
        };

        match content {
            Ok(content) => {
                info!("Successfully read file: {:?}", safe_path);
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: Some(json!({
                        "content": [{
                            "type": "text",
                            "text": content
                        }]
                    })),
                    error: None,
                }
            }
            Err(e) => {
                error!("Failed to read file {:?}: {}", safe_path, e);
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Failed to read file: {}", e),
                        data: None,
                    }),
                }
            }
        }
    }

    async fn read_file_lines(
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

        // Validate search path security
        let safe_path = match self.security.validate_path(search_path) {
            Ok(path) => path,
            Err(e) => {
                error!("Path validation failed: {}", e);
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Security error: {}", e),
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
                        "No files found matching pattern '{}' in '{}'",
                        pattern, search_path
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
                        message: format!("Search failed: {}", e),
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

        let glob_pattern = Pattern::new(pattern).map_err(|e| format!("Invalid pattern: {}", e))?;
        let mut results = Vec::new();

        let walker = if let Some(depth) = max_depth {
            WalkDir::new(root_path).max_depth(depth)
        } else {
            WalkDir::new(root_path)
        };

        for entry in walker {
            let entry = entry.map_err(|e| format!("Walk error: {}", e))?;
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
                        .map_err(|e| format!("Metadata error: {}", e))?;

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

        // Validate path security
        let safe_path = match self.security.validate_path(path_str) {
            Ok(path) => path,
            Err(e) => {
                error!("Path validation failed: {}", e);
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Security error: {}", e),
                        data: None,
                    }),
                };
            }
        };

        // Check content size
        if content.len() > MAX_FILE_SIZE {
            return MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32603,
                    message: format!(
                        "Content too large: {} bytes (max: {} bytes)",
                        content.len(),
                        MAX_FILE_SIZE
                    ),
                    data: None,
                }),
            };
        }

        // Create parent directory if it doesn't exist
        if let Some(parent) = safe_path.parent() {
            if let Err(e) = fs::create_dir_all(parent).await {
                error!("Failed to create parent directory {:?}: {}", parent, e);
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Failed to create parent directory: {}", e),
                        data: None,
                    }),
                };
            }
        }

        // Write file
        match fs::write(&safe_path, content).await {
            Ok(()) => {
                info!("Successfully wrote file: {:?}", safe_path);
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
                error!("Failed to write file {:?}: {}", safe_path, e);
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Failed to write file: {}", e),
                        data: None,
                    }),
                }
            }
        }
    }

    async fn handle_list_directory(&self, args: Value) -> MCPResponse {
        let request_id = Value::String(uuid::Uuid::new_v4().to_string());

        let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        // Validate path security
        let safe_path = match self.security.validate_path(path_str) {
            Ok(path) => path,
            Err(e) => {
                error!("Path validation failed: {}", e);
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Security error: {}", e),
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
                        message: format!("Failed to list directory: {}", e),
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
        "builtin.filesystem"
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
        ]
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse {
        match tool_name {
            "read_file" => self.handle_read_file(args).await,
            "write_file" => self.handle_write_file(args).await,
            "list_directory" => self.handle_list_directory(args).await,
            "search_files" => self.handle_search_files(args).await,
            _ => {
                let request_id = Value::String(uuid::Uuid::new_v4().to_string());
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32601,
                        message: format!("Tool '{}' not found in filesystem server", tool_name),
                        data: None,
                    }),
                }
            }
        }
    }
}

impl Default for FilesystemServer {
    fn default() -> Self {
        Self::new()
    }
}
