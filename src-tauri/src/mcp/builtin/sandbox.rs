use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use tempfile::TempDir;
use tokio::fs;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{error, info, warn};
use uuid::Uuid;

use super::{
    utils::constants::{DEFAULT_EXECUTION_TIMEOUT, MAX_CODE_SIZE, MAX_EXECUTION_TIMEOUT},
    BuiltinMCPServer,
};
use crate::mcp::{JSONSchema, JSONSchemaType, MCPError, MCPResponse, MCPTool};

pub struct SandboxServer;

impl SandboxServer {
    pub fn new() -> Self {
        Self
    }

    /// 작업 디렉터리 결정 (FilesystemServer와 동일한 로직)
    fn determine_execution_working_dir() -> std::path::PathBuf {
        use std::path::PathBuf;

        if let Ok(root) = std::env::var("SYNAPTICFLOW_PROJECT_ROOT") {
            PathBuf::from(root)
        } else {
            // FilesystemServer와 동일한 앱 워크스페이스 사용
            let tmp = std::env::temp_dir().join("synaptic-flow");

            // 디렉터리 생성 확인
            if let Err(e) = std::fs::create_dir_all(&tmp) {
                tracing::error!("Failed to create sandbox workspace: {:?}: {}", tmp, e);
            }

            info!("Using sandbox app workspace: {:?}", tmp);
            tmp
        }
    }

    fn create_execute_python_tool() -> MCPTool {
        MCPTool {
            name: "execute_python".to_string(),
            title: Some("Execute Python Code".to_string()),
            description: "Execute Python code in a sandboxed environment".to_string(),
            input_schema: JSONSchema {
                schema_type: JSONSchemaType::Object {
                    properties: Some({
                        let mut props = HashMap::new();
                        props.insert(
                            "code".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::String {
                                    min_length: Some(1),
                                    max_length: Some(MAX_CODE_SIZE as u32),
                                    pattern: None,
                                    format: None,
                                },
                                title: None,
                                description: Some("Python code to execute".to_string()),
                                default: None,
                                examples: Some(vec![json!("print('Hello, World!')")]),
                                enum_values: None,
                                const_value: None,
                            },
                        );
                        props.insert(
                            "timeout".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::Integer {
                                    minimum: Some(1),
                                    maximum: Some(MAX_EXECUTION_TIMEOUT as i64),
                                    exclusive_minimum: None,
                                    exclusive_maximum: None,
                                    multiple_of: None,
                                },
                                title: None,
                                description: Some("Timeout in seconds (default: 30)".to_string()),
                                default: Some(json!(DEFAULT_EXECUTION_TIMEOUT)),
                                examples: None,
                                enum_values: None,
                                const_value: None,
                            },
                        );
                        props
                    }),
                    required: Some(vec!["code".to_string()]),
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

    fn create_execute_typescript_tool() -> MCPTool {
        MCPTool {
            name: "execute_typescript".to_string(),
            title: Some("Execute TypeScript Code".to_string()),
            description: "Execute TypeScript code in a sandboxed environment using ts-node"
                .to_string(),
            input_schema: JSONSchema {
                schema_type: JSONSchemaType::Object {
                    properties: Some({
                        let mut props = HashMap::new();
                        props.insert(
                            "code".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::String {
                                    min_length: Some(1),
                                    max_length: Some(MAX_CODE_SIZE as u32),
                                    pattern: None,
                                    format: None,
                                },
                                title: None,
                                description: Some("TypeScript code to execute".to_string()),
                                default: None,
                                examples: Some(vec![json!("console.log('Hello, World!');")]),
                                enum_values: None,
                                const_value: None,
                            },
                        );
                        props.insert(
                            "timeout".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::Integer {
                                    minimum: Some(1),
                                    maximum: Some(MAX_EXECUTION_TIMEOUT as i64),
                                    exclusive_minimum: None,
                                    exclusive_maximum: None,
                                    multiple_of: None,
                                },
                                title: None,
                                description: Some("Timeout in seconds (default: 30)".to_string()),
                                default: Some(json!(DEFAULT_EXECUTION_TIMEOUT)),
                                examples: None,
                                enum_values: None,
                                const_value: None,
                            },
                        );
                        props
                    }),
                    required: Some(vec!["code".to_string()]),
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

    fn create_execute_shell_tool() -> MCPTool {
        MCPTool {
            name: "execute_shell".to_string(),
            title: Some("Execute Shell Command".to_string()),
            description: "Execute a shell command in the current environment".to_string(),
            input_schema: JSONSchema {
                schema_type: JSONSchemaType::Object {
                    properties: Some({
                        let mut props = HashMap::new();
                        props.insert(
                            "command".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::String {
                                    min_length: Some(1),
                                    max_length: Some(1000),
                                    pattern: None,
                                    format: None,
                                },
                                title: None,
                                description: Some("Shell command to execute".to_string()),
                                default: None,
                                examples: Some(vec![json!("ls -la"), json!("grep -r 'pattern' .")]),
                                enum_values: None,
                                const_value: None,
                            },
                        );
                        props.insert(
                            "timeout".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::Integer {
                                    minimum: Some(1),
                                    maximum: Some(300), // 5분 최대
                                    exclusive_minimum: None,
                                    exclusive_maximum: None,
                                    multiple_of: None,
                                },
                                title: None,
                                description: Some("Timeout in seconds (default: 30)".to_string()),
                                default: Some(json!(30)),
                                examples: None,
                                enum_values: None,
                                const_value: None,
                            },
                        );
                        props.insert(
                            "working_dir".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::String {
                                    min_length: Some(1),
                                    max_length: Some(1000),
                                    pattern: None,
                                    format: None,
                                },
                                title: None,
                                description: Some(
                                    "Working directory for command execution (optional)"
                                        .to_string(),
                                ),
                                default: None,
                                examples: None,
                                enum_values: None,
                                const_value: None,
                            },
                        );
                        props
                    }),
                    required: Some(vec!["command".to_string()]),
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

    async fn execute_code_in_sandbox(
        &self,
        command: &str,
        args: &[&str],
        code: &str,
        file_extension: &str,
        timeout_secs: u64,
    ) -> MCPResponse {
        let request_id = Value::String(uuid::Uuid::new_v4().to_string());

        // Validate code size
        if code.len() > MAX_CODE_SIZE {
            return MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32603,
                    message: format!(
                        "Code too large: {} bytes (max: {} bytes)",
                        code.len(),
                        MAX_CODE_SIZE
                    ),
                    data: None,
                }),
            };
        }

        // Create temporary directory for sandboxed execution
        let temp_dir = match TempDir::new() {
            Ok(dir) => dir,
            Err(e) => {
                error!("Failed to create temporary directory: {}", e);
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Failed to create sandbox: {e}"),
                        data: None,
                    }),
                };
            }
        };

