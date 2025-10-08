use boa_engine::{Context, Source};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{error, info};

use crate::session_isolation::{IsolatedProcessConfig, IsolationLevel};

use super::utils::constants::*;
use super::{utils, WorkspaceServer};
use crate::mcp::MCPResponse;

#[allow(dead_code)]
impl WorkspaceServer {
    /// Execute shell commands with isolation
    async fn execute_shell_with_isolation(
        &self,
        command: &str,
        isolation_level: IsolationLevel,
        timeout_secs: u64,
    ) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let session_id = self
            .session_manager
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());

        let workspace_path = self.get_workspace_dir();

        // Normalize shell command
        let normalized_command = Self::normalize_shell_command(command);

        let isolation_config = IsolatedProcessConfig {
            session_id: session_id.clone(),
            workspace_path: workspace_path.clone(),
            command: normalized_command,
            args: vec![],
            env_vars: HashMap::new(),
            isolation_level,
        };

        // Create isolated command
        let mut cmd = match self
            .isolation_manager
            .create_isolated_command(isolation_config)
            .await
        {
            Ok(cmd) => cmd,
            Err(e) => {
                return Self::error_response(
                    request_id,
                    -32603,
                    &format!("Failed to create isolated shell command: {e}"),
                );
            }
        };

        // Execute command with timeout
        let timeout_duration = Duration::from_secs(timeout_secs);
        let execution_result = timeout(timeout_duration, cmd.output()).await;

        match execution_result {
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
                    "Isolated shell command executed: {} (session: {}, exit: {})",
                    command, session_id, exit_code
                );

                Self::success_response(request_id, &result_text)
            }
            Ok(Err(e)) => {
                error!(
                    "Failed to execute isolated shell command '{}': {}",
                    command, e
                );
                Self::error_response(request_id, -32603, &format!("Execution error: {e}"))
            }
            Err(_) => {
                error!(
                    "Isolated shell command '{}' timed out after {} seconds",
                    command, timeout_secs
                );
                Self::error_response(
                    request_id,
                    -32603,
                    &format!("Command timed out after {timeout_secs} seconds"),
                )
            }
        }
    }

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

        let timeout_secs = utils::validate_timeout(args.get("timeout").and_then(|v| v.as_u64()));

        // Determine isolation level based on arguments or default to Medium
        let isolation_level = args
            .get("isolation")
            .and_then(|v| v.as_str())
            .map(|level| match level {
                "basic" => IsolationLevel::Basic,
                "high" => IsolationLevel::High,
                _ => IsolationLevel::Medium,
            })
            .unwrap_or(IsolationLevel::Medium);

        // Use new isolation-aware shell execution
        self.execute_shell_with_isolation(raw_command, isolation_level, timeout_secs)
            .await
    }

    pub async fn handle_eval_javascript(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let raw_code = match args.get("code").and_then(|v| v.as_str()) {
            Some(code) => code.to_string(),
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: code",
                );
            }
        };

        // Validate code size
        if raw_code.len() > MAX_CODE_SIZE {
            return Self::error_response(
                request_id,
                -32602,
                &format!(
                    "Code size {} exceeds maximum allowed size {}",
                    raw_code.len(),
                    MAX_CODE_SIZE
                ),
            );
        }

        let timeout_secs = utils::validate_timeout(args.get("timeout").and_then(|v| v.as_u64()));

        // Execute JavaScript code with timeout using spawn_blocking for Boa Context
        let timeout_duration = Duration::from_secs(timeout_secs.min(MAX_EXECUTION_TIMEOUT));

        let execution_result = timeout(timeout_duration, async {
            tokio::task::spawn_blocking(move || {
                // Create a new Boa context for JavaScript execution in blocking task
                let mut context = Context::default();

                match context.eval(Source::from_bytes(&raw_code)) {
                    Ok(result) => {
                        // Try to convert the result to a string representation
                        let output = match result.as_string() {
                            Some(js_string) => match js_string.to_std_string() {
                                Ok(s) => s,
                                Err(_) => result.display().to_string(),
                            },
                            None => result.display().to_string(),
                        };
                        Ok(output)
                    }
                    Err(e) => Err(format!("JavaScript execution error: {e}")),
                }
            })
            .await
            .map_err(|e| format!("Task join error: {e}"))?
        })
        .await;

        match execution_result {
            Ok(Ok(output)) => {
                info!("JavaScript evaluation completed successfully");
                Self::success_response(request_id, &format!("Result: {output}"))
            }
            Ok(Err(e)) => {
                error!("JavaScript evaluation failed: {}", e);
                Self::error_response(request_id, -32603, &format!("Evaluation error: {e}"))
            }
            Err(_) => {
                error!(
                    "JavaScript evaluation timed out after {} seconds",
                    timeout_secs
                );
                Self::error_response(
                    request_id,
                    -32603,
                    &format!("Evaluation timed out after {timeout_secs} seconds"),
                )
            }
        }
    }
}
