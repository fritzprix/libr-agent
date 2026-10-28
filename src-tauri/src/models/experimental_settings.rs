//! Backend model of the `experimentalSettings` settings row.
//!
//! Schema and defaults live here. Stored JSON may be missing fields, hold
//! values of the wrong type, or not parse at all; [`ExperimentalSettings::from_stored`]
//! always returns a complete, in-range object, falling back per field.
//!
//! The frontend's `ExperimentalSettings` type is generated from this model
//! (`src/lib/generated/`); `ts_bindings` fails when the checked-in file is stale.

use crate::repositories::settings_repository::SettingsRepository;
use serde::Serialize;
use serde_json::{Map, Value};

const SETTINGS_KEY: &str = "experimentalSettings";
const MAX_RESAMPLE_RETRIES: i64 = 20;

/// What to do when the model loops on tool calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ToolLoopRecoveryPolicy {
    /// Discard the looping assistant turn and request a fresh completion.
    ResampleThenBreak,
    /// Inject loop-prevention text into tool errors (opt-in).
    LegacyGuidance,
}

impl ToolLoopRecoveryPolicy {
    fn parse(policy: &str) -> Option<Self> {
        match policy {
            "resampleThenBreak" => Some(Self::ResampleThenBreak),
            "legacyGuidance" => Some(Self::LegacyGuidance),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ExperimentalSettings {
    pub inline_audio_attachment: bool,
    pub tool_loop_recovery_policy: ToolLoopRecoveryPolicy,
    /// Max clean resample retries before promoting to the circuit breaker.
    pub tool_loop_max_resample_retries: u32,
}

impl Default for ExperimentalSettings {
    fn default() -> Self {
        Self {
            inline_audio_attachment: true,
            tool_loop_recovery_policy: ToolLoopRecoveryPolicy::ResampleThenBreak,
            tool_loop_max_resample_retries: 2,
        }
    }
}

/// A known policy wins; otherwise the deprecated boolean
/// `toolLoopLegacyGuidanceEnabled` maps to one.
fn normalize_policy(
    map: &Map<String, Value>,
    default: ToolLoopRecoveryPolicy,
) -> ToolLoopRecoveryPolicy {
    if let Some(policy) = map
        .get("toolLoopRecoveryPolicy")
        .and_then(Value::as_str)
        .and_then(ToolLoopRecoveryPolicy::parse)
    {
        return policy;
    }
    match map
        .get("toolLoopLegacyGuidanceEnabled")
        .and_then(Value::as_bool)
    {
        Some(true) => ToolLoopRecoveryPolicy::LegacyGuidance,
        Some(false) => ToolLoopRecoveryPolicy::ResampleThenBreak,
        None => default,
    }
}

impl ExperimentalSettings {
    /// Normalizes a stored row value; `None`, broken JSON and non-object JSON
    /// yield the defaults.
    pub fn from_stored(raw: Option<&str>) -> Self {
        let defaults = Self::default();
        let Some(Value::Object(map)) = raw.and_then(|raw| serde_json::from_str(raw).ok()) else {
            return defaults;
        };

        Self {
            inline_audio_attachment: map
                .get("inlineAudioAttachment")
                .and_then(Value::as_bool)
                .unwrap_or(defaults.inline_audio_attachment),
            tool_loop_recovery_policy: normalize_policy(&map, defaults.tool_loop_recovery_policy),
            tool_loop_max_resample_retries: map
                .get("toolLoopMaxResampleRetries")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite())
                .map(|value| (value.trunc() as i64).clamp(0, MAX_RESAMPLE_RETRIES) as u32)
                .unwrap_or(defaults.tool_loop_max_resample_retries),
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
        let defaults = ExperimentalSettings::default();
        assert_eq!(ExperimentalSettings::from_stored(None), defaults);
        assert_eq!(
            ExperimentalSettings::from_stored(Some("{not json")),
            defaults
        );
        assert_eq!(ExperimentalSettings::from_stored(Some("null")), defaults);
        assert_eq!(ExperimentalSettings::from_stored(Some("{}")), defaults);
    }

    #[test]
    fn canonical_row_is_kept() {
        let settings = ExperimentalSettings::from_stored(Some(
            r#"{"inlineAudioAttachment":false,"toolLoopRecoveryPolicy":"legacyGuidance",
                "toolLoopMaxResampleRetries":4}"#,
        ));
        assert!(!settings.inline_audio_attachment);
        assert_eq!(
            settings.tool_loop_recovery_policy,
            ToolLoopRecoveryPolicy::LegacyGuidance
        );
        assert_eq!(settings.tool_loop_max_resample_retries, 4);
    }

    #[test]
    fn wrong_typed_fields_fall_back_individually() {
        let settings = ExperimentalSettings::from_stored(Some(
            r#"{"inlineAudioAttachment":"no","toolLoopRecoveryPolicy":"nope",
                "toolLoopMaxResampleRetries":3}"#,
        ));
        assert_eq!(settings.inline_audio_attachment, true);
        assert_eq!(
            settings.tool_loop_recovery_policy,
            ToolLoopRecoveryPolicy::ResampleThenBreak
        );
        assert_eq!(settings.tool_loop_max_resample_retries, 3);
    }

    #[test]
    fn legacy_guidance_flag_maps_to_a_policy() {
        let policy =
            |raw: &str| ExperimentalSettings::from_stored(Some(raw)).tool_loop_recovery_policy;
        assert_eq!(
            policy(r#"{"toolLoopLegacyGuidanceEnabled":true}"#),
            ToolLoopRecoveryPolicy::LegacyGuidance
        );
        assert_eq!(
            policy(r#"{"toolLoopLegacyGuidanceEnabled":false}"#),
            ToolLoopRecoveryPolicy::ResampleThenBreak
        );
        assert_eq!(
            policy(r#"{"toolLoopLegacyGuidanceEnabled":"yes"}"#),
            ToolLoopRecoveryPolicy::ResampleThenBreak
        );
        assert_eq!(
            policy(
                r#"{"toolLoopRecoveryPolicy":"resampleThenBreak","toolLoopLegacyGuidanceEnabled":true}"#
            ),
            ToolLoopRecoveryPolicy::ResampleThenBreak
        );
    }

    #[test]
    fn resample_retries_are_truncated_and_clamped() {
        let retries =
            |raw: &str| ExperimentalSettings::from_stored(Some(raw)).tool_loop_max_resample_retries;
        assert_eq!(retries(r#"{"toolLoopMaxResampleRetries":99}"#), 20);
        assert_eq!(retries(r#"{"toolLoopMaxResampleRetries":-3}"#), 0);
        assert_eq!(retries(r#"{"toolLoopMaxResampleRetries":2.9}"#), 2);
        assert_eq!(retries(r#"{"toolLoopMaxResampleRetries":"4"}"#), 2);
    }

    #[test]
    fn serialized_keys_match_the_stored_camel_case_names() {
        let value = serde_json::to_value(ExperimentalSettings::default()).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "inlineAudioAttachment": true,
                "toolLoopRecoveryPolicy": "resampleThenBreak",
                "toolLoopMaxResampleRetries": 2,
            })
        );
        assert_eq!(
            ExperimentalSettings::from_stored(Some(&value.to_string())),
            ExperimentalSettings::default()
        );
    }
}

/// Checked-in TS types for the frontend. `UPDATE_TS_BINDINGS=1` rewrites them;
/// otherwise a stale file fails the test.
#[cfg(test)]
mod ts_bindings {
    use super::{ExperimentalSettings, ToolLoopRecoveryPolicy};
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
    fn experimental_settings_bindings_are_current() {
        check::<ExperimentalSettings>();
        check::<ToolLoopRecoveryPolicy>();
    }
}
