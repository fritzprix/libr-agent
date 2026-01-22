use super::error::DbError;
use crate::entity::mcp_server;
use async_trait::async_trait;
use sea_orm::{DatabaseConnection, EntityTrait, Set, ActiveModelTrait, QueryOrder};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// MCP Server domain model
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPServer {
    pub name: String,
    pub config: Value,
    pub created_at: i64,
    pub updated_at: i64,
}

impl TryFrom<mcp_server::Model> for MCPServer {
    type Error = DbError;

    fn try_from(model: mcp_server::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            name: model.name,
            config: serde_json::from_str(&model.config)
                .map_err(|e| DbError::SerializationError(e.to_string()))?,
            created_at: model.created_at,
            updated_at: model.updated_at,
        })
    }
}

/// MCP Server repository trait
#[async_trait]
pub trait MCPServerRepository: Send + Sync {
    async fn create(&self, name: &str, config: Value) -> Result<MCPServer, DbError>;
    async fn get(&self, name: &str) -> Result<Option<MCPServer>, DbError>;
    async fn update(&self, name: &str, config: Option<Value>) -> Result<MCPServer, DbError>;
    async fn delete(&self, name: &str) -> Result<(), DbError>;
    async fn list(&self) -> Result<Vec<MCPServer>, DbError>;
}

/// SQLite implementation of MCPServerRepository
#[derive(Debug)]
pub struct SqliteMCPServerRepository {
    db: DatabaseConnection,
}

impl SqliteMCPServerRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MCPServerRepository for SqliteMCPServerRepository {
    async fn create(&self, name: &str, config: Value) -> Result<MCPServer, DbError> {
        let now = chrono::Utc::now().timestamp_millis();

        let exists = mcp_server::Entity::find_by_id(name)
            .one(&self.db)
            .await?;

        if exists.is_some() {
            return Err(DbError::InvalidInput(format!("MCP server config with name '{}' already exists", name)));
        }

        let model = mcp_server::ActiveModel {
            name: Set(name.to_string()),
            config: Set(config.to_string()),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let result = model.insert(&self.db).await?;
        MCPServer::try_from(result)
    }

    async fn get(&self, name: &str) -> Result<Option<MCPServer>, DbError> {
        let result = mcp_server::Entity::find_by_id(name)
            .one(&self.db)
            .await?;

        match result {
            Some(model) => Ok(Some(MCPServer::try_from(model)?)),
            None => Ok(None),
        }
    }

    async fn update(&self, name: &str, config: Option<Value>) -> Result<MCPServer, DbError> {
        let now = chrono::Utc::now().timestamp_millis();

        let existing = mcp_server::Entity::find_by_id(name)
            .one(&self.db)
            .await?;

        let mut active: mcp_server::ActiveModel = existing
            .ok_or_else(|| DbError::NotFound(format!("MCP server config with name '{}' not found", name)))?
            .into();

        if let Some(c) = config {
            active.config = Set(c.to_string());
        }
        active.updated_at = Set(now);

        let result = active.update(&self.db).await?;
        MCPServer::try_from(result)
    }

    async fn delete(&self, name: &str) -> Result<(), DbError> {
        mcp_server::Entity::delete_by_id(name)
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn list(&self) -> Result<Vec<MCPServer>, DbError> {
        let models = mcp_server::Entity::find()
            .order_by_asc(mcp_server::Column::CreatedAt)
            .all(&self.db)
            .await?;

        models
            .into_iter()
            .map(MCPServer::try_from)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::{Migrator, MigratorTrait};

    async fn setup_test_db() -> SqliteMCPServerRepository {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory database");

        Migrator::up(&db, None)
            .await
            .expect("Failed to run migrations");

        SqliteMCPServerRepository::new(db)
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let repo = setup_test_db().await;
        let config = serde_json::json!({ "command": "npx", "args": ["-y", "mcp-server"] });

        let server = repo.create("test-server", config.clone()).await.expect("Failed to create");
        assert_eq!(server.name, "test-server");
        assert_eq!(server.config, config);

        let retrieved = repo.get("test-server").await.expect("Failed to get");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-server");
    }

    #[tokio::test]
    async fn test_create_duplicate() {
        let repo = setup_test_db().await;
        let config = serde_json::json!({});

        repo.create("test-server", config.clone()).await.expect("Failed to create");
        let result = repo.create("test-server", config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update() {
        let repo = setup_test_db().await;
        let config = serde_json::json!({ "v": 1 });

        repo.create("test-server", config).await.expect("Failed to create");

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let new_config = serde_json::json!({ "v": 2 });
        let updated = repo.update("test-server", Some(new_config.clone())).await.expect("Failed to update");

        assert_eq!(updated.config, new_config);
        assert!(updated.updated_at > updated.created_at);
    }

    #[tokio::test]
    async fn test_delete() {
        let repo = setup_test_db().await;
        let config = serde_json::json!({});

        repo.create("test-server", config).await.expect("Failed to create");
        repo.delete("test-server").await.expect("Failed to delete");

        let retrieved = repo.get("test-server").await.expect("Failed to get");
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_list() {
        let repo = setup_test_db().await;

        repo.create("server-1", serde_json::json!({})).await.expect("Failed to create 1");
        // Ensure creation time difference
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        repo.create("server-2", serde_json::json!({})).await.expect("Failed to create 2");

        let list = repo.list().await.expect("Failed to list");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "server-1");
        assert_eq!(list[1].name, "server-2");
    }
}
