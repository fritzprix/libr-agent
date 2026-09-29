use crate::browser_profiles::{
    delete_imported_browser_profile, import_chrome_profiles, list_imported_profiles,
    BrowserProfileInfo, ImportReport,
};
use log::info;

/// Lists imported browser profiles (names/labels only — no filesystem paths).
#[tauri::command]
pub async fn list_browser_profiles() -> Result<Vec<BrowserProfileInfo>, String> {
    list_imported_profiles()
}

/// One-click import: discover installed browser profiles and copy into app-local storage.
#[tauri::command]
pub async fn import_browser_profiles() -> Result<ImportReport, String> {
    info!("Command: import_browser_profiles");
    let report = tokio::task::spawn_blocking(import_chrome_profiles)
        .await
        .map_err(|e| format!("Browser profile import task failed: {e}"))??;
    info!(
        "Browser profile import finished: imported={:?} skipped={:?}",
        report.imported, report.skipped
    );
    Ok(report)
}

/// Remove an imported browser profile and delete its app-local copy.
#[tauri::command]
pub async fn remove_browser_profile(name: String) -> Result<(), String> {
    info!("Command: remove_browser_profile name={name}");
    let name_for_task = name.clone();
    tokio::task::spawn_blocking(move || delete_imported_browser_profile(&name_for_task))
        .await
        .map_err(|e| format!("Browser profile remove task failed: {e}"))?
}
