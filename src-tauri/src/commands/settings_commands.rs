use crate::entity::settings;
use crate::state::get_database_connection;
use sea_orm::{ActiveModelTrait, EntityTrait, QueryOrder, Set};
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

impl From<settings::Model> for SettingDto {
    fn from(model: settings::Model) -> Self {
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
    let db = get_database_connection();
    let now = chrono::Utc::now().timestamp_millis();

    // Check if exists
    let existing = settings::Entity::find_by_id(&key)
        .one(db)
        .await
        .map_err(|e| format!("Failed to check setting existence: {}", e))?;

    let result = if let Some(existing_model) = existing {
        let mut active: settings::ActiveModel = existing_model.into();
        active.value = Set(value.to_string());
        active.updated_at = Set(now);
        active.update(db).await
    } else {
        let active = settings::ActiveModel {
            key: Set(key),
            value: Set(value.to_string()),
            created_at: Set(now),
            updated_at: Set(now),
        };
        active.insert(db).await
    };

    let model = result.map_err(|e| format!("Failed to set setting: {}", e))?;
    Ok(model.into())
}

#[command]
pub async fn get_setting(key: String) -> Result<Option<SettingDto>, String> {
    let db = get_database_connection();
    let setting = settings::Entity::find_by_id(key)
        .one(db)
        .await
        .map_err(|e| format!("Failed to get setting: {}", e))?;

    Ok(setting.map(|s| s.into()))
}

#[command]
pub async fn delete_setting(key: String) -> Result<(), String> {
    let db = get_database_connection();
    settings::Entity::delete_by_id(key)
        .exec(db)
        .await
        .map_err(|e| format!("Failed to delete setting: {}", e))?;
    Ok(())
}

#[command]
pub async fn list_settings() -> Result<Vec<SettingDto>, String> {
    let db = get_database_connection();
    let settings = settings::Entity::find()
        .order_by_asc(settings::Column::Key)
        .all(db)
        .await
        .map_err(|e| format!("Failed to list settings: {}", e))?;

    Ok(settings.into_iter().map(|s| s.into()).collect())
}
