use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::fs;
use tracing::{error, info};

use crate::mcp::{JSONSchema, JSONSchemaType, MCPError, MCPResponse, MCPTool};
use super::{BuiltinMCPServer, utils::{SecurityValidator, constants::MAX_FILE_SIZE}};

pub struct FilesystemServer {
    security: SecurityValidator,
}

impl FilesystemServer {
    pub fn new() -> Self {
        Self {
            security: SecurityValidator::default(),
        }
    }
    
    fn create_read_file_tool() -> MCPTool {
        MCPTool {
            name: "read_file".to_string(),
            title: Some("Read File".to_string()),
            description: "Read the contents of a file".to_string(),
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
        
        // Read file
        match fs::read_to_string(&safe_path).await {
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
                    message: format!("Content too large: {} bytes (max: {} bytes)", content.len(), MAX_FILE_SIZE),
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
        
        let path_str = args.get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");
        
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
                
                info!("Successfully listed directory: {:?} ({} items)", safe_path, items.len());
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
        ]
    }
    
    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse {
        match tool_name {
            "read_file" => self.handle_read_file(args).await,
            "write_file" => self.handle_write_file(args).await,
            "list_directory" => self.handle_list_directory(args).await,
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