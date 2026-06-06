use super::types::{
    CANCEL_INTERACTIVE_SHELL_INPUT_INTERNAL, SUBMIT_INTERACTIVE_SHELL_INPUT_INTERNAL,
};
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::MCPResult;
use serde_json::Value;
use tracing::info;

use super::workspace_server::WorkspaceServer;

impl WorkspaceServer {
    /// Dispatch a tool call to the appropriate handler method.
    /// This is extracted from the main mod.rs to reduce cognitive load.
    pub async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let logged_args = if tool_name == SUBMIT_INTERACTIVE_SHELL_INPUT_INTERNAL {
            serde_json::json!({ "redacted": true })
        } else {
            args.clone()
        };
        info!(
            "Workspace tool called: {} with args: {:?}",
            tool_name, logged_args
        );

        let target_session_id = session_id
            .clone()
            .unwrap_or_else(|| self.session_id.clone());

        match tool_name {
            // ── File operation tools ──────────────────────────────────────────
            "readFile" => self.handle_read_file(args, session_id).await,
            "writeFile" => self.handle_write_file(args, session_id).await,
            "listDirectory" => self.handle_list_directory(args, session_id).await,
            "importFiles" => self.handle_import_files(args, session_id).await,
            "search" => self.handle_search(args, session_id).await,
            // editFiles is the model-facing mutation tool. The legacy editFile and per-operation
            // aliases remain dispatchable for backward compatibility and internally normalize
            // into editFiles.
            "editFiles" => self.handle_edit_files(args, session_id).await,
            "editFile" => self.handle_edit_file(args, session_id).await,
            "replaceLines" => self.handle_replace_lines(args, session_id).await,
            "insertAfterLine" => self.handle_insert_after_line(args, session_id).await,
            "deleteLines" => self.handle_delete_lines(args, session_id).await,

            // ── Code execution tools ──────────────────────────────────────────
            // PRIMARY isolated shell execution tools (recommended)
            #[cfg(unix)]
            "runShell" => self.handle_run_shell(args, &target_session_id).await,
            #[cfg(windows)]
            "runPowerShell" => self.handle_run_shell(args, &target_session_id).await,
            // ADVANCED persistent shell execution tools (for state preservation)
            #[cfg(unix)]
            "runInPersistentShell" => self.handle_execute_shell(args, &target_session_id).await,
            #[cfg(windows)]
            "runInPersistentPowerShell" => {
                self.handle_execute_shell(args, &target_session_id).await
            }
            // Background process execution (platform-agnostic)
            "spawnProcess" => self.handle_spawn_process(args, &target_session_id).await,

            // Interactive shell input
            SUBMIT_INTERACTIVE_SHELL_INPUT_INTERNAL => {
                self.handle_submit_interactive_shell_input(args, &target_session_id)
                    .await
            }
            CANCEL_INTERACTIVE_SHELL_INPUT_INTERNAL => {
                self.handle_cancel_pending_execution(args, &target_session_id)
                    .await
            }

            // ── Export tools ──────────────────────────────────────────────────
            "export" => self.handle_export(args, session_id).await,

            // ── Terminal / Process management tools ───────────────────────────
            // ── Terminal / Process management tools ───────────────────────────
            "readProcessOutput" => {
                self.handle_read_process_output(args, &target_session_id)
                    .await
            }
            "listProcesses" => self.handle_list_processes(args, &target_session_id).await,
            "stopProcess" => self.handle_stop_process(args, &target_session_id).await,
            "waitForProcess" => self.handle_wait_for_process(args, &target_session_id).await,
            // Backward-compat alias: pollProcess was the old name for non-blocking status check.
            // Always inject timeout=0 so semantics are preserved.
            "pollProcess" => {
                let mut poll_args = args.clone();
                poll_args["timeout"] = serde_json::json!(0);
                self.handle_wait_for_process(poll_args, &target_session_id)
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
