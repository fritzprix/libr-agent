use super::AgentSessionManager;
use crate::models::chat::Message;

impl AgentSessionManager {
    /// Handle an LLM response from the frontend
    pub async fn handle_llm_response(
        &self,
        session_id: String,
        assistant_message: Message,
    ) -> Result<(), String> {
        crate::agent::llm::handle_llm_response(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
            assistant_message,
        )
        .await
    }

    pub async fn report_llm_streaming_issue(
        &self,
        report: crate::agent::llm::types::StreamingIssueReport,
    ) -> Result<crate::agent::llm::StreamingIssueOutcome, String> {
        crate::agent::llm::handle_streaming_issue(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            report,
        )
        .await
    }

    /// Handle LLM error from frontend
    pub async fn handle_llm_error(
        &self,
        session_id: String,
        error: crate::agent::llm::types::AgentRuntimeError,
    ) -> Result<(), String> {
        crate::agent::llm::handle_llm_error(
            &self.session_repo,
            &self.active_sessions,
            &self.app_handle,
            session_id,
            error,
        )
        .await
    }
}
