use crate::entity::{planning_goal, planning_scratchpad, planning_todo};
use crate::repositories::DbError;
use async_trait::async_trait;
use sea_orm::*;

#[async_trait]
pub trait PlanningRepository: Send + Sync {
    // --- Goals ---
    async fn create_goal(&self, session_id: &str, goal_text: &str) -> Result<i64, DbError>;
    async fn get_active_goal(
        &self,
        session_id: &str,
    ) -> Result<Option<planning_goal::Model>, DbError>;
    async fn update_goal(&self, session_id: &str, goal_text: &str) -> Result<bool, DbError>;
    async fn clear_goal(&self, session_id: &str) -> Result<bool, DbError>;

    // --- Session ---
    async fn clear_session(&self, session_id: &str) -> Result<(), DbError>;

    // --- Todos ---
    async fn add_todo(
        &self,
        session_id: &str,
        content: &str,
        description: Option<String>,
        priority: &str,
        parent_id: Option<i64>,
    ) -> Result<i64, DbError>;

    async fn check_todo(
        &self,
        id: i64,
        checked: bool,
        summary: Option<String>,
    ) -> Result<bool, DbError>;

    async fn get_todo(&self, id: i64) -> Result<Option<planning_todo::Model>, DbError>;

    async fn list_todos(
        &self,
        session_id: &str,
        include_checked: bool,
    ) -> Result<Vec<planning_todo::Model>, DbError>;

    async fn delete_todos(&self, session_id: &str, ids: Vec<i64>) -> Result<u64, DbError>;

    async fn check_todo_duplicate(&self, session_id: &str, content: &str) -> Result<bool, DbError>;

    async fn get_parent_todo(&self, id: i64) -> Result<Option<planning_todo::Model>, DbError>;
    async fn get_child_todos(&self, parent_id: i64) -> Result<Vec<planning_todo::Model>, DbError>;

    // --- Scratchpad ---
    async fn add_scratchpad(
        &self,
        session_id: &str,
        title: Option<String>,
        content: &str,
        source: Option<String>,
        tags: Option<String>,
    ) -> Result<i64, DbError>;

    async fn update_scratchpad(
        &self,
        session_id: &str,
        title: &str,
        new_title: Option<String>,
        content: &str,
    ) -> Result<bool, DbError>;

    async fn list_scratchpad(
        &self,
        session_id: &str,
    ) -> Result<Vec<planning_scratchpad::Model>, DbError>;

    async fn get_scratchpad_by_ids(
        &self,
        ids: Vec<i64>,
    ) -> Result<Vec<planning_scratchpad::Model>, DbError>;

    async fn delete_scratchpad_item(&self, session_id: &str, id: i64) -> Result<bool, DbError>;

    async fn check_scratchpad_limit(&self, session_id: &str) -> Result<u64, DbError>;
    async fn check_scratchpad_duplicate(
        &self,
        session_id: &str,
        title: &str,
    ) -> Result<bool, DbError>;

    // --- Context Summary ---
    async fn get_planning_summary(&self, session_id: &str) -> Result<String, DbError>;
}

#[derive(Debug)]
pub struct SqlitePlanningRepository {
    db: DatabaseConnection,
}

impl SqlitePlanningRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl PlanningRepository for SqlitePlanningRepository {
    // --- Goals ---

    async fn create_goal(&self, session_id: &str, goal_text: &str) -> Result<i64, DbError> {
        let now = chrono::Utc::now().timestamp_millis();

        let txn = self.db.begin().await.map_err(DbError::SeaOrmQueryFailed)?;

        // Archive existing active goals
        planning_goal::Entity::update_many()
            .col_expr(
                planning_goal::Column::Status,
                sea_orm::sea_query::Expr::value("archived"),
            )
            .filter(planning_goal::Column::SessionId.eq(session_id))
            .filter(planning_goal::Column::Status.eq("active"))
            .exec(&txn)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        let new_goal = planning_goal::ActiveModel {
            session_id: Set(session_id.to_string()),
            goal_text: Set(goal_text.to_string()),
            status: Set("active".to_string()),
            created_at: Set(now),
            ..Default::default()
        };

        let res = new_goal
            .insert(&txn)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        txn.commit().await.map_err(DbError::SeaOrmQueryFailed)?;

        Ok(res.id)
    }

