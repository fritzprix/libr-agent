//! Playbook repository for database operations on the playbooks table
//!
//! This repository provides a clean abstraction layer for all playbook-related
//! database operations, following the repository pattern for separation of concerns.

use super::error::DbError;
use crate::entity::playbook::{self, Entity as PlaybookEntity};
use crate::utils::pagination::{Page, PaginationParams};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, Order,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};

/// Playbook repository trait for abstraction and testability
#[async_trait::async_trait]
pub trait PlaybookRepository: Send + Sync {
    /// Create a new playbook
    async fn create_playbook(
        &self,
        id: String,
        assistant_id: String,
        goal: String,
        workflow: String,
    ) -> Result<playbook::Model, DbError>;

    /// Get a playbook by ID and assistant ID
    async fn get_playbook(
        &self,
        id: &str,
        assistant_id: &str,
    ) -> Result<Option<playbook::Model>, DbError>;

    /// List all playbooks (optionally filtered by assistant) with pagination
    async fn list_playbooks(
        &self,
        assistant_id: Option<&str>,
        pagination: PaginationParams,
    ) -> Result<Page<playbook::Model>, DbError>;

    /// Update a playbook
    async fn update_playbook(
        &self,
        id: &str,
        assistant_id: &str,
        goal: Option<String>,
        workflow: Option<String>,
        is_bookmarked: Option<bool>,
    ) -> Result<playbook::Model, DbError>;

    /// Delete a playbook
    async fn delete_playbook(&self, id: &str, assistant_id: &str) -> Result<(), DbError>;

    /// Delete all playbooks for an assistant
    async fn delete_by_assistant(&self, assistant_id: &str) -> Result<u64, DbError>;

    /// Search playbooks by goal text
    async fn search_playbooks(
        &self,
        assistant_id: &str,
        query: &str,
    ) -> Result<Vec<playbook::Model>, DbError>;

    /// Get bookmarked playbooks
    async fn get_bookmarked_playbooks(
        &self,
        assistant_id: &str,
    ) -> Result<Vec<playbook::Model>, DbError>;

    /// Count playbooks for an assistant
    async fn count_playbooks(&self, assistant_id: &str) -> Result<u64, DbError>;
}

/// SQLite implementation of PlaybookRepository using SeaORM
#[derive(Debug, Clone)]
pub struct SqlitePlaybookRepository {
    db: DatabaseConnection,
}

