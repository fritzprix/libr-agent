use crate::browser_profiles::{
    delete_imported_browser_profile, import_browser_profiles as import_browser_profiles_impl,
    list_discoverable_browser_profiles as list_discoverable_browser_profiles_impl,
    list_imported_profiles, list_running_browsers_for_import, list_running_browsers_for_profiles,
    open_imported_profile_for_signin as open_imported_profile_for_signin_impl,
    quit_browsers_for_profile_import as quit_browsers_for_profile_import_impl,
    set_default_browser_profile as set_default_browser_profile_impl, BrowserProfileInfo,
    DiscoverableBrowserProfile, ImportReport, QuitBrowsersReport,
};
use log::info;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserProfileImportReadiness {
    /// True when no targeted Chromium-family processes that lock cookies were detected.
    pub ready: bool,
    /// Friendly names such as "Chrome", "Edge" — scoped to the selected profiles when provided.
    pub running_browsers: Vec<String>,
}

/// Lists imported browser profiles (names/labels only — no filesystem paths).
#[tauri::command]
pub async fn list_browser_profiles() -> Result<Vec<BrowserProfileInfo>, String> {
    list_imported_profiles()
}

/// Lists installed browser Default profiles available to import (no filesystem paths).
#[tauri::command]
pub async fn list_discoverable_browser_profiles() -> Result<Vec<DiscoverableBrowserProfile>, String>
{
    tokio::task::spawn_blocking(list_discoverable_browser_profiles_impl)
        .await
        .map_err(|e| format!("Discoverable browser list failed: {e}"))
}

/// Check whether cookie import can succeed for the selected profiles (or all if omitted).
#[tauri::command]
pub async fn check_browser_profile_import_ready(
    profile_names: Option<Vec<String>>,
) -> Result<BrowserProfileImportReadiness, String> {
    let running_browsers = tokio::task::spawn_blocking(move || match profile_names {
        Some(names) if !names.is_empty() => list_running_browsers_for_profiles(&names),
        _ => list_running_browsers_for_import(),
    })
    .await
    .map_err(|e| format!("Browser readiness check failed: {e}"))?;
    Ok(BrowserProfileImportReadiness {
        ready: running_browsers.is_empty(),
        running_browsers,
    })
}

/// Quit locking browsers after explicit Settings consent.
/// When `profileNames` is set, only quit browsers needed for those profiles.
#[tauri::command]
pub async fn quit_browsers_for_profile_import(
    user_confirmed: bool,
    profile_names: Option<Vec<String>>,
) -> Result<QuitBrowsersReport, String> {
    info!(
        "Command: quit_browsers_for_profile_import confirmed={user_confirmed} profiles={profile_names:?}"
    );
    tokio::task::spawn_blocking(move || {
        quit_browsers_for_profile_import_impl(user_confirmed, profile_names)
    })
    .await
    .map_err(|e| format!("Browser quit task failed: {e}"))?
}

/// Import selected browser profiles (or all Default profiles when `profileNames` omitted).
#[tauri::command]
pub async fn import_browser_profiles(
    profile_names: Option<Vec<String>>,
    preferred_default: Option<String>,
) -> Result<ImportReport, String> {
    info!(
        "Command: import_browser_profiles profiles={profile_names:?} preferred_default={preferred_default:?}"
    );
    let report = tokio::task::spawn_blocking(move || {
        import_browser_profiles_impl(profile_names, preferred_default)
    })
    .await
    .map_err(|e| format!("Browser profile import task failed: {e}"))??;
    info!(
        "Browser profile import finished: imported={:?} skipped={:?}",
        report.imported, report.skipped
    );
    Ok(report)
}

/// Choose which imported profile agents use with `use_profile: true`.
#[tauri::command]
pub async fn set_default_browser_profile(name: String) -> Result<(), String> {
    info!("Command: set_default_browser_profile name={name}");
    let name_for_task = name.clone();
    tokio::task::spawn_blocking(move || set_default_browser_profile_impl(&name_for_task))
        .await
        .map_err(|e| format!("Set default browser profile task failed: {e}"))?
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

/// Open the imported Chromium profile in system Chrome for manual Google/site sign-in (no CDP).
#[tauri::command]
pub async fn open_browser_profile_for_signin(name: String) -> Result<(), String> {
    info!("Command: open_browser_profile_for_signin name={name}");
    let name_for_task = name.clone();
    tokio::task::spawn_blocking(move || open_imported_profile_for_signin_impl(&name_for_task))
        .await
        .map_err(|e| format!("Open browser profile for sign-in task failed: {e}"))?
}
