//! `GET /api/sessions/:id/events` — a session's activity as server-sent events.
//!
//! A frontend of the shared backend (#2700) follows one session with this
//! stream instead of polling `GET /api/sessions/:id` and `…/messages`.
//!
//! Events:
//! - `status`: the `GET /api/sessions/:id` body. Sent first, then whenever it
//!   changes (status, pending approvals, pending user prompt, …).
//! - `message`: one message object, as in `GET /api/sessions/:id/messages`.
//!   Sent when a message appears or its stored content changes; a client
//!   replaces any message it holds with the same `id`. Messages that existed
//!   when the stream opened are not replayed.
//! - `end`: the session no longer exists. The stream closes after it.
//!
//! Granularity is what the backend stores. Token-by-token model output is
//! produced in the frontend that runs the LLM call and is not in this stream.

use crate::agent::AgentSessionManager;
use crate::models::chat::Message;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use warp::{http::StatusCode, Rejection, Reply};

use super::helpers::{map_message_for_http, resolve_error_reply, resolve_http_session_ref};
use super::sessions::session_status_snapshot;

/// The stream also wakes on the session bus, so this only bounds how late a
/// change that does not touch the session status (a new message) is seen.
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Messages checked for changes on each pass: the most recent ones.
const WATCHED_MESSAGES: u64 = 50;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum SessionEvent {
    Status(serde_json::Value),
    Message(serde_json::Value),
    End,
}

impl SessionEvent {
    fn into_sse(self) -> warp::sse::Event {
        let (name, data) = match self {
            SessionEvent::Status(data) => ("status", data),
            SessionEvent::Message(data) => ("message", data),
            SessionEvent::End => ("end", serde_json::json!({})),
        };
        warp::sse::Event::default()
            .event(name)
            .data(data.to_string())
    }
}

/// Remembers what the client has been told, and turns a fresh snapshot into
/// the events that bring it up to date.
#[derive(Default)]
pub(super) struct EventCursor {
    last_status: Option<serde_json::Value>,
    /// Message id -> the JSON last sent (or, for history, last seen).
    messages: HashMap<String, serde_json::Value>,
    primed: bool,
}

impl EventCursor {
    pub(super) fn advance(
        &mut self,
        status: serde_json::Value,
        messages: Vec<Message>,
    ) -> Vec<SessionEvent> {
        let mut events = Vec::new();

        if self.last_status.as_ref() != Some(&status) {
            self.last_status = Some(status.clone());
            events.push(SessionEvent::Status(status));
        }

        for message in messages {
            let id = message.id.clone();
            let Ok(json) = serde_json::to_value(map_message_for_http(message)) else {
                continue;
            };
            if self.messages.get(&id) == Some(&json) {
                continue;
            }
            self.messages.insert(id, json.clone());
            // The first pass only records history; it is not replayed.
            if self.primed {
                events.push(SessionEvent::Message(json));
            }
        }
        self.primed = true;

        events
    }
}

/// One pass: read the session and return the events it produced, or `None`
/// when the session is gone.
async fn next_events(
    manager: &AgentSessionManager,
    session_id: &str,
    cursor: &mut EventCursor,
) -> Option<Vec<SessionEvent>> {
    let snapshot = session_status_snapshot(
        session_id,
        Some(manager.active_sessions().as_ref()),
        &manager.repositories().session,
    )
    .await;
    let status = match snapshot {
        Ok(Some(snapshot)) => serde_json::to_value(snapshot).ok()?,
        Ok(None) => return None,
        Err(error) => {
            // A transient read error must not end the stream.
            log::warn!("Session event stream: status read failed for {session_id}: {error}");
            return Some(Vec::new());
        }
    };

    use crate::repositories::message_repository::MessageRepository;
    let messages = crate::state::get_message_repository()
        .get_messages_by_session(session_id, WATCHED_MESSAGES)
        .await
        .unwrap_or_else(|error| {
            log::warn!("Session event stream: message read failed for {session_id}: {error}");
            Vec::new()
        });

    Some(cursor.advance(status, messages))
}

struct StreamState {
    manager: Arc<AgentSessionManager>,
    session_id: String,
    cursor: EventCursor,
    pending: std::collections::VecDeque<SessionEvent>,
    ended: bool,
    first_pass: bool,
}

