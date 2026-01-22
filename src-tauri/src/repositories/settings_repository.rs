use super::error::DbError;
use crate::entity::settings;
use async_trait::async_trait;
use sea_orm::{DatabaseConnection, EntityTrait, Set, ActiveModelTrait, QueryOrder};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Setting domain model
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Setting {
    pub key: String,
    pub value: Value,
    pub created_at: i64,
    pub updated_at: i64,
}

impl TryFrom<settings::Model> for Setting {
    type Error = DbError;

    fn try_from(model: settings::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            key: model.key,
            value: serde_json::from_str(&model.value)
                .map_err(|e| DbError::SerializationError(e.to_string()))?,
            created_at: model.created_at,
            updated_at: model.updated_at,
        })
    }
}

/// Settings repository trait
#[async_trait]
pub trait SettingsRepository: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Setting>, DbError>;
    async fn set(&self, key: &str, value: Value) -> Result<Setting, DbError>;
    async fn delete(&self, key: &str) -> Result<(), DbError>;
    async fn list(&self) -> Result<Vec<Setting>, DbError>;
}

/// SQLite implementation of SettingsRepository
#[derive(Debug)]
pub struct SqliteSettingsRepository {
    db: DatabaseConnection,
}

impl SqliteSettingsRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl SettingsRepository for SqliteSettingsRepository {
    async fn get(&self, key: &str) -> Result<Option<Setting>, DbError> {
        let result = settings::Entity::find_by_id(key)
            .one(&self.db)
            .await?;

        match result {
            Some(model) => Ok(Some(Setting::try_from(model)?)),
            None => Ok(None),
        }
    }

    async fn set(&self, key: &str, value: Value) -> Result<Setting, DbError> {
        let now = chrono::Utc::now().timestamp_millis();
        let value_str = value.to_string();

        let existing = settings::Entity::find_by_id(key)
            .one(&self.db)
            .await?;

        let model = if let Some(existing_model) = existing {
            let mut active: settings::ActiveModel = existing_model.into();
            active.value = Set(value_str);
            active.updated_at = Set(now);
            active.update(&self.db).await?
        } else {
            let active = settings::ActiveModel {
                key: Set(key.to_string()),
                value: Set(value_str),
                created_at: Set(now),
                updated_at: Set(now),
            };
            active.insert(&self.db).await?
        };

        Setting::try_from(model)
    }

    async fn delete(&self, key: &str) -> Result<(), DbError> {
        settings::Entity::delete_by_id(key)
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn list(&self) -> Result<Vec<Setting>, DbError> {
        let models = settings::Entity::find()
            .order_by_asc(settings::Column::Key)
            .all(&self.db)
            .await?;

        models
            .into_iter()
            .map(Setting::try_from)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::{Migrator, MigratorTrait};

    async fn setup_test_db() -> SqliteSettingsRepository {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory database");

        Migrator::up(&db, None)
            .await
            .expect("Failed to run migrations");

        SqliteSettingsRepository::new(db)
    }

    #[tokio::test]
    async fn test_set_and_get_setting() {
        let repo = setup_test_db().await;
        let value = serde_json::json!({ "theme": "dark" });

        let setting = repo.set("test_key", value.clone()).await.expect("Failed to set setting");
        assert_eq!(setting.key, "test_key");
        assert_eq!(setting.value, value);

        let retrieved = repo.get("test_key").await.expect("Failed to get setting");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().value, value);
    }

    #[tokio::test]
    async fn test_update_setting() {
        let repo = setup_test_db().await;

        repo.set("test_key", serde_json::json!(1)).await.expect("Failed to set initial");

        // Wait briefly to ensure timestamp update
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let updated = repo.set("test_key", serde_json::json!(2)).await.expect("Failed to update");
        assert_eq!(updated.value, serde_json::json!(2));
        assert!(updated.updated_at > updated.created_at);
    }

    #[tokio::test]
    async fn test_delete_setting() {
        let repo = setup_test_db().await;

        repo.set("test_key", serde_json::json!(true)).await.expect("Failed to set");
        repo.delete("test_key").await.expect("Failed to delete");

        let retrieved = repo.get("test_key").await.expect("Failed to get");
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_list_settings() {
        let repo = setup_test_db().await;

        repo.set("a", serde_json::json!(1)).await.expect("Failed to set a");
        repo.set("b", serde_json::json!(2)).await.expect("Failed to set b");

        let list = repo.list().await.expect("Failed to list");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].key, "a");
        assert_eq!(list[1].key, "b");
    }
}
