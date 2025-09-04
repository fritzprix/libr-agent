
use serde_json::Value;
use std::time::Duration;
use tempfile::TempDir;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{error, info, warn};

use super::utils::constants::*;
use super::{utils, WorkspaceServer};
use crate::mcp::MCPResponse;
use tokio::fs;

impl WorkspaceServer {
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
                    &format!("Failed to create temporary directory: {e}"),
                )
            }
        };

        // Write code to temporary file
        let script_path = temp_dir.path().join(format!("script{file_extension}"));
        if let Err(e) = fs::write(&script_path, code).await {
            return Self::error_response(
                request_id,
                -32603,
                &format!("Failed to write script file: {e}"),
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
                    serde_json::json!({
                        "success": true,
                        "stdout": stdout,
                        "stderr": stderr,
                        "exit_code": output.status.code()
                    })
                } else {
                    serde_json::json!({
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
                &format!("Failed to execute command: {e}"),
            ),
            Err(_) => Self::error_response(
                request_id,
                -32603,
                &format!("Command execution timed out after {timeout_secs} seconds"),
            ),
        }
    }

    pub async fn handle_execute_python(&self, args: Value) -> MCPResponse {
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

        let timeout_secs = utils::validate_timeout(args.get("timeout").and_then(|v| v.as_u64()));

        self.execute_code_in_sandbox("python3", &[], code, ".py", timeout_secs)
            .await
    }

    pub async fn handle_execute_typescript(&self, args: Value) -> MCPResponse {
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

        let timeout_secs = utils::validate_timeout(args.get("timeout").and_then(|v| v.as_u64()));

        if let Err(e) = std::str::from_utf8(code.as_bytes()) {
            error!("Invalid UTF-8 in TypeScript code: {}", e);
            return Self::error_response(request_id, -32603, "Invalid UTF-8 encoding in code");
        }

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

        if let Err(e) = fs::write(&ts_file, code).await {
            error!("Failed to write TypeScript file: {}", e);
            return Self::error_response(
                request_id,
                -32603,
                &format!("Failed to write TypeScript file: {e}"),
            );
        }

        let mut deno_cmd = Command::new("deno");
        deno_cmd
            .arg("run")
            .arg("--allow-read")
            .arg("--allow-write")
            .arg("--allow-net")
            .arg("--quiet")
            .arg(&ts_file);

        let work_dir = self.get_workspace_dir();
        deno_cmd.current_dir(&work_dir);

        deno_cmd.env_clear();
        deno_cmd.env("PATH", std::env::var("PATH").unwrap_or_default());
        if let Some(home_str) = work_dir.to_str() {
            deno_cmd.env("HOME", home_str);
        }

        let timeout_duration = Duration::from_secs(timeout_secs.min(MAX_EXECUTION_TIMEOUT));

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

    pub async fn handle_execute_shell(&self, args: Value) -> MCPResponse {
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

        let timeout_secs = utils::validate_timeout(args.get("timeout").and_then(|v| v.as_u64()));

        let working_dir = args.get("working_dir").and_then(|v| v.as_str());

        let work_dir = if let Some(dir) = working_dir {
            std::path::PathBuf::from(dir)
        } else {
            self.get_workspace_dir().to_path_buf()
        };

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
}
