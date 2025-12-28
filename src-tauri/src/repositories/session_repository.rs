use super::error::DbError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

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
    pub agent_config: Option<String>, // JSON string of agent configuration
    pub created_at: i64,
    pub updated_at: i64,
}

/// Session repository trait for abstraction and testability
#[async_trait]
pub trait SessionRepository: Send + Sync {
    /// Initialize the sessions table
    async fn create_table(&self) -> Result<(), DbError>;

    /// Insert or update session metadata
    async fn upsert_session(&self, session: &SessionMetadata) -> Result<(), DbError>;

    /// Get session metadata by ID
    async fn get_session(&self, session_id: &str) -> Result<Option<SessionMetadata>, DbError>;

    /// Update session status
    async fn update_status(&self, session_id: &str, status: SessionStatus) -> Result<(), DbError>;

    /// Get all sessions
    async fn get_all_sessions(&self) -> Result<Vec<SessionMetadata>, DbError>;

    /// Delete a session
    async fn delete_session(&self, session_id: &str) -> Result<(), DbError>;

    /// Delete index metadata for a specific session
    async fn delete_index_metadata(&self, session_id: &str) -> Result<(), DbError>;
}

/// SQLite implementation of SessionRepository
#[derive(Debug)]
pub struct SqliteSessionRepository {
    pool: SqlitePool,
}

impl SqliteSessionRepository {
    /// Create a new SQLite session repository with the given pool
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SessionRepository for SqliteSessionRepository {
    async fn create_table(&self) -> Result<(), DbError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                name TEXT,
                status TEXT NOT NULL DEFAULT 'idle',
                agent_config TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status);
            CREATE INDEX IF NOT EXISTS idx_sessions_created_at ON sessions(created_at);
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn upsert_session(&self, session: &SessionMetadata) -> Result<(), DbError> {
        sqlx::query(
            r#"
            INSERT INTO sessions (id, name, status, agent_config, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                status = excluded.status,
                agent_config = excluded.agent_config,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&session.id)
        .bind(&session.name)
        .bind(session.status.as_str())
        .bind(&session.agent_config)
        .bind(session.created_at)
        .bind(session.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_session(&self, session_id: &str) -> Result<Option<SessionMetadata>, DbError> {
        let result = sqlx::query(
            r#"
            SELECT id, name, status, agent_config, created_at, updated_at
            FROM sessions
            WHERE id = ?
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = result {
            let status_str: String = row.get("status");
            Ok(Some(SessionMetadata {
                id: row.get("id"),
                name: row.get("name"),
                status: SessionStatus::from_str(&status_str)?,
                agent_config: row.get("agent_config"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            }))
        } else {
            Ok(None)
        }
    }

    async fn update_status(&self, session_id: &str, status: SessionStatus) -> Result<(), DbError> {
        let now = chrono::Utc::now().timestamp_millis();
        sqlx::query(
            r#"
            UPDATE sessions
            SET status = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(status.as_str())
        .bind(now)
        .bind(session_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_all_sessions(&self) -> Result<Vec<SessionMetadata>, DbError> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, status, agent_config, created_at, updated_at
            FROM sessions
            ORDER BY updated_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let sessions: Result<Vec<SessionMetadata>, DbError> = rows
            .into_iter()
            .map(|row| {
                let status_str: String = row.get("status");
                Ok(SessionMetadata {
                    id: row.get("id"),
                    name: row.get("name"),
                    status: SessionStatus::from_str(&status_str)?,
                    agent_config: row.get("agent_config"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect();

        sessions
    }

    async fn delete_session(&self, session_id: &str) -> Result<(), DbError> {
        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete_index_metadata(&self, session_id: &str) -> Result<(), DbError> {
        sqlx::query("DELETE FROM message_index_meta WHERE session_id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_test_db() -> SqliteSessionRepository {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory database");

        let repo = SqliteSessionRepository::new(pool);
        repo.create_table()
            .await
            .expect("Failed to create sessions table");

        repo
    }

    #[tokio::test]
    async fn test_create_and_get_session() {
        let repo = setup_test_db().await;
        let now = chrono::Utc::now().timestamp_millis();

        let session = SessionMetadata {
            id: "test-session-1".to_string(),
            name: Some("Test Session".to_string()),
            status: SessionStatus::Idle,
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
            agent_config: None,
            created_at: now,
            updated_at: now,
        };

        repo.upsert_session(&session)
            .await
            .expect("Failed to insert session");

        // Update the session
        let updated_session = SessionMetadata {
            id: "test-session-update".to_string(),
            name: Some("Updated Name".to_string()),
            status: SessionStatus::Busy,
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