        // Write code to temporary file
        let script_path = temp_dir.path().join(format!("script{file_extension}"));
        if let Err(e) = fs::write(&script_path, code).await {
            error!("Failed to write script file: {}", e);
            return MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32603,
                    message: format!("Failed to write script: {e}"),
                    data: None,
                }),
            };
        }

        // Prepare command with arguments
        let mut cmd = Command::new(command);
        for arg in args {
            cmd.arg(arg);
        }
        cmd.arg(&script_path);

        // 결정된 작업 디렉터리 사용 (FilesystemServer와 동일한 규칙)
        let work_dir = Self::determine_execution_working_dir();
        info!("Sandbox execution working dir: {:?}", work_dir);
        cmd.current_dir(&work_dir);

        // Clear environment variables for isolation
        cmd.env_clear();
        cmd.env("PATH", std::env::var("PATH").unwrap_or_default());

        // HOME은 작업 디렉터리로 설정
        if let Some(home_str) = work_dir.to_str() {
            cmd.env("HOME", home_str);
        }

        // 임시 관련 변수는 temp_dir로 설정 (실제 임시 파일은 여기에)
        if let Some(tmp_str) = temp_dir.path().to_str() {
            cmd.env("TMPDIR", tmp_str);
            cmd.env("TEMP", tmp_str);
        }

        // Kill command timeout
        let timeout_duration = Duration::from_secs(timeout_secs.min(MAX_EXECUTION_TIMEOUT));

        // Execute command with timeout
        let execution_result = timeout(timeout_duration, cmd.output()).await;

        match execution_result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let success = output.status.success();

                let result_text = if success {
                    if stdout.trim().is_empty() && stderr.trim().is_empty() {
                        "Code executed successfully (no output)".to_string()
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
                    "Code execution completed. Success: {}, Output length: {}",
                    success,
                    result_text.len()
                );

                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: Some(json!({
                        "content": [{
                            "type": "text",
                            "text": result_text
                        }],
                        "isError": !success
                    })),
                    error: None,
                }
            }
            Ok(Err(e)) => {
                error!("Failed to execute command: {}", e);
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Execution error: {e}"),
                        data: None,
                    }),
                }
            }
            Err(_) => {
                warn!("Code execution timed out after {} seconds", timeout_secs);
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Execution timed out after {timeout_secs} seconds"),
                        data: None,
                    }),
                }
            }
        }
    }

    async fn handle_execute_python(&self, args: Value) -> MCPResponse {
        let request_id = Value::String(uuid::Uuid::new_v4().to_string());

        let code = match args.get("code").and_then(|v| v.as_str()) {
            Some(code) => code,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter: code".to_string(),
                        data: None,
                    }),
                };
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
        let request_id = Value::String(Uuid::new_v4().to_string());

        let code = match args.get("code").and_then(|v| v.as_str()) {
            Some(code) => code,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter: code".to_string(),
                        data: None,
                    }),
                };
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
            return MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32603,
                    message: "Invalid UTF-8 encoding in code".to_string(),
                    data: None,
                }),
            };
        }

        // 코드 크기 검증 (기존과 동일)
        if code.len() > MAX_CODE_SIZE {
            return MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32603,
                    message: format!(
                        "Code too large: {} bytes (max: {} bytes)",
                        code.len(),
                        MAX_CODE_SIZE
                    ),
                    data: None,
                }),
            };
        }

        // Deno 설치 확인
        let deno_check = Command::new("which").arg("deno").output().await;
        if deno_check.is_err() || !deno_check.unwrap().status.success() {
            error!("Deno not found on system");
            return MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32603,
                    message: "Deno is required for TypeScript execution.\\n\\n\
                        To install Deno automatically, run:\\n\
                        curl -fsSL https://deno.land/install.sh | sh\\n\\n\
                        Or using package managers:\\n\
                        - macOS: brew install deno\\n\
                        - Windows: winget install deno\\n\
                        - Linux: curl -fsSL https://deno.land/install.sh | sh\\n\\n\
                        After installation, restart the application.".to_string(),
                    data: None,
                }),
            };
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
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Failed to create temp directory: {e}"),
                        data: None,
                    }),
                };
            }
        };

        let ts_file = temp_dir.path().join("script.ts");

        // 코드 파일 쓰기 (직렬화 검증 후)
        if let Err(e) = fs::write(&ts_file, code).await {
            error!("Failed to write TypeScript file: {}", e);
            return MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32603,
                    message: format!("Failed to write TypeScript file: {e}"),
                    data: None,
                }),
            };
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
        let work_dir = Self::determine_execution_working_dir();
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
                        format!("Output:\\n{}", stdout.trim())
                    } else {
                        format!(
                            "Output:\\n{}\\n\\nWarnings/Errors:\\n{}",
                            stdout.trim(),
                            stderr.trim()
                        )
                    }
                } else {
                    format!(
                        "Execution failed (exit code: {}):\\n{}",
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

                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: Some(json!({
                        "content": [{
                            "type": "text",
                            "text": result_text
                        }],
                        "isError": !success
                    })),
                    error: None,
                }
            }
            Ok(Err(e)) => {
                error!("Failed to execute with Deno: {}", e);
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Deno execution error: {e}"),
                        data: None,
                    }),
                }
            }
            Err(_) => {
                warn!(
                    "TypeScript execution timed out after {} seconds",
                    timeout_secs
                );
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Execution timed out after {timeout_secs} seconds"),
                        data: None,
                    }),
                }
            }
        }
    }

    async fn handle_execute_shell(&self, args: Value) -> MCPResponse {
        let request_id = Value::String(uuid::Uuid::new_v4().to_string());

        let command_str = match args.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter: command".to_string(),
                        data: None,
                    }),
                };
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
            Self::determine_execution_working_dir()
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

                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: Some(json!({
                        "content": [{
                            "type": "text",
                            "text": result_text
                        }],
                        "isError": !success
                    })),
                    error: None,
                }
            }
            Ok(Err(e)) => {
                error!("Failed to execute shell command '{}': {}", command_str, e);
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Execution error: {e}"),
                        data: None,
                    }),
                }
            }
            Err(_) => {
                error!(
                    "Shell command '{}' timed out after {} seconds",
                    command_str, timeout_secs
                );
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Command timed out after {timeout_secs} seconds"),
                        data: None,
                    }),
                }
            }
        }
    }
}

#[async_trait]
impl BuiltinMCPServer for SandboxServer {
    fn name(&self) -> &str {
        "sandbox"
    }

    fn description(&self) -> &str {
        "Built-in code execution sandbox server"
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            Self::create_execute_python_tool(),
            Self::create_execute_typescript_tool(),
            Self::create_execute_shell_tool(),
        ]
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse {
        match tool_name {
            "execute_python" => self.handle_execute_python(args).await,
            "execute_typescript" => self.handle_execute_typescript(args).await,
            "execute_shell" => self.handle_execute_shell(args).await,
            _ => {
                let request_id = Value::String(uuid::Uuid::new_v4().to_string());
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32601,
                        message: format!("Tool '{tool_name}' not found in sandbox server"),
                        data: None,
                    }),
                }
            }
        }
    }
}

impl Default for SandboxServer {
    fn default() -> Self {
        Self::new()
    }
}
