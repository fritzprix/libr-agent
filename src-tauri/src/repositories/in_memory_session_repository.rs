use super::error::DbError;
use super::session_repository::{SessionMetadata, SessionRepository, SessionStatus};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// In-memory implementation of `SessionRepository` for ephemeral sessions
///
/// This implementation stores session data in memory only, without DB persistence.
/// Ideal for temporary sessions that are converted to persistent later.
///
/// # Features
/// - Zero DB interaction (instant operations)
/// - No race conditions (synchronous in-memory updates)
/// - Thread-safe with `Arc<RwLock>`
/// - Idempotent operations (safe to call multiple times)
///
/// # Use Cases
/// - Ephemeral agent sessions (temporary, client-side only)
/// - Testing without database setup
/// - Mock implementations for unit tests
#[derive(Debug, Clone)]
pub struct InMemorySessionRepository {
    sessions: Arc<RwLock<HashMap<String, SessionMetadata>>>,
}

impl InMemorySessionRepository {
    /// Create a new in-memory session repository
    ///
    /// # Example
    /// ```rust
    /// use tauri_mcp_agent_lib::repositories::InMemorySessionRepository;
    /// let repo = InMemorySessionRepository::new();
    /// ```
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the number of sessions currently stored
    ///
    /// Useful for testing and debugging
    pub async fn count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }

    /// Clear all sessions
    ///
    /// Useful for testing
    pub async fn clear(&self) {
        let mut sessions = self.sessions.write().await;
        sessions.clear();
    }
}