pub async fn get_session_events(
    id: String,
    manager: Arc<AgentSessionManager>,
) -> Result<Box<dyn Reply>, Rejection> {
    let session_id = match resolve_http_session_ref(&id).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(Box::new(resolve_error_reply(status, error))),
    };
    match manager.get_session(&session_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Ok(Box::new(resolve_error_reply(
                StatusCode::NOT_FOUND,
                format!("Session not found: {session_id}"),
            )))
        }
        Err(error) => {
            return Ok(Box::new(resolve_error_reply(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to validate session: {error}"),
            )))
        }
    }

    let state = StreamState {
        manager,
        session_id,
        cursor: EventCursor::default(),
        pending: std::collections::VecDeque::new(),
        ended: false,
        first_pass: true,
    };

    let stream = futures::stream::unfold(state, |mut state| async move {
        loop {
            if let Some(event) = state.pending.pop_front() {
                if event == SessionEvent::End {
                    state.ended = true;
                }
                return Some((Ok::<_, Infallible>(event.into_sse()), state));
            }
            if state.ended {
                return None;
            }

            if state.first_pass {
                state.first_pass = false;
            } else {
                // Subscribe before waiting so a status change is not missed.
                let notifier = crate::state::try_get_session_bus()
                    .map(|bus| bus.get_or_create(&state.session_id));
                match notifier {
                    Some(notifier) => {
                        tokio::select! {
                            _ = notifier.notified() => {}
                            _ = tokio::time::sleep(POLL_INTERVAL) => {}
                        }
                    }
                    None => tokio::time::sleep(POLL_INTERVAL).await,
                }
            }

            match next_events(&state.manager, &state.session_id, &mut state.cursor).await {
                Some(events) => state.pending.extend(events),
                None => state.pending.push_back(SessionEvent::End),
            }
        }
    });

    Ok(Box::new(warp::sse::reply(
        warp::sse::keep_alive().stream(stream),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::chat::Message;
    use serde_json::json;

    fn message(id: &str, text: &str) -> Message {
        serde_json::from_value(json!({
            "id": id,
            "sessionId": "s1",
            "role": "assistant",
            "content": [{ "type": "text", "text": text }],
        }))
        .expect("message fixture")
    }

    fn message_ids(events: &[SessionEvent]) -> Vec<String> {
        events
            .iter()
            .filter_map(|event| match event {
                SessionEvent::Message(json) => json["id"].as_str().map(str::to_string),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_first_pass_sends_status_and_does_not_replay_history() {
        let mut cursor = EventCursor::default();

        let events = cursor.advance(json!({ "status": "idle" }), vec![message("m1", "old")]);

        assert_eq!(
            events,
            vec![SessionEvent::Status(json!({ "status": "idle" }))]
        );
    }

    #[test]
    fn later_passes_send_only_what_changed() {
        let mut cursor = EventCursor::default();
        cursor.advance(json!({ "status": "idle" }), vec![message("m1", "old")]);

        // Nothing changed.
        let quiet = cursor.advance(json!({ "status": "idle" }), vec![message("m1", "old")]);
        assert!(quiet.is_empty(), "{quiet:?}");

        // Status changed and a message arrived.
        let busy = cursor.advance(
            json!({ "status": "busy" }),
            vec![message("m1", "old"), message("m2", "new")],
        );
        assert_eq!(busy[0], SessionEvent::Status(json!({ "status": "busy" })));
        assert_eq!(message_ids(&busy), vec!["m2"]);

        // A stored message was updated in place: it is sent again, same id.
        let updated = cursor.advance(
            json!({ "status": "busy" }),
            vec![message("m1", "old"), message("m2", "new, longer")],
        );
        assert_eq!(message_ids(&updated), vec!["m2"]);
        let SessionEvent::Message(sent) = &updated[0] else {
            panic!("expected a message event: {updated:?}");
        };
        assert_eq!(sent["content"][0]["text"], "new, longer");
    }

    #[test]
    fn events_are_named_for_sse() {
        let rendered = format!("{}", SessionEvent::End.into_sse());
        assert!(rendered.contains("event:end"), "{rendered}");
    }
}
