use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::MCPResult;
use crate::models::workspace_isolation::WorkspaceIsolationMode;
use serde_json::Value;
use tracing::info;

use super::persistent_shell::secrets::mask_result;
use super::workspace_server::WorkspaceServer;

const MAX_LOGGED_STRING_BYTES: usize = 128;

/// Tools whose results can hold shell output or command text; session secrets are masked in them.
const SHELL_OUTPUT_TOOLS: &[&str] = &[
    "runShell",
    "runPowerShell",
    "runInPersistentShell",
    "runInPersistentPowerShell",
    "spawnProcess",
    "readProcessOutput",
    "listProcesses",
    "stopProcess",
    "waitForProcess",
    "pollProcess",
];

const REDACTED_ARG_KEYS: &[&str] = &[
    "content",
    "old_string",
    "new_string",
    "new_value",
    "edits",
    "ops",
    "value",
    "command",
    "script",
    "input",
    "env",
];

fn summarize_redacted_value(value: &Value) -> Value {
    match value {
        Value::String(s) => Value::String(format!("<redacted: {} bytes>", s.len())),
        Value::Array(arr) => Value::String(format!("<redacted: {} items>", arr.len())),
        Value::Object(obj) => Value::String(format!("<redacted: {} keys>", obj.len())),
        Value::Null => Value::Null,
        _ => Value::String("<redacted>".to_string()),
    }
}

fn summarize_value_for_log(value: &Value) -> Value {
    match value {
        Value::String(s) if s.len() > MAX_LOGGED_STRING_BYTES => {
            let end = s
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|&i| i <= MAX_LOGGED_STRING_BYTES)
                .last()
                .unwrap_or(0);
            Value::String(format!("{}... ({} bytes)", &s[..end], s.len()))
        }
        Value::Array(arr) => Value::Array(arr.iter().map(summarize_value_for_log).collect()),
        Value::Object(map) => {
            let summarized = map
                .iter()
                .map(|(k, v)| {
                    let next = if REDACTED_ARG_KEYS.contains(&k.as_str()) {
                        summarize_redacted_value(v)
                    } else {
                        summarize_value_for_log(v)
                    };
                    (k.clone(), next)
                })
                .collect();
            Value::Object(summarized)
        }
        _ => value.clone(),
    }
}

impl WorkspaceServer {
    fn shell_tool_unavailable_result(&self, tool_name: &str) -> MCPResult {
        let (reason, guidance) = match self.workspace_isolation {
            WorkspaceIsolationMode::Docker => (
                format!(
                    "Tool '{tool_name}' is not available in Docker workspace isolation (Linux container shell only)"
                ),
                vec![
                    "Use workspace__runShell for one-shot commands".to_string(),
                    "Use workspace__runInPersistentShell for stateful shell sessions".to_string(),
                ],
            ),
            WorkspaceIsolationMode::Host if cfg!(windows) => (
                format!(
                    "Tool '{tool_name}' is not available on Windows host isolation (PowerShell only)"
                ),
                vec![
                    "Use workspace__runPowerShell for one-shot commands".to_string(),
                    "Use workspace__runInPersistentPowerShell for stateful shell sessions".to_string(),
                    "Enable Docker Mode to use bash/sh (workspace__runShell) in a Linux container".to_string(),
                ],
            ),
            WorkspaceIsolationMode::Host => (
                format!(
                    "Tool '{tool_name}' is not available on this host platform (bash/sh only)"
                ),
                vec![
                    "Use workspace__runShell for one-shot commands".to_string(),
                    "Use workspace__runInPersistentShell for stateful shell sessions".to_string(),
                ],
            ),
        };

        guided_error(ErrorCategory::InvalidInput, reason, ToolGroup::Workspace)
            .guidance(guidance)
            .to_mcp_result()
    }

    /// Dispatch a tool call to the appropriate handler method.
    /// This is extracted from the main mod.rs to reduce cognitive load.
    pub async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let logged_args = summarize_value_for_log(&args);
        info!(
            "Workspace tool called: {} with args: {:?}",
            tool_name, logged_args
        );

        let target_session_id = session_id
            .clone()
            .unwrap_or_else(|| self.session_id.clone());

