//! Backend model of the `advancedSettings` settings row.
//!
//! Schema and defaults live here. Stored JSON may be missing fields, hold
//! values of the wrong type, or not parse at all; [`AdvancedSettings::from_stored`]
//! always returns a complete, in-range object, falling back per field.
//!
//! The frontend's `AdvancedSettings` type is generated from this model
//! (`src/lib/generated/`); `ts_bindings` fails when the checked-in file is stale.

use crate::agent::concurrency::{
    DEFAULT_MAX_ACTIVE_AGENTS, DEFAULT_MAX_ACTIVE_PROCESSES, DEFAULT_MAX_SUSPENDED_AGENTS,
    DEFAULT_MAX_SUSPENDED_PROCESSES,
};
use crate::agent::tools::spillover::TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES;
use crate::repositories::settings_repository::SettingsRepository;
use serde::Serialize;
use serde_json::{Map, Value};

const SETTINGS_KEY: &str = "advancedSettings";
const CONCURRENCY_RANGE: (u32, u32) = (1, 256);
const LOOP_THRESHOLD_RANGE: (usize, usize) = (2, 20);
const LOOP_HARD_BREAK_OFFSET_RANGE: (usize, usize) = (1, 20);

/// Extended reasoning effort preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum ThinkingEffort {
    Off,
    Low,
    Medium,
    High,
    Auto,
}

