//! In-band excessive snapshot polling detection for wait-capable tools.
//!
//! Tracks consecutive identical outcome fingerprints locally so tools can return
//! guidance before the global circuit breaker threshold.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollTrackerVerdict {
    Ok,
    Excessive,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PollTracker {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_fingerprint: Option<String>,
    #[serde(default)]
    consecutive_identical: u32,
}

impl PollTracker {
    pub fn observe(&mut self, fingerprint: &str, threshold: u32) -> PollTrackerVerdict {
        if self.last_fingerprint.as_deref() == Some(fingerprint) {
            self.consecutive_identical = self.consecutive_identical.saturating_add(1);
        } else {
            self.last_fingerprint = Some(fingerprint.to_string());
            self.consecutive_identical = 1;
        }

        if self.consecutive_identical >= threshold {
            PollTrackerVerdict::Excessive
        } else {
            PollTrackerVerdict::Ok
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn consecutive_identical(&self) -> u32 {
        self.consecutive_identical
    }
}

pub fn poll_tracker_key(tool_name: &str, resource_id: &str) -> String {
    format!("{tool_name}:{resource_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observe_resets_on_fingerprint_change() {
        let mut tracker = PollTracker::default();
        assert_eq!(tracker.observe("running:1", 3), PollTrackerVerdict::Ok);
        assert_eq!(tracker.observe("running:1", 3), PollTrackerVerdict::Ok);
        assert_eq!(tracker.observe("running:2", 3), PollTrackerVerdict::Ok);
        assert_eq!(tracker.consecutive_identical(), 1);
    }

    #[test]
    fn observe_flags_excessive_at_threshold() {
        let mut tracker = PollTracker::default();
        assert_eq!(tracker.observe("running:1", 3), PollTrackerVerdict::Ok);
        assert_eq!(tracker.observe("running:1", 3), PollTrackerVerdict::Ok);
        assert_eq!(
            tracker.observe("running:1", 3),
            PollTrackerVerdict::Excessive
        );
    }
}
