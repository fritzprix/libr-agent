use super::error::DbError;
use crate::entity::mcp_server;
use async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::Value;

/// MCP Server repository trait for abstraction and testability
#[async_trait]
pub trait MCPServerRepository: Send + Sync {
    /// Create a new MCP server config (ID is auto-generated)
    async fn create(&self, name: &str, config: Value) -> Result<mcp_server::Model, DbError>;

    /// Get an MCP server config by ID (primary key)
    async fn get(&self, id: &str) -> Result<Option<mcp_server::Model>, DbError>;

    /// Get an MCP server config by name (for user lookups)
    async fn get_by_name(&self, name: &str) -> Result<Option<mcp_server::Model>, DbError>;

    /// Update an MCP server config (allows name change)
    async fn update(
        &self,
        id: &str,
        name: Option<&str>,
        config: Option<Value>,
    ) -> Result<mcp_server::Model, DbError>;

    /// Delete an MCP server config by ID
    async fn delete(&self, id: &str) -> Result<(), DbError>;

    /// List all MCP server configs
    async fn list(&self) -> Result<Vec<mcp_server::Model>, DbError>;

    /// Update tool count for an MCP server after verification/connection
    async fn update_tool_count(&self, id: &str, tool_count: i32) -> Result<(), DbError>;

    /// Update cached tool list (name + description) after verification.
    /// `tools_json` is a JSON array string: [{"name":"..","description":".."}]
    async fn update_cached_tools(
        &self,
        id: &str,
        tool_count: i32,
        tools_json: String,
    ) -> Result<(), DbError>;
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
        // Reject empty or whitespace-only server names
        let name = name.trim();
        if name.is_empty() {
            return Err(DbError::InvalidInput(
                "MCP server name cannot be empty or whitespace-only".to_string(),
            ));
        }

        let now = chrono::Utc::now().timestamp_millis();
        let id = cuid2::create_id(); // Auto-generate immutable ID

        // Check name uniqueness
        if self.get_by_name(name).await?.is_some() {
            return Err(DbError::DuplicateResource(format!(
                "MCP server with name '{}' already exists",
                name
            )));
        }

        let active = mcp_server::ActiveModel {
            id: Set(id.clone()),
            name: Set(name.to_string()),
            config: Set(config.to_string()),
            tool_count: Set(None), // NULL initially - will be populated during verification/connection
            cached_tools: Set(None), // NULL initially - populated after verifyServer or probe
            created_at: Set(now),
            updated_at: Set(now),
        };

