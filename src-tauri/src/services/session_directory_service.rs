use std::path::{Path, PathBuf};
use std::fs;
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug)]
pub struct SessionDirectoryService {
    base_data_dir: PathBuf,
    template_workspace: Arc<RwLock<Option<PathBuf>>>,
}

impl SessionDirectoryService {
    pub fn new(base_data_dir: PathBuf) -> Result<Self, String> {
        // Create base directory structure
        fs::create_dir_all(base_data_dir.join("workspaces"))
            .map_err(|e| format!("Failed to create workspaces directory: {e}"))?;

        fs::create_dir_all(base_data_dir.join("workspaces").join("templates"))
            .map_err(|e| format!("Failed to create templates directory: {e}"))?;

        fs::create_dir_all(base_data_dir.join("logs"))
            .map_err(|e| format!("Failed to create logs directory: {e}"))?;

        fs::create_dir_all(base_data_dir.join("config"))
            .map_err(|e| format!("Failed to create config directory: {e}"))?;

        fs::create_dir_all(base_data_dir.join("skills"))
            .map_err(|e| format!("Failed to create skills directory: {e}"))?;

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

        Ok(Self {
            base_data_dir,
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

    /// Async session workspace creation
    pub async fn create_session_workspace(&self, session_id: &str) -> Result<PathBuf, String> {
        let session_dir = self.base_data_dir.join("workspaces").join(session_id);

        // Create directory structure asynchronously
        tokio::fs::create_dir_all(&session_dir)
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
                self.copy_template_to_session(&template_path, &session_dir)
                    .await?;
            }
        }

        Ok(session_dir)
    }

    /// Async template copying
    async fn copy_template_to_session(
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
                tokio::fs::copy(&src, &dst)
                    .await
                    .map_err(|e| format!("Failed to copy file {item}: {e}"))?;
            }
        }

        // Create directories asynchronously
        let dirs_to_create = vec!["tmp", "projects", "downloads", "scripts"];
        for dir in dirs_to_create {
            tokio::fs::create_dir_all(session_dir.join(dir))
                .await
                .map_err(|e| format!("Failed to create directory {dir}: {e}"))?;
        }

        Ok(())
    }

    /// Get session workspace directory, ensuring it exists
    pub fn get_workspace_dir(&self, session_id: &str) -> PathBuf {
        let workspace_dir = self.base_data_dir.join("workspaces").join(session_id);

        // Ensure directory exists if we are calculating it
        if !workspace_dir.exists() {
             let _ = fs::create_dir_all(&workspace_dir);
        }

        workspace_dir
    }

    /// Remove workspace directory
    pub async fn remove_workspace(&self, session_id: &str) -> Result<(), String> {
        let workspace_dir = self.base_data_dir.join("workspaces").join(session_id);

        if workspace_dir.exists() {
            tokio::fs::remove_dir_all(&workspace_dir)
                .await
                .map_err(|e| format!("Failed to remove workspace directory: {e}"))?;
        }
        Ok(())
    }

    pub fn get_base_data_dir(&self) -> &PathBuf {
        &self.base_data_dir
    }

    pub fn get_logs_dir(&self) -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Some(local_data) = dirs::data_local_dir() {
                return local_data.join("com.fritzprix.libragent").join("logs");
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(home) = dirs::home_dir() {
                return home
                    .join("Library")
                    .join("Logs")
                    .join("com.fritzprix.libragent");
            }
        }

        // Fallback for Linux or if platform specific dirs fail
        self.base_data_dir.join("logs")
    }
}
