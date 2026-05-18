//! Scheduled task repository for database operations.
//!
//! The `message` field supports `@mention` syntax (e.g. `@playbook:goal`,
//! `@skill:name`) which is expanded at execution time by `resolve_message_references`.

use crate::entity::scheduled_task::{self, Entity as ScheduledTaskEntity};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel, Order,
    QueryFilter, QueryOrder, Set,
};

/// Parameters for creating a scheduled task
pub struct CreateScheduledTaskParams {
    pub id: String,
    pub name: String,
    pub cron_expression: String,
    pub schedule_timezone: String,
    pub assistant_id: String,
    pub group_id: Option<String>,
    pub group_name: Option<String>,
    pub message: String,
    pub yolo_mode: bool,
    pub created_by_session_id: Option<String>,
    pub workspace_override: Option<String>,
    pub next_run_at: Option<i64>,
}

/// Parameters for updating a scheduled task
pub struct UpdateScheduledTaskParams {
    pub name: Option<String>,
    pub cron_expression: Option<String>,
    pub schedule_timezone: Option<String>,
    pub assistant_id: Option<String>,
    pub group_id: Option<Option<String>>,
    pub group_name: Option<Option<String>>,
    pub message: Option<String>,
    pub yolo_mode: Option<bool>,
    pub workspace_override: Option<Option<String>>,
    pub enabled: Option<bool>,
    pub next_run_at: Option<Option<i64>>,
}

/// Scheduled task repository trait for abstraction and testability
#[async_trait::async_trait]
pub trait ScheduledTaskRepository: Send + Sync {
    /// Create a new scheduled task
    async fn create_scheduled_task(
        &self,
        params: CreateScheduledTaskParams,
    ) -> Result<scheduled_task::Model, DbErr>;

    /// Get a scheduled task by ID
    async fn get_scheduled_task(&self, id: &str) -> Result<Option<scheduled_task::Model>, DbErr>;

    /// List all scheduled tasks (optionally filtered by assistant)
    async fn list_scheduled_tasks(
        &self,
        assistant_id: Option<&str>,
    ) -> Result<Vec<scheduled_task::Model>, DbErr>;

    /// List enabled tasks whose next_run_at is <= the given epoch ms (due tasks)
    async fn list_due_tasks(&self, now_ms: i64) -> Result<Vec<scheduled_task::Model>, DbErr>;

    /// Update mutable fields of a scheduled task
    async fn update_scheduled_task(
        &self,
        id: &str,
        params: UpdateScheduledTaskParams,
    ) -> Result<scheduled_task::Model, DbErr>;

    /// Record that a task has just run: update session_id, last_run_at, next_run_at
    async fn record_run(
        &self,
        id: &str,
        session_id: Option<String>,
        last_run_at: i64,
        next_run_at: Option<i64>,
    ) -> Result<(), DbErr>;

    /// Delete a scheduled task
    async fn delete_scheduled_task(&self, id: &str) -> Result<(), DbErr>;
}

// ─── SQLite implementation ───────────────────────────────────────────────────

#[derive(Debug)]
pub struct SqliteScheduledTaskRepository {
    db: DatabaseConnection,
}

impl SqliteScheduledTaskRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn fetch_task(&self, id: &str) -> Result<scheduled_task::ActiveModel, DbErr> {
        ScheduledTaskEntity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound(format!("ScheduledTask {id} not found")))
            .map(|t| t.into_active_model())
    }
}

