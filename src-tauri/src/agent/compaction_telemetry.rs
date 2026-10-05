//! In-memory compaction telemetry for Session API / Harbor harvest.
//!
//! Events are session-scoped and ephemeral (not SQLite). Harbor harvest runs
//! before session delete, so the ring buffer is sufficient for eval v1.

use serde::Serialize;
use sha2::{Digest, Sha256};

pub const MAX_COMPACTION_TELEMETRY_EVENTS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionTelemetryPhase {
    Started,
    Succeeded,
    Failed,
    HardFallback,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionTelemetryEvent {
    pub seq: u32,
    pub phase: CompactionTelemetryPhase,
    pub at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condensed_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epoch_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_before: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_after_projection: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct CompactionTelemetryEventDraft {
    pub phase: CompactionTelemetryPhase,
    pub to_id: Option<String>,
    pub condensed_count: Option<usize>,
    pub error: Option<String>,
    pub epoch_path: Option<String>,
    pub fallback_path: Option<String>,
    pub prompt_tokens_before: Option<u64>,
    pub prompt_tokens_after_projection: Option<u64>,
}

impl Default for CompactionTelemetryPhase {
    fn default() -> Self {
        Self::Started
    }
}

pub fn utc_now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn summary_sha256_hex(summary: &str) -> String {
    let digest = Sha256::digest(summary.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn push_compaction_event(
    events: &mut Vec<CompactionTelemetryEvent>,
    draft: CompactionTelemetryEventDraft,
) {
    let seq = events
        .last()
        .map(|event| event.seq.saturating_add(1))
        .unwrap_or(1);
    events.push(CompactionTelemetryEvent {
        seq,
        phase: draft.phase,
        at: utc_now_rfc3339(),
        to_id: draft.to_id,
        condensed_count: draft.condensed_count,
        error: draft.error,
        epoch_path: draft.epoch_path,
        fallback_path: draft.fallback_path,
        prompt_tokens_before: draft.prompt_tokens_before,
        prompt_tokens_after_projection: draft.prompt_tokens_after_projection,
    });
    if events.len() > MAX_COMPACTION_TELEMETRY_EVENTS {
        let overflow = events.len() - MAX_COMPACTION_TELEMETRY_EVENTS;
        events.drain(0..overflow);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_compaction_event_caps_ring_and_sequences() {
        let mut events = Vec::new();
        for _ in 0..(MAX_COMPACTION_TELEMETRY_EVENTS + 3) {
            push_compaction_event(
                &mut events,
                CompactionTelemetryEventDraft {
                    phase: CompactionTelemetryPhase::Started,
                    ..CompactionTelemetryEventDraft::default()
                },
            );
        }
        assert_eq!(events.len(), MAX_COMPACTION_TELEMETRY_EVENTS);
        assert_eq!(events.first().map(|e| e.seq), Some(4));
        assert_eq!(
            events.last().map(|e| e.seq),
            Some((MAX_COMPACTION_TELEMETRY_EVENTS + 3) as u32)
        );
    }

    #[test]
    fn summary_sha256_hex_is_stable() {
        assert_eq!(
            summary_sha256_hex("hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }
}
