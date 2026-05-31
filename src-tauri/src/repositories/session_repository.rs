use super::error::DbError;
use async_trait::async_trait;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::entity::{prelude::*, session};

const SESSION_UPSERT_COLUMNS: [session::Column; 21] = [
    session::Column::Name,
    session::Column::Status,
    session::Column::Model,
    session::Column::Provider,
    session::Column::AgentConfig,
    session::Column::ParentSessionId,
    session::Column::LineageId,
    session::Column::Depth,
    session::Column::MaxDepth,
    session::Column::MaxFanout,
    session::Column::OrgId,
    session::Column::OrgName,
    session::Column::OrgRootSessionId,
    session::Column::UpdatedAt,
    session::Column::LastViewedAt,
    session::Column::LastMessageAt,
    session::Column::LastAttentionAt,
    session::Column::LastAttentionReason,
    session::Column::YoloMode,
    session::Column::UnsafeMode,
    session::Column::WorkspaceOverride,
];

fn coalesce_execution_flags(yolo_mode: bool, unsafe_mode: bool) -> (bool, bool) {
    if unsafe_mode {
        (false, true)
    } else if yolo_mode {
        (true, false)
    } else {
        (false, false)
    }
}

/// Session status enum representing the agent workflow state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Idle,
    Busy,
    Paused,
    Error,
}

impl SessionStatus {
    pub fn as_str(&self) -> &str {
        match self {
            SessionStatus::Idle => "idle",
            SessionStatus::Busy => "busy",
            SessionStatus::Paused => "paused",
            SessionStatus::Error => "error",
        }
    }
}

impl FromStr for SessionStatus {
    type Err = DbError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "idle" => Ok(SessionStatus::Idle),
            "busy" => Ok(SessionStatus::Busy),
            "paused" => Ok(SessionStatus::Paused),
            "error" => Ok(SessionStatus::Error),
            _ => Err(DbError::InvalidInput(format!(
                "Invalid session status: {}",
                s
            ))),
        }
    }
}

/// Session attention reason used for notification-style unread state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionAttentionReason {
    RecurringStop,
    PendingApproval,
}

impl SessionAttentionReason {
    pub fn as_str(&self) -> &str {
        match self {
            SessionAttentionReason::RecurringStop => "recurringStop",
            SessionAttentionReason::PendingApproval => "pendingApproval",
        }
    }
}

impl FromStr for SessionAttentionReason {
    type Err = DbError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "recurringStop" => Ok(SessionAttentionReason::RecurringStop),
            "pendingApproval" => Ok(SessionAttentionReason::PendingApproval),
            _ => Err(DbError::InvalidInput(format!(
                "Invalid session attention reason: {}",
                s
            ))),
        }
    }
}