#[async_trait::async_trait]
impl ScheduledTaskRepository for SqliteScheduledTaskRepository {
    async fn create_scheduled_task(
        &self,
        params: CreateScheduledTaskParams,
    ) -> Result<scheduled_task::Model, DbErr> {
        let now = chrono::Utc::now().timestamp_millis();
        let model = scheduled_task::ActiveModel {
            id: Set(params.id),
            name: Set(params.name),
            cron_expression: Set(params.cron_expression),
            schedule_timezone: Set(params.schedule_timezone),
            assistant_id: Set(params.assistant_id),
            group_id: Set(params.group_id),
            group_name: Set(params.group_name),
            message: Set(params.message),
            yolo_mode: Set(params.yolo_mode),
            created_by_session_id: Set(params.created_by_session_id),
            session_id: Set(None),
            workspace_override: Set(params.workspace_override),
            enabled: Set(true),
            last_run_at: Set(None),
            next_run_at: Set(params.next_run_at),
            created_at: Set(now),
            updated_at: Set(now),
        };
        model.insert(&self.db).await
    }

    async fn get_scheduled_task(&self, id: &str) -> Result<Option<scheduled_task::Model>, DbErr> {
        ScheduledTaskEntity::find_by_id(id).one(&self.db).await
    }

    async fn list_scheduled_tasks(
        &self,
        assistant_id: Option<&str>,
    ) -> Result<Vec<scheduled_task::Model>, DbErr> {
        let query =
            ScheduledTaskEntity::find().order_by(scheduled_task::Column::CreatedAt, Order::Asc);

        if let Some(aid) = assistant_id {
            query
                .filter(scheduled_task::Column::AssistantId.eq(aid))
                .all(&self.db)
                .await
        } else {
            query.all(&self.db).await
        }
    }

    async fn list_due_tasks(&self, now_ms: i64) -> Result<Vec<scheduled_task::Model>, DbErr> {
        ScheduledTaskEntity::find()
            .filter(scheduled_task::Column::Enabled.eq(true))
            .filter(scheduled_task::Column::NextRunAt.lte(now_ms))
            .order_by(scheduled_task::Column::NextRunAt, Order::Asc)
            .order_by(scheduled_task::Column::CreatedAt, Order::Asc)
            .order_by(scheduled_task::Column::Id, Order::Asc)
            .all(&self.db)
            .await
    }

    async fn update_scheduled_task(
        &self,
        id: &str,
        params: UpdateScheduledTaskParams,
    ) -> Result<scheduled_task::Model, DbErr> {
        let mut active = self.fetch_task(id).await?;
        let now = chrono::Utc::now().timestamp_millis();

        if let Some(v) = params.name {
            active.name = Set(v);
        }
        if let Some(v) = params.cron_expression {
            active.cron_expression = Set(v);
        }
        if let Some(v) = params.schedule_timezone {
            active.schedule_timezone = Set(v);
        }
        if let Some(v) = params.assistant_id {
            active.assistant_id = Set(v);
        }
        if let Some(v) = params.group_id {
            active.group_id = Set(v);
        }
        if let Some(v) = params.group_name {
            active.group_name = Set(v);
        }
        if let Some(v) = params.message {
            active.message = Set(v);
        }
        if let Some(v) = params.yolo_mode {
            active.yolo_mode = Set(v);
        }
        if let Some(v) = params.workspace_override {
            active.workspace_override = Set(v);
        }
        if let Some(v) = params.enabled {
            active.enabled = Set(v);
        }
        if let Some(v) = params.next_run_at {
            active.next_run_at = Set(v);
        }
        active.updated_at = Set(now);

        active.update(&self.db).await
    }

    async fn record_run(
        &self,
        id: &str,
        session_id: Option<String>,
        last_run_at: i64,
        next_run_at: Option<i64>,
    ) -> Result<(), DbErr> {
        let mut active = self.fetch_task(id).await?;
        let now = chrono::Utc::now().timestamp_millis();

        if session_id.is_some() {
            active.session_id = Set(session_id);
        }
        active.last_run_at = Set(Some(last_run_at));
        active.next_run_at = Set(next_run_at);
        active.updated_at = Set(now);

        active.update(&self.db).await?;
        Ok(())
    }

    async fn delete_scheduled_task(&self, id: &str) -> Result<(), DbErr> {
        ScheduledTaskEntity::delete_by_id(id).exec(&self.db).await?;
        Ok(())
    }
}