        if tool_name != "runShell" && crate::services::as_root_requested(&args) {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("Tool '{tool_name}' cannot run as root; only workspace__runShell accepts asRoot"),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Run the command with workspace__runShell and asRoot: true".to_string(),
            ])
            .to_mcp_result());
        }

        if !self.code_tools_profile().allows_shell_tool(tool_name) {
            return Ok(self.shell_tool_unavailable_result(tool_name));
        }

        let result = self
            .dispatch_tool(tool_name, args, session_id, &target_session_id)
            .await;
        if !SHELL_OUTPUT_TOOLS.contains(&tool_name) {
            return result;
        }
        let secrets = self.shell_manager.session_secrets(&target_session_id).await;
        result.map(|result| mask_result(result, &secrets))
    }

    async fn dispatch_tool(
        &self,
        tool_name: &str,
        args: Value,
        session_id: Option<String>,
        target_session_id: &str,
    ) -> Result<MCPResult, String> {
        match tool_name {
            // ── File operation tools ──────────────────────────────────────────
            "readFile" => self.handle_read_file(args, session_id).await,
            "writeFile" => self.handle_write_file(args, session_id).await,
            "listDirectory" => self.handle_list_directory(args, session_id).await,
            "importFiles" => self.handle_import_files(args, session_id).await,
            "globFiles" => self.handle_glob_files(args, session_id).await,
            "grepFiles" => self.handle_grep_files(args, session_id).await,
            "searchFiles" => self.handle_search(args, session_id).await,
            #[cfg(feature = "workspace-str-replace")]
            "strReplace" => self.handle_str_replace(args, session_id).await,
            "editObject" => self.handle_edit_object(args, session_id).await,
            #[cfg(feature = "workspace-edit-file")]
            // editFile is the model-facing mutation tool. Per-operation aliases remain
            // dispatchable for backward compatibility and internally normalize into editFile.
            "editFile" => self.handle_edit_file(args, session_id).await,
            #[cfg(feature = "workspace-edit-file")]
            "replaceLines" => self.handle_replace_lines(args, session_id).await,
            #[cfg(feature = "workspace-edit-file")]
            "insertAfterLine" => self.handle_insert_after_line(args, session_id).await,
            #[cfg(feature = "workspace-edit-file")]
            "deleteLines" => self.handle_delete_lines(args, session_id).await,

            // ── Code execution tools ──────────────────────────────────────────
            // PRIMARY isolated shell execution tools (recommended)
            "runShell" | "runPowerShell" => {
                self.handle_run_shell(args, target_session_id, tool_name)
                    .await
            }
            // ADVANCED persistent shell execution tools (for state preservation)
            "runInPersistentShell" | "runInPersistentPowerShell" => {
                self.handle_execute_shell(args, target_session_id, tool_name)
                    .await
            }
            // Background process execution (platform-agnostic)
            "spawnProcess" => self.handle_spawn_process(args, target_session_id).await,

            // ── Export tools ──────────────────────────────────────────────────
            "export" => self.handle_export(args, session_id).await,

            // ── Terminal / Process management tools ───────────────────────────
            "readProcessOutput" => {
                self.handle_read_process_output(args, target_session_id)
                    .await
            }
            "listProcesses" => self.handle_list_processes(args, target_session_id).await,
            "stopProcess" => self.handle_stop_process(args, target_session_id).await,
            "waitForProcess" => self.handle_wait_for_process(args, target_session_id).await,
            // Backward-compat alias: pollProcess was the old name for non-blocking status check.
            // Always inject timeout=0 so semantics are preserved.
            "pollProcess" => {
                let mut poll_args = args.clone();
                poll_args["timeout"] = serde_json::json!(0);
                self.handle_wait_for_process(poll_args, target_session_id)
                    .await
            }

            _ => Err(format!("Tool '{tool_name}' not found")),
        }
        .or_else(|e| {
            if e.contains("cancelled") || e.contains("interrupted") {
                return Err(e);
            }
            Ok(guided_error(ErrorCategory::InternalError, e, ToolGroup::Workspace).to_mcp_result())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn summarize_args_redacts_file_content_and_keeps_metadata() {
        let secret = "API_KEY=sk-live-secret-value\n";
        let args = json!({
            "path": ".env",
            "mode": "overwrite",
            "content": secret,
        });
        let logged = summarize_value_for_log(&args);
        assert_eq!(
            logged,
            json!({
                "path": ".env",
                "mode": "overwrite",
                "content": format!("<redacted: {} bytes>", secret.len()),
            })
        );
        let serialized = logged.to_string();
        assert!(!serialized.contains("sk-live-secret-value"));
    }

    #[test]
    fn summarize_args_redacts_edit_and_shell_payloads() {
        let args = json!({
            "path": "config.json",
            "old_string": "old-secret",
            "new_string": "new-secret",
            "ops": [{"op": "set", "path": "/token", "value": "tok_123"}],
            "edits": [{"line": 1, "new_value": "secret"}],
            "command": "echo $SECRET_TOKEN",
            "script": "export TOKEN=abc\ncurl ...",
            "env": {"TOKEN": "abc"}
        });
        let logged = summarize_value_for_log(&args);
        assert_eq!(logged["path"], json!("config.json"));
        assert_eq!(logged["old_string"], json!("<redacted: 10 bytes>"));
        assert_eq!(logged["new_string"], json!("<redacted: 10 bytes>"));
        assert_eq!(logged["ops"], json!("<redacted: 1 items>"));
        assert_eq!(logged["edits"], json!("<redacted: 1 items>"));
        assert_eq!(logged["command"], json!("<redacted: 18 bytes>"));
        assert_eq!(logged["script"], json!("<redacted: 25 bytes>"));
        assert_eq!(logged["env"], json!("<redacted: 1 keys>"));
    }

    #[test]
    fn summarize_args_truncates_large_non_sensitive_strings_on_char_boundary() {
        let long_pattern = "가".repeat(60); // 180 bytes in UTF-8
        let args = json!({
            "path": "src/main.rs",
            "pattern": long_pattern,
        });
        let logged = summarize_value_for_log(&args);
        let pattern_logged = logged["pattern"].as_str().expect("string summary");
        assert!(pattern_logged.ends_with("... (180 bytes)"));
        assert!(pattern_logged.len() < long_pattern.len());
    }
}
