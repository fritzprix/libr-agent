use super::{AgentService, SendSessionMessageResponse};
use crate::models::chat::{Message, MessageSource};
use crate::repositories::SessionMetadata;

impl AgentService {
    pub async fn send_message_to_session(
        manager: &crate::agent::AgentSessionManager,
        session_id: &str,
        content: String,
        source: Option<MessageSource>,
    ) -> Result<SendSessionMessageResponse, String> {
        let session = load_or_resume_session(manager, session_id).await?;
        let message = Message::new_user_message(
            session_id.to_string(),
            content,
            source,
            crate::agent::extract_assistant_id_from_session(&session),
        );
        let message_id = message.id.clone();

        let triggered = manager
            .inject_messages(session_id.to_string(), vec![message])
            .await?;
        let status = if triggered { "processed" } else { "queued" };

        Ok(SendSessionMessageResponse {
            message_id,
            status: status.to_string(),
        })
    }
}

async fn load_or_resume_session(
    manager: &crate::agent::AgentSessionManager,
    session_id: &str,
) -> Result<SessionMetadata, String> {
    let persisted_session = manager
        .get_session(session_id)
        .await?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;
    let is_active = {
        let active_sessions = manager.active_sessions_arc();
        let active = active_sessions.read().await;
        active.contains_key(session_id)
    };

    if is_active {
        Ok(persisted_session)
    } else {
        log::info!(
            "Auto-resuming inactive session before send_message_to_session: {}",
            session_id
        );
        let resumed_session = manager.resume_session(session_id).await?;
        manager.init_session_with_messages(session_id).await?;
        Ok(resumed_session)
    }
}
