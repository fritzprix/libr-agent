use std::path::PathBuf;

use tokio::process::Command as AsyncCommand;
use tracing::info;

/// Detect a valid Python installation on Windows, prioritizing non-Store versions.
pub async fn detect_python_path() -> Option<PathBuf> {
    if let Some(path) = detect_python_from_where().await {
        return Some(path);
    }

    let found_path = tokio::task::spawn_blocking(search_standard_python_locations)
        .await
        .unwrap_or(None);

    if let Some(path) = &found_path {
        info!("Detected Python via standard path search: {:?}", path);
    }

    found_path
}

async fn detect_python_from_where() -> Option<PathBuf> {
    let mut cmd = AsyncCommand::new("where");
    crate::utils::env::apply_isolated_env_async(&mut cmd);
    cmd.arg("python");

    let output = cmd.output().await.ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let path = PathBuf::from(line.trim());
        // Filter out WindowsApps shim which redirects to Microsoft Store.
        if !path.to_string_lossy().contains("WindowsApps") && path.exists() {
            if let Some(parent) = path.parent() {
                let detected = parent.to_path_buf();
                info!("Detected Python via 'where': {:?}", detected);
                return Some(detected);
            }
        }
    }

    None
}

fn search_standard_python_locations() -> Option<PathBuf> {
    for path in standard_python_roots().into_iter().flatten() {
        if path.join("python.exe").exists() {
            return Some(path);
        }

        // Standard per-user installs may live one level below `.../Programs/Python`.
        if path.exists() && path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                for entry in entries.flatten() {
                    let subpath = entry.path();
                    if subpath.join("python.exe").exists() {
                        return Some(subpath);
                    }
                }
            }
        }
    }

    None
}

fn standard_python_roots() -> Vec<Option<PathBuf>> {
    vec![
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|path| PathBuf::from(path).join("Anaconda3")),
        std::env::var("ProgramData")
            .ok()
            .map(|path| PathBuf::from(path).join("Anaconda3")),
        std::env::var("USERPROFILE")
            .ok()
            .map(|path| PathBuf::from(path).join("anaconda3")),
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|path| PathBuf::from(path).join("Programs").join("Python")),
    ]
}
