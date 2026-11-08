//! Loop-prevention thresholds, including the wait ladder for slow identical calls.

/// Wait-detection thresholds for slow identical calls (#2610).
///
/// A call whose recorded execution duration reaches `wait_duration_ms` is a **wait
/// call**. Identical wait calls skip the fast-loop ladder entirely: instead, after
/// `guide_count` consecutive identical wait calls the next call is short-circuited
/// with `WaitGuidance`, and after `break_count` further identical wait calls the
/// hard break trips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitPreventionSettings {
    pub wait_duration_ms: u64,
    pub guide_count: usize,
    pub break_count: usize,
}

impl Default for WaitPreventionSettings {
    fn default() -> Self {
        Self {
            wait_duration_ms: 10_000,
            guide_count: 10,
            break_count: 10,
        }
    }
}

/// Load loop-prevention thresholds from advanced settings.
///
/// Returns `(threshold, hard_break_offset, wait_settings)`:
/// - `threshold`: consecutive identical **(call, outcome)** streak length that
///   triggers soft natural recovery (default 3). Outcome text changes reset the
///   streak for both per-tool and multi-tool batch scanners.
/// - `hard_break_offset`: added to `threshold` for hard break (default **2**).
///   Soft blocks every count in `[threshold, hard_break)`. For repeated **errors**,
///   `NaturalRecoveryErrorEscalate` fires once at `hard_break - 1` when
///   `offset >= 2` (with offset=1 the escalate step is skipped: soft then hard).
/// - `wait_settings`: slow-call ladder that replaces the fast rule for identical
///   long-running calls (see [`WaitPreventionSettings`], #2610).
pub(crate) async fn load_loop_prevention_settings() -> (usize, usize, WaitPreventionSettings) {
    let settings = crate::models::advanced_settings::AdvancedSettings::load_global().await;
    (
        settings.loop_prevention_threshold,
        settings.loop_prevention_hard_break_offset,
        WaitPreventionSettings {
            wait_duration_ms: u64::from(settings.tool_wait_duration_secs).saturating_mul(1000),
            guide_count: settings.tool_wait_guide_count,
            break_count: settings.tool_wait_break_count,
        },
    )
}
