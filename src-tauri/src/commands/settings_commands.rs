use crate::repositories::settings_repository::SettingsRepository;
use crate::state::get_settings_repository;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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

#[command]
pub async fn set_setting(key: String, value: Value) -> Result<SettingDto, String> {
    let repo = get_settings_repository();
    let model = repo
        .set(&key, value)
        .await
        .map_err(|e| format!("Failed to set setting: {}", e))?;
    Ok(model.into())
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