/// Session metadata stored in SQLite
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadata {
    pub id: String,
    pub name: Option<String>,
    pub status: SessionStatus,
    pub model: String,
    pub provider: String,
    pub agent_config: Option<String>, // JSON string of agent configuration
    pub parent_session_id: Option<String>,
    pub lineage_id: Option<String>,
    pub depth: Option<u32>,
    pub max_depth: Option<u32>,
    pub max_fanout: Option<u32>,
    pub org_id: Option<String>,
    pub org_name: Option<String>,
    pub org_root_session_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_viewed_at: Option<i64>,
    pub last_message_at: Option<i64>,
    pub last_attention_at: Option<i64>,
    pub last_attention_reason: Option<SessionAttentionReason>,
    pub is_bookmarked: bool,
    pub yolo_mode: bool,
    pub unsafe_mode: bool,
    pub workspace_override: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionListCursor {
    pub updated_at: i64,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionListPage {
    pub items: Vec<SessionMetadata>,
    pub next_cursor: Option<SessionListCursor>,
}

impl TryFrom<session::Model> for SessionMetadata {
    type Error = DbError;

    fn try_from(model: session::Model) -> Result<Self, Self::Error> {
        let (yolo_mode, unsafe_mode) = coalesce_execution_flags(model.yolo_mode, model.unsafe_mode);
        Ok(SessionMetadata {
            id: model.id,
            name: model.name,
            status: SessionStatus::from_str(&model.status)?,
            model: model.model,
            provider: model.provider,
            agent_config: model.agent_config,
            parent_session_id: model.parent_session_id,
            lineage_id: model.lineage_id,
            depth: model.depth.and_then(|v| u32::try_from(v).ok()),
            max_depth: model.max_depth.and_then(|v| u32::try_from(v).ok()),
            max_fanout: model.max_fanout.and_then(|v| u32::try_from(v).ok()),
            org_id: model.org_id,
            org_name: model.org_name,
            org_root_session_id: model.org_root_session_id,
            created_at: model.created_at,
            updated_at: model.updated_at,
            last_viewed_at: model.last_viewed_at,
            last_message_at: model.last_message_at,
            last_attention_at: model.last_attention_at,
            last_attention_reason: model
                .last_attention_reason
                .as_deref()
                .map(SessionAttentionReason::from_str)
                .transpose()?,
            is_bookmarked: model.is_bookmarked,
            yolo_mode,
            unsafe_mode,
            workspace_override: model.workspace_override,
        })
    }
}

/// Session repository trait for abstraction and testability
#[async_trait]
pub trait SessionRepository: Send + Sync {
    /// Insert or update session metadata
    async fn upsert_session(&self, session: &SessionMetadata) -> Result<(), DbError>;

    /// Get session metadata by ID
    async fn get_session(&self, session_id: &str) -> Result<Option<SessionMetadata>, DbError>;

    /// Update session status
    async fn update_status(&self, session_id: &str, status: SessionStatus) -> Result<(), DbError>;

    /// Update session configuration (model, provider, and/or agent_config)
    async fn update_session_config(
        &self,
        session_id: &str,
        model: Option<String>,
        provider: Option<String>,
        agent_config: Option<String>,
    ) -> Result<(), DbError>;

    /// Update the user-visible session title without affecting activity ordering.
    async fn update_name(&self, session_id: &str, name: String) -> Result<(), DbError>;

    /// Get all sessions
    async fn get_all_sessions(&self) -> Result<Vec<SessionMetadata>, DbError>;

    /// List sessions ordered by most recent activity with cursor pagination.
    async fn list_sessions(
        &self,
        cursor: Option<SessionListCursor>,
        limit: u64,
    ) -> Result<SessionListPage, DbError>;

    /// List sessions that still have unread attention for notifications.
    async fn list_attention_sessions(&self) -> Result<Vec<SessionMetadata>, DbError>;

    /// Get direct child session IDs for a parent session ID
    async fn get_child_session_ids(&self, parent_session_id: &str) -> Result<Vec<String>, DbError>;

    /// Delete a session
    async fn delete_session(&self, session_id: &str) -> Result<(), DbError>;

    /// Delete only this session, orphaning its direct children (set their parent_session_id to NULL)
    async fn orphan_and_delete_session(&self, session_id: &str) -> Result<(), DbError>;

    /// Toggle the bookmark flag for a session
    async fn toggle_bookmark(&self, session_id: &str, bookmarked: bool) -> Result<(), DbError>;

    /// Update the YOLO mode flag for a session
    async fn update_yolo_mode(&self, session_id: &str, enabled: bool) -> Result<(), DbError>;

    /// Update the unsafe mode flag for a session
    async fn update_unsafe_mode(&self, session_id: &str, enabled: bool) -> Result<(), DbError>;

    /// Update both execution mode flags atomically for a session.
    async fn update_execution_mode(
        &self,
        session_id: &str,
        yolo_enabled: bool,
        unsafe_enabled: bool,
    ) -> Result<(), DbError>;

    /// Persist the workspace override path for a session (None clears it)
    async fn update_workspace_override(
        &self,
        session_id: &str,
        override_path: Option<String>,
    ) -> Result<(), DbError>;

    /// Persist org identity metadata for a session. Passing None clears the field.
    async fn update_org_identity(
        &self,
        session_id: &str,
        org_id: Option<String>,
        org_name: Option<String>,
        org_root_session_id: Option<String>,
    ) -> Result<(), DbError>;

    /// Persist the timestamp when the session was last viewed by the user.
    async fn update_last_viewed_at(
        &self,
        session_id: &str,
        last_viewed_at: i64,
    ) -> Result<(), DbError>;

    /// Persist an attention-worthy event for the session.
    async fn update_attention(
        &self,
        session_id: &str,
        last_attention_at: i64,
        reason: SessionAttentionReason,
    ) -> Result<(), DbError>;
}

/// SQLite implementation of SessionRepository using SeaORM
#[derive(Debug, Clone)]
pub struct SqliteSessionRepository {
    db: DatabaseConnection,
}

impl SqliteSessionRepository {
    /// Create a new SQLite session repository with the given database connection
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    fn build_active_model(session: &SessionMetadata) -> session::ActiveModel {
        let (yolo_mode, unsafe_mode) =
            coalesce_execution_flags(session.yolo_mode, session.unsafe_mode);
        session::ActiveModel {
            id: Set(session.id.clone()),
            name: Set(session.name.clone()),
            status: Set(session.status.as_str().to_string()),
            model: Set(session.model.clone()),
            provider: Set(session.provider.clone()),
            agent_config: Set(session.agent_config.clone()),
            parent_session_id: Set(session.parent_session_id.clone()),
            lineage_id: Set(session.lineage_id.clone()),
            depth: Set(session.depth.and_then(|value| i32::try_from(value).ok())),
            max_depth: Set(session
                .max_depth
                .and_then(|value| i32::try_from(value).ok())),
            max_fanout: Set(session
                .max_fanout
                .and_then(|value| i32::try_from(value).ok())),
            org_id: Set(session.org_id.clone()),
            org_name: Set(session.org_name.clone()),
            org_root_session_id: Set(session.org_root_session_id.clone()),
            created_at: Set(session.created_at),
            updated_at: Set(session.updated_at),
            last_viewed_at: Set(session.last_viewed_at),
            last_message_at: Set(session.last_message_at),
            last_attention_at: Set(session.last_attention_at),
            last_attention_reason: Set(session
                .last_attention_reason
                .as_ref()
                .map(SessionAttentionReason::as_str)
                .map(str::to_string)),
            is_bookmarked: Set(session.is_bookmarked),
            yolo_mode: Set(yolo_mode),
            unsafe_mode: Set(unsafe_mode),
            workspace_override: Set(session.workspace_override.clone()),
        }
    }

    async fn apply_partial_update(
        &self,
        active_model: session::ActiveModel,
    ) -> Result<(), DbError> {
        active_model.update(&self.db).await?;
        Ok(())
    }
}

#[async_trait]
impl SessionRepository for SqliteSessionRepository {
    async fn upsert_session(&self, session: &SessionMetadata) -> Result<(), DbError> {
        use sea_orm::sea_query::OnConflict;

        Session::insert(Self::build_active_model(session))
            .on_conflict(
                OnConflict::column(session::Column::Id)
                    .update_columns(SESSION_UPSERT_COLUMNS)
                    .to_owned(),
            )
            .exec(&self.db)
            .await?;

        Ok(())
    }

    async fn get_session(&self, session_id: &str) -> Result<Option<SessionMetadata>, DbError> {
        let result = Session::find_by_id(session_id).one(&self.db).await?;

        if let Some(model) = result {
            Ok(Some(SessionMetadata::try_from(model)?))
        } else {
            Ok(None)
        }
    }

    async fn update_status(&self, session_id: &str, status: SessionStatus) -> Result<(), DbError> {
        let now = chrono::Utc::now().timestamp_millis();

        session::ActiveModel {
            id: Set(session_id.to_string()),
            status: Set(status.as_str().to_string()),
            updated_at: Set(now),
            ..Default::default()
        }
        .update(&self.db)
        .await?;

        Ok(())
    }

    async fn update_session_config(
        &self,
        session_id: &str,
        model: Option<String>,
        provider: Option<String>,
        agent_config: Option<String>,
    ) -> Result<(), DbError> {
        let now = chrono::Utc::now().timestamp_millis();

        let mut active_model = session::ActiveModel {
            id: Set(session_id.to_string()),
            updated_at: Set(now),
            ..Default::default()
        };

        if let Some(m) = model {
            active_model.model = Set(m);
        }
        if let Some(p) = provider {
            active_model.provider = Set(p);
        }
        if let Some(ac) = agent_config {
            active_model.agent_config = Set(Some(ac));
        }

        active_model.update(&self.db).await?;

        Ok(())
    }

    async fn update_name(&self, session_id: &str, name: String) -> Result<(), DbError> {
        self.apply_partial_update(session::ActiveModel {
            id: Set(session_id.to_string()),
            name: Set(Some(name)),
            ..Default::default()
        })
        .await
    }

    async fn get_all_sessions(&self) -> Result<Vec<SessionMetadata>, DbError> {
        let models = Session::find()
            .order_by_desc(session::Column::UpdatedAt)
            .all(&self.db)
            .await?;

        let sessions: Result<Vec<SessionMetadata>, DbError> =
            models.into_iter().map(SessionMetadata::try_from).collect();

        sessions
    }

    async fn list_sessions(
        &self,
        cursor: Option<SessionListCursor>,
        limit: u64,
    ) -> Result<SessionListPage, DbError> {
        let normalized_limit = limit.clamp(1, 200);
        let mut condition = Condition::all();

        if let Some(cursor) = cursor {
            condition = condition.add(
                Condition::any()
                    .add(session::Column::UpdatedAt.lt(cursor.updated_at))
                    .add(
                        Condition::all()
                            .add(session::Column::UpdatedAt.eq(cursor.updated_at))
                            .add(session::Column::Id.lt(cursor.id)),
                    ),
            );
        }

        let mut models = Session::find()
            .filter(condition)
            .order_by_desc(session::Column::UpdatedAt)
            .order_by_desc(session::Column::Id)
            .limit(normalized_limit + 1)
            .all(&self.db)
            .await?;

        let next_cursor = if models.len() > normalized_limit as usize {
            models.truncate(normalized_limit as usize);
            models.last().map(|model| SessionListCursor {
                updated_at: model.updated_at,
                id: model.id.clone(),
            })
        } else {
            None
        };

        let items = models
            .into_iter()
            .map(SessionMetadata::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(SessionListPage { items, next_cursor })
    }

    async fn list_attention_sessions(&self) -> Result<Vec<SessionMetadata>, DbError> {
        let models = Session::find()
            .filter(session::Column::LastAttentionAt.is_not_null())
            .filter(
                Condition::any()
                    .add(session::Column::LastViewedAt.is_null())
                    .add(
                        Expr::col(session::Column::LastAttentionAt)
                            .gt(Expr::col(session::Column::LastViewedAt)),
                    ),
            )
            .order_by_desc(session::Column::LastAttentionAt)
            .order_by_desc(session::Column::UpdatedAt)
            .order_by_desc(session::Column::Id)
            .all(&self.db)
            .await?;

        models.into_iter().map(SessionMetadata::try_from).collect()
    }

    async fn get_child_session_ids(&self, parent_session_id: &str) -> Result<Vec<String>, DbError> {
        use sea_orm::{ColumnTrait, QueryFilter};

        let models = Session::find()
            .filter(session::Column::ParentSessionId.eq(parent_session_id))
            .all(&self.db)
            .await?;

        Ok(models.into_iter().map(|m| m.id).collect())
    }

    async fn delete_session(&self, session_id: &str) -> Result<(), DbError> {
        Session::delete_by_id(session_id).exec(&self.db).await?;
        Ok(())
    }

    async fn orphan_and_delete_session(&self, session_id: &str) -> Result<(), DbError> {
        // Nullify parent_session_id for direct children
        session::Entity::update_many()
            .col_expr(
                session::Column::ParentSessionId,
                Expr::value(Option::<String>::None),
            )
            .filter(session::Column::ParentSessionId.eq(session_id.to_string()))
            .exec(&self.db)
            .await?;

        // Delete only this session (children are now top-level orphans)
        Session::delete_by_id(session_id).exec(&self.db).await?;
        Ok(())
    }

    async fn toggle_bookmark(&self, session_id: &str, bookmarked: bool) -> Result<(), DbError> {
        self.apply_partial_update(session::ActiveModel {
            id: Set(session_id.to_string()),
            is_bookmarked: Set(bookmarked),
            ..Default::default()
        })
        .await
    }

    async fn update_yolo_mode(&self, session_id: &str, enabled: bool) -> Result<(), DbError> {
        self.apply_partial_update(session::ActiveModel {
            id: Set(session_id.to_string()),
            yolo_mode: Set(enabled),
            ..Default::default()
        })
        .await
    }

    async fn update_unsafe_mode(&self, session_id: &str, enabled: bool) -> Result<(), DbError> {
        self.apply_partial_update(session::ActiveModel {
            id: Set(session_id.to_string()),
            unsafe_mode: Set(enabled),
            ..Default::default()
        })
        .await
    }

    async fn update_execution_mode(
        &self,
        session_id: &str,
        yolo_enabled: bool,
        unsafe_enabled: bool,
    ) -> Result<(), DbError> {
        self.apply_partial_update(session::ActiveModel {
            id: Set(session_id.to_string()),
            yolo_mode: Set(yolo_enabled),
            unsafe_mode: Set(unsafe_enabled),
            ..Default::default()
        })
        .await
    }

    async fn update_workspace_override(
        &self,
        session_id: &str,
        override_path: Option<String>,
    ) -> Result<(), DbError> {
        self.apply_partial_update(session::ActiveModel {
            id: Set(session_id.to_string()),
            workspace_override: Set(override_path),
            ..Default::default()
        })
        .await
    }

    async fn update_org_identity(
        &self,
        session_id: &str,
        org_id: Option<String>,
        org_name: Option<String>,
        org_root_session_id: Option<String>,
    ) -> Result<(), DbError> {
        let now = chrono::Utc::now().timestamp_millis();

        session::ActiveModel {
            id: Set(session_id.to_string()),
            org_id: Set(org_id),
            org_name: Set(org_name),
            org_root_session_id: Set(org_root_session_id),
            updated_at: Set(now),
            ..Default::default()
        }
        .update(&self.db)
        .await?;

        Ok(())
    }

    async fn update_last_viewed_at(
        &self,
        session_id: &str,
        last_viewed_at: i64,
    ) -> Result<(), DbError> {
        session::Entity::update_many()
            .col_expr(session::Column::LastViewedAt, Expr::value(Some(last_viewed_at)))
            .col_expr(
                session::Column::LastAttentionAt,
                Expr::cust_with_values(
                    "CASE WHEN last_attention_at IS NOT NULL AND last_attention_at <= ? THEN NULL ELSE last_attention_at END",
                    [last_viewed_at],
                ),
            )
            .col_expr(
                session::Column::LastAttentionReason,
                Expr::cust_with_values(
                    "CASE WHEN last_attention_at IS NOT NULL AND last_attention_at <= ? THEN NULL ELSE last_attention_reason END",
                    [last_viewed_at],
                ),
            )
            .filter(session::Column::Id.eq(session_id.to_string()))
            .exec(&self.db)
            .await?;

        Ok(())
    }

    async fn update_attention(
        &self,
        session_id: &str,
        last_attention_at: i64,
        reason: SessionAttentionReason,
    ) -> Result<(), DbError> {
        session::ActiveModel {
            id: Set(session_id.to_string()),
            last_attention_at: Set(Some(last_attention_at)),
            last_attention_reason: Set(Some(reason.as_str().to_string())),
            ..Default::default()
        }
        .update(&self.db)
        .await?;

        Ok(())
    }
}
