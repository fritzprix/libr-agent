//! URL handling commands
//!
//! This module contains commands for handling external URLs and local paths.

/// Opens a URL in the user's default external web browser.
///
/// This command includes a security check to ensure only `http` or `https` URLs are opened.
#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), String> {
    // URL validation
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Only HTTP/HTTPS URLs are allowed".to_string());
    }

    // Use tauri-plugin-opener to open URL in external browser
    tauri_plugin_opener::open_url(&url, None::<&str>)
        .map_err(|e| format!("Failed to open URL: {e}"))?;

    Ok(())
}

/// Opens a local file or directory with the system default application.
///
/// Only absolute existing paths are allowed (e.g. paths returned from save dialogs).
#[tauri::command]
pub async fn open_path_with_default_app(path: String) -> Result<(), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("path is required".to_string());
    }

    let path_buf = std::path::PathBuf::from(trimmed);
    if !path_buf.is_absolute() {
        return Err("Only absolute paths are allowed".to_string());
    }
    if !path_buf.exists() {
        return Err(format!("Path does not exist: {trimmed}"));
    }

    tauri_plugin_opener::open_path(trimmed, None::<&str>)
        .map_err(|e| format!("Failed to open path: {e}"))?;

    Ok(())
}
