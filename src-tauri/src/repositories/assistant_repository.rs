//! Assistant repository for database operations on the assistants table
//!
//! This repository provides a clean abstraction layer for all assistant-related
//! database operations, following the repository pattern for separation of concerns.

use super::error::DbError;
use crate::entity::assistant::{self, Entity as AssistantEntity};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel,
    PaginatorTrait, QueryFilter, Set,
};

/// Assistant repository trait for abstraction and testability
#[async_trait::async_trait]
pub trait AssistantRepository: Send + Sync {
    /// Create a new assistant
    async fn create_assistant(
        &self,
        id: String,
        name: String,
        config: String,
    ) -> Result<assistant::Model, DbError>;

    /// Get an assistant by ID
    async fn get_assistant(&self, id: &str) -> Result<Option<assistant::Model>, DbError>;

    /// Get all assistants
    async fn list_assistants(&self) -> Result<Vec<assistant::Model>, DbError>;

    /// Update an assistant
    async fn update_assistant(
        &self,
        id: &str,
        name: Option<String>,
        config: Option<String>,
    ) -> Result<assistant::Model, DbError>;

    /// Delete an assistant by ID
    async fn delete_assistant(&self, id: &str) -> Result<(), DbError>;

    /// Check if an assistant with a given name exists
    async fn check_assistant_exists(&self, name: &str) -> Result<bool, DbError>;

    /// Search assistants by name (case-insensitive substring match)
    async fn search_assistants(&self, query: &str) -> Result<Vec<assistant::Model>, DbError>;

    /// Count total assistants
    async fn count_assistants(&self) -> Result<u64, DbError>;
}

/// SQLite implementation of AssistantRepository using SeaORM
#[derive(Debug, Clone)]
pub struct SqliteAssistantRepository {
    db: DatabaseConnection,
}

impl SqliteAssistantRepository {
    /// Create a new SQLite assistant repository with the given database connection
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl AssistantRepository for SqliteAssistantRepository {
    async fn create_assistant(
        &self,
        id: String,
        name: String,
        config: String,
    ) -> Result<assistant::Model, DbError> {
        let now = chrono::Utc::now().timestamp();

        let active_model = assistant::ActiveModel {
            id: Set(id),
            name: Set(name),
            config: Set(config),
            created_at: Set(now),
            updated_at: Set(now),
        };

        active_model
            .insert(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn get_assistant(&self, id: &str) -> Result<Option<assistant::Model>, DbError> {
        AssistantEntity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn list_assistants(&self) -> Result<Vec<assistant::Model>, DbError> {
        AssistantEntity::find()
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn update_assistant(
        &self,
        id: &str,
        name: Option<String>,
        config: Option<String>,
    ) -> Result<assistant::Model, DbError> {
        // Get existing assistant
        let assistant = self
            .get_assistant(id)
            .await?
            .ok_or_else(|| DbError::NotFound(format!("Assistant {} not found", id)))?;

        // Update only provided fields
        let mut active_model = assistant.into_active_model();
        if let Some(n) = name {
            active_model.name = Set(n);
        }
        if let Some(c) = config {
            active_model.config = Set(c);
        }
        active_model.updated_at = Set(chrono::Utc::now().timestamp());

        active_model
            .update(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn delete_assistant(&self, id: &str) -> Result<(), DbError> {
        let result = AssistantEntity::delete_by_id(id)
            .exec(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        if result.rows_affected == 0 {
            return Err(DbError::NotFound(format!("Assistant {} not found", id)));
        }

        Ok(())
    }

    async fn check_assistant_exists(&self, name: &str) -> Result<bool, DbError> {
        let count = AssistantEntity::find()
            .filter(assistant::Column::Name.eq(name))
            .count(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        Ok(count > 0)
    }

    async fn search_assistants(&self, query: &str) -> Result<Vec<assistant::Model>, DbError> {
        let query_pattern = format!("%{}%", query.to_lowercase());

        AssistantEntity::find()
            .filter(Expr::cust_with_values(
                "LOWER(name) LIKE ?",
                vec![sea_orm::Value::from(query_pattern)],
            ))
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn count_assistants(&self) -> Result<u64, DbError> {
        AssistantEntity::find()
            .count(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::ConnectionTrait;

    async fn setup_test_db() -> SqliteAssistantRepository {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory database");

        let schema = sea_orm::Schema::new(db.get_database_backend());
        let stmt = schema.create_table_from_entity(AssistantEntity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create assistants table");

        SqliteAssistantRepository::new(db)
    }

    #[tokio::test]
    async fn test_create_and_get_assistant() {
        let repo = setup_test_db().await;

        let assistant = repo
            .create_assistant(
                "test-id".to_string(),
                "Test Assistant".to_string(),
                r#"{"model": "gpt-4"}"#.to_string(),
            )
            .await
            .expect("Failed to create assistant");

        assert_eq!(assistant.id, "test-id");
        assert_eq!(assistant.name, "Test Assistant");

        let fetched = repo
            .get_assistant("test-id")
            .await
            .expect("Failed to get assistant");
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "Test Assistant");
    }

    #[tokio::test]
    async fn test_update_assistant() {
        let repo = setup_test_db().await;

        repo.create_assistant(
            "test-id".to_string(),
            "Original".to_string(),
            r#"{"model": "gpt-4"}"#.to_string(),
        )
        .await
        .expect("Failed to create");

        let updated = repo
            .update_assistant("test-id", Some("Updated".to_string()), None)
            .await
            .expect("Failed to update");

        assert_eq!(updated.name, "Updated");
    }

    #[tokio::test]
    async fn test_delete_assistant() {
        let repo = setup_test_db().await;

        repo.create_assistant("test-id".to_string(), "Test".to_string(), "{}".to_string())
            .await
            .expect("Failed to create");

        repo.delete_assistant("test-id")
            .await
            .expect("Failed to delete");

        let result = repo.get_assistant("test-id").await.expect("Failed to get");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_check_assistant_exists() {
        let repo = setup_test_db().await;

        let exists = repo
            .check_assistant_exists("NonExistent")
            .await
            .expect("Failed to check");
        assert!(!exists);

        repo.create_assistant("id1".to_string(), "TestName".to_string(), "{}".to_string())
            .await
            .expect("Failed to create");

        let exists = repo
            .check_assistant_exists("TestName")
            .await
            .expect("Failed to check");
        assert!(exists);
    }
}
