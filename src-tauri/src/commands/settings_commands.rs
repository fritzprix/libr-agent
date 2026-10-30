use crate::models::advanced_settings::AdvancedSettings;
use crate::models::display_settings::DisplaySettings;
use crate::models::experimental_settings::ExperimentalSettings;
use crate::models::system_settings::SystemSettings;
use crate::repositories::settings_repository::SettingsRepository;
use crate::state::get_settings_repository;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tauri::command;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingDto {
    pub key: String,
    pub value: Value, // JSON
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<crate::entity::settings::Model> for SettingDto {
    fn from(model: crate::entity::settings::Model) -> Self {
        let value = serde_json::from_str(&model.value).unwrap_or_else(|e| {
            log::warn!(
                "Failed to deserialize setting JSON for key '{}': {}; falling back to Value::Null",
                model.key,
                e
            );
            Value::Null
        });
        Self {
            key: model.key,
            value,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

fn apply_system_settings_side_effects(value: &Value) {
    let settings = SystemSettings::from_stored(Some(&value.to_string()));
    crate::utils::keep_awake::set_user_preference(settings.prevent_sleep_during_agent_work);
    crate::logger::set_log_retention_days(settings.log_retention_days);
    tokio::task::spawn_blocking(|| {
        let (removed_files, removed_bytes) = crate::logger::prune_archived_logs_now();
        if removed_files > 0 {
            log::info!(
                "🧹 Pruned {} archived log file(s) ({} bytes) after settings change",
                removed_files,
                removed_bytes
            );
        }
    });
}

#[command]
pub async fn set_setting(key: String, value: Value) -> Result<SettingDto, String> {
    let repo = get_settings_repository();
    let model = repo
        .set(&key, value)
        .await
        .map_err(|e| format!("Failed to set setting: {}", e))?;
    if key == "systemSettings" {
        if let Ok(parsed) = serde_json::from_str::<Value>(&model.value) {
            apply_system_settings_side_effects(&parsed);
        }
    }
    Ok(model.into())
}

#[command]
pub async fn update_settings(settings: HashMap<String, Value>) -> Result<Vec<SettingDto>, String> {
    let repo = get_settings_repository();
    let system_settings = settings.get("systemSettings").cloned();
    let models = repo
        .set_many(settings)
        .await
        .map_err(|e| format!("Failed to update settings: {}", e))?;
    if let Some(value) = system_settings {
        apply_system_settings_side_effects(&value);
    }
    Ok(models.into_iter().map(|s| s.into()).collect())
}

#[command]
pub async fn get_setting(key: String) -> Result<Option<SettingDto>, String> {
    let repo = get_settings_repository();
    let model = repo
        .get(&key)
        .await
        .map_err(|e| format!("Failed to get setting: {}", e))?;
    Ok(model.map(|s| s.into()))
}

#[command]
pub async fn delete_setting(key: String) -> Result<(), String> {
    let repo = get_settings_repository();
    repo.delete(&key)
        .await
        .map_err(|e| format!("Failed to delete setting: {}", e))?;
    Ok(())
}

#[command]
pub async fn list_settings() -> Result<Vec<SettingDto>, String> {
    let repo = get_settings_repository();
    let models = repo
        .list()
        .await
        .map_err(|e| format!("Failed to list settings: {}", e))?;
    Ok(models.into_iter().map(|s| s.into()).collect())
}

#[command]
pub async fn get_advanced_settings() -> Result<AdvancedSettings, String> {
    Ok(AdvancedSettings::load_global().await)
}

#[command]
pub async fn get_experimental_settings() -> Result<ExperimentalSettings, String> {
    Ok(ExperimentalSettings::load_global().await)
}

#[command]
pub async fn get_display_settings() -> Result<DisplaySettings, String> {
    Ok(DisplaySettings::load_global().await)
}

#[command]
pub async fn get_system_settings() -> Result<SystemSettings, String> {
    Ok(SystemSettings::load_global().await)
}