        let model = active.insert(&self.db).await?;
        Ok(model)
    }

    async fn get(&self, id: &str) -> Result<Option<mcp_server::Model>, DbError> {
        let result = mcp_server::Entity::find_by_id(id).one(&self.db).await?;
        Ok(result)
    }

    async fn get_by_name(&self, name: &str) -> Result<Option<mcp_server::Model>, DbError> {
        let result = mcp_server::Entity::find()
            .filter(mcp_server::Column::Name.eq(name))
            .one(&self.db)
            .await?;
        Ok(result)
    }

    async fn update(
        &self,
        id: &str,
        name: Option<&str>,
        config: Option<Value>,
    ) -> Result<mcp_server::Model, DbError> {
        let now = chrono::Utc::now().timestamp_millis();

        let existing = mcp_server::Entity::find_by_id(id).one(&self.db).await?;

        let model = if let Some(existing_model) = existing {
            // Validate name uniqueness if changing name
            if let Some(new_name) = name {
                if let Ok(Some(other)) = self.get_by_name(new_name).await {
                    if other.id != id {
                        return Err(DbError::DuplicateResource(format!(
                            "MCP server with name '{}' already exists",
                            new_name
                        )));
                    }
                }
            }

            let mut active: mcp_server::ActiveModel = existing_model.into();
            if let Some(new_name) = name {
                active.name = Set(new_name.to_string());
            }
            if let Some(new_config) = config {
                active.config = Set(new_config.to_string());
                // Invalidate both cached_tools AND tool_count when config changes — they may be stale
                active.cached_tools = Set(None);
                active.tool_count = Set(None);
            }
            active.updated_at = Set(now);
            active.update(&self.db).await?
        } else {
            return Err(DbError::ResourceNotFound(format!(
                "MCP server not found: {}",
                id
            )));
        };

        Ok(model)
    }

    async fn delete(&self, id: &str) -> Result<(), DbError> {
        mcp_server::Entity::delete_by_id(id).exec(&self.db).await?;
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

    async fn update_tool_count(&self, id: &str, tool_count: i32) -> Result<(), DbError> {
        let now = chrono::Utc::now().timestamp_millis();

        let existing = mcp_server::Entity::find_by_id(id).one(&self.db).await?;

        if let Some(model) = existing {
            let mut active: mcp_server::ActiveModel = model.into();
            active.tool_count = Set(Some(tool_count));
            active.updated_at = Set(now);
            active.update(&self.db).await?;
            Ok(())
        } else {
            Err(DbError::NotFound(format!("Server '{}' not found", id)))
        }
    }

    async fn update_cached_tools(
        &self,
        id: &str,
        tool_count: i32,
        tools_json: String,
    ) -> Result<(), DbError> {
        let now = chrono::Utc::now().timestamp_millis();

        let existing = mcp_server::Entity::find_by_id(id).one(&self.db).await?;

        if let Some(model) = existing {
            let mut active: mcp_server::ActiveModel = model.into();
            active.tool_count = Set(Some(tool_count));
            active.cached_tools = Set(Some(tools_json));
            active.updated_at = Set(now);
            active.update(&self.db).await?;
            Ok(())
        } else {
            Err(DbError::NotFound(format!("Server '{}' not found", id)))
        }
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

        // Test Create (ID is auto-generated)
        let saved = repo
            .create(name, config.clone())
            .await
            .expect("Failed to create server");
        assert_eq!(saved.name, name);
        assert_eq!(saved.config, config.to_string());
        assert!(!saved.id.is_empty()); // ID should be generated

        // Test Get by ID
        let retrieved = repo.get(&saved.id).await.expect("Failed to get server");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.name, name);
        assert_eq!(retrieved.id, saved.id);

        // Test Get by name
        let retrieved_by_name = repo.get_by_name(name).await.expect("Failed to get by name");
        assert!(retrieved_by_name.is_some());
        assert_eq!(retrieved_by_name.unwrap().id, saved.id);
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
        let created = repo
            .create(name, serde_json::json!({"v": 1}))
            .await
            .unwrap();

        // Update config only
        let new_config = serde_json::json!({"v": 2});
        let updated = repo
            .update(&created.id, None, Some(new_config.clone()))
            .await
            .expect("Failed to update");

        assert_eq!(updated.config, new_config.to_string());
        assert_eq!(updated.name, name); // Name unchanged

        // Update name only
        let new_name = "renamed_server";
        let updated = repo
            .update(&created.id, Some(new_name), None)
            .await
            .expect("Failed to update name");

        assert_eq!(updated.name, new_name);

        // Verify can get by new name
        let by_name = repo.get_by_name(new_name).await.unwrap();
        assert!(by_name.is_some());
        assert_eq!(by_name.unwrap().id, created.id);
    }

    #[tokio::test]
    async fn test_delete_server() {
        let repo = setup_test_db().await;
        let name = "delete_server";

        let created = repo.create(name, serde_json::json!({})).await.unwrap();
        repo.delete(&created.id).await.expect("Failed to delete");

        let result = repo
            .get(&created.id)
            .await
            .expect("Failed to get after delete");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_update_tool_count() {
        let repo = setup_test_db().await;
        let name = "tool_count_server";

        let created = repo.create(name, serde_json::json!({})).await.unwrap();

        // Update tool count
        repo.update_tool_count(&created.id, 42)
            .await
            .expect("Failed to update tool count");

        let result = repo.get(&created.id).await.expect("Failed to get").unwrap();
        assert_eq!(result.tool_count, Some(42));
    }

    #[tokio::test]
    async fn test_update_cached_tools() {
        let repo = setup_test_db().await;
        let created = repo
            .create("cache_server", serde_json::json!({}))
            .await
            .unwrap();

        let tools_json = r#"[{"name":"foo","description":"does foo"}]"#.to_string();
        repo.update_cached_tools(&created.id, 1, tools_json.clone())
            .await
            .expect("Failed to update cached tools");

        let result = repo.get(&created.id).await.unwrap().unwrap();
        assert_eq!(result.tool_count, Some(1));
        assert_eq!(result.cached_tools.as_deref(), Some(tools_json.as_str()));
    }

    #[tokio::test]
    async fn test_config_update_invalidates_tool_cache() {
        // Regression: updating config must clear BOTH cached_tools AND tool_count.
        let repo = setup_test_db().await;
        let created = repo
            .create("invalidate_server", serde_json::json!({"v": 1}))
            .await
            .unwrap();

        // Populate cache
        repo.update_cached_tools(
            &created.id,
            5,
            r#"[{"name":"t1","description":""}]"#.to_string(),
        )
        .await
        .unwrap();

        // Verify cache is populated
        let cached = repo.get(&created.id).await.unwrap().unwrap();
        assert_eq!(cached.tool_count, Some(5));
        assert!(cached.cached_tools.is_some());

        // Update config — must invalidate both
        repo.update(&created.id, None, Some(serde_json::json!({"v": 2})))
            .await
            .expect("Failed to update config");

        let after = repo.get(&created.id).await.unwrap().unwrap();
        assert!(
            after.tool_count.is_none(),
            "tool_count must be cleared when config changes"
        );
        assert!(
            after.cached_tools.is_none(),
            "cached_tools must be cleared when config changes"
        );
    }

    #[tokio::test]
    async fn test_name_only_update_preserves_tool_cache() {
        // Regression: renaming a server must NOT invalidate the tool cache.
        let repo = setup_test_db().await;
        let created = repo
            .create("before_rename", serde_json::json!({}))
            .await
            .unwrap();

        repo.update_cached_tools(
            &created.id,
            3,
            r#"[{"name":"t1","description":""}]"#.to_string(),
        )
        .await
        .unwrap();

        // Rename only — no config change
        repo.update(&created.id, Some("after_rename"), None)
            .await
            .expect("Failed to rename");

        let after = repo.get(&created.id).await.unwrap().unwrap();
        assert_eq!(
            after.tool_count,
            Some(3),
            "tool_count must be preserved on name-only update"
        );
        assert!(
            after.cached_tools.is_some(),
            "cached_tools must be preserved on name-only update"
        );
    }
}
