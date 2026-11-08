use super::AgentSessionManager;
use crate::models::chat::Message;

impl AgentSessionManager {
    /// Start an agent workflow for a session
    pub async fn start_workflow(
        &self,
        session_id: String,
        user_message: Message,
    ) -> Result<(), String> {
        self.ensure_session_active(&session_id).await?;
        crate::agent::workflow::start_workflow(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
            user_message,
        )
        .await
    }

    /// Re-open a force-terminated session for an explicit user or API message.
    ///
    /// The teardown fence (issue #2633) is cleared only here and at the other
    /// explicit lifecycle entry points (the resume command, an explicit
    /// user/API message, session create-and-start, session reload, and
    /// regenerate). Background proxy bootstrap never clears it — it skips
    /// creation while fenced. Automatic wakes — MCP channel notifications,
    /// scheduled jobs, compaction continuation, background resume — must never
    /// call this, so a terminated session cannot be resurrected by them.
    pub async fn allow_explicit_message_after_terminate(&self, session_id: &str) {
        self.proxy_manager.clear_teardown_fence(session_id).await;
    }

    /// Pause a running workflow
    pub async fn pause_workflow(&self, session_id: String) -> Result<(), String> {
        crate::agent::workflow::pause_workflow(
            &self.session_repo,
            &self.active_sessions,
            &self.app_handle,
            session_id,
        )
        .await
    }

    /// Resume a paused workflow
    /// Reload the session (this prepares the Docker runtime again, or fails when the container
    /// cannot be used) and then resume its workflow. Retry after an error must use this:
    /// [`Self::resume_workflow`] alone skips the runtime when the session is already in memory.
    ///
    /// Both callers (`agent_resume_workflow`, `POST /api/sessions/:id/resume`) are
    /// explicit user/API actions, so this entry lifts a force-stop fence before
    /// anything else — otherwise the bootstrap skip would refuse the resume and
    /// leave the session in `Error` (issue #2633 G3.1).
    pub async fn resume_session_and_workflow(&self, session_id: &str) -> Result<(), String> {
        self.allow_explicit_message_after_terminate(session_id)
            .await;
        self.resume_session(session_id)
            .await
            .map_err(|e| format!("Failed to resume session: {}", e))?;
        self.resume_workflow(session_id.to_string())
            .await
            .map_err(|e| format!("Failed to resume workflow: {}", e))
    }

    pub async fn resume_workflow(&self, session_id: String) -> Result<(), String> {
        self.ensure_session_active(&session_id).await?;
        crate::agent::workflow::resume_workflow(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
        )
        .await
    }

    /// Discard the last completed assistant turn (idle only) and re-run completion.
    pub async fn regenerate_last_turn(
        &self,
        session_id: String,
    ) -> Result<crate::agent::workflow::TruncateLastTurnPlan, String> {
        self.ensure_session_active(&session_id).await?;
        crate::agent::workflow::regenerate_last_turn(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
        )
        .await
    }

    /// Terminate a running workflow
    pub async fn terminate_session(&self, session_id: String) -> Result<(), String> {
        crate::agent::workflow::terminate_session(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
        )
        .await
    }

    /// Cancel a running workflow
    pub async fn cancel_workflow(
        &self,
        session_id: String,
    ) -> Result<crate::commands::agent_commands::CancelWorkflowResult, String> {
        crate::agent::workflow::cancel_workflow(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
        )
        .await
    }

    /// Handle tool execution result from frontend
    pub async fn handle_tool_result(
        &self,
        session_id: String,
        tool_call_id: String,
        result: crate::commands::agent_commands::ToolExecutionResult,
    ) -> Result<(), String> {
        crate::agent::workflow::continue_workflow_after_tool(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
            tool_call_id,
            result,
        )
        .await
    }
}
