use serde_json::Value;
use std::sync::atomic::Ordering;

use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, ToolGroup,
};
use crate::mcp::types::MCPResult;

use super::super::super::{utils, WorkspaceServer, PERSISTENT_SHELL_TOOL, RUN_SHELL_TOOL};
use super::super::validation;
use super::policy::{evaluate_shell_policy, ShellPolicyAction, ShellPolicyContext};

impl WorkspaceServer {
    fn unsafe_mode_bypasses_shell_policy(&self) -> bool {
        let Some(active_sessions) = crate::state::try_get_active_sessions() else {
            return false;
        };

        let Ok(active) = active_sessions.try_read() else {
            return false;
        };

        active
            .get(&self.session_id)
            .map(|session| session.unsafe_mode.load(Ordering::Relaxed))
            .unwrap_or(false)
    }

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

    fn sync_timeout_exceeded_result(&self, timeout_secs: u64, max_timeout: u64) -> MCPResult {
        guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "Timeout ({} seconds) exceeds the sync execution limit ({} seconds)",
                timeout_secs, max_timeout
            ),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            format!(
                "Use {} for commands longer than {} seconds",
                "spawnProcess", max_timeout
            ),
            "Background processes do not block the active agent workflow".to_string(),
            format!(
                "{} and {} stay bounded because they run synchronously",
                RUN_SHELL_TOOL, PERSISTENT_SHELL_TOOL
            ),
        ])
        .to_mcp_result()
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

        // If user input is required, suspend the same synchronous tool call until the UI submits it.
        if require_input || auto_detect {
            return self
                .handle_interactive_shell(raw_command, &args, session_id)
                .await;
        }

        // Sync mode: persistent shell execution
        let requested_timeout = args.get("timeout").and_then(|v| v.as_u64());
        let timeout_secs = match utils::resolve_sync_timeout(requested_timeout) {
            Ok(timeout) => timeout,
            Err(max_timeout) => {
                let attempted_timeout =
                    requested_timeout.unwrap_or_else(utils::default_sync_execution_timeout);
                return Ok(self.sync_timeout_exceeded_result(attempted_timeout, max_timeout));
            }
        };

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
        let requested_timeout = args.get("timeout").and_then(|v| v.as_u64());
        let timeout_secs = match utils::resolve_sync_timeout(requested_timeout) {
            Ok(timeout) => timeout,
            Err(max_timeout) => {
                let attempted_timeout =
                    requested_timeout.unwrap_or_else(utils::default_sync_execution_timeout);
                return Ok(self.sync_timeout_exceeded_result(attempted_timeout, max_timeout));
            }
        };

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

        if self.unsafe_mode_bypasses_shell_policy() {
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
                "YOLO does not bypass policy blocks; use Unsafe mode only if you intentionally accept the risk"
                    .to_string(),
            ])
            .to_mcp_result(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::context::registry::ContextRegistry;
    use crate::agent::state::{AgentSession, CompactionRuntimeState, PendingEventManager};
    use crate::repositories::{SessionMetadata, SessionStatus};
    use crate::session::SessionManager;
    use std::collections::HashMap;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::sync::RwLock;
    use tokio_util::sync::CancellationToken;

    fn build_session_metadata(session_id: &str) -> SessionMetadata {
        let now = chrono::Utc::now().timestamp_millis();
        SessionMetadata {
            id: session_id.to_string(),
            name: Some("Shell Policy Test".to_string()),
            status: SessionStatus::Idle,
            model: "test-model".to_string(),
            provider: "test-provider".to_string(),
            agent_config: None,
            parent_session_id: None,
            lineage_id: None,
            depth: None,
            max_depth: None,
            max_fanout: None,
            org_id: None,
            org_name: None,
            org_root_session_id: None,
            created_at: now,
            updated_at: now,
            last_viewed_at: None,
            last_message_at: None,
            last_attention_at: None,
            last_attention_reason: None,
            is_bookmarked: false,
            yolo_mode: false,
            unsafe_mode: true,
            workspace_override: None,
        }
    }

    fn build_active_agent_session(metadata: SessionMetadata) -> AgentSession {
        AgentSession {
            metadata,
            is_running: false,
            active_permit: None,
            status_transition: Arc::new(RwLock::new(None)),
            transition_lock: Arc::new(tokio::sync::Mutex::new(())),
            cancellation_token: CancellationToken::new(),
            yolo_mode: Arc::new(AtomicBool::new(false)),
            unsafe_mode: Arc::new(AtomicBool::new(true)),
            cancel_pending: Arc::new(AtomicBool::new(false)),
            pending_execution: None,
            messages: Arc::new(RwLock::new(Vec::new())),
            cache_initialized: Arc::new(AtomicBool::new(true)),
            last_synced_at: Arc::new(RwLock::new(None)),
            repeated_thinking_retry_count: Arc::new(RwLock::new(0)),
            pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
            pending_approvals: Arc::new(RwLock::new(HashMap::new())),
            context_registry: Arc::new(ContextRegistry::new()),
            compact_context: Arc::new(RwLock::new(None)),
            compaction: CompactionRuntimeState::new(),
            expected_response_id: Arc::new(RwLock::new(None)),
            cached_stable_prompt: Arc::new(RwLock::new(None)),
            last_completion_request: Arc::new(RwLock::new(None)),
            last_submitted_input_message_id: Arc::new(RwLock::new(None)),
        }
    }

    async fn register_unsafe_active_session(session_id: &str) {
        let active_sessions = if let Some(existing) = crate::state::try_get_active_sessions() {
            existing.clone()
        } else {
            let sessions = Arc::new(RwLock::new(HashMap::new()));
            crate::state::init_active_sessions(sessions.clone());
            sessions
        };

        active_sessions.write().await.insert(
            session_id.to_string(),
            build_active_agent_session(build_session_metadata(session_id)),
        );
    }

    #[tokio::test]
    async fn unsafe_mode_bypasses_shell_policy_blocks() {
        let temp_dir = tempdir().expect("temp dir");
        let session_id = "workspace-shell-unsafe-bypass";
        register_unsafe_active_session(session_id).await;

        let session_manager = Arc::new(
            SessionManager::new_with_base_dir(temp_dir.path().to_path_buf())
                .expect("session manager"),
        );
        let server = WorkspaceServer::new(session_id.to_string(), session_manager);
        let workspace_path = temp_dir.path().join(session_id);

        let result = server.apply_shell_policy_block(
            "runShell",
            "cat ~/.ssh/id_rsa",
            &workspace_path,
            None,
            None,
        );

        assert!(
            result.is_none(),
            "unsafe mode should bypass shell policy blocks"
        );
    }
}
