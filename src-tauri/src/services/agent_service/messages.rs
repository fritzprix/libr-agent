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
            extract_assistant_id_from_config(session_id, session.agent_config.as_ref()),
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

fn extract_assistant_id_from_config(
    session_id: &str,
    agent_config: Option<&String>,
) -> Option<String> {
    let config_str = agent_config?;
    let config: serde_json::Value = match serde_json::from_str(config_str) {
        Ok(value) => value,
        Err(error) => {
            log::warn!(
                "Invalid session.agent_config JSON for session {} (assistant_id will be None): {}",
                session_id,
                error
            );
            return None;
        }
    };

    let assistant_id_value = config
        .get("assistant_id")
        .or_else(|| config.get("assistantId"))
        .or_else(|| config.get("id"));

    match assistant_id_value {
        Some(value) => match value.as_str() {
            Some(assistant_id) => Some(assistant_id.to_string()),
            None => {
                log::warn!(
                    "session.agent_config assistant id field is not a string for session {} (assistant_id will be None)",
                    session_id
                );
                None
            }
        },
        None => {
            log::warn!(
                "No assistant id field found in session.agent_config for session {} (expected one of: assistant_id, assistantId, id)",
                session_id
            );
            None
        }
    }
}
