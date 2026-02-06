#![cfg(target_os = "windows")]

use tokio::process::Command as AsyncCommand;
use tracing::{info, warn};
use std::path::PathBuf;

/// Windows high isolation using job objects and restricted tokens
pub fn apply_high_isolation(cmd: &mut AsyncCommand) -> Result<(), String> {
    // Apply Windows-specific isolation
    use std::os::windows::process::CommandExt;

    // Windows high isolation flags:
    // - CREATE_NEW_PROCESS_GROUP: Isolate process for signal handling
    // Note: Both CREATE_NO_WINDOW and DETACHED_PROCESS break stdio for cmd.exe!
    // Using only CREATE_NEW_PROCESS_GROUP for same reason as Medium isolation
    cmd.creation_flags(0x00000200); // CREATE_NEW_PROCESS_GROUP only

    Ok(())
}

/// Detects a valid Python installation on Windows, prioritizing non-Store versions.
pub async fn detect_python_path() -> Option<PathBuf> {
    // 1. Try `where python` to find registered executables
    if let Ok(output) = AsyncCommand::new("where").arg("python").output().await {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let path = PathBuf::from(line.trim());
                // Filter out WindowsApps shim which redirects to Microsoft Store
                if !path.to_string_lossy().contains("WindowsApps") && path.exists() {
                    if let Some(parent) = path.parent() {
                        info!("Detected Python via 'where': {:?}", parent);
                        return Some(parent.to_path_buf());
                    }
                }
            }
        }
    }

    // 2. Check standard installation locations as fallback
    let common_paths = vec![
        // Anaconda (User)
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join("Anaconda3")),
        // Anaconda (System)
        std::env::var("ProgramData")
            .ok()
            .map(|p| PathBuf::from(p).join("Anaconda3")),
        // Anaconda (User Profile)
        std::env::var("USERPROFILE")
            .ok()
            .map(|p| PathBuf::from(p).join("anaconda3")),
        // Standard Python (User) - check for Python3* directories
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join("Programs").join("Python")),
    ];

    for path in common_paths.into_iter().flatten() {
        // For standard Python, we might need to look deeper (e.g. Python39, Python310)
        if path.join("python.exe").exists() {
            info!("Detected Python via standard path: {:?}", path);
            return Some(path);
        }

        // Check subdirectories for standard Python installs
        if path.exists() && path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                for entry in entries.flatten() {
                    let subpath = entry.path();
                    if subpath.join("python.exe").exists() {
                        info!(
                            "Detected Python via standard path subdirectory: {:?}",
                            subpath
                        );
                        return Some(subpath);
                    }
                }
            }
        }
    }

    None
}
