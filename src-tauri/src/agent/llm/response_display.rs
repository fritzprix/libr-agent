//! `MessageDisplay` hooks (#3008): the text shown for a completed assistant
//! message. The stored message and the model context keep the original.

use crate::agent::state::AgentSession;
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;
use crate::services::hooks::lifecycle::{message_display, HookSources};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

fn message_text(message: &Message) -> String {
    message
        .content
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `message` with its text blocks replaced by one holding `text`; every other
/// block keeps its place.
pub(super) fn with_display_text(message: &Message, text: String) -> Message {
    let mut shown = message.clone();
    let position = shown
        .content
        .iter()
        .position(|content| matches!(content, MCPContent::Text { .. }));
    let Some(position) = position else {
        return shown;
    };
    shown
        .content
        .retain(|content| !matches!(content, MCPContent::Text { .. }));
    shown.content.insert(position, MCPContent::Text { text });
    shown
}

/// What the `MessageDisplay` step decided about the response it just showed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TurnGate {
    /// The turn that was admitted is still the live turn.
    Continue,
    /// The admitted turn was cancelled or reset while the hook ran.
    Discard,
    /// The session was removed while the hook ran.
    SessionGone,
}

/// The cancellation token of the turn that was admitted before the display
/// hook runs, or `None` when the session is already gone.
///
/// The session's token is *replaced*, not only cancelled, by a terminate
/// (`workflow/cancel.rs`) and by a Stop followed by a new start
/// (`workflow/start.rs`), so a token read after the hook says nothing about
/// the turn that was admitted. Callers capture this first and hand it to
/// [`check_admitted_turn`] afterwards.
pub(super) async fn admitted_turn_token(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Option<CancellationToken> {
    active_sessions
        .read()
        .await
        .get(session_id)
        .map(|session| session.cancellation_token.clone())
}

/// Whether a response admitted under `admitted` may still be used.
///
/// `Discard` when the admitted turn's own token is cancelled, so a terminate or
/// a Stop-then-start that replaced the session's token while the hook ran still
/// drops the response. The session's current token is also honoured: it covers
/// a plain `cancel` that leaves the same token in place.
pub(super) async fn check_admitted_turn(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    admitted: &CancellationToken,
) -> TurnGate {
    let active = active_sessions.read().await;
    let Some(session) = active.get(session_id) else {
        return TurnGate::SessionGone;
    };
    if admitted.is_cancelled() || session.cancellation_token.is_cancelled() {
        return TurnGate::Discard;
    }
    TurnGate::Continue
}

/// Run the `MessageDisplay` hooks for `message` and return the replacement text,
/// if any. A message without text does not fire the event.
pub(super) async fn display_override(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    message: &Message,
) -> Option<String> {
    let text = message_text(message);
    if text.trim().is_empty() {
        return None;
    }
    let sources = HookSources::for_session(session_id)?;
    // The turn is identified by the user message that opened it.
    let messages = active_sessions
        .read()
        .await
        .get(session_id)
        .map(|session| session.messages.clone());
    let turn_id = match messages {
        Some(messages) => messages
            .read()
            .await
            .iter()
            .rev()
            .find(|cached| cached.role == "user")
            .map(|cached| cached.id.clone()),
        None => None,
    };
    message_display(
        &sources,
        session_id,
        turn_id.as_deref().unwrap_or(&message.id),
        &message.id,
        &text,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::state::build_test_session;

    const SESSION: &str = "sess-display-gate";

    async fn session_map() -> Arc<RwLock<HashMap<String, AgentSession>>> {
        let session = build_test_session(SESSION);
        let mut map = HashMap::new();
        map.insert(session.metadata.id.clone(), session);
        Arc::new(RwLock::new(map))
    }

    #[test]
    fn display_text_replaces_the_shown_copy_only() {
        let original: Message = serde_json::from_value(serde_json::json!({
            "id": "m1",
            "sessionId": "s1",
            "role": "assistant",
            "content": [
                { "type": "text", "text": "first" },
                { "type": "text", "text": "second" },
            ],
        }))
        .expect("message");
        assert_eq!(message_text(&original), "first\nsecond");

        let shown = with_display_text(&original, "redacted".to_string());
        assert_eq!(shown.content.len(), 1);
        assert_eq!(message_text(&shown), "redacted");
        assert_eq!(shown.id, original.id);
        assert_eq!(message_text(&original), "first\nsecond");
    }

    #[tokio::test]
    async fn terminate_while_the_display_hook_runs_discards_the_response() {
        let sessions = session_map().await;
        let admitted = admitted_turn_token(&sessions, SESSION)
            .await
            .expect("session exists");

        // What `terminate_session_with_dispatcher` does while the hook runs:
        // cancel the turn's token, then install a fresh one.
        let replacement_is_live = {
            let mut active = sessions.write().await;
            let session = active.get_mut(SESSION).expect("session exists");
            session.cancellation_token.cancel();
            session.cancellation_token = CancellationToken::new();
            !session.cancellation_token.is_cancelled()
        };

        assert!(
            replacement_is_live,
            "a terminated session installs an uncancelled replacement token"
        );
        assert_eq!(
            check_admitted_turn(&sessions, SESSION, &admitted).await,
            TurnGate::Discard,
            "the admitted turn was cancelled even though the session now holds a fresh token"
        );
    }

    #[tokio::test]
    async fn stop_then_restart_while_the_display_hook_runs_discards_the_response() {
        let sessions = session_map().await;
        let admitted = admitted_turn_token(&sessions, SESSION)
            .await
            .expect("session exists");

        // Stop cancels the token; the next `start_workflow` resets the session
        // execution state, which installs a fresh, uncancelled token.
        {
            let mut active = sessions.write().await;
            let session = active.get_mut(SESSION).expect("session exists");
            session.cancellation_token.cancel();
            session.cancellation_token = CancellationToken::new();
        }

        assert_eq!(
            check_admitted_turn(&sessions, SESSION, &admitted).await,
            TurnGate::Discard,
            "a Stop followed by a restart still discards the response of the stopped turn"
        );
    }

    #[tokio::test]
    async fn a_live_turn_keeps_its_displayed_text() {
        let sessions = session_map().await;
        let admitted = admitted_turn_token(&sessions, SESSION)
            .await
            .expect("session exists");

        assert_eq!(
            check_admitted_turn(&sessions, SESSION, &admitted).await,
            TurnGate::Continue
        );
    }

    #[tokio::test]
    async fn a_plain_cancel_of_the_same_token_discards_the_response() {
        let sessions = session_map().await;
        let admitted = admitted_turn_token(&sessions, SESSION)
            .await
            .expect("session exists");

        sessions
            .write()
            .await
            .get_mut(SESSION)
            .expect("session exists")
            .cancellation_token
            .cancel();

        assert_eq!(
            check_admitted_turn(&sessions, SESSION, &admitted).await,
            TurnGate::Discard
        );
    }

    #[tokio::test]
    async fn a_removed_session_discards_the_response() {
        let sessions = session_map().await;
        let admitted = admitted_turn_token(&sessions, SESSION)
            .await
            .expect("session exists");

        sessions.write().await.remove(SESSION);

        assert_eq!(
            check_admitted_turn(&sessions, SESSION, &admitted).await,
            TurnGate::SessionGone
        );
    }
}
