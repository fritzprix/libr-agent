use super::AgentService;
use crate::session::get_session_manager;

/// Returns true if the path points to a restricted system directory that agents
/// should not be allowed to use as a workspace.
pub fn is_restricted_system_path(path: &std::path::Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        let restricted_prefixes = [
            "c:\\windows",
            "c:\\program files",
            "c:\\program files (x86)",
            "c:\\programdata",
            "c:\\system volume information",
        ];

        let path_components: Vec<_> = path.components().collect();

        for prefix in &restricted_prefixes {
            let prefix_components: Vec<_> = std::path::Path::new(prefix).components().collect();

            if path_components.len() < prefix_components.len() {
                continue;
            }

            let mut matches = true;
            for (path_component, prefix_component) in
                path_components.iter().zip(prefix_components.iter())
            {
                use std::path::{Component, Prefix};

                let path_disk = match path_component {
                    Component::Prefix(prefix) => match prefix.kind() {
                        Prefix::Disk(disk) | Prefix::VerbatimDisk(disk) => {
                            Some(disk.to_ascii_lowercase())
                        }
                        _ => None,
                    },
                    _ => None,
                };

                let prefix_disk = match prefix_component {
                    Component::Prefix(prefix) => match prefix.kind() {
                        Prefix::Disk(disk) | Prefix::VerbatimDisk(disk) => {
                            Some(disk.to_ascii_lowercase())
                        }
                        _ => None,
                    },
                    _ => None,
                };

                if let (Some(left), Some(right)) = (path_disk, prefix_disk) {
                    if left != right {
                        matches = false;
                        break;
                    }
                } else {
                    let path_str = path_component.as_os_str().to_string_lossy().to_lowercase();
                    let prefix_str = prefix_component
                        .as_os_str()
                        .to_string_lossy()
                        .to_lowercase();
                    if path_str != prefix_str {
                        matches = false;
                        break;
                    }
                }
            }

            if matches {
                return true;
            }
        }

        false
    }

    #[cfg(not(target_os = "windows"))]
    {
        let path_lower = std::path::PathBuf::from(path.to_string_lossy().to_lowercase());

        let restricted_prefixes = [
            "/etc",
            "/sys",
            "/proc",
            "/dev",
            "/run",
            "/boot",
            "/bin",
            "/sbin",
            "/lib",
            "/lib64",
            "/usr/bin",
            "/usr/sbin",
            "/usr/lib",
            "/system",
            "/library",
        ];

        restricted_prefixes
            .iter()
            .any(|prefix| path_lower.starts_with(prefix))
    }
}

impl AgentService {
    /// Validates a workspace override path and registers it for the given session.
    ///
    /// The path must be absolute, must exist, and must be a directory.
    pub async fn validate_and_register_workspace_override(
        path_str: &str,
        session_id: &str,
    ) -> Result<(), String> {
        let Ok(session_manager) = get_session_manager() else {
            log::warn!("Failed to get session manager for workspace override");
            return Ok(());
        };
        let path = std::path::PathBuf::from(path_str);
        if !path.is_absolute() {
            return Err("Workspace path must be absolute".to_string());
        }

        if is_restricted_system_path(&path) {
            return Err(format!(
                "Workspace path '{}' is a restricted system directory and cannot be used as an agent workspace",
                path_str
            ));
        }

        match tokio::fs::metadata(&path).await {
            Ok(metadata) => {
                if !metadata.is_dir() {
                    return Err("Workspace path must be a directory".to_string());
                }
            }
            Err(err) => {
                return Err(format!("Workspace path is not accessible: {}", err));
            }
        }
        session_manager
            .register_session_override(session_id, path)
            .await
    }
}