impl Default for InMemorySessionRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionRepository for InMemorySessionRepository {
    /// Insert or update session metadata in memory
    ///
    /// This operation is instant (no DB I/O) and idempotent.
    async fn upsert_session(&self, session: &SessionMetadata) -> Result<(), DbError> {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id.clone(), session.clone());
        log::debug!(
            "InMemory: Upserted session {} (name: {:?})",
            session.id,
            session.name
        );
        Ok(())
    }

    /// Get session metadata by ID
    ///
    /// Returns None if session doesn't exist.
    async fn get_session(&self, session_id: &str) -> Result<Option<SessionMetadata>, DbError> {
        let sessions = self.sessions.read().await;
        let result = sessions.get(session_id).cloned();
        log::debug!(
            "InMemory: Get session {} -> {:?}",
            session_id,
            result.is_some()
        );
        Ok(result)
    }

    /// Update session status
    ///
    /// This operation is idempotent - if session doesn't exist, it silently succeeds.
    /// This matches the behavior needed for ephemeral sessions during creation.
    async fn update_status(&self, session_id: &str, status: SessionStatus) -> Result<(), DbError> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.status = status.clone();
            session.updated_at = chrono::Utc::now().timestamp_millis();
            log::debug!(
                "InMemory: Updated status for {} to {:?}",
                session_id,
                status
            );
        } else {
            log::debug!(
                "InMemory: Status update for non-existent session {} (idempotent skip)",
                session_id
            );
        }
        Ok(())
    }

    /// Update agent configuration
    ///
    /// Idempotent operation - silently succeeds if session doesn't exist.
    async fn update_session_config(
        &self,
        session_id: &str,
        model: Option<String>,
        provider: Option<String>,
        agent_config: Option<String>,
    ) -> Result<(), DbError> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            if let Some(m) = model {
                session.model = m;
            }
            if let Some(p) = provider {
                session.provider = p;
            }
            if let Some(ac) = agent_config {
                session.agent_config = Some(ac);
            }
            session.updated_at = chrono::Utc::now().timestamp_millis();
            log::debug!("InMemory: Updated session config for {}", session_id);
        } else {
            log::debug!(
                "InMemory: Config update for non-existent session {} (idempotent skip)",
                session_id
            );
        }
        Ok(())
    }

    /// Get all sessions stored in memory
    ///
    /// Returns sessions in arbitrary order (`HashMap` iteration order).
    async fn get_all_sessions(&self) -> Result<Vec<SessionMetadata>, DbError> {
        let sessions = self.sessions.read().await;
        let result: Vec<SessionMetadata> = sessions.values().cloned().collect();
        log::debug!("InMemory: Get all sessions -> {} sessions", result.len());
        Ok(result)
    }

    async fn get_child_session_ids(&self, parent_session_id: &str) -> Result<Vec<String>, DbError> {
        let sessions = self.sessions.read().await;
        let children = sessions
            .values()
            .filter(|s| s.parent_session_id.as_deref() == Some(parent_session_id))
            .map(|s| s.id.clone())
            .collect();
        Ok(children)
    }

    /// Delete a session from memory
    ///
    /// Idempotent - succeeds even if session doesn't exist.
    async fn delete_session(&self, session_id: &str) -> Result<(), DbError> {
        let mut sessions = self.sessions.write().await;
        let removed = sessions.remove(session_id);
        log::debug!(
            "InMemory: Delete session {} -> {}",
            session_id,
            if removed.is_some() {
                "removed"
            } else {
                "not found"
            }
        );
        Ok(())
    }

    async fn orphan_and_delete_session(&self, session_id: &str) -> Result<(), DbError> {
        let mut sessions = self.sessions.write().await;
        // Nullify parent_session_id for direct children
        for s in sessions.values_mut() {
            if s.parent_session_id.as_deref() == Some(session_id) {
                s.parent_session_id = None;
            }
        }
        sessions.remove(session_id);
        Ok(())
    }

    async fn toggle_bookmark(&self, session_id: &str, bookmarked: bool) -> Result<(), DbError> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.is_bookmarked = bookmarked;
        }
        Ok(())
    }

    async fn update_yolo_mode(&self, session_id: &str, enabled: bool) -> Result<(), DbError> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.yolo_mode = enabled;
        }
        Ok(())
    }

    async fn update_workspace_override(
        &self,
        session_id: &str,
        override_path: Option<String>,
    ) -> Result<(), DbError> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.workspace_override = override_path;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::InMemorySessionRepository;
    use crate::repositories::session_repository::{
        SessionMetadata, SessionRepository, SessionStatus,
    };

    #[tokio::test]
    async fn test_new_repository_is_empty() {
        let repo = InMemorySessionRepository::new();
        assert_eq!(repo.count().await, 0);
    }

    #[tokio::test]
    async fn test_upsert_and_get_session() {
        let repo = InMemorySessionRepository::new();
        let session = SessionMetadata {
            id: "test-session".to_string(),
            name: Some("Test Session".to_string()),
            status: SessionStatus::Idle,
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            is_bookmarked: false,
            agent_config: Some("{}".to_string()),
            parent_session_id: None,
            lineage_id: None,
            depth: None,
            max_depth: None,
            max_fanout: None,
            created_at: 1234567890,
            updated_at: 1234567890,
            yolo_mode: false,
            workspace_override: None,
        };

        // Upsert session
        repo.upsert_session(&session).await.unwrap();
        assert_eq!(repo.count().await, 1);

        // Get session
        let retrieved = repo.get_session("test-session").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "test-session");
    }

    #[tokio::test]
    async fn test_update_status() {
        let repo = InMemorySessionRepository::new();
        let session = SessionMetadata {
            id: "test-session".to_string(),
            name: Some("Test".to_string()),
            status: SessionStatus::Idle,
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            is_bookmarked: false,
            agent_config: None,
            parent_session_id: None,
            lineage_id: None,
            depth: None,
            max_depth: None,
            max_fanout: None,
            created_at: 100,
            updated_at: 100,
            yolo_mode: false,
            workspace_override: None,
        };

        repo.upsert_session(&session).await.unwrap();

        // Update status
        repo.update_status("test-session", SessionStatus::Busy)
            .await
            .unwrap();

        // Verify status changed
        let updated = repo.get_session("test-session").await.unwrap().unwrap();
        assert_eq!(updated.status, SessionStatus::Busy);
        assert!(updated.updated_at > 100); // Timestamp should be updated
    }

    #[tokio::test]
    async fn test_update_status_nonexistent_session_is_idempotent() {
        let repo = InMemorySessionRepository::new();

        // Should not fail even if session doesn't exist
        let result = repo.update_status("nonexistent", SessionStatus::Busy).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_session() {
        let repo = InMemorySessionRepository::new();
        let session = SessionMetadata {
            id: "test-session".to_string(),
            name: None,
            status: SessionStatus::Idle,
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            is_bookmarked: false,
            agent_config: None,
            parent_session_id: None,
            lineage_id: None,
            depth: None,
            max_depth: None,
            max_fanout: None,
            created_at: 100,
            updated_at: 100,
            yolo_mode: false,
            workspace_override: None,
        };

        repo.upsert_session(&session).await.unwrap();
        assert_eq!(repo.count().await, 1);

        // Delete session
        repo.delete_session("test-session").await.unwrap();
        assert_eq!(repo.count().await, 0);

        // Verify session is gone
        let retrieved = repo.get_session("test-session").await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_get_all_sessions() {
        let repo = InMemorySessionRepository::new();

        // Add multiple sessions
        for i in 0..3 {
            let session = SessionMetadata {
                id: format!("session-{}", i),
                name: Some(format!("Session {}", i)),
                status: SessionStatus::Idle,
                model: "gpt-4".to_string(),
                provider: "openai".to_string(),
                is_bookmarked: false,
                agent_config: None,
                parent_session_id: None,
                lineage_id: None,
                depth: None,
                max_depth: None,
                max_fanout: None,
                created_at: 100,
                updated_at: 100,
                yolo_mode: false,
                workspace_override: None,
            };
            repo.upsert_session(&session).await.unwrap();
        }

        // Get all sessions
        let all = repo.get_all_sessions().await.unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn test_clear() {
        let repo = InMemorySessionRepository::new();

        // Add sessions
        for i in 0..5 {
            let session = SessionMetadata {
                id: format!("session-{}", i),
                name: None,
                status: SessionStatus::Idle,
                model: "gpt-4".to_string(),
                provider: "openai".to_string(),
                is_bookmarked: false,
                agent_config: None,
                parent_session_id: None,
                lineage_id: None,
                depth: None,
                max_depth: None,
                max_fanout: None,
                created_at: 100,
                updated_at: 100,
                yolo_mode: false,
                workspace_override: None,
            };
            repo.upsert_session(&session).await.unwrap();
        }

        assert_eq!(repo.count().await, 5);

        // Clear all
        repo.clear().await;
        assert_eq!(repo.count().await, 0);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        use std::sync::Arc;
        use tokio::task;

        let repo = Arc::new(InMemorySessionRepository::new());

        // Spawn multiple tasks to upsert sessions concurrently
        let mut handles = vec![];
        for i in 0..10 {
            let repo_clone = repo.clone();
            let handle = task::spawn(async move {
                let session = SessionMetadata {
                    id: format!("session-{}", i),
                    name: None,
                    status: SessionStatus::Idle,
                    model: "gpt-4".to_string(),
                    provider: "openai".to_string(),
                    is_bookmarked: false,
                    agent_config: None,
                    created_at: 100,
                    updated_at: 100,
                    parent_session_id: None,
                    lineage_id: None,
                    depth: None,
                    max_depth: None,
                    max_fanout: None,
                    yolo_mode: false,
                    workspace_override: None,
                };
                repo_clone.upsert_session(&session).await.unwrap();
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify all sessions were added
        assert_eq!(repo.count().await, 10);
    }
}
