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

    /// Mark verification as pending, optionally clearing cached verification results first.
    async fn mark_verification_pending(&self, id: &str, clear_cache: bool) -> Result<(), DbError>;

    /// Persist a verification error for the given server.
    async fn set_verification_error(&self, id: &str, error: String) -> Result<(), DbError>;
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
            verification_status: Set(Some("pending".to_string())),
            last_verification_error: Set(None),
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
            active.verification_status = Set(Some("success".to_string()));
            active.last_verification_error = Set(None);
            active.updated_at = Set(now);
            active.update(&self.db).await?;
            Ok(())
        } else {
            Err(DbError::NotFound(format!("Server '{}' not found", id)))
        }
    }

    async fn mark_verification_pending(&self, id: &str, clear_cache: bool) -> Result<(), DbError> {
        let now = chrono::Utc::now().timestamp_millis();
        let existing = mcp_server::Entity::find_by_id(id).one(&self.db).await?;

        if let Some(model) = existing {
            let mut active: mcp_server::ActiveModel = model.into();
            active.verification_status = Set(Some("pending".to_string()));
            active.last_verification_error = Set(None);
            if clear_cache {
                active.tool_count = Set(None);
                active.cached_tools = Set(None);
            }
            active.updated_at = Set(now);
            active.update(&self.db).await?;
            Ok(())
        } else {
            Err(DbError::NotFound(format!("Server '{}' not found", id)))
        }
    }

    async fn set_verification_error(&self, id: &str, error: String) -> Result<(), DbError> {
        let now = chrono::Utc::now().timestamp_millis();
        let existing = mcp_server::Entity::find_by_id(id).one(&self.db).await?;

        if let Some(model) = existing {
            let mut active: mcp_server::ActiveModel = model.into();
            active.verification_status = Set(Some("error".to_string()));
            active.last_verification_error = Set(Some(error));
            active.updated_at = Set(now);
            active.update(&self.db).await?;
            Ok(())
        } else {
            Err(DbError::NotFound(format!("Server '{}' not found", id)))
        }
    }
}

// Unit tests moved to tests/mcp_server_repository_tests.rs (integration tests)
// because cargo test --lib fails on Windows due to DLL issues with the test binary.
// Run: cargo test --tests mcp_server_repository
