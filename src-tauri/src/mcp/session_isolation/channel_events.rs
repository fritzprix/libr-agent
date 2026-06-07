use crate::mcp::types::{ChannelNotification, ChannelPermissionVerdict};
use log::warn;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use tokio::sync::mpsc::{self, Sender};

/// Bounded buffer for native channel events per session.
pub const CHANNEL_EVENT_BUFFER_SIZE: usize = 1024;

pub type ChannelEventSender = Sender<SessionChannelEvent>;
pub type ChannelEventReceiver = mpsc::Receiver<SessionChannelEvent>;

pub fn create_channel_event_bus() -> (ChannelEventSender, ChannelEventReceiver) {
    mpsc::channel(CHANNEL_EVENT_BUFFER_SIZE)
}

static DETACHED_CHANNEL_RECEIVERS: LazyLock<Mutex<Vec<ChannelEventReceiver>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

/// Creates a sender whose receiver stays alive for the process lifetime.
///
/// Use in tests that construct `SessionMCPManager` without spawning
/// `spawn_session_channel_dispatch_task`. Without a live receiver, bounded
/// `try_send()` calls fail with `TrySendError::Closed`.
pub fn create_detached_channel_event_sender() -> ChannelEventSender {
    let (tx, rx) = create_channel_event_bus();
    DETACHED_CHANNEL_RECEIVERS
        .lock()
        .expect("detached channel receiver lock poisoned")
        .push(rx);
    tx
}

/// Non-blocking send used from the synchronous stdio codec decoder.
pub fn try_emit_channel_event(tx: &ChannelEventSender, event: SessionChannelEvent, source: &str) {
    if let Err(error) = tx.try_send(event) {
        match error {
            mpsc::error::TrySendError::Full(_) => {
                warn!(
                    "Channel event buffer full (capacity {}) for '{}'; dropping event",
                    CHANNEL_EVENT_BUFFER_SIZE, source
                );
            }
            mpsc::error::TrySendError::Closed(_) => {
                warn!(
                    "Channel event receiver dropped for '{}'; dropping event",
                    source
                );
            }
        }
    }
}

/// Events emitted by the channel-aware stdio transport for a single session.
#[derive(Debug, Clone)]
pub enum SessionChannelEvent {
    Message {
        server_name: String,
        notification: ChannelNotification,
    },
    PermissionVerdict {
        server_name: String,
        verdict: ChannelPermissionVerdict,
    },
}

pub fn flatten_meta_object(value: &serde_json::Value) -> HashMap<String, String> {
    let Some(object) = value.as_object() else {
        return HashMap::new();
    };

    object
        .iter()
        .map(|(key, value)| (key.clone(), meta_value_to_string(value)))
        .collect()
}

fn meta_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Number(number) => number.to_string(),
        serde_json::Value::Bool(flag) => flag.to_string(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

pub fn normalize_channel_method(method: &str) -> Option<ChannelMethodKind> {
    match method {
        "claude/channel" | "notifications/claude/channel" => Some(ChannelMethodKind::Message),
        "claude/channel/permission" | "notifications/claude/channel/permission" => {
            Some(ChannelMethodKind::PermissionVerdict)
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelMethodKind {
    Message,
    PermissionVerdict,
}

pub fn try_parse_channel_event(line: &[u8], server_name: &str) -> Option<SessionChannelEvent> {
    let line_str = std::str::from_utf8(line).ok()?;
    let json: serde_json::Value = serde_json::from_str(line_str).ok()?;
    if json.get("id").is_some() {
        return None;
    }

    let method = json.get("method")?.as_str()?;
    let kind = normalize_channel_method(method)?;
    let params = json.get("params")?;

    match kind {
        ChannelMethodKind::Message => {
            let content = params.get("content")?.as_str()?.to_string();
            let meta = params
                .get("meta")
                .map(flatten_meta_object)
                .unwrap_or_default();

            Some(SessionChannelEvent::Message {
                server_name: server_name.to_string(),
                notification: ChannelNotification { content, meta },
            })
        }
        ChannelMethodKind::PermissionVerdict => {
            let verdict: ChannelPermissionVerdict = serde_json::from_value(params.clone()).ok()?;
            Some(SessionChannelEvent::PermissionVerdict {
                server_name: server_name.to_string(),
                verdict,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detached_sender_keeps_channel_open() {
        let tx = create_detached_channel_event_sender();
        let result = tx.try_send(SessionChannelEvent::Message {
            server_name: "telegram".to_string(),
            notification: ChannelNotification {
                content: "ping".to_string(),
                meta: HashMap::new(),
            },
        });
        assert!(result.is_ok(), "detached sender should not be closed immediately");
    }

    #[test]
    fn parses_channel_message_notification() {
        let line = br#"{"jsonrpc":"2.0","method":"claude/channel","params":{"content":"hello","meta":{"chat_id":"1","sender_id":42}}}"#;
        let event =
            try_parse_channel_event(line, "telegram").expect("channel notification should parse");

        match event {
            SessionChannelEvent::Message {
                server_name,
                notification,
            } => {
                assert_eq!(server_name, "telegram");
                assert_eq!(notification.content, "hello");
                assert_eq!(notification.meta.get("chat_id"), Some(&"1".to_string()));
                assert_eq!(notification.meta.get("sender_id"), Some(&"42".to_string()));
            }
            SessionChannelEvent::PermissionVerdict { .. } => {
                panic!("expected message event");
            }
        }
    }

    #[test]
    fn parses_channel_permission_verdict() {
        let line = br#"{"jsonrpc":"2.0","method":"notifications/claude/channel/permission","params":{"request_id":"abc12","behavior":"allow"}}"#;
        let event =
            try_parse_channel_event(line, "telegram").expect("permission verdict should parse");

        match event {
            SessionChannelEvent::PermissionVerdict {
                server_name,
                verdict,
            } => {
                assert_eq!(server_name, "telegram");
                assert_eq!(verdict.request_id, "abc12");
                assert_eq!(verdict.behavior, "allow");
            }
            SessionChannelEvent::Message { .. } => {
                panic!("expected permission verdict event");
            }
        }
    }
}
