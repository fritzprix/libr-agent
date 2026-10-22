use crate::agent::llm::types::CompactRequest;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub enum CompactionKind {
    Manual,
    Preflight,
}

impl CompactionKind {
    pub fn mode_label(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Preflight => "preflight",
        }
    }

    pub fn blocks_workflow(&self) -> bool {
        !matches!(self, Self::Manual)
    }

    pub fn resumes_completion_after_compact(&self) -> bool {
        matches!(self, Self::Preflight)
    }
}

#[derive(Debug, Clone)]
pub struct InFlightCompaction {
    pub kind: CompactionKind,
    pub current_tail_id: Option<String>,
    pub started_at_ms: i64,
}

#[derive(Debug, Clone)]
pub enum CompactionPhase {
    Idle,
    InFlight(InFlightCompaction),
}

#[derive(Debug, Clone)]
pub struct CompactionSnapshot {
    pub phase: CompactionPhase,
    pub last_compacted_tail_id: Option<String>,
    pub retry_attempt: u32,
    pub recovery_phase: CompactionRecoveryPhase,
    pub summary_retry_count: u32,
}

impl CompactionSnapshot {
    pub fn started_at_ms(&self) -> Option<i64> {
        match &self.phase {
            CompactionPhase::Idle => None,
            CompactionPhase::InFlight(in_flight) => Some(in_flight.started_at_ms),
        }
    }

    pub fn mode_label(&self) -> &'static str {
        match &self.phase {
            CompactionPhase::Idle => "manual",
            CompactionPhase::InFlight(in_flight) => in_flight.kind.mode_label(),
        }
    }

    pub fn blocks_workflow(&self) -> bool {
        match &self.phase {
            CompactionPhase::Idle => false,
            CompactionPhase::InFlight(in_flight) => in_flight.kind.blocks_workflow(),
        }
    }

    /// True while any compaction kind is in flight (Manual or Preflight).
    ///
    /// Unlike [`Self::blocks_workflow`], this is true for Manual too — callers
    /// that must avoid racing a new workflow against an in-flight compact
    /// (e.g. user-message enqueue) should use this.
    pub fn is_in_flight(&self) -> bool {
        matches!(self.phase, CompactionPhase::InFlight(_))
    }
}

#[derive(Debug, Clone)]
pub enum CompactionBeginOutcome {
    Started,
    AlreadyInFlight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionReuseOutcome {
    NotInFlight,
    NoChange,
    Promoted,
}

#[derive(Debug, Clone)]
pub enum CompactionResumeAction {
    Nothing,
    ResumeCompletion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionRecoveryPhase {
    CacheAligned,
    OverflowRecovery,
    DegradedTools,
}

#[derive(Debug, Clone)]
pub struct CompactionRuntimeState {
    /// Closed runtime phase model for active compaction work.
    phase: Arc<RwLock<CompactionPhase>>,

    /// The ID of the last message in the stack at the moment compaction was triggered.
    /// This survives successful settlement so future triggers can skip same-tail work.
    last_compacted_tail_id: Arc<RwLock<Option<String>>>,

    /// Retry ladder state for budget-related compaction overflows.
    retry_attempt: Arc<RwLock<u32>>,

    /// Phase ladder for compaction overflow recovery.
    recovery_phase: Arc<RwLock<CompactionRecoveryPhase>>,

    /// Retry counter for low-quality or empty compaction summaries.
    summary_retry_count: Arc<RwLock<u32>>,

    /// Most recent compact request payload, retained while compaction is in flight so
    /// the backend can re-emit it without involving the frontend error path.
    current_request: Arc<RwLock<Option<CompactRequest>>>,

