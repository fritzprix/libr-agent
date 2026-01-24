use super::error::DbError;
use crate::entity::mcp_server;
use async_trait::async_trait;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde_json::Value;

/// MCP Server repository trait for abstraction and testability
#[async_trait]
pub trait MCPServerRepository: Send + Sync {
    /// Create a new MCP server config
    async fn create(&self, name: &str, config: Value) -> Result<mcp_server::Model, DbError>;

    /// Get an MCP server config by name
    async fn get(&self, name: &str) -> Result<Option<mcp_server::Model>, DbError>;

    /// Update an MCP server config
    async fn update(&self, name: &str, config: Value) -> Result<mcp_server::Model, DbError>;

    /// Delete an MCP server config
    async fn delete(&self, name: &str) -> Result<(), DbError>;

    /// List all MCP server configs
    async fn list(&self) -> Result<Vec<mcp_server::Model>, DbError>;
}

/// SQLite implementation of MCPServerRepository using SeaORM
#[derive(Debug, Clone)]
pub struct SqliteMCPServerRepository {
    db: DatabaseConnection,
}

impl SqliteMCPServerRepository {
    /// Create a new SQLite MCP server repository with the given database connection
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MCPServerRepository for SqliteMCPServerRepository {
    async fn create(&self, name: &str, config: Value) -> Result<mcp_server::Model, DbError> {
        let now = chrono::Utc::now().timestamp_millis();

        // Check if exists
        let exists = mcp_server::Entity::find_by_id(name).one(&self.db).await?;

        if exists.is_some() {
            return Err(DbError::DuplicateResource(format!(
                "MCP server config with name '{}' already exists",
                name
            )));
        }

        let active = mcp_server::ActiveModel {
            name: Set(name.to_string()),
            config: Set(config.to_string()),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let model = active.insert(&self.db).await?;
        Ok(model)
    }

    async fn get(&self, name: &str) -> Result<Option<mcp_server::Model>, DbError> {
        let result = mcp_server::Entity::find_by_id(name).one(&self.db).await?;
        Ok(result)
    }

    async fn update(&self, name: &str, config: Value) -> Result<mcp_server::Model, DbError> {
        let now = chrono::Utc::now().timestamp_millis();

        let existing = mcp_server::Entity::find_by_id(name).one(&self.db).await?;

        let model = if let Some(existing_model) = existing {
            let mut active: mcp_server::ActiveModel = existing_model.into();
            active.config = Set(config.to_string());
            active.updated_at = Set(now);
            active.update(&self.db).await?
        } else {
            return Err(DbError::ResourceNotFound(format!(
                "MCP server config not found: {}",
                name
            )));
        };

        Ok(model)
    }

    async fn delete(&self, name: &str) -> Result<(), DbError> {
        mcp_server::Entity::delete_by_id(name)
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn list(&self) -> Result<Vec<mcp_server::Model>, DbError> {
        use sea_orm::QueryOrder;
        let results = mcp_server::Entity::find()
            .order_by_asc(mcp_server::Column::CreatedAt)
            .all(&self.db)
            .await?;
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::mcp_server;
    use sea_orm::{ConnectionTrait, Database, Schema};

    async fn setup_test_db() -> SqliteMCPServerRepository {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory database");

        let schema = Schema::new(db.get_database_backend());
        let stmt = schema.create_table_from_entity(mcp_server::Entity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create mcp_server table");

        SqliteMCPServerRepository::new(db)
    }

    #[tokio::test]
    async fn test_create_and_get_server() {
        let repo = setup_test_db().await;

        let name = "test_server";
        let config = serde_json::json!({"cmd": "test"});

        // Test Create
        let saved = repo
            .create(name, config.clone())
            .await
            .expect("Failed to create server");
        assert_eq!(saved.name, name);
        assert_eq!(saved.config, config.to_string());

        // Test Get
        let retrieved = repo.get(name).await.expect("Failed to get server");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.name, name);
    }

    #[tokio::test]
    async fn test_create_duplicate_fails() {
        let repo = setup_test_db().await;
        let name = "dup_server";

        repo.create(name, serde_json::json!({})).await.unwrap();

        let result = repo.create(name, serde_json::json!({})).await;
        assert!(matches!(result, Err(DbError::DuplicateResource(_))));
    }

    #[tokio::test]
    async fn test_update_server() {
        let repo = setup_test_db().await;

        let name = "update_server";
        repo.create(name, serde_json::json!({"v": 1}))
            .await
            .unwrap();

        // Update
        let new_config = serde_json::json!({"v": 2});
        let updated = repo
            .update(name, new_config.clone())
            .await
            .expect("Failed to update");

        assert_eq!(updated.config, new_config.to_string());
    }

    #[tokio::test]
    async fn test_delete_server() {
        let repo = setup_test_db().await;
        let name = "delete_server";

        repo.create(name, serde_json::json!({})).await.unwrap();
        repo.delete(name).await.expect("Failed to delete");

        let result = repo.get(name).await.expect("Failed to get after delete");
        assert!(result.is_none());
    }
}