impl ThinkingEffort {
    fn parse(effort: &str) -> Option<Self> {
        match effort {
            "off" => Some(Self::Off),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AdvancedSettings {
    pub max_retries: u32,
    /// Milliseconds.
    pub retry_delay: u64,
    pub diff_context_lines: usize,
    pub default_max_output_tokens: u64,
    /// Not clamped here; the spillover module owns its byte bounds.
    pub tool_result_inline_limit_bytes: usize,
    /// `0` means no default limit.
    pub default_session_max_depth: u32,
    /// `0` means no default limit.
    pub default_session_max_fanout: u32,
    pub max_concurrent_active_sessions: u32,
    pub max_suspended_sessions: u32,
    pub max_concurrent_active_processes: u32,
    pub max_suspended_processes: u32,
    pub loop_prevention_threshold: usize,
    pub loop_prevention_hard_break_offset: usize,
    pub thinking_loop_min_pattern_length: u64,
    pub thinking_loop_min_repetitions: u64,
    pub max_recent_media_messages: u64,
    pub thinking_effort: ThinkingEffort,
}

impl Default for AdvancedSettings {
    fn default() -> Self {
        Self {
            max_retries: 1,
            retry_delay: 5000,
            diff_context_lines: 3,
            default_max_output_tokens: 8192,
            tool_result_inline_limit_bytes: TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES,
            default_session_max_depth: 0,
            default_session_max_fanout: 0,
            max_concurrent_active_sessions: DEFAULT_MAX_ACTIVE_AGENTS,
            max_suspended_sessions: DEFAULT_MAX_SUSPENDED_AGENTS,
            max_concurrent_active_processes: DEFAULT_MAX_ACTIVE_PROCESSES,
            max_suspended_processes: DEFAULT_MAX_SUSPENDED_PROCESSES,
            loop_prevention_threshold: 3,
            loop_prevention_hard_break_offset: 2,
            thinking_loop_min_pattern_length: 256,
            thinking_loop_min_repetitions: 4,
            max_recent_media_messages: 1,
            thinking_effort: ThinkingEffort::Off,
        }
    }
}

const MAX_RECENT_MEDIA_MESSAGES: i64 = 5;

/// A known effort wins; otherwise a legacy numeric `thinkingBudget` maps to one
/// (`0` off, `-1` auto, up to 2048 low, up to 16384 medium, above that high).
fn normalize_thinking_effort(
    effort: Option<&Value>,
    legacy_budget: Option<&Value>,
) -> ThinkingEffort {
    if let Some(effort) = effort
        .and_then(Value::as_str)
        .and_then(ThinkingEffort::parse)
    {
        return effort;
    }
    match legacy_budget.and_then(Value::as_f64) {
        Some(0.0) => ThinkingEffort::Off,
        Some(-1.0) => ThinkingEffort::Auto,
        Some(budget) if budget <= 2048.0 => ThinkingEffort::Low,
        Some(budget) if budget <= 16384.0 => ThinkingEffort::Medium,
        Some(_) => ThinkingEffort::High,
        None => ThinkingEffort::Off,
    }
}

fn uint(map: &Map<String, Value>, key: &str) -> Option<u64> {
    map.get(key).and_then(Value::as_u64)
}

fn uint_in<T: TryFrom<u64> + Ord + Copy>(
    map: &Map<String, Value>,
    key: &str,
    (min, max): (T, T),
    default: T,
) -> T {
    uint(map, key)
        .map(|value| T::try_from(value).unwrap_or(max).clamp(min, max))
        .unwrap_or(default)
}

impl AdvancedSettings {
    /// Normalizes a stored row value; `None`, broken JSON and non-object JSON
    /// yield the defaults.
    pub fn from_stored(raw: Option<&str>) -> Self {
        let defaults = Self::default();
        let Some(Value::Object(map)) = raw.and_then(|raw| serde_json::from_str(raw).ok()) else {
            return defaults;
        };

        Self {
            max_retries: uint_in(&map, "maxRetries", (0, u32::MAX), defaults.max_retries),
            retry_delay: uint(&map, "retryDelay").unwrap_or(defaults.retry_delay),
            diff_context_lines: uint_in(
                &map,
                "diffContextLines",
                (0, usize::MAX),
                defaults.diff_context_lines,
            ),
            default_max_output_tokens: uint(&map, "defaultMaxOutputTokens")
                .filter(|tokens| *tokens > 0)
                .unwrap_or(defaults.default_max_output_tokens),
            tool_result_inline_limit_bytes: uint_in(
                &map,
                "toolResultInlineLimitBytes",
                (0, usize::MAX),
                defaults.tool_result_inline_limit_bytes,
            ),
            default_session_max_depth: uint_in(
                &map,
                "defaultSessionMaxDepth",
                (0, u32::MAX),
                defaults.default_session_max_depth,
            ),
            default_session_max_fanout: uint_in(
                &map,
                "defaultSessionMaxFanout",
                (0, u32::MAX),
                defaults.default_session_max_fanout,
            ),
            max_concurrent_active_sessions: uint_in(
                &map,
                "maxConcurrentActiveSessions",
                CONCURRENCY_RANGE,
                defaults.max_concurrent_active_sessions,
            ),
            max_suspended_sessions: uint_in(
                &map,
                "maxSuspendedSessions",
                CONCURRENCY_RANGE,
                defaults.max_suspended_sessions,
            ),
            max_concurrent_active_processes: uint_in(
                &map,
                "maxConcurrentActiveProcesses",
                CONCURRENCY_RANGE,
                defaults.max_concurrent_active_processes,
            ),
            max_suspended_processes: uint_in(
                &map,
                "maxSuspendedProcesses",
                CONCURRENCY_RANGE,
                defaults.max_suspended_processes,
            ),
            loop_prevention_threshold: uint_in(
                &map,
                "loopPreventionThreshold",
                LOOP_THRESHOLD_RANGE,
                defaults.loop_prevention_threshold,
            ),
            loop_prevention_hard_break_offset: uint_in(
                &map,
                "loopPreventionHardBreakOffset",
                LOOP_HARD_BREAK_OFFSET_RANGE,
                defaults.loop_prevention_hard_break_offset,
            ),
            thinking_loop_min_pattern_length: uint(&map, "thinkingLoopMinPatternLength")
                .unwrap_or(defaults.thinking_loop_min_pattern_length),
            thinking_loop_min_repetitions: uint(&map, "thinkingLoopMinRepetitions")
                .unwrap_or(defaults.thinking_loop_min_repetitions),
            max_recent_media_messages: map
                .get("maxRecentMediaMessages")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite())
                .map(|value| (value.trunc() as i64).clamp(1, MAX_RECENT_MEDIA_MESSAGES) as u64)
                .unwrap_or(defaults.max_recent_media_messages),
            thinking_effort: normalize_thinking_effort(
                map.get("thinkingEffort"),
                map.get("thinkingBudget"),
            ),
        }
    }

    /// Reads and normalizes the stored row; defaults when the repository is
    /// not initialized yet or the read fails.
    pub async fn load(repo: &impl SettingsRepository) -> Self {
        match repo.get(SETTINGS_KEY).await {
            Ok(Some(model)) => Self::from_stored(Some(&model.value)),
            _ => Self::default(),
        }
    }

    /// [`Self::load`] through the global repository.
    pub async fn load_global() -> Self {
        match crate::state::try_get_settings_repository() {
            Some(repo) => Self::load(repo).await,
            None => Self::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_row_and_broken_json_yield_defaults() {
        let defaults = AdvancedSettings::default();
        assert_eq!(AdvancedSettings::from_stored(None), defaults);
        assert_eq!(AdvancedSettings::from_stored(Some("{not json")), defaults);
        assert_eq!(AdvancedSettings::from_stored(Some("[1,2]")), defaults);
        assert_eq!(AdvancedSettings::from_stored(Some("{}")), defaults);
    }

    #[test]
    fn partial_row_keeps_present_fields_and_fills_the_rest() {
        let settings = AdvancedSettings::from_stored(Some(
            r#"{"maxRetries":5,"defaultSessionMaxDepth":3,"thinkingEffort":"high"}"#,
        ));
        assert_eq!(settings.max_retries, 5);
        assert_eq!(settings.default_session_max_depth, 3);
        assert_eq!(settings.thinking_effort, ThinkingEffort::High);
        assert_eq!(settings.retry_delay, 5000);
        assert_eq!(settings.diff_context_lines, 3);
    }

    #[test]
    fn wrong_typed_fields_fall_back_individually() {
        let settings = AdvancedSettings::from_stored(Some(
            r#"{"maxRetries":"many","retryDelay":-1,"diffContextLines":7,"thinkingEffort":4}"#,
        ));
        assert_eq!(settings.max_retries, 1);
        assert_eq!(settings.retry_delay, 5000);
        assert_eq!(settings.diff_context_lines, 7);
        assert_eq!(settings.thinking_effort, ThinkingEffort::Off);
    }

    fn effort(raw: &str) -> &'static str {
        match AdvancedSettings::from_stored(Some(raw)).thinking_effort {
            ThinkingEffort::Off => "off",
            ThinkingEffort::Low => "low",
            ThinkingEffort::Medium => "medium",
            ThinkingEffort::High => "high",
            ThinkingEffort::Auto => "auto",
        }
    }

    #[test]
    fn unknown_effort_falls_back_to_the_legacy_budget_then_off() {
        assert_eq!(effort(r#"{"thinkingEffort":"medium"}"#), "medium");
        assert_eq!(effort(r#"{"thinkingEffort":"extreme"}"#), "off");
        assert_eq!(effort(r#"{"thinkingBudget":0}"#), "off");
        assert_eq!(effort(r#"{"thinkingBudget":-1}"#), "auto");
        assert_eq!(effort(r#"{"thinkingBudget":2048}"#), "low");
        assert_eq!(effort(r#"{"thinkingBudget":2049}"#), "medium");
        assert_eq!(effort(r#"{"thinkingBudget":16384}"#), "medium");
        assert_eq!(effort(r#"{"thinkingBudget":16385}"#), "high");
        assert_eq!(effort(r#"{"thinkingBudget":"big"}"#), "off");
        assert_eq!(
            effort(r#"{"thinkingEffort":"low","thinkingBudget":99999}"#),
            "low"
        );
    }

    #[test]
    fn effort_serializes_as_its_lowercase_name() {
        assert_eq!(
            serde_json::to_value(ThinkingEffort::Medium).unwrap(),
            serde_json::json!("medium")
        );
    }

    #[test]
    fn recent_media_messages_are_truncated_and_clamped_to_one_through_five() {
        let media = |raw: &str| AdvancedSettings::from_stored(Some(raw)).max_recent_media_messages;
        assert_eq!(media(r#"{"maxRecentMediaMessages":3.9}"#), 3);
        assert_eq!(media(r#"{"maxRecentMediaMessages":0}"#), 1);
        assert_eq!(media(r#"{"maxRecentMediaMessages":-4}"#), 1);
        assert_eq!(media(r#"{"maxRecentMediaMessages":50}"#), 5);
        assert_eq!(media(r#"{"maxRecentMediaMessages":"2"}"#), 1);
    }

    #[test]
    fn bounded_fields_are_clamped() {
        let settings = AdvancedSettings::from_stored(Some(
            r#"{"maxConcurrentActiveSessions":0,"maxSuspendedSessions":9999,
                "loopPreventionThreshold":1,"loopPreventionHardBreakOffset":500}"#,
        ));
        assert_eq!(settings.max_concurrent_active_sessions, 1);
        assert_eq!(settings.max_suspended_sessions, 256);
        assert_eq!(settings.loop_prevention_threshold, 2);
        assert_eq!(settings.loop_prevention_hard_break_offset, 20);
    }

    #[test]
    fn serialized_keys_match_the_stored_camel_case_names() {
        let value = serde_json::to_value(AdvancedSettings::default()).unwrap();
        let keys: Vec<_> = value.as_object().unwrap().keys().cloned().collect();
        assert!(keys.contains(&"maxConcurrentActiveSessions".to_string()));
        assert!(keys.contains(&"defaultSessionMaxFanout".to_string()));
        assert_eq!(
            AdvancedSettings::from_stored(Some(&value.to_string())),
            AdvancedSettings::default()
        );
    }
}

/// Checked-in TS types for the frontend. `UPDATE_TS_BINDINGS=1` rewrites them;
/// otherwise a stale file fails the test.
#[cfg(test)]
mod ts_bindings {
    use super::{AdvancedSettings, ThinkingEffort};
    use std::path::Path;
    use ts_rs::{Config, TS};

    const OUT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/generated");

    fn check<T: TS + 'static>() {
        // Values are JSON numbers, never bigint.
        let cfg = Config::new().with_large_int("number");
        let path = Path::new(OUT_DIR).join(T::output_path().expect("exportable type"));
        let expected = T::export_to_string(&cfg).expect("bindings");
        if std::env::var_os("UPDATE_TS_BINDINGS").is_some() {
            std::fs::write(&path, &expected).expect("write bindings");
            return;
        }
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(
            current,
            expected,
            "{} is stale; run `UPDATE_TS_BINDINGS=1 pnpm rust:test --lib ts_bindings`",
            path.display()
        );
    }

    #[test]
    fn advanced_settings_bindings_are_current() {
        check::<AdvancedSettings>();
        check::<ThinkingEffort>();
    }
}
