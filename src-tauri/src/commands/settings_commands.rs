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
        Self {
            key: model.key,
            value: serde_json::from_str(&model.value).unwrap_or(Value::Null),
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

fn apply_system_settings_side_effects(value: &Value) {
    if let Ok(settings) =
        serde_json::from_value::<crate::lifecycle::settings::SystemSettings>(value.clone())
    {
        crate::utils::keep_awake::set_user_preference(
            settings.prevent_sleep_during_agent_work_or_default(),
        );
    }
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
