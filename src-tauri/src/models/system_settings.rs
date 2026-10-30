//! Backend model of the `systemSettings` settings row.
//!
//! Schema and defaults live here. Stored JSON may be missing fields, hold
//! values of the wrong type, or not parse at all; [`SystemSettings::from_stored`]
//! always returns a complete object, falling back per field.
//!
//! The frontend's `SystemSettings` type is generated from this model
//! (`src/lib/generated/`); `ts_bindings` fails when the checked-in file is stale.

use crate::repositories::settings_repository::SettingsRepository;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const SETTINGS_KEY: &str = "systemSettings";

/// Shell sandboxing strength.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum IsolationLevel {
    Basic,
    Medium,
    High,
}

impl From<IsolationLevel> for crate::session_isolation::IsolationLevel {
    fn from(level: IsolationLevel) -> Self {
        match level {
            IsolationLevel::Basic => Self::Basic,
            IsolationLevel::Medium => Self::Medium,
            IsolationLevel::High => Self::High,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SystemSettings {
    #[serde(rename = "maxFileUploadSizeMB")]
    pub max_file_upload_size_mb: u64,
    pub web_action_timeout_seconds: u64,
    pub http_server_port: u16,
    pub http_server_expose: bool,
    pub mcp_server_startup_timeout_seconds: u64,
    /// `0` means no limit.
    pub mcp_tool_timeout_seconds: u64,
    pub search_index_frequency_minutes: u64,
    /// `0` means no minimum.
    pub scheduled_task_minimum_interval_minutes: u64,
    pub shell_isolation_level: IsolationLevel,
    pub shell_runtime_bootstrap: bool,
    pub skills_directory: String,
    /// Inhibit idle sleep while LibrAgent is running. Does not keep the display on.
    pub prevent_sleep_during_agent_work: bool,
    /// Archived app logs older than this are deleted. `0` means no age limit.
    pub log_retention_days: u64,
}

impl Default for SystemSettings {
    fn default() -> Self {
        Self {
            max_file_upload_size_mb: 50,
            web_action_timeout_seconds: 30,
            http_server_port: 3030,
            http_server_expose: false,
            mcp_server_startup_timeout_seconds: crate::config::mcp_startup_timeout_seconds(),
            mcp_tool_timeout_seconds: crate::config::mcp_tool_call_timeout_seconds(),
            search_index_frequency_minutes: 5,
            scheduled_task_minimum_interval_minutes: 0,
            shell_isolation_level: IsolationLevel::Medium,
            shell_runtime_bootstrap: false,
            skills_directory: String::new(),
            prevent_sleep_during_agent_work: true,
            log_retention_days: 30,
        }
    }
}

fn parsed<T: for<'de> Deserialize<'de>>(map: &Map<String, Value>, key: &str, default: T) -> T {
    map.get(key)
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or(default)
}

impl SystemSettings {
    /// Normalizes a stored row value; `None`, broken JSON and non-object JSON
    /// yield the defaults.
    pub fn from_stored(raw: Option<&str>) -> Self {
        let defaults = Self::default();
        let Some(Value::Object(map)) = raw.and_then(|raw| serde_json::from_str(raw).ok()) else {
            return defaults;
        };

        Self {
            max_file_upload_size_mb: parsed(
                &map,
                "maxFileUploadSizeMB",
                defaults.max_file_upload_size_mb,
            ),
            web_action_timeout_seconds: parsed(
                &map,
                "webActionTimeoutSeconds",
                defaults.web_action_timeout_seconds,
            ),
            http_server_port: parsed(&map, "httpServerPort", defaults.http_server_port),
            http_server_expose: parsed(&map, "httpServerExpose", defaults.http_server_expose),
            mcp_server_startup_timeout_seconds: parsed(
                &map,
                "mcpServerStartupTimeoutSeconds",
                defaults.mcp_server_startup_timeout_seconds,
            ),
            mcp_tool_timeout_seconds: parsed(
                &map,
                "mcpToolTimeoutSeconds",
                defaults.mcp_tool_timeout_seconds,
            ),
            search_index_frequency_minutes: parsed(
                &map,
                "searchIndexFrequencyMinutes",
                defaults.search_index_frequency_minutes,
            ),
            scheduled_task_minimum_interval_minutes: parsed(
                &map,
                "scheduledTaskMinimumIntervalMinutes",
                defaults.scheduled_task_minimum_interval_minutes,
            ),
            shell_isolation_level: parsed(
                &map,
                "shellIsolationLevel",
                defaults.shell_isolation_level,
            ),
            shell_runtime_bootstrap: parsed(
                &map,
                "shellRuntimeBootstrap",
                defaults.shell_runtime_bootstrap,
            ),
            skills_directory: parsed(&map, "skillsDirectory", defaults.skills_directory),
            prevent_sleep_during_agent_work: parsed(
                &map,
                "preventSleepDuringAgentWork",
                defaults.prevent_sleep_during_agent_work,
            ),
            log_retention_days: parsed(&map, "logRetentionDays", defaults.log_retention_days),
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
        let defaults = SystemSettings::default();
        assert_eq!(SystemSettings::from_stored(None), defaults);
        assert_eq!(SystemSettings::from_stored(Some("{not json")), defaults);
        assert_eq!(SystemSettings::from_stored(Some("[1]")), defaults);
        assert_eq!(SystemSettings::from_stored(Some("{}")), defaults);
    }

    #[test]
    fn partial_row_keeps_present_fields_and_fills_the_rest() {
        let settings = SystemSettings::from_stored(Some(
            r#"{"maxFileUploadSizeMB":10,"httpServerPort":4000,"shellIsolationLevel":"high",
                "scheduledTaskMinimumIntervalMinutes":15,"preventSleepDuringAgentWork":false,
                "logRetentionDays":0}"#,
        ));
        assert_eq!(settings.log_retention_days, 0);
        assert_eq!(settings.max_file_upload_size_mb, 10);
        assert_eq!(settings.http_server_port, 4000);
        assert_eq!(settings.shell_isolation_level, IsolationLevel::High);
        assert_eq!(settings.scheduled_task_minimum_interval_minutes, 15);
        assert!(!settings.prevent_sleep_during_agent_work);
        assert_eq!(settings.web_action_timeout_seconds, 30);
        assert_eq!(settings.search_index_frequency_minutes, 5);
    }

    #[test]
    fn wrong_typed_fields_fall_back_individually() {
        let settings = SystemSettings::from_stored(Some(
            r#"{"httpServerPort":70000,"httpServerExpose":"yes","shellIsolationLevel":"max",
                "webActionTimeoutSeconds":-1,"shellRuntimeBootstrap":true,"skillsDirectory":3}"#,
        ));
        let defaults = SystemSettings::default();
        assert_eq!(settings.http_server_port, defaults.http_server_port);
        assert_eq!(settings.http_server_expose, defaults.http_server_expose);
        assert_eq!(
            settings.shell_isolation_level,
            defaults.shell_isolation_level
        );
        assert_eq!(
            settings.web_action_timeout_seconds,
            defaults.web_action_timeout_seconds
        );
        assert_eq!(settings.skills_directory, "");
        assert!(settings.shell_runtime_bootstrap);
    }

    #[test]
    fn serialized_keys_match_the_stored_names() {
        let value = serde_json::to_value(SystemSettings::default()).unwrap();
        let keys = value.as_object().unwrap();
        assert!(keys.contains_key("maxFileUploadSizeMB"));
        assert!(keys.contains_key("mcpServerStartupTimeoutSeconds"));
        assert!(keys.contains_key("logRetentionDays"));
        assert_eq!(keys.len(), 13);
        assert_eq!(
            SystemSettings::from_stored(Some(&value.to_string())),
            SystemSettings::default()
        );
    }

    #[test]
    fn isolation_level_maps_to_the_session_isolation_level() {
        use crate::session_isolation::IsolationLevel as Session;
        assert!(matches!(
            Session::from(IsolationLevel::Basic),
            Session::Basic
        ));
        assert!(matches!(
            Session::from(IsolationLevel::Medium),
            Session::Medium
        ));
        assert!(matches!(Session::from(IsolationLevel::High), Session::High));
    }
}

/// Checked-in TS types for the frontend. `UPDATE_TS_BINDINGS=1` rewrites them;
/// otherwise a stale file fails the test.
#[cfg(test)]
mod ts_bindings {
    use super::{IsolationLevel, SystemSettings};
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
    fn system_settings_bindings_are_current() {
        check::<SystemSettings>();
        check::<IsolationLevel>();
    }
}
