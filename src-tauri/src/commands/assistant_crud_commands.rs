use crate::entity::assistant;
use crate::state::get_database_connection;
use sea_orm::{ActiveModelTrait, EntityTrait, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::command;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantDto {
    pub id: String,
    pub name: String,
    pub config: Value, // JSON config
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<assistant::Model> for AssistantDto {
    fn from(model: assistant::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            config: serde_json::from_str(&model.config).unwrap_or(Value::Null),
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[command]
pub async fn create_assistant(
    id: String,
    name: String,
    config: Value,
) -> Result<AssistantDto, String> {
    let db = get_database_connection();
    let now = chrono::Utc::now().timestamp_millis();

    let assistant = assistant::ActiveModel {
        id: Set(id),
        name: Set(name),
        config: Set(config.to_string()),
        created_at: Set(now),
        updated_at: Set(now),
    };

    let result = assistant
        .insert(db)
        .await
        .map_err(|e| format!("Failed to create assistant: {}", e))?;

    Ok(result.into())
}

#[command]
pub async fn update_assistant(
    id: String,
    name: Option<String>,
    config: Option<Value>,
) -> Result<AssistantDto, String> {
    let db = get_database_connection();
    let now = chrono::Utc::now().timestamp_millis();

    let mut assistant: assistant::ActiveModel = assistant::Entity::find_by_id(&id)
        .one(db)
        .await
        .map_err(|e| format!("Failed to find assistant: {}", e))?
        .ok_or_else(|| "Assistant not found".to_string())?
        .into();

    if let Some(name) = name {
        assistant.name = Set(name);
    }
    if let Some(config) = config {
        assistant.config = Set(config.to_string());
    }
    assistant.updated_at = Set(now);

    let result = assistant
        .update(db)
        .await
        .map_err(|e| format!("Failed to update assistant: {}", e))?;

    Ok(result.into())
}

#[command]
pub async fn delete_assistant(id: String) -> Result<(), String> {
    let db = get_database_connection();
    assistant::Entity::delete_by_id(id)
        .exec(db)
        .await
        .map_err(|e| format!("Failed to delete assistant: {}", e))?;
    Ok(())
}

#[command]
pub async fn list_assistants() -> Result<Vec<AssistantDto>, String> {
    let db = get_database_connection();
    let assistants = assistant::Entity::find()
        .order_by_asc(assistant::Column::CreatedAt)
        .all(db)
        .await
        .map_err(|e| format!("Failed to list assistants: {}", e))?;

    Ok(assistants.into_iter().map(|a| a.into()).collect())
}

#[command]
pub async fn get_assistant(id: String) -> Result<Option<AssistantDto>, String> {
    let db = get_database_connection();
    let assistant = assistant::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| format!("Failed to get assistant: {}", e))?;

    Ok(assistant.map(|a| a.into()))
}
