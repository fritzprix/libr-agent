use crate::services::SecureFileManager;
use log::{error, info, warn};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use tokio::fs as async_fs;
use tokio::time::{Duration, Instant};

static SESSION_MANAGER: OnceLock<SessionManager> = OnceLock::new();

#[derive(Clone, Debug, serde::Serialize)]
pub struct SessionWorkspaceInfo {
    pub session_id: String,
    #[serde(serialize_with = "serialize_pathbuf")]
    pub workspace_path: PathBuf,
    #[serde(serialize_with = "serialize_instant")]
    pub created_at: Instant,
    #[serde(serialize_with = "serialize_instant")]
    pub last_accessed: Instant,
    pub is_template: bool,
}

fn serialize_pathbuf<S>(path: &Path, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&path.to_string_lossy())
}

fn serialize_instant<S>(instant: &Instant, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let duration_since_start = instant.elapsed();
    serializer.serialize_u64(duration_since_start.as_secs())
}

#[derive(Clone, Debug)]
pub struct SessionManager {
    current_session: Arc<RwLock<Option<String>>>,
    base_data_dir: PathBuf,
    workspace_pool: Arc<RwLock<HashMap<String, SessionWorkspaceInfo>>>,
    template_workspace: Arc<RwLock<Option<PathBuf>>>,
}

impl SessionManager {
    pub fn new() -> Result<Self, String> {
        let base_data_dir = dirs::data_dir()
            .ok_or_else(|| "Failed to get system data directory".to_string())?
            .join("com.fritzprix.synapticflow");

        // Create base directory structure
        fs::create_dir_all(base_data_dir.join("workspaces"))
            .map_err(|e| format!("Failed to create workspaces directory: {e}"))?;

        fs::create_dir_all(base_data_dir.join("workspaces").join("templates"))
            .map_err(|e| format!("Failed to create templates directory: {e}"))?;

        fs::create_dir_all(base_data_dir.join("logs"))
            .map_err(|e| format!("Failed to create logs directory: {e}"))?;

        fs::create_dir_all(base_data_dir.join("config"))
            .map_err(|e| format!("Failed to create config directory: {e}"))?;

        // Create default workspace
        let default_workspace = base_data_dir.join("workspaces").join("default");
        fs::create_dir_all(&default_workspace)
            .map_err(|e| format!("Failed to create default workspace: {e}"))?;

        // Initialize template workspace
        let template_workspace = base_data_dir
            .join("workspaces")
            .join("templates")
            .join("base");
        fs::create_dir_all(&template_workspace)
            .map_err(|e| format!("Failed to create template workspace: {e}"))?;

        // Create basic template structure
        Self::setup_template_workspace(&template_workspace)?;

        info!("SessionManager initialized with base directory: {base_data_dir:?}");

        Ok(Self {
            current_session: Arc::new(RwLock::new(None)),
            base_data_dir,
            workspace_pool: Arc::new(RwLock::new(HashMap::new())),
            template_workspace: Arc::new(RwLock::new(Some(template_workspace))),
        })
    }

    /// Setup basic template workspace structure
    fn setup_template_workspace(template_path: &Path) -> Result<(), String> {
        // Create common directories that sessions might need
        let dirs_to_create = vec!["tmp", "projects", "downloads", "scripts"];

        for dir in dirs_to_create {
            fs::create_dir_all(template_path.join(dir))
                .map_err(|e| format!("Failed to create template directory {dir}: {e}"))?;
        }

        // Create a basic welcome script
        let welcome_script = r#"#!/bin/bash
echo "Welcome to your isolated workspace!"
echo "Session ID: $(basename "$PWD")"
echo "Workspace: $PWD"
echo "Available tools: python3, typescript/deno, shell commands"
"#;

        fs::write(template_path.join("welcome.sh"), welcome_script)
            .map_err(|e| format!("Failed to create welcome script: {e}"))?;

        // Make script executable on Unix systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(template_path.join("welcome.sh"))
                .map_err(|e| format!("Failed to get script metadata: {e}"))?
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(template_path.join("welcome.sh"), perms)
                .map_err(|e| format!("Failed to set script permissions: {e}"))?;
        }

