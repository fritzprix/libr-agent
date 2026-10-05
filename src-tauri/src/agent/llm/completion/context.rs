use crate::agent::state::AgentSession;
use crate::repositories::settings_repository::SettingsRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextManagementSettings {
    pub(crate) context_strategy: String,
    pub(crate) window_size: usize,
    pub(crate) max_input_context: usize,
    pub(crate) tool_call_group_visible_count: usize,
    pub(crate) model_max_limit: usize,
}

impl ContextManagementSettings {
    pub fn context_strategy(&self) -> &str {
        &self.context_strategy
    }

    pub fn window_size(&self) -> usize {
        self.window_size
    }

    pub fn max_input_context(&self) -> usize {
        self.max_input_context
    }

    pub fn tool_call_group_visible_count(&self) -> usize {
        self.tool_call_group_visible_count
    }

    pub fn model_max_limit(&self) -> usize {
        self.model_max_limit
    }
}

pub(crate) fn default_context_management_settings() -> ContextManagementSettings {
    ContextManagementSettings {
        context_strategy: "compact".to_string(),
        window_size: 20,
        max_input_context: 49152,
        tool_call_group_visible_count: 4,
        model_max_limit: 128_000,
    }
}

pub(crate) fn apply_context_management_setting(
    settings: &mut ContextManagementSettings,
    key: &str,
    value: &serde_json::Value,
) {
    match key {
        "contextStrategy" => {
            if let Some(strategy) = value.as_str() {
                settings.context_strategy = strategy.to_string();
            }
        }
        "windowSize" => {
            if let Some(window_size) = value.as_u64() {
                settings.window_size = window_size as usize;
            }
        }
        "maxInputContext" => {
            if let Some(max_input_context) = value.as_u64() {
                settings.max_input_context = max_input_context as usize;
            }
        }
        "toolCallGroupVisibleCount" => {
            if let Some(visible_count) = value.as_u64() {
                settings.tool_call_group_visible_count = visible_count as usize;
            }
        }
        _ => {}
    }
}

pub fn resolve_context_management_settings(
    legacy_settings_blob: Option<&serde_json::Value>,
    direct_settings: &HashMap<String, serde_json::Value>,
) -> ContextManagementSettings {
    let mut settings = default_context_management_settings();

    if let Some(legacy_blob) = legacy_settings_blob {
        if let Some(legacy_object) = legacy_blob.as_object() {
            for (key, value) in legacy_object {
                apply_context_management_setting(&mut settings, key, value);
            }
        }
    }

    for (key, value) in direct_settings {
        apply_context_management_setting(&mut settings, key, value);
    }

    settings
}

pub(crate) async fn load_context_management_settings() -> ContextManagementSettings {
    let settings_repo = crate::state::get_settings_repository();
    let legacy_settings_blob = settings_repo
        .get("settings")
        .await
        .unwrap_or(None)
        .and_then(|model| serde_json::from_str::<serde_json::Value>(&model.value).ok());

    let mut direct_settings = HashMap::new();
    for key in [
        "contextStrategy",
        "windowSize",
        "maxInputContext",
        "toolCallGroupVisibleCount",
    ] {
        if let Some(model) = settings_repo.get(key).await.unwrap_or(None) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&model.value) {
                direct_settings.insert(key.to_string(), value);
            }
        }
    }

    let mut direct_keys: Vec<String> = direct_settings.keys().cloned().collect();
    direct_keys.sort();

    let settings =
        resolve_context_management_settings(legacy_settings_blob.as_ref(), &direct_settings);

    log::info!(
        "🧭 Loaded context management settings: strategy={}, window_size={}, max_input_context={}, tool_call_group_visible_count={}, model_max_limit={}, legacy_blob_present={}, direct_keys={}",
        settings.context_strategy,
        settings.window_size,
        settings.max_input_context,
        settings.tool_call_group_visible_count,
        settings.model_max_limit,
        legacy_settings_blob.is_some(),
        if direct_keys.is_empty() {
            "<none>".to_string()
        } else {
            direct_keys.join(",")
        }
    );

    settings
}

/// Apply a session-scoped `maxInputContext` override onto an in-memory copy.
///
/// Mutates only the passed `ContextManagementSettings` value — never the
/// settings repository / global UI config.
pub(crate) fn apply_session_max_input_context_override(
    settings: &mut ContextManagementSettings,
    override_tokens: Option<usize>,
) {
    if let Some(tokens) = override_tokens.filter(|value| *value > 0) {
        settings.max_input_context = tokens;
    }
}

/// Load global context-management settings, then overlay any session override
/// onto the returned copy. Global settings storage is read-only here.
pub(crate) async fn load_context_management_settings_for_session(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> ContextManagementSettings {
    let mut settings = load_context_management_settings().await;
    let override_tokens = {
        let active = active_sessions.read().await;
        active
            .get(session_id)
            .and_then(|session| session.max_input_context_override)
    };
    if let Some(tokens) = override_tokens.filter(|value| *value > 0) {
        log::info!(
            "Using session maxInputContext override (global settings unchanged): session={}, value={}",
            session_id,
            tokens
        );
        apply_session_max_input_context_override(&mut settings, Some(tokens));
    }
    settings
}

pub fn uses_compaction_strategy(context_strategy: &str) -> bool {
    context_strategy == "compact"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_session_override_replaces_max_input_context() {
        let mut settings = default_context_management_settings();
        assert_eq!(settings.max_input_context(), 49_152);
        apply_session_max_input_context_override(&mut settings, Some(43_000));
        assert_eq!(settings.max_input_context(), 43_000);
    }

    #[test]
    fn apply_session_override_ignores_none_and_zero() {
        let mut settings = default_context_management_settings();
        apply_session_max_input_context_override(&mut settings, None);
        assert_eq!(settings.max_input_context(), 49_152);
        apply_session_max_input_context_override(&mut settings, Some(0));
        assert_eq!(settings.max_input_context(), 49_152);
    }
}
