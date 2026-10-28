//! Backend model of the `displaySettings` settings row.
//!
//! Schema and defaults live here. Stored JSON may be missing fields, hold
//! values of the wrong type, or not parse at all; [`DisplaySettings::from_stored`]
//! always returns a complete object, falling back per field.
//!
//! The frontend's `DisplaySettings` type is generated from this model
//! (`src/lib/generated/`); `ts_bindings` fails when the checked-in file is stale.

use crate::repositories::settings_repository::SettingsRepository;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const SETTINGS_KEY: &str = "displaySettings";
/// Webview zoom factors offered in Settings and walked by Ctrl/Cmd +/-.
const UI_SCALE_STEPS: [f64; 5] = [0.9, 1.0, 1.1, 1.25, 1.5];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum MetricDisplayMode {
    Tooltip,
    Inline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum PrefillDisplayFormat {
    Time,
    TokensPerSecond,
}

/// Tool call display verbosity. `Simple` hides params/results/errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ToolDetailLevel {
    Simple,
    Developer,
}

/// Chat message chrome: full-width document stream vs classic bubbles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum MessageLayoutStyle {
    Document,
    Bubble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ColorTheme {
    Neutral,
    Amber,
    Violet,
    Ocean,
    Forest,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DisplaySettings {
    pub metric_display_mode: MetricDisplayMode,
    pub prefill_display_format: PrefillDisplayFormat,
    pub show_token_speed: bool,
    pub compact_metrics: bool,
    pub tool_detail_level: ToolDetailLevel,
    pub font_family: String,
    /// Webview zoom factor; one of the supported steps (0.9, 1, 1.1, 1.25, 1.5).
    pub ui_scale: f64,
    pub message_layout: MessageLayoutStyle,
    pub color_theme: ColorTheme,
    /// Show release/hub feature discovery tips on the agent hub.
    pub show_feature_tips: bool,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            metric_display_mode: MetricDisplayMode::Inline,
            prefill_display_format: PrefillDisplayFormat::Time,
            show_token_speed: true,
            compact_metrics: false,
            tool_detail_level: ToolDetailLevel::Simple,
            font_family: "Pretendard".to_string(),
            ui_scale: 1.1,
            message_layout: MessageLayoutStyle::Document,
            color_theme: ColorTheme::Neutral,
            show_feature_tips: true,
        }
    }
}

fn parsed<T: for<'de> Deserialize<'de>>(map: &Map<String, Value>, key: &str, default: T) -> T {
    map.get(key)
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or(default)
}

impl DisplaySettings {
    /// Normalizes a stored row value; `None`, broken JSON and non-object JSON
    /// yield the defaults.
    pub fn from_stored(raw: Option<&str>) -> Self {
        let defaults = Self::default();
        let Some(Value::Object(map)) = raw.and_then(|raw| serde_json::from_str(raw).ok()) else {
            return defaults;
        };

        Self {
            metric_display_mode: parsed(&map, "metricDisplayMode", defaults.metric_display_mode),
            prefill_display_format: parsed(
                &map,
                "prefillDisplayFormat",
                defaults.prefill_display_format,
            ),
            show_token_speed: parsed(&map, "showTokenSpeed", defaults.show_token_speed),
            compact_metrics: parsed(&map, "compactMetrics", defaults.compact_metrics),
            tool_detail_level: parsed(&map, "toolDetailLevel", defaults.tool_detail_level),
            font_family: map
                .get("fontFamily")
                .and_then(Value::as_str)
                .filter(|family| !family.is_empty())
                .map(str::to_string)
                .unwrap_or(defaults.font_family),
            ui_scale: map
                .get("uiScale")
                .and_then(Value::as_f64)
                .filter(|scale| UI_SCALE_STEPS.contains(scale))
                .unwrap_or(defaults.ui_scale),
            message_layout: parsed(&map, "messageLayout", defaults.message_layout),
            color_theme: parsed(&map, "colorTheme", defaults.color_theme),
            show_feature_tips: parsed(&map, "showFeatureTips", defaults.show_feature_tips),
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
        let defaults = DisplaySettings::default();
        assert_eq!(DisplaySettings::from_stored(None), defaults);
        assert_eq!(DisplaySettings::from_stored(Some("{not json")), defaults);
        assert_eq!(DisplaySettings::from_stored(Some("null")), defaults);
        assert_eq!(DisplaySettings::from_stored(Some("{}")), defaults);
    }

    #[test]
    fn older_rows_get_the_fields_added_later() {
        let settings = DisplaySettings::from_stored(Some(
            r#"{"metricDisplayMode":"tooltip","prefillDisplayFormat":"tokensPerSecond",
                "showTokenSpeed":false,"compactMetrics":true,"toolDetailLevel":"developer",
                "fontFamily":"Inter"}"#,
        ));
        assert_eq!(settings.metric_display_mode, MetricDisplayMode::Tooltip);
        assert_eq!(
            settings.prefill_display_format,
            PrefillDisplayFormat::TokensPerSecond
        );
        assert!(!settings.show_token_speed);
        assert!(settings.compact_metrics);
        assert_eq!(settings.tool_detail_level, ToolDetailLevel::Developer);
        assert_eq!(settings.font_family, "Inter");
        assert_eq!(settings.message_layout, MessageLayoutStyle::Document);
        assert_eq!(settings.color_theme, ColorTheme::Neutral);
        assert_eq!(settings.ui_scale, 1.1);
        assert!(settings.show_feature_tips);
    }

