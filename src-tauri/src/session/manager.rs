use log::info;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tokio::time::{Duration, Instant};

use super::types::{SessionStats, SessionWorkspaceInfo};
use crate::services::SessionDirectoryService;

#[derive(Clone, Debug)]
pub struct SessionManager {
    pub(crate) directory_service: SessionDirectoryService,
    pub(crate) workspace_pool: Arc<RwLock<HashMap<String, SessionWorkspaceInfo>>>,
}

impl SessionManager {
    pub fn new() -> Result<Self, String> {
        let base_data_dir = dirs::data_dir()
            .ok_or_else(|| "Failed to get system data directory".to_string())?
            .join("com.fritzprix.libragent");

        Self::new_with_base_dir(base_data_dir)
    }

    pub fn new_with_base_dir(base_data_dir: PathBuf) -> Result<Self, String> {
        let directory_service = SessionDirectoryService::new(base_data_dir.clone())?;

        info!("SessionManager initialized with base directory: {base_data_dir:?}");

        Ok(Self {
            directory_service,
            workspace_pool: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Fast session creation using template workspace (delegated to Directory Service)
    async fn create_session_workspace_async(&self, session_id: &str) -> Result<PathBuf, String> {
        let session_dir = self
            .directory_service
            .create_session_workspace(session_id)
            .await?;

        // Add to workspace pool
        let workspace_info = SessionWorkspaceInfo {
            session_id: session_id.to_string(),
            workspace_path: session_dir.clone(),
            workspace_override: None,
            created_at: Instant::now(),
            last_accessed: Instant::now(),
            is_template: false,
        };

        {
            let mut pool = self
                .workspace_pool
                .write()
                .map_err(|e| format!("Failed to write workspace pool: {e}"))?;
            pool.insert(session_id.to_string(), workspace_info);
        }

        Ok(session_dir)
    }

    pub fn get_session_workspace_dir_by_id(&self, session_id: &str) -> PathBuf {
        // Try to find in pool first to see if there is an override
        if let Ok(pool) = self.workspace_pool.read() {
            if let Some(info) = pool.get(session_id) {
                if let Some(override_path) = &info.workspace_override {
                    return override_path.clone();
                }
                // Also return the standard path if found in pool
                return info.workspace_path.clone();
            }
        }

        // Delegate directory creation/retrieval to service
        let final_dir = self.directory_service.get_workspace_dir(session_id);

        // Lazy load: Add to pool if missing
        if let Ok(mut pool) = self.workspace_pool.write() {
            // Double check inside write lock
            if let Some(info) = pool.get(session_id) {
                if let Some(override_path) = &info.workspace_override {
                    return override_path.clone();
                }
                return info.workspace_path.clone();
            }

            let workspace_info = SessionWorkspaceInfo {
                session_id: session_id.to_string(),
                workspace_path: final_dir.clone(),
                workspace_override: None,
                created_at: Instant::now(),
                last_accessed: Instant::now(),
                is_template: false,
            };
            pool.insert(session_id.to_string(), workspace_info);
            info!("Lazy loaded workspace info for session: {}", session_id);
        }

        final_dir
    }

    pub fn get_base_data_dir(&self) -> &PathBuf {
        self.directory_service.get_base_data_dir()
    }

    pub fn get_logs_dir(&self) -> PathBuf {
        self.directory_service.get_logs_dir()
    }

    // Expose directory service for other services (like cleanup)
    pub fn get_directory_service(&self) -> &SessionDirectoryService {
        &self.directory_service
    }

    pub fn list_sessions(&self) -> Result<Vec<String>, String> {
        // We still need to read the directory to list sessions, but we use the base path from service
        let workspaces_dir = self
            .directory_service
            .get_base_data_dir()
            .join("workspaces");

        let entries = std::fs::read_dir(&workspaces_dir)
            .map_err(|e| format!("Failed to read workspaces directory: {e}"))?;

        let mut sessions = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
            if entry
                .file_type()
                .map_err(|e| format!("Failed to get file type: {e}"))?
                .is_dir()
            {
                if let Some(name) = entry.file_name().to_str() {
                    // Skip template directories
                    if name != "templates" {
                        sessions.push(name.to_string());
                    }
                }
            }
        }

        sessions.sort();
        Ok(sessions)
    }

    /// Get all active sessions with their workspace information
    pub fn get_active_sessions(&self) -> Result<Vec<SessionWorkspaceInfo>, String> {
        let pool = self
            .workspace_pool
            .read()
            .map_err(|e| format!("Failed to read workspace pool: {e}"))?;

        let mut sessions: Vec<SessionWorkspaceInfo> = pool.values().cloned().collect();
        sessions.sort_by(|a, b| b.last_accessed.cmp(&a.last_accessed));
        Ok(sessions)
    }

    /// Get specific session info
    pub fn get_session_info(&self, session_id: &str) -> Option<SessionWorkspaceInfo> {
        let pool = self.workspace_pool.read().ok()?;
        pool.get(session_id).cloned()
    }

    /// Pre-allocate sessions for faster switching
    pub async fn pre_allocate_sessions(&self, count: usize) -> Result<Vec<String>, String> {
        let mut allocated_sessions = Vec::new();

        for i in 0..count {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let session_id = format!("pool-{timestamp}-{i}");
            self.create_session_workspace_async(&session_id).await?;
            allocated_sessions.push(session_id);
        }

        info!(
            "Pre-allocated {} sessions for fast switching",
            allocated_sessions.len()
        );
        Ok(allocated_sessions)
    }

    /// Get or create a session from the pool (instant switching)
    pub async fn get_pooled_session(&self) -> Result<String, String> {
        // First check if there are any pre-allocated sessions
        let unused_session_id = {
            let pool = self
                .workspace_pool
                .read()
                .map_err(|e| format!("Failed to read workspace pool: {e}"))?;

            // Find unused pool sessions (session_id starts with "pool-")
            pool.values()
                .find(|info| info.session_id.starts_with("pool-"))
                .map(|info| info.session_id.clone())
        };

        if let Some(session_id) = unused_session_id {
            // Rename this pooled session to a unique session
            let timestamp_nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let new_session_id = format!("session-{timestamp_nanos}");
            self.rename_session(&session_id, &new_session_id).await?;
            Ok(new_session_id)
        } else {
            // No pooled sessions available, create a new one
            let timestamp_nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let new_session_id = format!("session-{timestamp_nanos}");
            self.create_session_workspace_async(&new_session_id).await?;
            Ok(new_session_id)
        }
    }

    /// Rename a session in the pool
    async fn rename_session(
        &self,
        old_session_id: &str,
        new_session_id: &str,
    ) -> Result<(), String> {
        // Extract session info from pool first
        let (old_path, mut session_info) = {
            let mut pool = self
                .workspace_pool
                .write()
                .map_err(|e| format!("Failed to write workspace pool: {e}"))?;

            if let Some(mut session_info) = pool.remove(old_session_id) {
                let old_path = session_info.workspace_path.clone();
                session_info.session_id = new_session_id.to_string();
                session_info.last_accessed = Instant::now();
                (old_path, session_info)
            } else {
                return Ok(()); // Session not found, nothing to rename
            }
        };

        // Perform async operation without holding lock
        let new_path = old_path
            .parent()
            .ok_or("Invalid workspace path")?
            .join(new_session_id);

        tokio::fs::rename(&old_path, &new_path)
            .await
            .map_err(|e| format!("Failed to rename workspace directory: {e}"))?;

        session_info.workspace_path = new_path;

        // Reacquire lock to insert updated session info
        {
            let mut pool = self
                .workspace_pool
                .write()
                .map_err(|e| format!("Failed to write workspace pool: {e}"))?;
            pool.insert(new_session_id.to_string(), session_info);
        }

        info!("Renamed session '{old_session_id}' to '{new_session_id}'");

        Ok(())
    }

    /// Set a workspace override path for a session
    pub async fn set_workspace_override(
        &self,
        session_id: &str,
        override_path: PathBuf,
    ) -> Result<(), String> {
        let mut pool = self
            .workspace_pool
            .write()
            .map_err(|e| format!("Failed to write workspace pool: {e}"))?;

        if let Some(session_info) = pool.get_mut(session_id) {
            session_info.workspace_override = Some(override_path);
            session_info.last_accessed = Instant::now();
            Ok(())
        } else {
            Err(format!("Session '{session_id}' not found"))
        }
    }

    /// Remove a workspace override path for a session
    pub async fn remove_workspace_override(&self, session_id: &str) -> Result<(), String> {
        let mut pool = self
            .workspace_pool
            .write()
            .map_err(|e| format!("Failed to write workspace pool: {e}"))?;

        if let Some(session_info) = pool.get_mut(session_id) {
            session_info.workspace_override = None;
            session_info.last_accessed = Instant::now();
            Ok(())
        } else {
            Err(format!("Session '{session_id}' not found"))
        }
    }

    /// Register a workspace override path for a session before it is created
    pub async fn register_session_override(
        &self,
        session_id: &str,
        override_path: PathBuf,
    ) -> Result<(), String> {
        // Prepare default path and ensure it exists BEFORE taking the write lock.
        // This is necessary because spawn_blocking().await cannot be called while holding
        // a non-Send RwLockWriteGuard (pool).
        let directory_service = self.directory_service.clone();
        let sid = session_id.to_string();
        let default_path = tokio::task::spawn_blocking(move || {
            directory_service.get_workspace_dir(&sid)
        })
        .await
        .map_err(|e| format!("Failed to compute workspace path: {e}"))?;

        let mut pool = self
            .workspace_pool
            .write()
            .map_err(|e| format!("Failed to write workspace pool: {e}"))?;

        // Check if session already exists
        if let Some(session_info) = pool.get_mut(session_id) {
            // Update existing override
            session_info.workspace_override = Some(override_path);
            session_info.last_accessed = Instant::now();
        } else {
            // Register new override for future session
            let workspace_info = SessionWorkspaceInfo {
                session_id: session_id.to_string(),
                workspace_path: default_path,
                workspace_override: Some(override_path),
                created_at: Instant::now(),
                last_accessed: Instant::now(),
                is_template: false,
            };
            pool.insert(session_id.to_string(), workspace_info);
        }

        info!("Registered workspace override for session '{}'", session_id);
        Ok(())
    }

    /// Clean up old or unused sessions
    pub async fn cleanup_old_sessions(
        &self,
        max_age_hours: u64,
        keep_recent: usize,
    ) -> Result<usize, String> {
        let max_age = Duration::from_secs(max_age_hours * 3600);
        let now = Instant::now();
        let mut sessions_to_remove = Vec::new();

        {
            let pool = self
                .workspace_pool
                .read()
                .map_err(|e| format!("Failed to read workspace pool: {e}"))?;

            let mut sorted_sessions: Vec<_> = pool.values().collect();
            sorted_sessions.sort_by(|a, b| b.last_accessed.cmp(&a.last_accessed));

            // Keep the most recent sessions, remove older ones
            for (index, session_info) in sorted_sessions.iter().enumerate() {
                if index >= keep_recent && now.duration_since(session_info.last_accessed) > max_age
                {
                    sessions_to_remove.push(session_info.session_id.clone());
                }
            }
        }

        let mut removed_count = 0;
        for session_id in sessions_to_remove {
            if let Ok(()) = self.remove_session(&session_id).await {
                removed_count += 1;
            }
        }

        info!("Cleaned up {removed_count} old sessions");
        Ok(removed_count)
    }

    /// Remove a specific session
    pub async fn remove_session(&self, session_id: &str) -> Result<(), String> {
        // Remove session from pool, returning error if not found
        {
            let mut pool = self
                .workspace_pool
                .write()
                .map_err(|e| format!("Failed to write workspace pool: {e}"))?;

            if pool.remove(session_id).is_none() {
                return Err(format!("Session '{session_id}' not found in pool"));
            }
        }

        // Remove the workspace directory via directory service
        self.directory_service.remove_workspace(session_id).await?;

        info!("Removed session '{session_id}' and its workspace");
        Ok(())
    }

    /// Get session statistics
    pub fn get_session_stats(&self) -> Result<SessionStats, String> {
        let pool = self
            .workspace_pool
            .read()
            .map_err(|e| format!("Failed to read workspace pool: {e}"))?;

        let now = Instant::now();
        let total_sessions = pool.len();
        let active_sessions = pool
            .values()
            .filter(|info| now.duration_since(info.last_accessed) < Duration::from_secs(3600))
            .count();

        let pool_sessions = pool
            .values()
            .filter(|info| info.session_id.starts_with("pool-"))
            .count();

        Ok(SessionStats {
            total_sessions,
            active_sessions,
            pool_sessions,
        })
    }
}
