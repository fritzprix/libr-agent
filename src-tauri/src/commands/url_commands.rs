//! URL handling commands
//!
//! This module contains commands for handling external URLs.

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
