use super::{AgentService, SendSessionMessageResponse};
use crate::models::chat::{Message, MessageSource};
use crate::repositories::{SessionMetadata, SessionStatus};

impl AgentService {
    pub async fn send_message_to_session(
        manager: &crate::agent::AgentSessionManager,
        session_id: &str,
        content: String,
        source: Option<MessageSource>,
        reset: bool,
    ) -> Result<SendSessionMessageResponse, String> {
        // First verify that the session exists in the database
        let exists = manager.get_session(session_id).await?.is_some();
        if !exists {
            return Err(format!("Session not found: {}", session_id));
        }

        if reset {
            manager.reset_session(session_id).await?;
        }

        let session = load_or_resume_session(manager, session_id).await?;

        let message = Message::new_user_message(
            session_id.to_string(),
            content,
            source,
            crate::agent::extract_assistant_id_from_session(&session),
        );
        let message_id = message.id.clone();

        // Explicit API message: a previous force-stop fence ends here (issue #2633).
        manager
            .allow_explicit_message_after_terminate(session_id)
            .await;

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

    if is_active && !needs_docker_setup_retry(&persisted_session) {
        Ok(persisted_session)
    } else if is_active {
        // A failed Docker setup leaves the session active in memory. `resume_session` runs the
        // setup again; on failure it stores the cause as `statusReason` and returns it.
        manager.resume_session(session_id).await
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

fn needs_docker_setup_retry(session: &SessionMetadata) -> bool {
    session.workspace_isolation
        == crate::models::workspace_isolation::WorkspaceIsolationMode::Docker
        && session.status == SessionStatus::Error
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::workspace_isolation::WorkspaceIsolationMode;

    fn session(isolation: WorkspaceIsolationMode, status: SessionStatus) -> SessionMetadata {
        SessionMetadata {
            workspace_isolation: isolation,
            status,
            ..SessionMetadata::test_fixture("session")
        }
    }

    #[test]
    fn only_a_docker_session_in_error_retries_the_setup() {
        assert!(needs_docker_setup_retry(&session(
            WorkspaceIsolationMode::Docker,
            SessionStatus::Error
        )));
        assert!(!needs_docker_setup_retry(&session(
            WorkspaceIsolationMode::Docker,
            SessionStatus::Idle
        )));
        assert!(!needs_docker_setup_retry(&session(
            WorkspaceIsolationMode::Host,
            SessionStatus::Error
        )));
    }
}