    /// Ephemeral compaction telemetry ring for Session API / Harbor harvest.
    telemetry_events:
        Arc<RwLock<Vec<crate::agent::compaction_telemetry::CompactionTelemetryEvent>>>,
}

impl CompactionRuntimeState {
    pub fn new() -> Self {
        Self {
            phase: Arc::new(RwLock::new(CompactionPhase::Idle)),
            last_compacted_tail_id: Arc::new(RwLock::new(None)),
            retry_attempt: Arc::new(RwLock::new(0)),
            recovery_phase: Arc::new(RwLock::new(CompactionRecoveryPhase::CacheAligned)),
            summary_retry_count: Arc::new(RwLock::new(0)),
            current_request: Arc::new(RwLock::new(None)),
            telemetry_events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn with_test_state(phase: CompactionPhase, last_tail_id: Option<String>) -> Self {
        Self {
            phase: Arc::new(RwLock::new(phase)),
            last_compacted_tail_id: Arc::new(RwLock::new(last_tail_id)),
            retry_attempt: Arc::new(RwLock::new(0)),
            recovery_phase: Arc::new(RwLock::new(CompactionRecoveryPhase::CacheAligned)),
            summary_retry_count: Arc::new(RwLock::new(0)),
            current_request: Arc::new(RwLock::new(None)),
            telemetry_events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn push_telemetry_event(
        &self,
        draft: crate::agent::compaction_telemetry::CompactionTelemetryEventDraft,
    ) {
        let mut events = self.telemetry_events.write().await;
        crate::agent::compaction_telemetry::push_compaction_event(&mut events, draft);
    }

    pub async fn telemetry_events(
        &self,
    ) -> Vec<crate::agent::compaction_telemetry::CompactionTelemetryEvent> {
        self.telemetry_events.read().await.clone()
    }

    pub async fn clear_telemetry_events(&self) {
        self.telemetry_events.write().await.clear();
    }

    pub async fn snapshot(&self) -> CompactionSnapshot {
        CompactionSnapshot {
            phase: self.phase.read().await.clone(),
            last_compacted_tail_id: self.last_compacted_tail_id.read().await.clone(),
            retry_attempt: *self.retry_attempt.read().await,
            recovery_phase: *self.recovery_phase.read().await,
            summary_retry_count: *self.summary_retry_count.read().await,
        }
    }

    pub async fn last_compacted_tail_id(&self) -> Option<String> {
        self.last_compacted_tail_id.read().await.clone()
    }

    pub async fn clear_in_flight_state(&self, clear_last_compacted_tail_id: bool) {
        *self.phase.write().await = CompactionPhase::Idle;
        *self.summary_retry_count.write().await = 0;
        *self.current_request.write().await = None;

        if clear_last_compacted_tail_id {
            *self.last_compacted_tail_id.write().await = None;
        }
    }

    pub async fn clear_runtime_state(&self, clear_last_compacted_tail_id: bool) {
        self.clear_in_flight_state(clear_last_compacted_tail_id)
            .await;
        self.reset_recovery_progress().await;
    }

    pub async fn retry_attempt(&self) -> u32 {
        *self.retry_attempt.read().await
    }

    pub async fn recovery_phase(&self) -> CompactionRecoveryPhase {
        *self.recovery_phase.read().await
    }

    pub async fn summary_retry_count(&self) -> u32 {
        *self.summary_retry_count.read().await
    }

    pub async fn increment_summary_retry_count(&self) -> u32 {
        let mut retry_count = self.summary_retry_count.write().await;
        *retry_count += 1;
        *retry_count
    }

    pub async fn reset_summary_retry_count(&self) {
        *self.summary_retry_count.write().await = 0;
    }

    pub async fn current_request(&self) -> Option<CompactRequest> {
        self.current_request.read().await.clone()
    }

    pub async fn set_current_request(&self, request: CompactRequest) {
        *self.current_request.write().await = Some(request);
    }

    pub async fn increment_retry_attempt(&self) -> u32 {
        let mut retry_attempt = self.retry_attempt.write().await;
        *retry_attempt += 1;
        *retry_attempt
    }

    pub async fn reset_retry_attempt(&self) {
        *self.retry_attempt.write().await = 0;
    }

    pub async fn transition_to_overflow_recovery(&self) {
        *self.retry_attempt.write().await = 0;
        *self.recovery_phase.write().await = CompactionRecoveryPhase::OverflowRecovery;
    }

    pub async fn transition_to_degraded_tools(&self) {
        *self.retry_attempt.write().await = 0;
        *self.recovery_phase.write().await = CompactionRecoveryPhase::DegradedTools;
    }

    pub async fn reset_recovery_progress(&self) {
        *self.retry_attempt.write().await = 0;
        *self.recovery_phase.write().await = CompactionRecoveryPhase::CacheAligned;
    }

    pub async fn set_recovery_progress(
        &self,
        recovery_phase: CompactionRecoveryPhase,
        retry_attempt: u32,
    ) {
        *self.retry_attempt.write().await = retry_attempt;
        *self.recovery_phase.write().await = recovery_phase;
    }

    pub async fn try_begin(
        &self,
        kind: CompactionKind,
        current_tail_id: Option<String>,
        started_at_ms: i64,
    ) -> CompactionBeginOutcome {
        let mut phase = self.phase.write().await;
        if matches!(&*phase, CompactionPhase::InFlight(_)) {
            return CompactionBeginOutcome::AlreadyInFlight;
        }

        *phase = CompactionPhase::InFlight(InFlightCompaction {
            kind,
            current_tail_id: current_tail_id.clone(),
            started_at_ms,
        });
        *self.last_compacted_tail_id.write().await = current_tail_id;
        *self.summary_retry_count.write().await = 0;
        *self.current_request.write().await = None;
        CompactionBeginOutcome::Started
    }

    pub async fn arm_resume_completion(&self) -> CompactionReuseOutcome {
        let mut phase = self.phase.write().await;
        match &mut *phase {
            CompactionPhase::Idle => CompactionReuseOutcome::NotInFlight,
            CompactionPhase::InFlight(in_flight) => match &in_flight.kind {
                CompactionKind::Manual => {
                    in_flight.kind = CompactionKind::Preflight;
                    CompactionReuseOutcome::Promoted
                }
                CompactionKind::Preflight => CompactionReuseOutcome::NoChange,
            },
        }
    }

    pub async fn complete_success(&self) -> CompactionResumeAction {
        self.reset_recovery_progress().await;
        self.reset_summary_retry_count().await;
        *self.current_request.write().await = None;
        match std::mem::replace(&mut *self.phase.write().await, CompactionPhase::Idle) {
            CompactionPhase::Idle => CompactionResumeAction::Nothing,
            CompactionPhase::InFlight(in_flight) => match in_flight.kind {
                CompactionKind::Manual => CompactionResumeAction::Nothing,
                CompactionKind::Preflight => CompactionResumeAction::ResumeCompletion,
            },
        }
    }

    pub fn is_settled(&self) -> bool {
        match self.phase.try_read() {
            Ok(phase) => matches!(&*phase, CompactionPhase::Idle),
            Err(_) => false,
        }
    }
}

impl Default for CompactionRuntimeState {
    fn default() -> Self {
        Self::new()
    }
}