    #[test]
    fn valid_values_are_kept() {
        let settings = DisplaySettings::from_stored(Some(
            r#"{"messageLayout":"bubble","colorTheme":"amber","showFeatureTips":false,"uiScale":1}"#,
        ));
        assert_eq!(settings.message_layout, MessageLayoutStyle::Bubble);
        assert_eq!(settings.color_theme, ColorTheme::Amber);
        assert!(!settings.show_feature_tips);
        assert_eq!(settings.ui_scale, 1.0);
    }

    #[test]
    fn invalid_values_fall_back_individually() {
        let settings = DisplaySettings::from_stored(Some(
            r#"{"messageLayout":"wide","colorTheme":"invalid-theme","showTokenSpeed":"yes",
                "fontFamily":"","uiScale":3,"compactMetrics":true}"#,
        ));
        let defaults = DisplaySettings::default();
        assert_eq!(settings.message_layout, defaults.message_layout);
        assert_eq!(settings.color_theme, defaults.color_theme);
        assert_eq!(settings.show_token_speed, defaults.show_token_speed);
        assert_eq!(settings.font_family, defaults.font_family);
        assert_eq!(settings.ui_scale, defaults.ui_scale);
        assert!(settings.compact_metrics);
    }

    #[test]
    fn every_ui_scale_step_is_accepted() {
        for step in UI_SCALE_STEPS {
            let raw = format!(r#"{{"uiScale":{step}}}"#);
            assert_eq!(DisplaySettings::from_stored(Some(&raw)).ui_scale, step);
        }
    }

    #[test]
    fn serialized_row_round_trips_with_camel_case_names() {
        let value = serde_json::to_value(DisplaySettings::default()).unwrap();
        assert_eq!(value["metricDisplayMode"], "inline");
        assert_eq!(value["prefillDisplayFormat"], "time");
        assert_eq!(value["uiScale"], 1.1);
        assert_eq!(
            DisplaySettings::from_stored(Some(&value.to_string())),
            DisplaySettings::default()
        );
        let tokens = serde_json::to_value(PrefillDisplayFormat::TokensPerSecond).unwrap();
        assert_eq!(tokens, "tokensPerSecond");
    }
}

/// Checked-in TS types for the frontend. `UPDATE_TS_BINDINGS=1` rewrites them;
/// otherwise a stale file fails the test.
#[cfg(test)]
mod ts_bindings {
    use super::{
        ColorTheme, DisplaySettings, MessageLayoutStyle, MetricDisplayMode, PrefillDisplayFormat,
        ToolDetailLevel,
    };
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
    fn display_settings_bindings_are_current() {
        check::<DisplaySettings>();
        check::<MetricDisplayMode>();
        check::<PrefillDisplayFormat>();
        check::<ToolDetailLevel>();
        check::<MessageLayoutStyle>();
        check::<ColorTheme>();
    }
}
