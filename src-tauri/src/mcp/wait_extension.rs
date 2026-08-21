//! Declarative wait-capable tool contract (`x-libragent-wait`).
//!
//! Documents how a builtin tool supports snapshot polling vs blocking wait so
//! loop-recovery, PollTracker, and tool authors share one schema.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Where in-band PollTracker state is stored for snapshot polling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PollTrackerScope {
    /// Caller agent session (`AgentSession.tool_poll_trackers`).
    Session,
    /// Workspace process registry entry (`ProcessEntry.poll_tracker`).
    ProcessRegistry,
}

/// Literal argument match for snapshot or blocking wait modes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitModeMatch {
    /// When set, snapshot mode requires this boolean value on `wait_param`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait: Option<bool>,
    /// When set, snapshot mode requires this timeout seconds value on `timeout_param`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

/// Tool-level extension declaring wait/poll semantics (serialized as `x-libragent-wait`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibragentWaitExtension {
    /// Input parameter that identifies the polled resource (`sessionId`, `processId`, …).
    pub resource_id_param: String,
    /// Boolean parameter toggling blocking wait, when present (`wait`, `waitForResponse`, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_param: Option<String>,
    /// Timeout parameter in seconds, when present (`timeout`, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_param: Option<String>,
    /// Argument values that mean non-blocking snapshot/poll (discouraged for tight loops).
    pub snapshot_mode: WaitModeMatch,
    /// Preferred blocking wait argument pattern.
    pub blocking_mode: WaitModeMatch,
    /// structuredContent field emitted in tool results for circuit-breaker fingerprints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_fingerprint_field: Option<String>,
    /// Where PollTracker state lives for in-band excessive polling guidance.
    pub poll_tracker_scope: PollTrackerScope,
}

impl LibragentWaitExtension {
    pub fn check_session() -> Self {
        Self {
            resource_id_param: "sessionId".to_string(),
            wait_param: Some("wait".to_string()),
            timeout_param: Some("timeout".to_string()),
            snapshot_mode: WaitModeMatch {
                wait: Some(false),
                timeout: None,
            },
            blocking_mode: WaitModeMatch {
                wait: Some(true),
                timeout: None,
            },
            loop_fingerprint_field: Some("loopFingerprint".to_string()),
            poll_tracker_scope: PollTrackerScope::Session,
        }
    }

    pub fn wait_for_process() -> Self {
        Self {
            resource_id_param: "processId".to_string(),
            wait_param: None,
            timeout_param: Some("timeout".to_string()),
            snapshot_mode: WaitModeMatch {
                wait: None,
                timeout: Some(0),
            },
            blocking_mode: WaitModeMatch {
                wait: None,
                timeout: None,
            },
            loop_fingerprint_field: Some("loopFingerprint".to_string()),
            poll_tracker_scope: PollTrackerScope::ProcessRegistry,
        }
    }
}

/// Returns the expected JSON shape for docs/tests.
pub fn check_session_wait_extension_json() -> Value {
    json!(LibragentWaitExtension::check_session())
}

pub fn wait_for_process_wait_extension_json() -> Value {
    json!(LibragentWaitExtension::wait_for_process())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_session_extension_serializes_camel_case_keys() {
        let value = serde_json::to_value(LibragentWaitExtension::check_session()).unwrap();
        assert_eq!(value["resourceIdParam"], "sessionId");
        assert_eq!(value["waitParam"], "wait");
        assert_eq!(value["snapshotMode"]["wait"], false);
        assert_eq!(value["blockingMode"]["wait"], true);
        assert_eq!(value["pollTrackerScope"], "session");
    }

    #[test]
    fn wait_for_process_extension_uses_timeout_snapshot_mode() {
        let value = serde_json::to_value(LibragentWaitExtension::wait_for_process()).unwrap();
        assert_eq!(value["resourceIdParam"], "processId");
        assert!(value.get("waitParam").is_none());
        assert_eq!(value["timeoutParam"], "timeout");
        assert_eq!(value["snapshotMode"]["timeout"], 0);
        assert_eq!(value["pollTrackerScope"], "processRegistry");
    }
}
