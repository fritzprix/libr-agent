use super::error::DbError;
use async_trait::async_trait;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
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
    pub created_at: i64,
    pub updated_at: i64,
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
            created_at: model.created_at,
            updated_at: model.updated_at,
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

    /// Delete a session
    async fn delete_session(&self, session_id: &str) -> Result<(), DbError>;
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
            created_at: Set(session.created_at),
            updated_at: Set(session.updated_at),
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
                        session::Column::UpdatedAt,
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

    async fn delete_session(&self, session_id: &str) -> Result<(), DbError> {
        Session::delete_by_id(session_id).exec(&self.db).await?;
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
            created_at: now,
            updated_at: now,
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
            created_at: now,
            updated_at: now,
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
                created_at: now,
                updated_at: now + i,
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
            created_at: now,
            updated_at: now,
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
}
