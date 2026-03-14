use super::error::DbError;
use crate::entity::settings;
use async_trait::async_trait;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set, TransactionTrait};
use serde_json::Value;
use std::collections::HashMap;

/// Settings repository trait for abstraction and testability
#[async_trait]
pub trait SettingsRepository: Send + Sync {
    /// Get a setting by key
    async fn get(&self, key: &str) -> Result<Option<settings::Model>, DbError>;

    /// Set a setting (insert or update)
    async fn set(&self, key: &str, value: Value) -> Result<settings::Model, DbError>;

    /// Set multiple settings in a batch (insert or update)
    async fn set_many(
        &self,
        settings: HashMap<String, Value>,
    ) -> Result<Vec<settings::Model>, DbError>;

    /// Delete a setting by key
    async fn delete(&self, key: &str) -> Result<(), DbError>;

    /// List all settings
    async fn list(&self) -> Result<Vec<settings::Model>, DbError>;
}

/// `SQLite` implementation of `SettingsRepository` using `SeaORM`
#[derive(Debug, Clone)]
pub struct SqliteSettingsRepository {
    db: DatabaseConnection,
}

impl SqliteSettingsRepository {
    /// Create a new `SQLite` settings repository with the given database connection
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl SettingsRepository for SqliteSettingsRepository {
    async fn get(&self, key: &str) -> Result<Option<settings::Model>, DbError> {
        let result = settings::Entity::find_by_id(key).one(&self.db).await?;
        Ok(result)
    }

    async fn set(&self, key: &str, value: Value) -> Result<settings::Model, DbError> {
        let now = chrono::Utc::now().timestamp_millis();

        // Check if exists
        let existing = settings::Entity::find_by_id(key).one(&self.db).await?;

        let model = if let Some(existing_model) = existing {
            let mut active: settings::ActiveModel = existing_model.into();
            active.value = Set(value.to_string());
            active.updated_at = Set(now);
            active.update(&self.db).await?
        } else {
            let active = settings::ActiveModel {
                key: Set(key.to_string()),
                value: Set(value.to_string()),
                created_at: Set(now),
                updated_at: Set(now),
            };
            active.insert(&self.db).await?
        };

        Ok(model)
    }

    async fn set_many(
        &self,
        settings: HashMap<String, Value>,
    ) -> Result<Vec<settings::Model>, DbError> {
        let now = chrono::Utc::now().timestamp_millis();
        let txn = self.db.begin().await?;

        let mut results = Vec::new();

        for (key, value) in settings {
            // Check if exists within transaction
            let existing = settings::Entity::find_by_id(&key).one(&txn).await?;

            let model = if let Some(existing_model) = existing {
                let mut active: settings::ActiveModel = existing_model.into();
                active.value = Set(value.to_string());
                active.updated_at = Set(now);
                active.update(&txn).await?
            } else {
                let active = settings::ActiveModel {
                    key: Set(key.to_string()),
                    value: Set(value.to_string()),
                    created_at: Set(now),
                    updated_at: Set(now),
                };
                active.insert(&txn).await?
            };
            results.push(model);
        }

        txn.commit().await?;
        Ok(results)
    }

    async fn delete(&self, key: &str) -> Result<(), DbError> {
        settings::Entity::delete_by_id(key).exec(&self.db).await?;
        Ok(())
    }

    async fn list(&self) -> Result<Vec<settings::Model>, DbError> {
        use sea_orm::QueryOrder;
        let results = settings::Entity::find()
            .order_by_asc(settings::Column::Key)
            .all(&self.db)
            .await?;
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::settings;
    use sea_orm::{ConnectionTrait, Database, Schema};

    async fn setup_test_db() -> SqliteSettingsRepository {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory database");

        let schema = Schema::new(db.get_database_backend());
        let stmt = schema.create_table_from_entity(settings::Entity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create settings table");

        SqliteSettingsRepository::new(db)
    }

    #[tokio::test]
    async fn test_set_and_get_setting() {
        let repo = setup_test_db().await;

        let key = "test_key";
        let value = serde_json::json!({"foo": "bar"});

        // Test Set
        let saved = repo
            .set(key, value.clone())
            .await
            .expect("Failed to set setting");
        assert_eq!(saved.key, key);
        assert_eq!(saved.value, value.to_string());

        // Test Get
        let retrieved = repo.get(key).await.expect("Failed to get setting");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.key, key);
        assert_eq!(retrieved.value, value.to_string());
    }

    #[tokio::test]
    async fn test_update_setting() {
        let repo = setup_test_db().await;

        let key = "update_key";
        let initial_value = serde_json::json!("initial");
        repo.set(key, initial_value)
            .await
            .expect("Failed to set initial");

        // Update
        let new_value = serde_json::json!("updated");
        let updated = repo
            .set(key, new_value.clone())
            .await
            .expect("Failed to update");

        assert_eq!(updated.value, new_value.to_string());
        assert!(updated.updated_at >= updated.created_at);
    }

    #[tokio::test]
    async fn test_delete_setting() {
        let repo = setup_test_db().await;

        let key = "delete_key";
        repo.set(key, serde_json::json!(true))
            .await
            .expect("Failed to set");

        repo.delete(key).await.expect("Failed to delete");

        let result = repo.get(key).await.expect("Failed to get after delete");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_settings() {
        let repo = setup_test_db().await;

        repo.set("a", serde_json::json!(1)).await.unwrap();
        repo.set("b", serde_json::json!(2)).await.unwrap();

        let list = repo.list().await.expect("Failed to list");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].key, "a");
        assert_eq!(list[1].key, "b");
    }

    #[tokio::test]
    async fn test_set_many_settings() {
        let repo = setup_test_db().await;

        let mut settings = std::collections::HashMap::new();
        settings.insert("key1".to_string(), serde_json::json!("value1"));
        settings.insert("key2".to_string(), serde_json::json!("value2"));

        let results = repo.set_many(settings).await.expect("Failed to set many");
        assert_eq!(results.len(), 2);

        // Verify key1
        let val1 = repo.get("key1").await.expect("Failed to get key1");
        assert!(val1.is_some());
        assert_eq!(val1.unwrap().value, "\"value1\"");

        // Verify key2
        let val2 = repo.get("key2").await.expect("Failed to get key2");
        assert!(val2.is_some());
        assert_eq!(val2.unwrap().value, "\"value2\"");

        // Update existing via set_many
        let mut updates = std::collections::HashMap::new();
        updates.insert("key1".to_string(), serde_json::json!("updated_value1"));
        repo.set_many(updates).await.expect("Failed to update many");

        let val1_updated = repo.get("key1").await.expect("Failed to get updated key1");
        assert_eq!(val1_updated.unwrap().value, "\"updated_value1\"");
    }
}