    async fn get_active_goal(
        &self,
        session_id: &str,
    ) -> Result<Option<planning_goal::Model>, DbError> {
        planning_goal::Entity::find()
            .filter(planning_goal::Column::SessionId.eq(session_id))
            .filter(planning_goal::Column::Status.eq("active"))
            .one(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn update_goal(&self, session_id: &str, goal_text: &str) -> Result<bool, DbError> {
        let res = planning_goal::Entity::update_many()
            .col_expr(
                planning_goal::Column::GoalText,
                sea_orm::sea_query::Expr::value(goal_text),
            )
            .filter(planning_goal::Column::SessionId.eq(session_id))
            .filter(planning_goal::Column::Status.eq("active"))
            .exec(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        Ok(res.rows_affected > 0)
    }

    async fn clear_goal(&self, session_id: &str) -> Result<bool, DbError> {
        let res = planning_goal::Entity::update_many()
            .col_expr(
                planning_goal::Column::Status,
                sea_orm::sea_query::Expr::value("cleared"),
            )
            .filter(planning_goal::Column::SessionId.eq(session_id))
            .filter(planning_goal::Column::Status.eq("active"))
            .exec(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        Ok(res.rows_affected > 0)
    }

    // --- Session ---

    async fn clear_session(&self, session_id: &str) -> Result<(), DbError> {
        let txn = self.db.begin().await.map_err(DbError::SeaOrmQueryFailed)?;

        planning_goal::Entity::delete_many()
            .filter(planning_goal::Column::SessionId.eq(session_id))
            .exec(&txn)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        planning_todo::Entity::delete_many()
            .filter(planning_todo::Column::SessionId.eq(session_id))
            .exec(&txn)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        planning_scratchpad::Entity::delete_many()
            .filter(planning_scratchpad::Column::SessionId.eq(session_id))
            .exec(&txn)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        txn.commit().await.map_err(DbError::SeaOrmQueryFailed)?;
        Ok(())
    }

    // --- Todos ---

    async fn add_todo(
        &self,
        session_id: &str,
        content: &str,
        description: Option<String>,
        priority: &str,
        parent_id: Option<i64>,
    ) -> Result<i64, DbError> {
        let now = chrono::Utc::now().timestamp_millis();

        let new_todo = planning_todo::ActiveModel {
            session_id: Set(session_id.to_string()),
            content: Set(content.to_string()),
            description: Set(description),
            priority: Set(priority.to_string()),
            parent_id: Set(parent_id),
            status: Set("pending".to_string()),
            is_checked: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        let res = new_todo
            .insert(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        Ok(res.id)
    }

    async fn check_todo(
        &self,
        id: i64,
        checked: bool,
        summary: Option<String>,
    ) -> Result<bool, DbError> {
        let now = chrono::Utc::now().timestamp_millis();
        let status = if checked { "completed" } else { "pending" };

        let txn = self.db.begin().await.map_err(DbError::SeaOrmQueryFailed)?;

        let todo = planning_todo::Entity::find_by_id(id)
            .one(&txn)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        if let Some(t) = todo {
            let mut active: planning_todo::ActiveModel = t.into();
            active.is_checked = Set(checked);
            active.status = Set(status.to_string());
            active.updated_at = Set(now);

            if let Some(s) = summary {
                let current_desc = match &active.description {
                    Set(Some(d)) => d.as_str(),
                    _ => "",
                };
                let new_desc = if current_desc.is_empty() {
                    s
                } else {
                    format!("{} - {}", current_desc, s)
                };
                active.description = Set(Some(new_desc));
            }

            active
                .update(&txn)
                .await
                .map_err(DbError::SeaOrmQueryFailed)?;

            txn.commit().await.map_err(DbError::SeaOrmQueryFailed)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn get_todo(&self, id: i64) -> Result<Option<planning_todo::Model>, DbError> {
        planning_todo::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn list_todos(
        &self,
        session_id: &str,
        include_checked: bool,
    ) -> Result<Vec<planning_todo::Model>, DbError> {
        let mut query = planning_todo::Entity::find()
            .filter(planning_todo::Column::SessionId.eq(session_id))
            .order_by_asc(planning_todo::Column::CreatedAt);

        if !include_checked {
            query = query.filter(planning_todo::Column::IsChecked.eq(false));
        }

        query
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn delete_todos(&self, session_id: &str, ids: Vec<i64>) -> Result<u64, DbError> {
        let res = planning_todo::Entity::delete_many()
            .filter(planning_todo::Column::SessionId.eq(session_id))
            .filter(planning_todo::Column::Id.is_in(ids))
            .exec(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        Ok(res.rows_affected)
    }

    async fn check_todo_duplicate(&self, session_id: &str, content: &str) -> Result<bool, DbError> {
        let count = planning_todo::Entity::find()
            .filter(planning_todo::Column::SessionId.eq(session_id))
            .filter(
                sea_orm::sea_query::Expr::expr(sea_orm::sea_query::Func::lower(
                    sea_orm::sea_query::Expr::col(planning_todo::Column::Content),
                ))
                .eq(content.to_lowercase()),
            )
            .count(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        Ok(count > 0)
    }

    async fn get_parent_todo(&self, id: i64) -> Result<Option<planning_todo::Model>, DbError> {
        // This is a bit tricky: 'id' here is presumably the child's ID, need to find its parent?
        // Or is 'id' the parent ID? Based on name, it suggests fetching THE parent of a todo.
        // Let's implement getting the parent of a given todo ID.
        let child = planning_todo::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        if let Some(c) = child {
            if let Some(pid) = c.parent_id {
                planning_todo::Entity::find_by_id(pid)
                    .one(&self.db)
                    .await
                    .map_err(DbError::SeaOrmQueryFailed)
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    async fn get_child_todos(&self, parent_id: i64) -> Result<Vec<planning_todo::Model>, DbError> {
        planning_todo::Entity::find()
            .filter(planning_todo::Column::ParentId.eq(parent_id))
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    // --- Scratchpad ---

    async fn add_scratchpad(
        &self,
        session_id: &str,
        title: Option<String>,
        content: &str,
        source: Option<String>,
        tags: Option<String>,
    ) -> Result<i64, DbError> {
        let now = chrono::Utc::now().timestamp_millis();

        let new_item = planning_scratchpad::ActiveModel {
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

    async fn update_scratchpad(
        &self,
        session_id: &str,
        title: &str,
        new_title: Option<String>,
        content: &str,
    ) -> Result<bool, DbError> {
        let now = chrono::Utc::now().timestamp_millis();

        let item = planning_scratchpad::Entity::find()
            .filter(planning_scratchpad::Column::SessionId.eq(session_id))
            .filter(planning_scratchpad::Column::Title.eq(title))
            .one(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        if let Some(i) = item {
            let mut active: planning_scratchpad::ActiveModel = i.into();
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

    async fn list_scratchpad(
        &self,
        session_id: &str,
    ) -> Result<Vec<planning_scratchpad::Model>, DbError> {
        planning_scratchpad::Entity::find()
            .filter(planning_scratchpad::Column::SessionId.eq(session_id))
            .order_by_desc(planning_scratchpad::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn get_scratchpad_by_ids(
        &self,
        ids: Vec<i64>,
    ) -> Result<Vec<planning_scratchpad::Model>, DbError> {
        planning_scratchpad::Entity::find()
            .filter(planning_scratchpad::Column::Id.is_in(ids))
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn delete_scratchpad_item(&self, session_id: &str, id: i64) -> Result<bool, DbError> {
        let res = planning_scratchpad::Entity::delete_many()
            .filter(planning_scratchpad::Column::Id.eq(id))
            .filter(planning_scratchpad::Column::SessionId.eq(session_id))
            .exec(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        Ok(res.rows_affected > 0)
    }

    async fn check_scratchpad_limit(&self, session_id: &str) -> Result<u64, DbError> {
        planning_scratchpad::Entity::find()
            .filter(planning_scratchpad::Column::SessionId.eq(session_id))
            .count(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn check_scratchpad_duplicate(
        &self,
        session_id: &str,
        title: &str,
    ) -> Result<bool, DbError> {
        let count = planning_scratchpad::Entity::find()
            .filter(planning_scratchpad::Column::SessionId.eq(session_id))
            .filter(planning_scratchpad::Column::Title.eq(title))
            .count(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        Ok(count > 0)
    }

    // --- Context Summary ---

    async fn get_planning_summary(&self, session_id: &str) -> Result<String, DbError> {
        let goal_model = self.get_active_goal(session_id).await?;
        let goal_text = goal_model
            .map(|g| g.goal_text)
            .unwrap_or_else(|| "No active goal".to_string());

        let all_todos = self.list_todos(session_id, true).await?;
        let total = all_todos.len();
        let checked = all_todos.iter().filter(|t| t.is_checked).count();
        let unchecked = total - checked;

        Ok(format!(
            "\n\nGoal: \"{}\"\n\nCurrent progress:\n  - Total: {} todos\n  - Unchecked: {}\n  - Checked: {}",
            goal_text, total, unchecked, checked
        ))
    }
}
