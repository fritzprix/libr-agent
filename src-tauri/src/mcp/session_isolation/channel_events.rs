use crate::mcp::types::{ChannelNotification, ChannelPermissionVerdict};
use log::warn;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{self, Sender};

const CHANNEL_DROP_METRICS_LOG_INTERVAL: Duration = Duration::from_secs(60);

static CHANNEL_EVENTS_DROPPED_BUFFER_FULL: AtomicU64 = AtomicU64::new(0);
static CHANNEL_EVENTS_DROPPED_RECEIVER_CLOSED: AtomicU64 = AtomicU64::new(0);
static CHANNEL_DROP_METRICS_LAST_LOG: LazyLock<Mutex<Instant>> =
    LazyLock::new(|| Mutex::new(Instant::now()));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChannelDropKind {
    BufferFull,
    ReceiverClosed,
}

fn record_channel_event_drop(kind: ChannelDropKind, source: &str) {
    match kind {
        ChannelDropKind::BufferFull => {
            CHANNEL_EVENTS_DROPPED_BUFFER_FULL.fetch_add(1, Ordering::Relaxed);
        }
        ChannelDropKind::ReceiverClosed => {
            CHANNEL_EVENTS_DROPPED_RECEIVER_CLOSED.fetch_add(1, Ordering::Relaxed);
        }
    }

    maybe_log_channel_drop_metrics(source);
}

fn maybe_log_channel_drop_metrics(trigger_source: &str) {
    let mut last_log = CHANNEL_DROP_METRICS_LAST_LOG
        .lock()
        .expect("channel drop metrics lock poisoned");
    if last_log.elapsed() < CHANNEL_DROP_METRICS_LOG_INTERVAL {
        return;
    }

    let buffer_full = CHANNEL_EVENTS_DROPPED_BUFFER_FULL.swap(0, Ordering::Relaxed);
    let receiver_closed = CHANNEL_EVENTS_DROPPED_RECEIVER_CLOSED.swap(0, Ordering::Relaxed);

    if buffer_full > 0 || receiver_closed > 0 {
        warn!(
            "Channel event drops in last {}s (triggered by '{}'): buffer_full={}, receiver_closed={}",
            CHANNEL_DROP_METRICS_LOG_INTERVAL.as_secs(),
            trigger_source,
            buffer_full,
            receiver_closed
        );
    }

    *last_log = Instant::now();
}

/// Test-only snapshot of channel event drop counters (not reset).
#[doc(hidden)]
pub fn channel_drop_metrics_snapshot_for_test() -> (u64, u64) {
    (
        CHANNEL_EVENTS_DROPPED_BUFFER_FULL.load(Ordering::Relaxed),
        CHANNEL_EVENTS_DROPPED_RECEIVER_CLOSED.load(Ordering::Relaxed),
    )
}

/// Test-only reset of channel event drop counters.
#[doc(hidden)]
pub fn reset_channel_drop_metrics_for_test() {
    CHANNEL_EVENTS_DROPPED_BUFFER_FULL.store(0, Ordering::Relaxed);
    CHANNEL_EVENTS_DROPPED_RECEIVER_CLOSED.store(0, Ordering::Relaxed);
    if let Ok(mut last_log) = CHANNEL_DROP_METRICS_LAST_LOG.lock() {
        *last_log = Instant::now();
    }
}

/// Test-only: move the drop-metrics log window into the past so the next drop flushes counters.
#[doc(hidden)]
pub fn advance_channel_drop_metrics_log_window_for_test() {
    if let Ok(mut last_log) = CHANNEL_DROP_METRICS_LAST_LOG.lock() {
        *last_log = Instant::now()
            .checked_sub(CHANNEL_DROP_METRICS_LOG_INTERVAL + Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
    }
}

/// Bounded buffer for native channel events per session.
pub const CHANNEL_EVENT_BUFFER_SIZE: usize = 1024;

/// Maximum allowed length for channel message `content` (bytes).
pub const MAX_CHANNEL_CONTENT_BYTES: usize = 8192;

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
                record_channel_event_drop(ChannelDropKind::BufferFull, source);
            }
            mpsc::error::TrySendError::Closed(_) => {
                record_channel_event_drop(ChannelDropKind::ReceiverClosed, source);
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
            let content = params.get("content")?.as_str()?;
            if content.len() > MAX_CHANNEL_CONTENT_BYTES {
                warn!(
                    "Channel message content too large ({} bytes) from '{}'; dropping",
                    content.len(),
                    server_name
                );
                return None;
            }
            let content = content.to_string();
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
        assert!(
            result.is_ok(),
            "detached sender should not be closed immediately"
        );
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
    fn rejects_oversized_channel_message_content() {
        let oversized = "x".repeat(MAX_CHANNEL_CONTENT_BYTES + 1);
        let line = format!(
            r#"{{"jsonrpc":"2.0","method":"claude/channel","params":{{"content":"{}"}}}}"#,
            oversized
        );
        assert!(
            try_parse_channel_event(line.as_bytes(), "telegram").is_none(),
            "oversized channel content should be dropped"
        );
    }

    #[test]
    fn accepts_channel_message_at_content_limit() {
        let content = "x".repeat(MAX_CHANNEL_CONTENT_BYTES);
        let line = format!(
            r#"{{"jsonrpc":"2.0","method":"claude/channel","params":{{"content":"{}"}}}}"#,
            content
        );
        let event = try_parse_channel_event(line.as_bytes(), "telegram")
            .expect("content at limit should parse");
        match event {
            SessionChannelEvent::Message { notification, .. } => {
                assert_eq!(notification.content.len(), MAX_CHANNEL_CONTENT_BYTES);
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