        Ok(())
    }

    pub fn set_session(&self, session_id: String) -> Result<(), String> {
        info!("Setting session to: {session_id}");

        // Check if session already exists in pool
        let workspace_info = {
            let pool = self
                .workspace_pool
                .read()
                .map_err(|e| format!("Failed to read workspace pool: {e}"))?;
            pool.get(&session_id).cloned()
        };

        let _session_dir = if let Some(mut info) = workspace_info {
            // Update last accessed time
            info.last_accessed = Instant::now();
            {
                let mut pool = self
                    .workspace_pool
                    .write()
                    .map_err(|e| format!("Failed to write workspace pool: {e}"))?;
                pool.insert(session_id.clone(), info.clone());
            }
            info.workspace_path
        } else {
            // Create new session workspace quickly
            self.create_session_workspace_fast(&session_id)?
        };

        // Update current session
        {
            let mut current = self
                .current_session
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {e}"))?;
            *current = Some(session_id.clone());
        }

        info!("Session set successfully: {session_id}");
        Ok(())
    }

    /// Fast session creation using template workspace
    fn create_session_workspace_fast(&self, session_id: &str) -> Result<PathBuf, String> {
        let session_dir = self.base_data_dir.join("workspaces").join(session_id);

        // Create directory structure first
        fs::create_dir_all(&session_dir)
            .map_err(|e| format!("Failed to create session directory '{session_id}': {e}"))?;

        // Copy from template if available (fast copy-on-write approach)
        let template_path_option = {
            if let Ok(template_lock) = self.template_workspace.read() {
                template_lock.as_ref().cloned()
            } else {
                None
            }
        };

        if let Some(template_path) = template_path_option {
            if template_path.exists() {
                self.copy_template_to_session(&template_path, &session_dir)?;
            }
        }

        // Add to workspace pool
        let workspace_info = SessionWorkspaceInfo {
            session_id: session_id.to_string(),
            workspace_path: session_dir.clone(),
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

    /// Efficiently copy template to new session workspace
    fn copy_template_to_session(
        &self,
        template_path: &Path,
        session_dir: &Path,
    ) -> Result<(), String> {
        use std::fs;

        // Copy essential directories and files
        let items_to_copy = vec!["tmp", "projects", "downloads", "scripts", "welcome.sh"];

        for item in items_to_copy {
            let src = template_path.join(item);
            let dst = session_dir.join(item);

            if src.exists() {
                if src.is_dir() {
                    // For directories, create and copy contents
                    fs::create_dir_all(&dst)
                        .map_err(|e| format!("Failed to create directory {item}: {e}"))?;

                    // Copy directory contents recursively but efficiently
                    self.copy_dir_contents(&src, &dst)?;
                } else {
                    // For files, simple copy
                    fs::copy(&src, &dst).map_err(|e| format!("Failed to copy file {item}: {e}"))?;
                }
            }
        }

        Ok(())
    }

    /// Efficiently copy directory contents
    #[allow(clippy::only_used_in_recursion)]
    fn copy_dir_contents(&self, src: &Path, dst: &Path) -> Result<(), String> {
        use std::fs;

        for entry in fs::read_dir(src).map_err(|e| format!("Failed to read directory: {e}"))? {
            let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if src_path.is_dir() {
                fs::create_dir_all(&dst_path)
                    .map_err(|e| format!("Failed to create directory: {e}"))?;
                self.copy_dir_contents(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path).map_err(|e| format!("Failed to copy file: {e}"))?;
            }
        }

        Ok(())
    }

    /// Async version of session switching (non-blocking)
    pub async fn set_session_async(&self, session_id: String) -> Result<(), String> {
        info!("Setting session asynchronously to: {session_id}");

        // Check if session already exists in pool
        let workspace_info = {
            let pool = self
                .workspace_pool
                .read()
                .map_err(|e| format!("Failed to read workspace pool: {e}"))?;
            pool.get(&session_id).cloned()
        };

        let _session_dir = if let Some(mut info) = workspace_info {
            // Update last accessed time
            info.last_accessed = Instant::now();
            {
                let mut pool = self
                    .workspace_pool
                    .write()
                    .map_err(|e| format!("Failed to write workspace pool: {e}"))?;
                pool.insert(session_id.clone(), info.clone());
            }
            info.workspace_path
        } else {
            // Create new session workspace asynchronously
            self.create_session_workspace_async(&session_id).await?
        };

        // Update current session
        {
            let mut current = self
                .current_session
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {e}"))?;
            *current = Some(session_id.clone());
        }

        info!("Session set asynchronously: {session_id}");
        Ok(())
    }

    /// Async session workspace creation
    async fn create_session_workspace_async(&self, session_id: &str) -> Result<PathBuf, String> {
        let session_dir = self.base_data_dir.join("workspaces").join(session_id);

        // Create directory structure asynchronously
        async_fs::create_dir_all(&session_dir)
            .await
            .map_err(|e| format!("Failed to create session directory '{session_id}': {e}"))?;

        // Copy from template if available (async)
        let template_path_option = {
            if let Ok(template_lock) = self.template_workspace.read() {
                template_lock.as_ref().cloned()
            } else {
                None
            }
        };

        if let Some(template_path) = template_path_option {
            if template_path.exists() {
                self.copy_template_to_session_async(&template_path, &session_dir)
                    .await?;
            }
        }

        // Add to workspace pool
        let workspace_info = SessionWorkspaceInfo {
            session_id: session_id.to_string(),
            workspace_path: session_dir.clone(),
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

    /// Async template copying
    async fn copy_template_to_session_async(
        &self,
        template_path: &Path,
        session_dir: &Path,
    ) -> Result<(), String> {
        // Copy essential files asynchronously
        let items_to_copy = vec!["welcome.sh"];

        for item in items_to_copy {
            let src = template_path.join(item);
            let dst = session_dir.join(item);

            if src.exists() && src.is_file() {
                async_fs::copy(&src, &dst)
                    .await
                    .map_err(|e| format!("Failed to copy file {item}: {e}"))?;
            }
        }

        // Create directories asynchronously
        let dirs_to_create = vec!["tmp", "projects", "downloads", "scripts"];
        for dir in dirs_to_create {
            async_fs::create_dir_all(session_dir.join(dir))
                .await
                .map_err(|e| format!("Failed to create directory {dir}: {e}"))?;
        }

        Ok(())
    }

    pub fn get_current_session(&self) -> Option<String> {
        match self.current_session.read() {
            Ok(session) => session.clone(),
            Err(e) => {
                error!("Failed to read current session: {e}");
                None
            }
        }
    }

    pub fn get_session_workspace_dir(&self) -> PathBuf {
        let session_id = self
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());

        let workspace_dir = self.base_data_dir.join("workspaces").join(session_id);

        // Ensure directory exists
        if let Err(e) = fs::create_dir_all(&workspace_dir) {
            warn!("Failed to create workspace directory {workspace_dir:?}: {e}");
            // Fallback to default workspace
            let default_dir = self.base_data_dir.join("workspaces").join("default");
            if let Err(e) = fs::create_dir_all(&default_dir) {
                error!("Failed to create default workspace: {e}");
            }
            return default_dir;
        }

        workspace_dir
    }

    pub fn get_base_data_dir(&self) -> &PathBuf {
        &self.base_data_dir
    }

    pub fn get_logs_dir(&self) -> PathBuf {
        self.base_data_dir.join("logs")
    }

    pub fn list_sessions(&self) -> Result<Vec<String>, String> {
        let workspaces_dir = self.base_data_dir.join("workspaces");

        let entries = fs::read_dir(&workspaces_dir)
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

        async_fs::rename(&old_path, &new_path)
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
        let workspace_path = {
            let mut pool = self
                .workspace_pool
                .write()
                .map_err(|e| format!("Failed to write workspace pool: {e}"))?;

            if let Some(session_info) = pool.remove(session_id) {
                session_info.workspace_path
            } else {
                return Err(format!("Session '{session_id}' not found in pool"));
            }
        };

        // Remove the workspace directory
        if workspace_path.exists() {
            async_fs::remove_dir_all(&workspace_path)
                .await
                .map_err(|e| format!("Failed to remove workspace directory: {e}"))?;
        }

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
            current_session: self.get_current_session(),
        })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionStats {
    pub total_sessions: usize,
    pub active_sessions: usize,
    pub pool_sessions: usize,
    pub current_session: Option<String>,
}

impl SessionManager {
    /// Get a SecureFileManager instance configured for the current session's workspace
    pub fn get_file_manager(&self) -> Arc<SecureFileManager> {
        let workspace_dir = self.get_session_workspace_dir();
        Arc::new(SecureFileManager::new_with_base_dir(workspace_dir))
    }
}

pub fn get_session_manager() -> Result<&'static SessionManager, String> {
    SESSION_MANAGER.get_or_init(|| {
        SessionManager::new().unwrap_or_else(|e| {
            error!("Failed to initialize SessionManager: {e}");
            // Create fallback session manager with temp directory
            let temp_base = std::env::temp_dir().join("com.fritzprix.synapticflow");
            let _ = std::fs::create_dir_all(temp_base.join("workspaces").join("default"));
            let _ = std::fs::create_dir_all(temp_base.join("workspaces").join("templates"));
            let _ = std::fs::create_dir_all(temp_base.join("logs"));
            let _ = std::fs::create_dir_all(temp_base.join("config"));

            SessionManager {
                current_session: Arc::new(RwLock::new(None)),
                base_data_dir: temp_base,
                workspace_pool: Arc::new(RwLock::new(HashMap::new())),
                template_workspace: Arc::new(RwLock::new(None)),
            }
        })
    });
    Ok(SESSION_MANAGER.get().unwrap())
}
