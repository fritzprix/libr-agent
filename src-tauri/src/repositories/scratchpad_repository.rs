use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use std::future::Future;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::entity::scratchpad;
use crate::repositories::DbError;

#[async_trait]
pub trait ScratchpadRepository: Send + Sync {
    async fn add_scratchpad(
        &self,
        session_id: &str,
        title: Option<String>,
        content: &str,
        source: Option<String>,
        tags: Option<String>,
    ) -> Result<i32, DbError>;

    async fn update_scratchpad(
        &self,
        session_id: &str,
        title: &str,
        new_title: Option<String>,
        content: &str,
    ) -> Result<bool, DbError>;

    async fn update_scratchpad_by_id(
        &self,
        session_id: &str,
        id: i64,
        content: &str,
        new_title: Option<String>,
    ) -> Result<bool, DbError>;

    async fn list_scratchpad(&self, session_id: &str) -> Result<Vec<scratchpad::Model>, DbError>;

    async fn get_scratchpad_by_ids(&self, ids: Vec<i64>)
        -> Result<Vec<scratchpad::Model>, DbError>;

    async fn delete_scratchpad_item(&self, session_id: &str, id: i64) -> Result<bool, DbError>;

    async fn check_scratchpad_limit(&self, session_id: &str) -> Result<u64, DbError>;

    async fn check_scratchpad_duplicate(
        &self,
        session_id: &str,
        title: &str,
    ) -> Result<bool, DbError>;

    async fn clear_session(&self, session_id: &str) -> Result<u64, DbError>;
}

#[derive(Clone)]
pub struct SqliteScratchpadRepository {
    db: DatabaseConnection,
    write_lock: Arc<Mutex<()>>,
}

impl SqliteScratchpadRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    async fn run_serialized_write<F, Fut, T>(
        &self,
        op_name: &'static str,
        f: F,
    ) -> Result<T, DbError>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<T, DbError>> + Send,
    {
        let _guard = self.write_lock.lock().await;
        let start = std::time::Instant::now();
        let result = f().await;
        let duration = start.elapsed();
        log::debug!(
            target: "scratchpad_repository",
            "Serialized write operation '{}' completed in {:?}",
            op_name,
            duration
        );
        result
    }
}

#[async_trait]
impl ScratchpadRepository for SqliteScratchpadRepository {
    async fn add_scratchpad(
        &self,
        session_id: &str,
        title: Option<String>,
        content: &str,
        source: Option<String>,
        tags: Option<String>,
    ) -> Result<i32, DbError> {
        self.run_serialized_write("add_scratchpad", || {
            let title = title.clone();
            let source = source.clone();
            let tags = tags.clone();
            async move {
                let now = chrono::Utc::now().timestamp_millis();

                let new_item = scratchpad::ActiveModel {
                    session_id: Set(session_id.to_string()),
                    content: Set(content.to_string()),
                    title: Set(title),
                    source: Set(source),
                    tags: Set(tags),
                    created_at: Set(now),
                    updated_at: Set(now),
                    ..Default::default()
                };

                let res = new_item
                    .insert(&self.db)
                    .await
                    .map_err(DbError::SeaOrmQueryFailed)?;

                Ok(res.id)
            }
        })
        .await
    }

    async fn update_scratchpad(
        &self,
        session_id: &str,
        title: &str,
        new_title: Option<String>,
        content: &str,
    ) -> Result<bool, DbError> {
        self.run_serialized_write("update_scratchpad", || {
            let new_title = new_title.clone();
            async move {
                let now = chrono::Utc::now().timestamp_millis();

                let item = scratchpad::Entity::find()
                    .filter(scratchpad::Column::SessionId.eq(session_id))
                    .filter(scratchpad::Column::Title.eq(title))
                    .one(&self.db)
                    .await
                    .map_err(DbError::SeaOrmQueryFailed)?;

                if let Some(i) = item {
                    let mut active: scratchpad::ActiveModel = i.into();
                    active.content = Set(content.to_string());
                    if let Some(nt) = new_title {
                        active.title = Set(Some(nt));
                    }
                    active.updated_at = Set(now);

                    active
                        .update(&self.db)
                        .await
                        .map_err(DbError::SeaOrmQueryFailed)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        })
        .await
    }

    async fn update_scratchpad_by_id(
        &self,
        session_id: &str,
        id: i64,
        content: &str,
        new_title: Option<String>,
    ) -> Result<bool, DbError> {
        self.run_serialized_write("update_scratchpad_by_id", || {
            let new_title = new_title.clone();
            async move {
                let now = chrono::Utc::now().timestamp_millis();

                let item = scratchpad::Entity::find()
                    .filter(scratchpad::Column::SessionId.eq(session_id))
                    .filter(scratchpad::Column::Id.eq(id))
                    .one(&self.db)
                    .await
                    .map_err(DbError::SeaOrmQueryFailed)?;

                if let Some(i) = item {
                    let mut active: scratchpad::ActiveModel = i.into();
                    active.content = Set(content.to_string());
                    if let Some(nt) = new_title {
                        active.title = Set(Some(nt));
                    }
                    active.updated_at = Set(now);

                    active
                        .update(&self.db)
                        .await
                        .map_err(DbError::SeaOrmQueryFailed)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        })
        .await
    }

    async fn list_scratchpad(&self, session_id: &str) -> Result<Vec<scratchpad::Model>, DbError> {
        scratchpad::Entity::find()
            .filter(scratchpad::Column::SessionId.eq(session_id))
            .order_by_desc(scratchpad::Column::CreatedAt)
            .order_by_desc(scratchpad::Column::Id)
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn get_scratchpad_by_ids(
        &self,
        ids: Vec<i64>,
    ) -> Result<Vec<scratchpad::Model>, DbError> {
        scratchpad::Entity::find()
            .filter(scratchpad::Column::Id.is_in(ids))
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn delete_scratchpad_item(&self, session_id: &str, id: i64) -> Result<bool, DbError> {
        self.run_serialized_write("delete_scratchpad_item", || async move {
            let res = scratchpad::Entity::delete_many()
                .filter(scratchpad::Column::Id.eq(id))
                .filter(scratchpad::Column::SessionId.eq(session_id))
                .exec(&self.db)
                .await
                .map_err(DbError::SeaOrmQueryFailed)?;

            Ok(res.rows_affected > 0)
        })
        .await
    }

    async fn check_scratchpad_limit(&self, session_id: &str) -> Result<u64, DbError> {
        scratchpad::Entity::find()
            .filter(scratchpad::Column::SessionId.eq(session_id))
            .count(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn check_scratchpad_duplicate(
        &self,
        session_id: &str,
        title: &str,
    ) -> Result<bool, DbError> {
        let count = scratchpad::Entity::find()
            .filter(scratchpad::Column::SessionId.eq(session_id))
            .filter(scratchpad::Column::Title.eq(title))
            .count(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        Ok(count > 0)
    }

    async fn clear_session(&self, session_id: &str) -> Result<u64, DbError> {
        self.run_serialized_write("clear_session", || async move {
            let res = scratchpad::Entity::delete_many()
                .filter(scratchpad::Column::SessionId.eq(session_id))
                .exec(&self.db)
                .await
                .map_err(DbError::SeaOrmQueryFailed)?;
            Ok(res.rows_affected)
        })
        .await
    }
}
