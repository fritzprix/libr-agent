use crate::models::chat::Message;
use std::collections::{HashMap, HashSet};
use tokio::sync::oneshot;

/// Tracks the state of pending tool executions for a conversational turn
#[derive(Debug)]
pub struct PendingToolExecution {
    pub message_id: String,
    pub total_expected: usize,
    /// Maps tool_call_id to tool_name for event emission
    pub tool_names: HashMap<String, String>,
    /// Tool call IDs expected for the current message execution
    pub expected_tool_call_ids: HashSet<String>,
    /// Tool call IDs already completed for the current message execution
    pub completed_tool_call_ids: HashSet<String>,
    /// History rows that must wait until this batch completes so tool-call /
    /// tool-result pairing stays contiguous for provider APIs.
    pub deferred_history_append: Vec<Message>,
}

/// Pending events waiting to be processed by the workflow
#[derive(Debug, Clone)]
pub enum PendingEvent {
    Message(String), // Stores Message ID
}

/// Manages pending events for a session
#[derive(Debug, Default)]
pub struct PendingEventManager {
    events: Vec<PendingEvent>,
}

impl PendingEventManager {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn add(&mut self, event: PendingEvent) {
        self.events.push(event);
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Drain the next pending message id (FIFO), leaving the rest queued.
    pub fn drain_one_message(&mut self) -> Option<String> {
        let index = self
            .events
            .iter()
            .position(|event| matches!(event, PendingEvent::Message(_)))?;
        match self.events.remove(index) {
            PendingEvent::Message(id) => Some(id),
        }
    }

    /// Re-insert a message at the FIFO front after a failed durable claim/cancel.
    pub fn restore_front_message(&mut self, message_id: String) {
        self.events.insert(0, PendingEvent::Message(message_id));
    }

    /// Re-insert multiple messages at the FIFO front, preserving order.
    pub fn restore_front_pending_messages(&mut self, message_ids: &[String]) {
        for message_id in message_ids.iter().rev() {
            self.events
                .insert(0, PendingEvent::Message(message_id.clone()));
        }
    }

    /// Whether a specific message id is currently waiting.
    pub fn contains_message(&self, message_id: &str) -> bool {
        self.events.iter().any(|event| match event {
            PendingEvent::Message(id) => id == message_id,
        })
    }

    /// Remove a specific pending message id. Returns true when found.
    pub fn remove_message(&mut self, message_id: &str) -> bool {
        let before = self.events.len();
        self.events.retain(|event| match event {
            PendingEvent::Message(id) => id != message_id,
        });
        self.events.len() != before
    }

    /// Snapshot of pending message ids in FIFO order.
    pub fn message_ids(&self) -> Vec<String> {
        self.events
            .iter()
            .map(|event| match event {
                PendingEvent::Message(id) => id.clone(),
            })
            .collect()
    }

    /// Drain all pending messages and return their IDs
    pub fn drain_messages(&mut self) -> Vec<String> {
        let mut messages = Vec::new();
        self.events.retain(|event| match event {
            PendingEvent::Message(id) => {
                messages.push(id.clone());
                false
            }
        });
        messages
    }

    pub fn count(&self) -> usize {
        self.events.len()
    }

    #[allow(dead_code)]
    pub fn has_pending(&self) -> bool {
        !self.events.is_empty()
    }
}

/// Data for a pending tool execution approval
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingApprovalKind {
    Standard,
    Hard,
    /// A command that runs as root (runShell `asRoot`). No execution mode skips it.
    Root,
}

/// Data for a pending tool execution approval
#[derive(Debug)]
pub struct PendingApprovalData {
    pub sender: oneshot::Sender<bool>,
    pub tool_name: String,
    pub arguments: String,
    pub approval_kind: PendingApprovalKind,
    pub request_id: Option<String>,
    pub description: Option<String>,
    pub input_preview: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_event_manager_flow() {
        let mut manager = PendingEventManager::new();
        assert!(!manager.has_pending());

        manager.add(PendingEvent::Message("msg1".into()));
        manager.add(PendingEvent::Message("msg2".into()));
        assert!(manager.has_pending());
        assert_eq!(manager.count(), 2);
        assert_eq!(manager.message_ids(), vec!["msg1", "msg2"]);

        let first = manager.drain_one_message();
        assert_eq!(first.as_deref(), Some("msg1"));
        assert_eq!(manager.count(), 1);

        assert!(manager.remove_message("msg2"));
        assert!(!manager.has_pending());
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_clear_events() {
        let mut manager = PendingEventManager::new();
        manager.add(PendingEvent::Message("msg1".into()));

        manager.clear();
        assert!(!manager.has_pending());
        assert_eq!(manager.count(), 0);
        assert!(manager.drain_messages().is_empty());
    }
}
