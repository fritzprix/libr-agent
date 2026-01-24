use crate::entity::knowledge;
use crate::repositories::DbError;
use async_trait::async_trait;
use sea_orm::*;

#[async_trait]
pub trait KnowledgeRepository: Send + Sync {
    async fn create_knowledge(
        &self,
        assistant_id: String,
        title: String,
        content: String,
        source: Option<String>,
        tags: Option<String>,
    ) -> Result<knowledge::Model, DbError>;

    async fn get_knowledge(
        &self,
        id: i64,
        assistant_id: &str,
    ) -> Result<Option<knowledge::Model>, DbError>;

    async fn list_knowledge(&self, assistant_id: &str) -> Result<Vec<knowledge::Model>, DbError>;

    async fn delete_knowledge(&self, id: i64, assistant_id: &str) -> Result<(), DbError>;

    async fn delete_by_assistant(&self, assistant_id: &str) -> Result<u64, DbError>;

    async fn search_knowledge(
        &self,
        assistant_id: &str,
        query: &str,
        limit: Option<u64>,
    ) -> Result<Vec<knowledge::Model>, DbError>;

    async fn count_knowledge(&self, assistant_id: &str) -> Result<u64, DbError>;
}

#[derive(Debug)]
pub struct SqliteKnowledgeRepository {
    db: DatabaseConnection,
}

impl SqliteKnowledgeRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl KnowledgeRepository for SqliteKnowledgeRepository {
    async fn create_knowledge(
        &self,
        assistant_id: String,
        title: String,
        content: String,
        source: Option<String>,
        tags: Option<String>,
    ) -> Result<knowledge::Model, DbError> {
        let now = chrono::Utc::now().timestamp_millis();

        let active_model = knowledge::ActiveModel {
            id: NotSet,
            assistant_id: Set(assistant_id),
            title: Set(title),
            content: Set(content),
            source: Set(source),
            tags: Set(tags),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let insert_result = knowledge::Entity::insert(active_model)
            .exec(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        // Fetch the inserted record
        knowledge::Entity::find_by_id(insert_result.last_insert_id)
            .one(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?
            .ok_or_else(|| DbError::NotFound("Knowledge entry not found after insert".to_string()))
    }

    async fn get_knowledge(
        &self,
        id: i64,
        assistant_id: &str,
    ) -> Result<Option<knowledge::Model>, DbError> {
        knowledge::Entity::find()
            .filter(knowledge::Column::Id.eq(id))
            .filter(knowledge::Column::AssistantId.eq(assistant_id))
            .one(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn list_knowledge(&self, assistant_id: &str) -> Result<Vec<knowledge::Model>, DbError> {
        knowledge::Entity::find()
            .filter(knowledge::Column::AssistantId.eq(assistant_id))
            .order_by_desc(knowledge::Column::UpdatedAt)
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn delete_knowledge(&self, id: i64, assistant_id: &str) -> Result<(), DbError> {
        let delete_result = knowledge::Entity::delete_many()
            .filter(knowledge::Column::Id.eq(id))
            .filter(knowledge::Column::AssistantId.eq(assistant_id))
            .exec(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        if delete_result.rows_affected == 0 {
            return Err(DbError::NotFound(format!(
                "Knowledge entry {} not found for assistant {}",
                id, assistant_id
            )));
        }

        Ok(())
    }

    async fn delete_by_assistant(&self, assistant_id: &str) -> Result<u64, DbError> {
        let delete_result = knowledge::Entity::delete_many()
            .filter(knowledge::Column::AssistantId.eq(assistant_id))
            .exec(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        Ok(delete_result.rows_affected)
    }

    async fn search_knowledge(
        &self,
        assistant_id: &str,
        query: &str,
        limit: Option<u64>,
    ) -> Result<Vec<knowledge::Model>, DbError> {
        let search_pattern = format!("%{}%", query);

        let mut select = knowledge::Entity::find()
            .filter(knowledge::Column::AssistantId.eq(assistant_id))
            .filter(
                Condition::any()
                    .add(knowledge::Column::Title.like(&search_pattern))
                    .add(knowledge::Column::Content.like(&search_pattern))
                    .add(knowledge::Column::Tags.like(&search_pattern)),
            )
            .order_by_desc(knowledge::Column::UpdatedAt);

        if let Some(limit_val) = limit {
            select = select.limit(limit_val);
        }

        select
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn count_knowledge(&self, assistant_id: &str) -> Result<u64, DbError> {
        knowledge::Entity::find()
            .filter(knowledge::Column::AssistantId.eq(assistant_id))
            .count(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, Schema};

    async fn create_test_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to test database");

        let schema = Schema::new(db.get_database_backend());
        let stmt = schema.create_table_from_entity(knowledge::Entity);

        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create knowledge table");

        db
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let db = create_test_db().await;
        let repo = SqliteKnowledgeRepository::new(db);

        let created = repo
            .create_knowledge(
                "test-assistant".to_string(),
                "Test Title".to_string(),
                "Test content".to_string(),
                Some("test source".to_string()),
                Some("[\"tag1\", \"tag2\"]".to_string()),
            )
            .await
            .unwrap();

        assert_eq!(created.title, "Test Title");
        assert_eq!(created.content, "Test content");

        let retrieved = repo
            .get_knowledge(created.id, "test-assistant")
            .await
            .unwrap()
            .expect("Knowledge should exist");

        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.title, "Test Title");
    }

    #[tokio::test]
    async fn test_list() {
        let db = create_test_db().await;
        let repo = SqliteKnowledgeRepository::new(db);

        repo.create_knowledge(
            "test-assistant".to_string(),
            "Title 1".to_string(),
            "Content 1".to_string(),
            None,
            None,
        )
        .await
        .unwrap();

        repo.create_knowledge(
            "test-assistant".to_string(),
            "Title 2".to_string(),
            "Content 2".to_string(),
            None,
            None,
        )
        .await
        .unwrap();

        let list = repo.list_knowledge("test-assistant").await.unwrap();
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_delete() {
        let db = create_test_db().await;
        let repo = SqliteKnowledgeRepository::new(db);

        let created = repo
            .create_knowledge(
                "test-assistant".to_string(),
                "To Delete".to_string(),
                "Content".to_string(),
                None,
                None,
            )
            .await
            .unwrap();

        repo.delete_knowledge(created.id, "test-assistant")
            .await
            .unwrap();

        let retrieved = repo
            .get_knowledge(created.id, "test-assistant")
            .await
            .unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_search() {
        let db = create_test_db().await;
        let repo = SqliteKnowledgeRepository::new(db);

        repo.create_knowledge(
            "test-assistant".to_string(),
            "Rust Programming".to_string(),
            "Content about Rust".to_string(),
            None,
            None,
        )
        .await
        .unwrap();

        repo.create_knowledge(
            "test-assistant".to_string(),
            "Python Guide".to_string(),
            "Content about Python".to_string(),
            None,
            None,
        )
        .await
        .unwrap();

        let results = repo
            .search_knowledge("test-assistant", "Rust", None)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Programming");
    }
}
