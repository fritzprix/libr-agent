//! Session-level orchestration for the shared user prompt (#2543).
//!
//! One place admits the prompt, emits the request event, waits under the shared
//! timeout, and clears the slot. Callers (the shell password flow today, #2544
//! and #2547 next) get the outcome and never touch the slot directly.
//!
//! Only the waiting side emits [`AgentEvent::UserPromptResolved`]. Answer and
//! cancel messages travel through the same oneshot channel as an answer, so one
//! outcome produces exactly one resolved event no matter who resolved it.

use super::AgentSessionManager;
use crate::agent::events::AgentEvent;
use crate::agent::state::{
    admit_user_prompt, pending_user_prompt_summary, take_pending_user_prompt, wait_for_user_prompt,
    PendingUserPromptSummary, UserPromptAnswer, UserPromptOutcome, UserPromptPayload,
    WaitingToolCallRecord,
};
use crate::agent::tauri_events::emit_agent_event;

/// Per-session slot lookup shared by the query commands.
async fn prompt_slot(
    manager: &AgentSessionManager,
    session_id: &str,
) -> Result<crate::agent::state::UserPromptSlot, String> {
    let active = manager.active_sessions().read().await;
    let session = active
        .get(session_id)
        .ok_or_else(|| format!("Session not found: {}", session_id))?;
    Ok(session.pending_user_prompt.clone())
}

/// The prompt currently waiting for a session, if any (requirement 7).
///
/// Returns `None` for an unknown session: a caller asking for a closed session
/// has no pending prompt, and that is not an error.
pub async fn pending_user_prompt_in(
    manager: &AgentSessionManager,
    session_id: &str,
) -> Option<PendingUserPromptSummary> {
    let active = manager.active_sessions().read().await;
    let session = active.get(session_id)?;
    pending_user_prompt_summary(&session.pending_user_prompt).await
}

impl AgentSessionManager {
    /// Ask the local user for a `secret` or `question` and wait for the answer.
    ///
    /// Refused when another prompt already waits for this session (requirement
    /// 2). The slot is always cleared before returning, so a timeout never
    /// blocks the next request (requirements 5 and 9).
    pub async fn request_user_prompt(
        &self,
        session_id: &str,
        payload: UserPromptPayload,
    ) -> Result<UserPromptOutcome, String> {
        let slot = prompt_slot(self, session_id).await?;

        // No UI means nobody can answer: fail now rather than after 300 s.
        if crate::state::get_app_handle().is_none() {
            return Err("User prompt requires the desktop UI runtime".to_string());
        }

        let (request_id, receiver) = admit_user_prompt(&slot, payload.clone()).await?;
        let settings = crate::state::try_get_settings_repository()
            .map(|repo| repo as &dyn crate::repositories::SettingsRepository);
        let waiting_record = WaitingToolCallRecord::open(settings, session_id).await;

        if let Err(error) = emit_agent_event(
            &self.app_handle,
            AgentEvent::UserPromptRequested {
                session_id: session_id.to_string(),
                request_id: request_id.clone(),
                payload,
            },
        ) {
            // Nobody saw the dialog, so nobody can answer it.
            let _ = take_pending_user_prompt(&slot, &request_id).await;
            waiting_record.close().await;
            return Err(format!("Failed to open the user prompt: {}", error));
        }

        log::info!(
            "Waiting for user prompt on session {} (request_id={})",
            session_id,
            request_id
        );

        let outcome = wait_for_user_prompt(&slot, &request_id, receiver).await;
        waiting_record.close().await;

        log::info!(
            "User prompt resolved on session {} (request_id={}, outcome={})",
            session_id,
            request_id,
            outcome.as_event_str()
        );

        // Single emitter for every outcome, including Stop and timeout.
        let _ = emit_agent_event(
            &self.app_handle,
            AgentEvent::UserPromptResolved {
                session_id: session_id.to_string(),
                request_id,
                outcome: outcome.as_event_str().to_string(),
            },
        );

        Ok(outcome)
    }

    /// Deliver an answer to the waiting prompt (requirement 4).
    ///
    /// A `secret` answer reaches the waiting caller only; it is never written to
    /// the message history, an event, or a log.
    pub async fn answer_user_prompt(
        &self,
        session_id: &str,
        request_id: &str,
        answer: UserPromptAnswer,
    ) -> Result<bool, String> {
        let slot = prompt_slot(self, session_id).await?;

        let prompt = take_pending_user_prompt(&slot, request_id)
            .await
            .ok_or_else(|| crate::agent::state::USER_PROMPT_GONE_MESSAGE.to_string())?;

        log::info!(
            "Delivering answer for prompt on session {} (request_id={}, kind={:?})",
            session_id,
            request_id,
            prompt.payload.kind()
        );

        // False means the caller already gave up (timeout or Stop): nothing saved.
        Ok(prompt.answer(answer))
    }

    /// Cancel a waiting prompt without answering it (requirement 4).
    pub async fn cancel_user_prompt(
        &self,
        session_id: &str,
        request_id: &str,
    ) -> Result<bool, String> {
        self.answer_user_prompt(session_id, request_id, UserPromptAnswer::Cancelled)
            .await
    }
}
