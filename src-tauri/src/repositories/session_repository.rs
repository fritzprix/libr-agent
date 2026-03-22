use super::error::DbError;
use async_trait::async_trait;
use sea_orm::sea_query::Expr;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::entity::{prelude::*, session};

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
    pub created_at: i64,
    pub updated_at: i64,
    pub last_viewed_at: Option<i64>,
    pub last_message_at: Option<i64>,
    pub last_attention_at: Option<i64>,
    pub last_attention_reason: Option<SessionAttentionReason>,
    pub is_bookmarked: bool,
    pub yolo_mode: bool,
    pub workspace_override: Option<String>,
}

impl TryFrom<session::Model> for SessionMetadata {
    type Error = DbError;

    fn try_from(model: session::Model) -> Result<Self, Self::Error> {
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
            yolo_mode: model.yolo_mode,
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

    /// Get all sessions
    async fn get_all_sessions(&self) -> Result<Vec<SessionMetadata>, DbError>;

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

    /// Persist the workspace override path for a session (None clears it)
    async fn update_workspace_override(
        &self,
        session_id: &str,
        override_path: Option<String>,
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
}

#[async_trait]
impl SessionRepository for SqliteSessionRepository {
    async fn upsert_session(&self, session: &SessionMetadata) -> Result<(), DbError> {
        use sea_orm::sea_query::OnConflict;

        let model = session::ActiveModel {
            id: Set(session.id.clone()),
            name: Set(session.name.clone()),
            status: Set(session.status.as_str().to_string()),
            model: Set(session.model.clone()),
            provider: Set(session.provider.clone()),
            agent_config: Set(session.agent_config.clone()),
            parent_session_id: Set(session.parent_session_id.clone()),
            lineage_id: Set(session.lineage_id.clone()),
            depth: Set(session.depth.and_then(|v| i32::try_from(v).ok())),
            max_depth: Set(session.max_depth.and_then(|v| i32::try_from(v).ok())),
            max_fanout: Set(session.max_fanout.and_then(|v| i32::try_from(v).ok())),
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
            yolo_mode: Set(session.yolo_mode),
            workspace_override: Set(session.workspace_override.clone()),
        };

        Session::insert(model)
            .on_conflict(
                OnConflict::column(session::Column::Id)
                    .update_columns([
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
                        session::Column::UpdatedAt,
                        session::Column::LastViewedAt,
                        session::Column::LastMessageAt,
                        session::Column::LastAttentionAt,
                        session::Column::LastAttentionReason,
                        session::Column::YoloMode,
                        session::Column::WorkspaceOverride,
                    ])
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

    async fn get_all_sessions(&self) -> Result<Vec<SessionMetadata>, DbError> {
        use sea_orm::QueryOrder;

        let models = Session::find()
            .order_by_desc(session::Column::UpdatedAt)
            .all(&self.db)
            .await?;

        let sessions: Result<Vec<SessionMetadata>, DbError> =
            models.into_iter().map(SessionMetadata::try_from).collect();

        sessions
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
        session::ActiveModel {
            id: Set(session_id.to_string()),
            is_bookmarked: Set(bookmarked),
            ..Default::default()
        }
        .update(&self.db)
        .await?;

        Ok(())
    }

    async fn update_yolo_mode(&self, session_id: &str, enabled: bool) -> Result<(), DbError> {
        session::ActiveModel {
            id: Set(session_id.to_string()),
            yolo_mode: Set(enabled),
            ..Default::default()
        }
        .update(&self.db)
        .await?;

        Ok(())
    }

    async fn update_workspace_override(
        &self,
        session_id: &str,
        override_path: Option<String>,
    ) -> Result<(), DbError> {
        session::ActiveModel {
            id: Set(session_id.to_string()),
            workspace_override: Set(override_path),
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
        let existing = Session::find_by_id(session_id.to_string())
            .one(&self.db)
            .await?;
        let should_clear_attention = existing
            .as_ref()
            .and_then(|session| session.last_attention_at)
            .is_some_and(|last_attention_at| last_viewed_at >= last_attention_at);

        let mut model = session::ActiveModel {
            id: Set(session_id.to_string()),
            last_viewed_at: Set(Some(last_viewed_at)),
            ..Default::default()
        };
        if should_clear_attention {
            model.last_attention_at = Set(None);
            model.last_attention_reason = Set(None);
        }

        model.update(&self.db).await?;

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

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_db() -> SqliteSessionRepository {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory database");

        // Run migrations
        use migration::{Migrator, MigratorTrait};
        Migrator::up(&db, None)
            .await
            .expect("Failed to run migrations");

        SqliteSessionRepository::new(db)
    }

    #[tokio::test]
    async fn test_create_and_get_session() {
        let repo = setup_test_db().await;
        let now = chrono::Utc::now().timestamp_millis();

        let session = SessionMetadata {
            id: "test-session-1".to_string(),
            name: Some("Test Session".to_string()),
            status: SessionStatus::Idle,
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            agent_config: Some(r#"{"model": "gpt-4"}"#.to_string()),
            parent_session_id: None,
            lineage_id: None,
            depth: None,
            max_depth: None,
            max_fanout: None,
            created_at: now,
            updated_at: now,
            last_viewed_at: None,
            last_message_at: None,
            last_attention_at: None,
            last_attention_reason: None,
            is_bookmarked: false,
            yolo_mode: false,
            workspace_override: None,
        };

        // Test upsert
        repo.upsert_session(&session)
            .await
            .expect("Failed to upsert session");

        // Test get
        let retrieved = repo
            .get_session("test-session-1")
            .await
            .expect("Failed to get session")
            .expect("Session not found");

        assert_eq!(retrieved.id, "test-session-1");
        assert_eq!(retrieved.name, Some("Test Session".to_string()));
        assert_eq!(retrieved.status, SessionStatus::Idle);
    }

    #[tokio::test]
    async fn test_update_session_status() {
        let repo = setup_test_db().await;
        let now = chrono::Utc::now().timestamp_millis();

        let session = SessionMetadata {
            id: "test-session-2".to_string(),
            name: None,
            status: SessionStatus::Idle,
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            agent_config: None,
            parent_session_id: None,
            lineage_id: None,
            depth: None,
            max_depth: None,
            max_fanout: None,
            created_at: now,
            updated_at: now,
            last_viewed_at: None,
            last_message_at: None,
            last_attention_at: None,
            last_attention_reason: None,
            is_bookmarked: false,
            yolo_mode: false,
            workspace_override: None,
        };

        repo.upsert_session(&session)
            .await
            .expect("Failed to upsert session");

        // Small delay to ensure timestamp changes
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Update status to Busy
        repo.update_status("test-session-2", SessionStatus::Busy)
            .await
            .expect("Failed to update status");

        // Verify status changed
        let retrieved = repo
            .get_session("test-session-2")
            .await
            .expect("Failed to get session")
            .expect("Session not found");

        assert_eq!(retrieved.status, SessionStatus::Busy);
        assert!(retrieved.updated_at > now); // Updated timestamp should be newer
    }

    #[tokio::test]
    async fn test_get_all_sessions() {
        let repo = setup_test_db().await;
        let now = chrono::Utc::now().timestamp_millis();
        // Create multiple sessions
        for i in 1..=3 {
            let session = SessionMetadata {
                id: format!("test-session-{}", i),
                name: Some(format!("Session {}", i)),
                status: SessionStatus::Idle,
                model: "gpt-4".to_string(),
                provider: "openai".to_string(),
                agent_config: None,
                parent_session_id: None,
                lineage_id: None,
                depth: None,
                max_depth: None,
                max_fanout: None,
                created_at: now,
                updated_at: now + i,
                last_viewed_at: None,
                last_message_at: None,
                last_attention_at: None,
                last_attention_reason: None,
                is_bookmarked: false,
                yolo_mode: false,
                workspace_override: None,
            };

            repo.upsert_session(&session)
                .await
                .expect("Failed to upsert session");
        }

        // Get all sessions
        let sessions = repo
            .get_all_sessions()
            .await
            .expect("Failed to get all sessions");

        assert_eq!(sessions.len(), 3);
        // Should be ordered by updated_at DESC
        assert_eq!(sessions[0].id, "test-session-3");
        assert_eq!(sessions[1].id, "test-session-2");
        assert_eq!(sessions[2].id, "test-session-1");
    }

    #[tokio::test]
    async fn test_session_status_serialization() {
        assert_eq!(SessionStatus::Idle.as_str(), "idle");
        assert_eq!(SessionStatus::Busy.as_str(), "busy");
        assert_eq!(SessionStatus::Paused.as_str(), "paused");
        assert_eq!(SessionStatus::Error.as_str(), "error");

        assert_eq!(
            SessionStatus::from_str("idle").unwrap(),
            SessionStatus::Idle
        );
        assert_eq!(
            SessionStatus::from_str("busy").unwrap(),
            SessionStatus::Busy
        );
        assert_eq!(
            SessionStatus::from_str("paused").unwrap(),
            SessionStatus::Paused
        );
        assert_eq!(
            SessionStatus::from_str("error").unwrap(),
            SessionStatus::Error
        );

        assert!(SessionStatus::from_str("invalid").is_err());
    }

    #[tokio::test]
    async fn test_upsert_updates_existing_session() {
        let repo = setup_test_db().await;
        let now = chrono::Utc::now().timestamp_millis();

        let session = SessionMetadata {
            id: "test-session-update".to_string(),
            name: Some("Original Name".to_string()),
            status: SessionStatus::Idle,
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            agent_config: None,
            parent_session_id: None,
            lineage_id: None,
            depth: None,
            max_depth: None,
            max_fanout: None,
            created_at: now,
            updated_at: now,
            last_viewed_at: None,
            last_message_at: None,
            last_attention_at: None,
            last_attention_reason: None,
            is_bookmarked: false,
            yolo_mode: false,
            workspace_override: None,
        };

        repo.upsert_session(&session)
            .await
            .expect("Failed to upsert session");

        // Update the session
        let updated_session = SessionMetadata {
            id: "test-session-update".to_string(),
            name: Some("Updated Name".to_string()),
            status: SessionStatus::Busy,
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            agent_config: Some(r#"{"updated": true}"#.to_string()),
            created_at: now,
            updated_at: now + 1000,
            last_viewed_at: None,
            last_message_at: None,
            last_attention_at: None,
            last_attention_reason: None,
            is_bookmarked: false,
            yolo_mode: false,
            parent_session_id: None,
            lineage_id: None,
            depth: None,
            max_depth: None,
            max_fanout: None,
            workspace_override: None,
        };

        repo.upsert_session(&updated_session)
            .await
            .expect("Failed to update session");

        // Verify updates
        let retrieved = repo
            .get_session("test-session-update")
            .await
            .expect("Failed to get session")
            .expect("Session not found");

        assert_eq!(retrieved.name, Some("Updated Name".to_string()));
        assert_eq!(retrieved.status, SessionStatus::Busy);
        assert_eq!(
            retrieved.agent_config,
            Some(r#"{"updated": true}"#.to_string())
        );
    }

    #[tokio::test]
    async fn test_delete_session() {
        let repo = setup_test_db().await;
        let now = chrono::Utc::now().timestamp_millis();

        let session = SessionMetadata {
            id: "test-session-delete".to_string(),
            name: Some("To Be Deleted".to_string()),
            status: SessionStatus::Idle,
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            agent_config: None,
            created_at: now,
            updated_at: now,
            last_viewed_at: None,
            last_message_at: None,
            last_attention_at: None,
            last_attention_reason: None,
            is_bookmarked: false,
            yolo_mode: false,
            parent_session_id: None,
            lineage_id: None,
            depth: None,
            max_depth: None,
            max_fanout: None,
            workspace_override: None,
        };

        repo.upsert_session(&session)
            .await
            .expect("Failed to insert session");

        // Verify it exists
        let retrieved = repo.get_session("test-session-delete").await.unwrap();
        assert!(retrieved.is_some());

        // Delete it
        repo.delete_session("test-session-delete")
            .await
            .expect("Failed to delete session");

        // Verify it's gone
        let retrieved = repo.get_session("test-session-delete").await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_toggle_bookmark() {
        let repo = setup_test_db().await;
        let now = chrono::Utc::now().timestamp_millis();

        let session = SessionMetadata {
            id: "test-bookmark".to_string(),
            name: Some("Bookmarked Session".to_string()),
            status: SessionStatus::Idle,
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            agent_config: None,
            parent_session_id: None,
            lineage_id: None,
            depth: None,
            max_depth: None,
            max_fanout: None,
            created_at: now,
            updated_at: now,
            last_viewed_at: None,
            last_message_at: None,
            last_attention_at: None,
            last_attention_reason: None,
            is_bookmarked: false,
            yolo_mode: false,
            workspace_override: None,
        };

        repo.upsert_session(&session)
            .await
            .expect("Failed to upsert session");

        // Bookmark it
        repo.toggle_bookmark("test-bookmark", true)
            .await
            .expect("Failed to bookmark session");

        let retrieved = repo.get_session("test-bookmark").await.unwrap().unwrap();
        assert!(retrieved.is_bookmarked);

        // Unbookmark it
        repo.toggle_bookmark("test-bookmark", false)
            .await
            .expect("Failed to unbookmark session");

        let retrieved = repo.get_session("test-bookmark").await.unwrap().unwrap();
        assert!(!retrieved.is_bookmarked);
    }
}