impl SqlitePlaybookRepository {
    /// Create a new SQLite playbook repository with the given database connection
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl PlaybookRepository for SqlitePlaybookRepository {
    async fn create_playbook(
        &self,
        id: String,
        assistant_id: String,
        goal: String,
        workflow: String,
    ) -> Result<playbook::Model, DbError> {
        let now = chrono::Utc::now().timestamp_millis();

        let active_model = playbook::ActiveModel {
            id: Set(id),
            assistant_id: Set(assistant_id),
            goal: Set(goal),
            initial_command: Set(None),
            workflow: Set(workflow),
            success_criteria: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            is_bookmarked: Set(false),
        };

        active_model
            .insert(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn get_playbook(
        &self,
        id: &str,
        assistant_id: &str,
    ) -> Result<Option<playbook::Model>, DbError> {
        PlaybookEntity::find()
            .filter(playbook::Column::Id.eq(id))
            .filter(playbook::Column::AssistantId.eq(assistant_id))
            .one(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn list_playbooks(
        &self,
        assistant_id: Option<&str>,
        pagination: PaginationParams,
    ) -> Result<Page<playbook::Model>, DbError> {
        let page_size = pagination.page_size;
        let offset = pagination.page.saturating_sub(1).saturating_mul(page_size);

        // Apply filters
        let mut query = PlaybookEntity::find();

        if let Some(aid) = assistant_id {
            query = query.filter(playbook::Column::AssistantId.eq(aid));
        }

        // Get total count
        let total = query
            .clone()
            .count(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        // Get paginated items
        let items = query
            .order_by(playbook::Column::UpdatedAt, Order::Desc)
            .limit(page_size)
            .offset(offset)
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        Ok(Page::new(items, pagination.page, page_size, total))
    }

    async fn update_playbook(
        &self,
        id: &str,
        assistant_id: &str,
        goal: Option<String>,
        workflow: Option<String>,
        is_bookmarked: Option<bool>,
    ) -> Result<playbook::Model, DbError> {
        // Get existing playbook
        let playbook = self.get_playbook(id, assistant_id).await?.ok_or_else(|| {
            DbError::NotFound(format!(
                "Playbook {} not found for assistant {}",
                id, assistant_id
            ))
        })?;

        // Update only provided fields
        let mut active_model = playbook.into_active_model();
        if let Some(g) = goal {
            active_model.goal = Set(g);
        }
        if let Some(w) = workflow {
            active_model.workflow = Set(w);
        }
        if let Some(b) = is_bookmarked {
            active_model.is_bookmarked = Set(b);
        }
        active_model.updated_at = Set(chrono::Utc::now().timestamp_millis());

        active_model
            .update(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn delete_playbook(&self, id: &str, assistant_id: &str) -> Result<(), DbError> {
        let result = PlaybookEntity::delete_many()
            .filter(playbook::Column::Id.eq(id))
            .filter(playbook::Column::AssistantId.eq(assistant_id))
            .exec(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        if result.rows_affected == 0 {
            return Err(DbError::NotFound(format!(
                "Playbook {} not found for assistant {}",
                id, assistant_id
            )));
        }

        Ok(())
    }

    async fn delete_by_assistant(&self, assistant_id: &str) -> Result<u64, DbError> {
        let result = PlaybookEntity::delete_many()
            .filter(playbook::Column::AssistantId.eq(assistant_id))
            .exec(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        Ok(result.rows_affected)
    }

    async fn search_playbooks(
        &self,
        assistant_id: &str,
        query: &str,
    ) -> Result<Vec<playbook::Model>, DbError> {
        let query_pattern = format!("%{}%", query.to_lowercase());

        PlaybookEntity::find()
            .filter(playbook::Column::AssistantId.eq(assistant_id))
            .filter(Expr::cust_with_values(
                "LOWER(goal) LIKE ?",
                vec![sea_orm::Value::from(query_pattern)],
            ))
            .order_by(playbook::Column::UpdatedAt, Order::Desc)
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn get_bookmarked_playbooks(
        &self,
        assistant_id: &str,
    ) -> Result<Vec<playbook::Model>, DbError> {
        PlaybookEntity::find()
            .filter(playbook::Column::AssistantId.eq(assistant_id))
            .filter(playbook::Column::IsBookmarked.eq(true))
            .order_by(playbook::Column::UpdatedAt, Order::Desc)
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn count_playbooks(&self, assistant_id: &str) -> Result<u64, DbError> {
        PlaybookEntity::find()
            .filter(playbook::Column::AssistantId.eq(assistant_id))
            .count(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::ConnectionTrait;

    async fn setup_test_db() -> SqlitePlaybookRepository {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory database");

        let schema = sea_orm::Schema::new(db.get_database_backend());
        let stmt = schema.create_table_from_entity(PlaybookEntity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create playbooks table");

        SqlitePlaybookRepository::new(db)
    }

    #[tokio::test]
    async fn test_create_and_get_playbook() {
        let repo = setup_test_db().await;

        let playbook = repo
            .create_playbook(
                "pb-1".to_string(),
                "asst-1".to_string(),
                "Test Goal".to_string(),
                r#"{"steps": []}"#.to_string(),
            )
            .await
            .expect("Failed to create playbook");

        assert_eq!(playbook.id, "pb-1");
        assert_eq!(playbook.goal, "Test Goal");

        let fetched = repo
            .get_playbook("pb-1", "asst-1")
            .await
            .expect("Failed to get playbook");
        assert!(fetched.is_some());
    }

    #[tokio::test]
    async fn test_list_with_pagination() {
        let repo = setup_test_db().await;
        let assistant_id = "asst-1";

        for i in 0..15 {
            repo.create_playbook(
                format!("pb-{}", i),
                assistant_id.to_string(),
                format!("Goal {}", i),
                "{}".to_string(),
            )
            .await
            .expect("Failed to create");
        }

        let page1 = repo
            .list_playbooks(
                Some(assistant_id),
                PaginationParams {
                    page: 1,
                    page_size: 10,
                },
            )
            .await
            .expect("Failed to list page 1");

        assert_eq!(page1.items.len(), 10);
        assert_eq!(page1.total_items, 15);

        let page2 = repo
            .list_playbooks(
                Some(assistant_id),
                PaginationParams {
                    page: 2,
                    page_size: 10,
                },
            )
            .await
            .expect("Failed to list page 2");

        assert_eq!(page2.items.len(), 5);
    }
}
