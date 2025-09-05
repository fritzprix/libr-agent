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
    /// LLM이 생성한 Shell 명령어의 따옴표 문제를 자동으로 보정
    fn normalize_shell_command(raw_command: &str) -> String {
        let mut normalized = raw_command.to_string();

        // 1. 불완전한 따옴표 쌍 감지 및 보정
        let double_quote_count = normalized.chars().filter(|&c| c == '"').count();
        let single_quote_count = normalized.chars().filter(|&c| c == '\'').count();

        // 2. 홀수 개의 따옴표가 있으면 마지막에 추가
        if double_quote_count % 2 != 0 {
            normalized.push('"');
            info!("Shell command: Added missing double quote");
        }
        if single_quote_count % 2 != 0 {
            normalized.push('\'');
            info!("Shell command: Added missing single quote");
        }

        // 3. 연속된 따옴표 보정 패턴들
        // "echo "hello"" -> "echo \"hello\""
        if normalized.contains("\"\"") {
            normalized = Self::fix_consecutive_quotes(&normalized);
        }

        normalized
    }

    /// 연속된 따옴표를 문맥에 따라 보정
    fn fix_consecutive_quotes(input: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if i + 1 < chars.len() && chars[i] == '"' && chars[i + 1] == '"' {
                // 연속된 따옴표 발견
                if i > 0 && chars[i - 1] != ' ' && chars[i - 1] != '=' {
                    // 앞에 공백이나 등호가 없으면 첫 번째는 이스케이프
                    result.push('\\');
                    result.push('"');
                    i += 1; // 두 번째 따옴표는 다음 루프에서 처리
                } else if i + 2 < chars.len() && chars[i + 2] != ' ' {
                    // 뒤에 공백이 없으면 두 번째는 이스케이프
                    result.push('"');
                    result.push('\\');
                    result.push('"');
                    i += 2;
                } else {
                    // 기본적으로 첫 번째만 남기고 두 번째 제거
                    result.push('"');
                    i += 2;
                }
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }

        result
    }

    /// Python/TypeScript 코드의 문자열 문제를 보정
    fn normalize_code_string(raw_code: &str, language: &str) -> String {
        let mut normalized = raw_code.to_string();

        // 1. 불완전한 따옴표 쌍 감지 및 보정
        let double_quote_count = normalized.chars().filter(|&c| c == '"').count();
        let single_quote_count = normalized.chars().filter(|&c| c == '\'').count();

        // 2. 언어별 특수 처리
        match language {
            "python" => {
                // Python에서 홀수 개의 따옴표 보정
                if double_quote_count % 2 != 0 {
                    normalized.push('"');
                    info!("Python code: Added missing double quote");
                }
                if single_quote_count % 2 != 0 {
                    normalized.push('\'');
                    info!("Python code: Added missing single quote");
                }

            }
            "typescript" => {
                // TypeScript에서 홀수 개의 따옴표 보정
                if double_quote_count % 2 != 0 {
                    normalized.push('"');
                    info!("TypeScript code: Added missing double quote");
                }
                if single_quote_count % 2 != 0 {
                    normalized.push('\'');
                    info!("TypeScript code: Added missing single quote");
                }
            }
            _ => {
                // 일반적인 따옴표 균형 보정
                if double_quote_count % 2 != 0 {
                    normalized.push('"');
                    info!("Code: Added missing double quote");
                }
                if single_quote_count % 2 != 0 {
                    normalized.push('\'');
                    info!("Code: Added missing single quote");
                }
            }
        }

        normalized
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
                let success = output.status.success();
                let exit_code = output.status.code().unwrap_or(-1);

                // 표준 MCP 응답 형식으로 변환
                let result_text = if success {
                    if stdout.trim().is_empty() && stderr.trim().is_empty() {
                        "Code executed successfully (no output)".to_string()
                    } else if stderr.trim().is_empty() {
                        format!("Code executed successfully:\n{}", stdout.trim())
                    } else {
                        format!(
                            "Code executed successfully:\nOutput:\n{}\n\nWarnings/Errors:\n{}",
                            stdout.trim(),
                            stderr.trim()
                        )
                    }
                } else {
                    format!(
                        "Code execution failed (exit code: {}):\nSTDOUT:\n{}\n\nSTDERR:\n{}",
                        exit_code,
                        stdout.trim(),
                        stderr.trim()
                    )
                };

                info!(
                    "Code execution completed. Command: {}, Success: {}, Output length: {}",
                    command, success, result_text.len()
                );

                // 표준 MCP 형식으로 응답
                Self::success_response(request_id, &result_text)
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

        let raw_code = match args.get("code").and_then(|v| v.as_str()) {
            Some(code) => code,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: code",
                );
            }
        };

        // LLM 입력 정규화
        let normalized_code = Self::normalize_code_string(raw_code, "python");

        // 보정이 발생한 경우 로그 출력
        if normalized_code != raw_code {
            info!("Python code normalized for execution");
        }

        let timeout_secs = utils::validate_timeout(args.get("timeout").and_then(|v| v.as_u64()));

        self.execute_code_in_sandbox("python3", &[], &normalized_code, ".py", timeout_secs)
            .await
    }

    pub async fn handle_execute_typescript(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let raw_code = match args.get("code").and_then(|v| v.as_str()) {
            Some(code) => code,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: code",
                );
            }
        };

        // LLM 입력 정규화
        let normalized_code = Self::normalize_code_string(raw_code, "typescript");

        // 보정이 발생한 경우 로그 출력
        if normalized_code != raw_code {
            info!("TypeScript code normalized for execution");
        }

        let timeout_secs = utils::validate_timeout(args.get("timeout").and_then(|v| v.as_u64()));

        if let Err(e) = std::str::from_utf8(normalized_code.as_bytes()) {
            error!("Invalid UTF-8 in TypeScript code: {}", e);
            return Self::error_response(request_id, -32603, "Invalid UTF-8 encoding in code");
        }

        if normalized_code.len() > MAX_CODE_SIZE {
            return Self::error_response(
                request_id,
                -32603,
                &format!(
                    "Code too large: {} bytes (max: {} bytes)",
                    normalized_code.len(),
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

        if let Err(e) = fs::write(&ts_file, &normalized_code).await {
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

        let raw_command = match args.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: command",
                );
            }
        };

        // LLM 입력 정규화
        let command_str = Self::normalize_shell_command(raw_command);

        // 보정이 발생한 경우 로그 출력
        if command_str != raw_command {
            info!(
                "Shell command normalized: '{}' -> '{}'",
                raw_command, command_str
            );
        }

        let timeout_secs = utils::validate_timeout(args.get("timeout").and_then(|v| v.as_u64()));

        let working_dir = args.get("working_dir").and_then(|v| v.as_str());

        let work_dir = if let Some(dir) = working_dir {
            std::path::PathBuf::from(dir)
        } else {
            self.get_workspace_dir().to_path_buf()
        };

        let mut cmd = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", &command_str]);
            cmd
        } else {
            let mut cmd = Command::new("sh");
            cmd.args(["-c", &command_str]);
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
