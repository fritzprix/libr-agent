use log::{error, info, warn};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};

static SESSION_MANAGER: OnceLock<SessionManager> = OnceLock::new();

pub struct SessionManager {
    current_session: Arc<RwLock<Option<String>>>,
    base_data_dir: PathBuf,
}

impl SessionManager {
    pub fn new() -> Result<Self, String> {
        let base_data_dir = dirs::data_dir()
            .ok_or_else(|| "Failed to get system data directory".to_string())?
            .join("com.synaptic-flow.app");

        // Create base directory structure
        fs::create_dir_all(base_data_dir.join("workspaces"))
            .map_err(|e| format!("Failed to create workspaces directory: {e}"))?;

        fs::create_dir_all(base_data_dir.join("logs"))
            .map_err(|e| format!("Failed to create logs directory: {e}"))?;

        fs::create_dir_all(base_data_dir.join("config"))
            .map_err(|e| format!("Failed to create config directory: {e}"))?;

        // Create default workspace
        let default_workspace = base_data_dir.join("workspaces").join("default");
        fs::create_dir_all(&default_workspace)
            .map_err(|e| format!("Failed to create default workspace: {e}"))?;

        info!("SessionManager initialized with base directory: {base_data_dir:?}");

        Ok(Self {
            current_session: Arc::new(RwLock::new(None)),
            base_data_dir,
        })
    }

    pub fn set_session(&self, session_id: String) -> Result<(), String> {
        info!("Setting session to: {session_id}");

        // Create session workspace directory if it doesn't exist
        let session_dir = self.base_data_dir.join("workspaces").join(&session_id);
        fs::create_dir_all(&session_dir)
            .map_err(|e| format!("Failed to create session directory '{session_id}': {e}"))?;

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
                    sessions.push(name.to_string());
                }
            }
        }

        sessions.sort();
        Ok(sessions)
    }
}

pub fn get_session_manager() -> Result<&'static SessionManager, String> {
    SESSION_MANAGER.get_or_init(|| {
        SessionManager::new().unwrap_or_else(|e| {
            error!("Failed to initialize SessionManager: {e}");
            // Create fallback session manager with temp directory
            let temp_base = std::env::temp_dir().join("com.synaptic-flow.app");
            let _ = std::fs::create_dir_all(temp_base.join("workspaces").join("default"));
            let _ = std::fs::create_dir_all(temp_base.join("logs"));
            let _ = std::fs::create_dir_all(temp_base.join("config"));

            SessionManager {
                current_session: Arc::new(RwLock::new(None)),
                base_data_dir: temp_base,
            }
        })
    });
    Ok(SESSION_MANAGER.get().unwrap())
}
