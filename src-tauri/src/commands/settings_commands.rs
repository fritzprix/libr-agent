use crate::repositories::SettingsRepository;
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

impl From<crate::repositories::Setting> for SettingDto {
    fn from(model: crate::repositories::Setting) -> Self {
        Self {
            key: model.key,
            value: model.value,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[command]
pub async fn set_setting(key: String, value: Value) -> Result<SettingDto, String> {
    let repo = get_settings_repository();
    repo.set(&key, value)
        .await
        .map(|s| s.into())
        .map_err(|e| e.to_string())
}

#[command]
pub async fn get_setting(key: String) -> Result<Option<SettingDto>, String> {
    let repo = get_settings_repository();
    repo.get(&key)
        .await
        .map(|s| s.map(|v| v.into()))
        .map_err(|e| e.to_string())
}

#[command]
pub async fn delete_setting(key: String) -> Result<(), String> {
    let repo = get_settings_repository();
    repo.delete(&key).await.map_err(|e| e.to_string())
}

#[command]
pub async fn list_settings() -> Result<Vec<SettingDto>, String> {
    let repo = get_settings_repository();
    repo.list()
        .await
        .map(|list| list.into_iter().map(|s| s.into()).collect())
        .map_err(|e| e.to_string())
}
