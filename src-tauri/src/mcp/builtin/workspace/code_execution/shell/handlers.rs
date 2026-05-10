use serde_json::Value;

use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, ToolGroup,
};
use crate::mcp::types::MCPResult;

use super::super::super::{utils, WorkspaceServer, PERSISTENT_SHELL_TOOL, RUN_SHELL_TOOL};
use super::super::validation;
use super::policy::{evaluate_shell_policy, ShellPolicyAction, ShellPolicyContext};

impl WorkspaceServer {
    #[cfg(windows)]
    fn validate_windows_shell_syntax(&self, raw_command: &str) -> Option<MCPResult> {
        if !validation::contains_unquoted_andand(raw_command) {
            return None;
        }

        Some(
            guided_error(
                ErrorCategory::InvalidInput,
                "Invalid PowerShell syntax: '&&' is not supported by PowerShell 5.1",
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use ';' to chain commands in PowerShell".to_string(),
                "Example: cd src; pnpm test".to_string(),
                "If you need conditional execution, use 'if ($LASTEXITCODE -eq 0) { ... }'"
                    .to_string(),
            ])
            .to_mcp_result(),
        )
    }

    #[cfg(not(windows))]
    fn validate_windows_shell_syntax(&self, _raw_command: &str) -> Option<MCPResult> {
        None
    }

    fn validate_sync_timeout(&self, timeout_secs: u64) -> Option<MCPResult> {
        let sync_max = crate::config::default_execution_timeout();
        if timeout_secs <= sync_max {
            return None;
        }

        Some(
            guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Timeout ({} seconds) exceeds maximum ({} seconds)",
                    timeout_secs, sync_max
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                format!(
                    "Use spawnProcess for commands longer than {} seconds",
                    sync_max
                ),
                "spawnProcess runs in background and returns process_id".to_string(),
                format!(
                    "Current maximum timeout: {}s (LIBRAGENT_DEFAULT_EXECUTION_TIMEOUT)",
                    sync_max
                ),
            ])
            .to_mcp_result(),
        )
    }

    pub async fn handle_execute_shell(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        let raw_command = match args.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => {
                return Ok(missing_param_error("command", ToolGroup::Workspace));
            }
        };

        if let Some(result) = self.validate_windows_shell_syntax(raw_command) {
            return Ok(result);
        }

        let workspace_path = self
            .session_manager
            .get_session_workspace_dir_by_id(session_id);
        let current_dir = self
            .shell_manager
            .get_shell_cwd(session_id)
            .await
            .map(std::path::PathBuf::from);
        if let Some(result) = self.apply_shell_policy_block(
            PERSISTENT_SHELL_TOOL,
            raw_command,
            &workspace_path,
            current_dir.as_deref(),
            None,
        ) {
            return Ok(result);
        }

        // Check for requireUserInput parameter or auto-detect privilege escalation
        let require_input = args
            .get("requireUserInput")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let auto_detect = validation::detect_privilege_escalation(raw_command);

        // If user input required, return UIResource for interactive execution
        if require_input || auto_detect {
            return self
                .handle_interactive_shell(raw_command, &args, session_id)
                .await;
        }

        // Sync mode: persistent shell execution
        let timeout_secs = utils::validate_timeout(args.get("timeout").and_then(|v| v.as_u64()));

        // Enforce maximum sync timeout
        if let Some(result) = self.validate_sync_timeout(timeout_secs) {
            return Ok(result);
        }

        // Execute with persistent shell (state preservation)
        self.execute_shell_persistent(raw_command, PERSISTENT_SHELL_TOOL, timeout_secs, session_id)
            .await
    }

    /// Handle primary isolated shell execution (new tool)
    pub async fn handle_run_shell(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        let raw_command = match args.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => {
                return Ok(missing_param_error("command", ToolGroup::Workspace));
            }
        };

        if let Some(result) = self.validate_windows_shell_syntax(raw_command) {
            return Ok(result);
        }

        // Check for interactive patterns but allow execution (removed blocking heuristic)
        if validation::is_likely_interactive_command(raw_command) {
            tracing::debug!(
                "Command '{}' matches interactive patterns but execution is allowed",
                raw_command
            );
        }

        // Parse optional environment variables
        let env_vars = args
            .get("env")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let workspace_path = self
            .session_manager
            .get_session_workspace_dir_by_id(session_id);
        if let Some(result) = self.apply_shell_policy_block(
            RUN_SHELL_TOOL,
            raw_command,
            &workspace_path,
            None,
            Some(&env_vars),
        ) {
            return Ok(result);
        }

        // Get timeout (use default if not specified)
        let timeout_secs = utils::validate_timeout(args.get("timeout").and_then(|v| v.as_u64()));

        // Enforce maximum sync timeout
        if let Some(result) = self.validate_sync_timeout(timeout_secs) {
            return Ok(result);
        }

        // Execute with configured isolation level (always workspace root anchored)
        let isolation_level = utils::get_shell_isolation_level().await;
        self.execute_shell_with_isolation(
            raw_command,
            RUN_SHELL_TOOL,
            isolation_level,
            timeout_secs,
            session_id,
            env_vars,
        )
        .await
    }

    /// Handle async shell execution (separate tool)
    pub async fn handle_spawn_process(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        let raw_command = match args.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => {
                return Ok(missing_param_error("command", ToolGroup::Workspace));
            }
        };

        if let Some(result) = self.validate_windows_shell_syntax(raw_command) {
            return Ok(result);
        }

        // Async mode does not support interactive input
        let require_input = args
            .get("requireUserInput")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if require_input {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Background processes cannot prompt for interactive input".to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                format!(
                    "Use {} (sync mode) for commands requiring user input",
                    PERSISTENT_SHELL_TOOL
                ),
                format!(
                    "{} supports requireUserInput for sudo/interactive commands",
                    PERSISTENT_SHELL_TOOL
                ),
                "Async processes run in the background without user interaction".to_string(),
            ])
            .to_mcp_result());
        }

        // Async mode does not support interactive input validation (heuristic removed)
        if validation::is_likely_interactive_command(raw_command) {
            tracing::warn!(
                "Async command likely interactive: {} (execution allowed)",
                raw_command
            );
        }

        let env_vars = args.get("env").and_then(|v| v.as_object()).map(|obj| {
            obj.iter()
                .map(|(key, value)| (key.clone(), value.as_str().unwrap_or("").to_string()))
                .collect::<std::collections::HashMap<_, _>>()
        });

        let workspace_path = self
            .session_manager
            .get_session_workspace_dir_by_id(session_id);
        if let Some(result) = self.apply_shell_policy_block(
            "spawnProcess",
            raw_command,
            &workspace_path,
            None,
            env_vars.as_ref(),
        ) {
            return Ok(result);
        }

        // Execute in background
        self.execute_shell_async(raw_command, &args, session_id)
            .await
    }

    pub(crate) fn apply_shell_policy_block(
        &self,
        tool_name: &str,
        command: &str,
        workspace_path: &std::path::Path,
        current_dir: Option<&std::path::Path>,
        environment: Option<&std::collections::HashMap<String, String>>,
    ) -> Option<MCPResult> {
        let decision = evaluate_shell_policy(ShellPolicyContext {
            tool_name,
            command,
            workspace_dir: Some(workspace_path),
            current_dir,
            environment,
            force_approval: false,
        });

        if decision.action != ShellPolicyAction::Block {
            return None;
        }

        Some(
            guided_error(
                ErrorCategory::PermissionDenied,
                format!("Shell command blocked by policy: {}", decision.reason),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use workspace file tools for normal file inspection and editing".to_string(),
                "Avoid protected home/system credential locations in shell commands".to_string(),
                "If you need this access, change the policy rather than relying on YOLO"
                    .to_string(),
            ])
            .to_mcp_result(),
        )
    }
}
