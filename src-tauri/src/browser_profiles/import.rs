use std::path::{Path, PathBuf};

use chrono::Utc;
use log::{info, warn};
use walkdir::WalkDir;

use super::discover::{
    browser_default_priority, discover_browser_profiles, friendly_browser_label,
    DiscoveredBrowserProfile, ProfileEngine,
};
use super::firefox::export_firefox_cookies_to_profile;
use super::registry::{
    imported_profile_user_data_dir, load_registry, remove_imported_profile, save_registry,
    set_default_imported_profile, upsert_imported_profile, ImportedProfile, ImportKind,
};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
    pub warnings: Vec<String>,
    /// Friendly browser names still running **among the import targets** (e.g. "Chrome").
    #[serde(default)]
    pub running_browsers: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuitBrowsersReport {
    /// Browsers we attempted to quit (friendly names).
    pub attempted: Vec<String>,
    /// Still detected after quit + short wait.
    pub still_running: Vec<String>,
    pub ready: bool,
}

/// Installed browser Default profile the user can choose to import (no filesystem paths).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverableBrowserProfile {
    pub name: String,
    pub label: String,
    pub browser_id: String,
    pub browser_label: String,
}

/// Friendly names of browsers that typically lock cookie databases while open.
pub fn list_running_browsers_for_import() -> Vec<String> {
    detect_running_browser_names()
        .into_iter()
        .map(str::to_string)
        .collect()
}

/// Running browsers that matter for the given imported/discoverable profile names only.
pub fn list_running_browsers_for_profiles(profile_names: &[String]) -> Vec<String> {
    // Slug → label needs no filesystem discovery (names are `{browser_id}_default` / `_profile_N`).
    let wanted = friendly_labels_for_profile_names(profile_names, &[]);
    list_running_browsers_for_labels(&wanted)
}

fn list_running_browsers_for_labels(wanted: &[String]) -> Vec<String> {
    if wanted.is_empty() {
        return list_running_browsers_for_import();
    }
    list_running_browsers_for_import()
        .into_iter()
        .filter(|name| wanted.iter().any(|w| w == name))
        .collect()
}

/// Map registry/discoverable slugs to friendly process labels without re-scanning disks
/// when `discovered` is empty — slugs already encode `browser_id`.
fn friendly_labels_for_profile_names(
    profile_names: &[String],
    discovered: &[DiscoveredBrowserProfile],
) -> Vec<String> {
    let mut labels = Vec::new();
    for name in profile_names {
        let browser_id = discovered
            .iter()
            .find(|p| p.name == *name)
            .map(|p| p.browser_id.as_str())
            .or_else(|| browser_id_from_profile_slug(name));
        if let Some(browser_id) = browser_id {
            let label = friendly_browser_label(browser_id).to_string();
            if !labels.contains(&label) {
                labels.push(label);
            }
        }
    }
    labels
}

fn browser_id_from_profile_slug(name: &str) -> Option<&str> {
    if let Some(id) = name.strip_suffix("_default") {
        return (!id.is_empty()).then_some(id);
    }
    if let Some((id, rest)) = name.split_once("_profile_") {
        if !id.is_empty() && !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
            return Some(id);
        }
    }
    None
}

fn friendly_labels_for_discovered(profiles: &[DiscoveredBrowserProfile]) -> Vec<String> {
    let mut labels = Vec::new();
    for profile in profiles {
        let label = friendly_browser_label(&profile.browser_id).to_string();
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    labels
}

/// Profiles available to import on this machine (Default profiles only; no paths).
pub fn list_discoverable_browser_profiles() -> Vec<DiscoverableBrowserProfile> {
    let mut profiles: Vec<_> = discover_browser_profiles()
        .into_iter()
        .filter(|profile| profile.name.ends_with("_default"))
        .map(|profile| DiscoverableBrowserProfile {
            name: profile.name,
            label: profile.label,
            browser_id: profile.browser_id.clone(),
            browser_label: friendly_browser_label(&profile.browser_id).to_string(),
        })
        .collect();
    profiles.sort_by_key(|p| browser_default_priority(&p.browser_id));
    profiles
}

/// Quit detected browsers after explicit user consent (`user_confirmed` must be true).
///
/// When `profile_names` is set, only quit browsers needed for those profiles
/// (e.g. importing Edge must not ask to kill Chrome).
pub fn quit_browsers_for_profile_import(
    user_confirmed: bool,
    profile_names: Option<Vec<String>>,
) -> Result<QuitBrowsersReport, String> {
    if !user_confirmed {
        return Err(
            "Browser quit was not confirmed. Approve the prompt in Settings first.".to_string(),
        );
    }

    // Resolve wanted labels once from slugs — no filesystem discovery on each poll.
    let wanted_labels: Option<Vec<String>> = match &profile_names {
        Some(names) if !names.is_empty() => {
            Some(friendly_labels_for_profile_names(names, &[]))
        }
        _ => None,
    };

    let running_for_scope = || match &wanted_labels {
        Some(wanted) => list_running_browsers_for_labels(wanted),
        None => list_running_browsers_for_import(),
    };

    let before = running_for_scope();
    if before.is_empty() {
        info!("quit_browsers_for_profile_import: no locking browsers detected");
        return Ok(QuitBrowsersReport {
            attempted: vec![],
            still_running: vec![],
            ready: true,
        });
    }

    info!(
        "quit_browsers_for_profile_import: user confirmed quit for {:?}",
        before
    );
    quit_allowlisted_browser_processes(&before);

    std::thread::sleep(std::time::Duration::from_millis(1500));

    let still_running = running_for_scope();
    if !still_running.is_empty() {
        warn!(
            "quit_browsers_for_profile_import: still running after quit: {:?}",
            still_running
        );
        quit_allowlisted_browser_processes(&still_running);
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }

    let still_running = running_for_scope();
    Ok(QuitBrowsersReport {
        attempted: before,
        still_running: still_running.clone(),
        ready: still_running.is_empty(),
    })
}

/// One-click / Settings import: copy Default profiles from each installed browser.
///
/// - Chromium family (Chrome/Edge/Brave/…): full User Data copy (cookies + Local State).
/// - Firefox: cookie export for CDP injection (automation remains Chromium-based).
///
/// `profile_names`: when set, only those discoverable slugs are imported (e.g. `edge_default`).
/// `preferred_default`: registry key to mark as the agent `use_profile` default when imported.
pub fn import_chrome_profiles() -> Result<ImportReport, String> {
    import_browser_profiles(None, None)
}

pub fn import_browser_profiles(
    profile_names: Option<Vec<String>>,
    preferred_default: Option<String>,
) -> Result<ImportReport, String> {
    let mut discovered: Vec<_> = discover_browser_profiles()
        .into_iter()
        .filter(|profile| profile.name.ends_with("_default"))
        .collect();

    if let Some(names) = &profile_names {
        if !names.is_empty() {
            discovered.retain(|profile| names.iter().any(|n| n == &profile.name));
        }
    }

    discovered.sort_by_key(|profile| browser_default_priority(&profile.browser_id));

    let wanted_labels = friendly_labels_for_discovered(&discovered);

    if discovered.is_empty() {
        return Ok(ImportReport {
            imported: vec![],
            skipped: vec![],
            warnings: vec![
                "No matching Chrome/Edge/Brave/Chromium/Firefox Default profiles were found."
                    .to_string(),
            ],
            running_browsers: list_running_browsers_for_labels(&wanted_labels),
        });
    }

    let mut report = import_discovered(&discovered, preferred_default.as_deref())?;
    // Reuse labels from the already-scanned discover list (do not re-walk browser dirs).
    report.running_browsers = list_running_browsers_for_labels(&wanted_labels);
    if !report.running_browsers.is_empty() && !report.skipped.is_empty() {
        report.warnings.insert(
            0,
            format!(
                "Still open (needed for this import): {}. Quit those apps, then try again.",
                report.running_browsers.join(", ")
            ),
        );
    }
    Ok(report)
}

/// Remove an imported profile from the registry and delete its app-local copy.
pub fn delete_imported_browser_profile(name: &str) -> Result<(), String> {
    remove_imported_profile(name)
}

/// Set which imported profile agents use with `use_profile: true`.
pub fn set_default_browser_profile(name: &str) -> Result<(), String> {
    set_default_imported_profile(name)
}

fn import_discovered(
    discovered: &[DiscoveredBrowserProfile],
    preferred_default: Option<&str>,
) -> Result<ImportReport, String> {
    let mut registry = load_registry()?;
    let mut imported = Vec::new();
    let mut skipped = Vec::new();
    let mut warnings = Vec::new();
    let mut made_preferred_default = false;

    for profile in discovered {
        match import_one_profile(profile) {
            Ok((dest_user_data, kind, copy_warnings)) => {
                warnings.extend(copy_warnings);
                let set_as_default = preferred_default == Some(profile.name.as_str())
                    || (preferred_default.is_none()
                        && registry.default_profile.is_none()
                        && !made_preferred_default);
                upsert_imported_profile(
                    &mut registry,
                    profile.name.clone(),
                    ImportedProfile {
                        label: profile.label.clone(),
                        user_data_dir: dest_user_data,
                        source_label: profile.label.clone(),
                        source_browser: profile.browser_id.clone(),
                        import_kind: kind,
                        imported_at: Utc::now(),
                    },
                    set_as_default,
                );
                if set_as_default {
                    made_preferred_default = true;
                }
                imported.push(profile.name.clone());
                info!(
                    "Imported browser profile '{}' ({:?})",
                    profile.name, profile.engine
                );
            }
            Err(error) => {
                let message = format!("Failed to import '{}': {error}", profile.label);
                warn!("{message}");
                warnings.push(message);
                skipped.push(profile.name.clone());
            }
        }
    }

    // If preferred default failed to import but something else succeeded and no default exists,
    // fall back to first imported.
    if registry.default_profile.is_none() {
        if let Some(first) = imported.first() {
            registry.default_profile = Some(first.clone());
        }
    }

    save_registry(&registry)?;

    Ok(ImportReport {
        imported,
        skipped,
        warnings,
        // Caller fills scoped running_browsers from the already-discovered set.
        running_browsers: vec![],
    })
}

fn import_one_profile(
    profile: &DiscoveredBrowserProfile,
) -> Result<(PathBuf, ImportKind, Vec<String>), String> {
    match profile.engine {
        ProfileEngine::ChromiumUserData => {
            let (dest, warnings) = import_chromium_profile_atomically(profile)?;
            Ok((dest, ImportKind::ChromiumUserData, warnings))
        }
        ProfileEngine::FirefoxCookies => {
            let (dest, warnings) = import_firefox_profile_atomically(profile)?;
            Ok((dest, ImportKind::FirefoxCookies, warnings))
        }
    }
}

/// Stage into a temp User Data tree, validate cookies + Local State, then atomically replace.
fn import_chromium_profile_atomically(
    profile: &DiscoveredBrowserProfile,
) -> Result<(PathBuf, Vec<String>), String> {
    let dest_user_data = imported_profile_user_data_dir(&profile.name)?;
    let staging_user_data = dest_user_data.with_file_name(format!(
        "{}.staging-{}",
        profile.name,
        std::process::id()
    ));
    let backup_user_data = dest_user_data.with_file_name(format!("{}.bak", profile.name));

    cleanup_dir_if_exists(&staging_user_data)?;
    cleanup_dir_if_exists(&backup_user_data)?;

    std::fs::create_dir_all(&staging_user_data)
        .map_err(|e| format!("Failed to create staging profile directory: {e}"))?;

    let staging_default = staging_user_data.join("Default");
    let copy_result = (|| {
        let mut warnings = copy_profile_directory(&profile.profile_dir, &staging_default)?;
        warnings.extend(copy_local_state(&profile.user_data_root, &staging_user_data)?);
        validate_imported_auth_material(
            &profile.profile_dir,
            &profile.user_data_root,
            &staging_user_data,
        )?;
        Ok(warnings)
    })();

    let warnings = match copy_result {
        Ok(warnings) => warnings,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging_user_data);
            return Err(error);
        }
    };

    finalize_atomic_replace(&dest_user_data, &staging_user_data, &backup_user_data)?;
    Ok((dest_user_data, warnings))
}

fn import_firefox_profile_atomically(
    profile: &DiscoveredBrowserProfile,
) -> Result<(PathBuf, Vec<String>), String> {
    let dest_user_data = imported_profile_user_data_dir(&profile.name)?;
    let staging_user_data = dest_user_data.with_file_name(format!(
        "{}.staging-{}",
        profile.name,
        std::process::id()
    ));
    let backup_user_data = dest_user_data.with_file_name(format!("{}.bak", profile.name));

    cleanup_dir_if_exists(&staging_user_data)?;
    cleanup_dir_if_exists(&backup_user_data)?;

    let (_, mut warnings) =
        export_firefox_cookies_to_profile(&profile.profile_dir, &staging_user_data).inspect_err(
            |_| {
                let _ = std::fs::remove_dir_all(&staging_user_data);
            },
        )?;

    finalize_atomic_replace(&dest_user_data, &staging_user_data, &backup_user_data)?;
    warnings.push(
        "Firefox cookies will be injected into Chromium when use_profile=true.".to_string(),
    );
    Ok((dest_user_data, warnings))
}

fn finalize_atomic_replace(
    dest_user_data: &Path,
    staging_user_data: &Path,
    backup_user_data: &Path,
) -> Result<(), String> {
    if dest_user_data.exists() {
        std::fs::rename(dest_user_data, backup_user_data).map_err(|e| {
            let _ = std::fs::remove_dir_all(staging_user_data);
            format!("Failed to backup previous imported profile: {e}")
        })?;
    }

    if let Err(error) = std::fs::rename(staging_user_data, dest_user_data) {
        let _ = std::fs::remove_dir_all(dest_user_data);
        if backup_user_data.exists() {
            let _ = std::fs::rename(backup_user_data, dest_user_data);
        }
        let _ = std::fs::remove_dir_all(staging_user_data);
        return Err(format!("Failed to finalize imported profile: {error}"));
    }

    let _ = std::fs::remove_dir_all(backup_user_data);
    Ok(())
}

fn cleanup_dir_if_exists(path: &Path) -> Result<(), String> {
    if path.exists() {
        std::fs::remove_dir_all(path)
            .map_err(|e| format!("Failed to clear temporary directory {}: {e}", path.display()))?;
    }
    Ok(())
}

fn should_skip_entry(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    matches!(
        name,
        "SingletonLock"
            | "SingletonCookie"
            | "SingletonSocket"
            | "lockfile"
            | "RunningChromeVersion"
            | ".parentlock"
    ) || name.ends_with(".lock")
        || name.ends_with("lockfile")
}

fn profile_has_auth_cookies(profile_dir: &Path) -> bool {
    profile_dir.join("Cookies").is_file() || profile_dir.join("Network").join("Cookies").is_file()
}

fn copy_local_state(user_data_root: &Path, dest_user_data: &Path) -> Result<Vec<String>, String> {
    let source = user_data_root.join("Local State");
    let dest = dest_user_data.join("Local State");
    if !source.is_file() {
        return Ok(vec![]);
    }
    match std::fs::copy(&source, &dest) {
        Ok(_) => Ok(vec![]),
        Err(e) => Err(format!(
            "Failed to copy Local State (cookie encryption key): {e}"
        )),
    }
}

fn validate_imported_auth_material(
    source_profile: &Path,
    source_user_data_root: &Path,
    dest_user_data: &Path,
) -> Result<(), String> {
    let dest_default = dest_user_data.join("Default");
    let source_had_auth = profile_has_auth_cookies(source_profile);
    let source_had_local_state = source_user_data_root.join("Local State").is_file();

    if source_had_auth && !profile_has_auth_cookies(&dest_default) {
        let lock_hint = running_chromium_browser_lock_hint().unwrap_or_else(|| {
            "Your browser is still open or finishing in the background.".to_string()
        });
        return Err(format!(
            "{lock_hint} Close every browser window (and quit from the tray icon if you see one), then try again."
        ));
    }

    if (source_had_auth || source_had_local_state)
        && !dest_user_data.join("Local State").is_file()
    {
        let lock_hint = running_chromium_browser_lock_hint().unwrap_or_else(|| {
            "Your browser is still open or finishing in the background.".to_string()
        });
        return Err(format!(
            "{lock_hint} Close every browser window (and quit from the tray icon if you see one), then try again."
        ));
    }

    Ok(())
}

/// Detect browsers that typically lock Cookies / Local State while open.
fn running_chromium_browser_lock_hint() -> Option<String> {
    let running = detect_running_browser_names();
    if running.is_empty() {
        return None;
    }
    Some(format!(
        "Still open: {}.",
        running.join(", ")
    ))
}

/// Snapshot of running process names/args for browser lock detection.
///
/// On macOS, `ps -o comm=` truncates to ~16 chars (so "Google Chrome" is unreliable).
/// Prefer full `args=` on Unix; fall back to `comm=` only if args is empty.
fn running_process_blob() -> String {
    #[cfg(windows)]
    {
        let Ok(output) = std::process::Command::new("tasklist")
            .args(["/FO", "CSV", "/NH"])
            .output()
        else {
            return String::new();
        };
        String::from_utf8(output.stdout).unwrap_or_default()
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        if let Ok(output) = std::process::Command::new("ps")
            .args(["-A", "-o", "args="])
            .output()
        {
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                if !stdout.trim().is_empty() {
                    return stdout;
                }
            }
        }
        let Ok(output) = std::process::Command::new("ps")
            .args(["-A", "-o", "comm="])
            .output()
        else {
            return String::new();
        };
        String::from_utf8(output.stdout).unwrap_or_default()
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        String::new()
    }
}

fn detect_running_browser_names() -> Vec<&'static str> {
    let blob = running_process_blob();
    if blob.trim().is_empty() {
        return Vec::new();
    }

    #[cfg(windows)]
    {
        return windows_browser_names_from_tasklist(&blob);
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        return unix_browser_names_from_ps(&blob);
    }

    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        Vec::new()
    }
}

/// Windows: match exact tasklist image names only.
///
/// Important: never substring-match `msedge` — `msedgewebview2.exe` (WebView2) is common on
/// Windows (including Tauri apps) and does **not** lock Chrome/Edge profile Cookies.
///
/// Kept available on all targets so unit tests can cover the WebView2 false-positive case.
fn windows_browser_names_from_tasklist(blob: &str) -> Vec<&'static str> {
    let lower = blob.to_ascii_lowercase();
    let mut names = Vec::new();
    for (exe, label) in [
        ("\"chrome.exe\"", "Chrome"),
        ("\"msedge.exe\"", "Edge"),
        ("\"brave.exe\"", "Brave"),
        ("\"chromium.exe\"", "Chromium"),
        ("\"vivaldi.exe\"", "Vivaldi"),
        ("\"firefox.exe\"", "Firefox"),
    ] {
        if lower.contains(exe) && !names.contains(&label) {
            names.push(label);
        }
    }
    names
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn unix_browser_names_from_ps(blob: &str) -> Vec<&'static str> {
    let lower = blob.to_ascii_lowercase();
    let mut names = Vec::new();

    // Most-specific first. Never match bare "chrome" inside "chromium".
    let checks: &[(&[&str], &str)] = &[
        (&["microsoft edge", "microsoft-edge", "/msedge "], "Edge"),
        (&["brave browser", "brave-browser"], "Brave"),
        (&["vivaldi"], "Vivaldi"),
        (&["firefox"], "Firefox"),
        (&["chromium"], "Chromium"),
        (
            &["google chrome", "google-chrome", "google chrome helper"],
            "Chrome",
        ),
    ];

    for (needles, label) in checks {
        if needles.iter().any(|n| lower.contains(n)) && !names.contains(label) {
            names.push(*label);
        }
    }

    // Linux Google Chrome often appears as a bare `chrome` process (not chromium).
    if !names.iter().any(|n| *n == "Chrome" || *n == "Chromium") && lower.contains("chrome") {
        names.push("Chrome");
    }

    names
}

/// Quit allowlisted browser processes for the friendly labels in `browsers`.
///
/// Soft-then-hard on every platform (user consent already required by caller):
/// - Windows: `taskkill /IM` then `taskkill /IM /F`
/// - macOS: AppleScript `quit` then `killall -9` (SIGKILL; unsaved tabs may be lost)
/// - Linux: `pkill -TERM -x` then `pkill -KILL -x` (exact basename; never WebView2)
///
/// Never touches `msedgewebview2` (system/Tauri WebView2).
fn quit_allowlisted_browser_processes(browsers: &[String]) {
    if browsers.is_empty() {
        return;
    }

    #[cfg(windows)]
    {
        let targets = allowlisted_windows_process_targets(browsers);
        if targets.is_empty() {
            return;
        }
        for exe in &targets {
            // Soft first (lets Chrome flush), then force.
            let _ = std::process::Command::new("taskkill")
                .args(["/IM", exe])
                .output();
        }
        std::thread::sleep(std::time::Duration::from_millis(800));
        for exe in &targets {
            let _ = std::process::Command::new("taskkill")
                .args(["/IM", exe, "/F"])
                .output();
        }
    }

    #[cfg(target_os = "macos")]
    {
        // Soft quit via AppleScript, then SIGKILL — same soft→hard pattern as Windows/Linux.
        for app in macos_app_names(browsers) {
            let _ = std::process::Command::new("osascript")
                .args(["-e", &format!("tell application \"{app}\" to quit")])
                .output();
        }
        std::thread::sleep(std::time::Duration::from_millis(800));
        for app in macos_app_names(browsers) {
            let _ = std::process::Command::new("killall").args(["-9", app]).output();
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Exact process-name match (`-x`), not full cmdline (`-f`), to avoid killing
        // unrelated helpers whose argv happens to contain "chrome".
        let names = linux_process_names(browsers);
        for name in &names {
            let _ = std::process::Command::new("pkill")
                .args(["-TERM", "-x", name])
                .output();
        }
        std::thread::sleep(std::time::Duration::from_millis(800));
        for name in &names {
            let _ = std::process::Command::new("pkill")
                .args(["-KILL", "-x", name])
                .output();
        }
    }
}

#[cfg(windows)]
fn allowlisted_windows_process_targets(browsers: &[String]) -> Vec<&'static str> {
    let mut out = Vec::new();
    for name in browsers {
        let targets: &[&str] = match name.as_str() {
            "Chrome" => &["chrome.exe"],
            "Edge" => &["msedge.exe"], // never msedgewebview2.exe
            "Brave" => &["brave.exe"],
            "Chromium" => &["chromium.exe"],
            "Vivaldi" => &["vivaldi.exe"],
            "Firefox" => &["firefox.exe"],
            _ => &[],
        };
        for t in targets {
            if !out.contains(t) {
                out.push(*t);
            }
        }
    }
    out
}

#[cfg(target_os = "macos")]
fn macos_app_names(browsers: &[String]) -> Vec<&'static str> {
    let mut out = Vec::new();
    for name in browsers {
        let app = match name.as_str() {
            "Chrome" => Some("Google Chrome"),
            "Edge" => Some("Microsoft Edge"),
            "Brave" => Some("Brave Browser"),
            "Chromium" => Some("Chromium"),
            "Vivaldi" => Some("Vivaldi"),
            "Firefox" => Some("Firefox"),
            _ => None,
        };
        if let Some(app) = app {
            if !out.contains(&app) {
                out.push(app);
            }
        }
    }
    out
}

#[cfg(target_os = "linux")]
fn linux_process_names(browsers: &[String]) -> Vec<&'static str> {
    let mut out = Vec::new();
    for name in browsers {
        // Exact basenames for `pkill -x` only (never `-f` cmdline match).
        // Prefer packaged names; omit bare "chrome" so we never target chromedriver/etc.
        let names: &[&str] = match name.as_str() {
            "Chrome" => &["google-chrome", "google-chrome-stable"],
            "Edge" => &["microsoft-edge", "microsoft-edge-stable", "msedge"],
            "Brave" => &["brave-browser", "brave"],
            "Chromium" => &["chromium", "chromium-browser"],
            "Vivaldi" => &["vivaldi"],
            "Firefox" => &["firefox"],
            _ => &[],
        };
        for n in names {
            if !out.contains(n) {
                out.push(*n);
            }
        }
    }
    out
}

/// Copy a Chrome profile directory into `dest_profile` (…/User Data/Default).
fn copy_profile_directory(source: &Path, dest_profile: &Path) -> Result<Vec<String>, String> {
    if !source.is_dir() {
        return Err(format!(
            "Source profile directory does not exist: {}",
            source.display()
        ));
    }

    std::fs::create_dir_all(dest_profile)
        .map_err(|e| format!("Failed to create import destination: {e}"))?;

    let mut warnings = Vec::new();
    let mut copied_any = false;

    for entry in WalkDir::new(source).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if should_skip_entry(path) {
            continue;
        }
        let relative = match path.strip_prefix(source) {
            Ok(rel) => rel,
            Err(_) => continue,
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = dest_profile.join(relative);
        if entry.file_type().is_dir() {
            if let Err(e) = std::fs::create_dir_all(&target) {
                warnings.push(format!("Skipped directory {}: {e}", relative.display()));
            }
            continue;
        }
        if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::copy(path, &target) {
                Ok(_) => copied_any = true,
                Err(e) => {
                    warnings.push(format!(
                        "Skipped locked/unreadable file {}: {e}",
                        relative.display()
                    ));
                }
            }
        }
    }

    if !copied_any {
        return Err(
            "No profile files could be copied (browser may be locking them). Close the browser and retry."
                .to_string(),
        );
    }

    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn skips_singleton_lock_files() {
        assert!(should_skip_entry(Path::new("SingletonLock")));
        assert!(should_skip_entry(Path::new("foo.lock")));
        assert!(!should_skip_entry(Path::new("Cookies")));
    }

    #[test]
    fn windows_tasklist_ignores_webview2_but_detects_real_edge() {
        let webview_only = r#""msedgewebview2.exe","14500","Console","1","34,452 K""#;
        assert!(
            windows_browser_names_from_tasklist(webview_only).is_empty(),
            "WebView2 must not be treated as Edge"
        );

        let mixed = r#"
"chrome.exe","100","Console","1","1 K"
"msedgewebview2.exe","200","Console","1","1 K"
"msedge.exe","300","Console","1","1 K"
"#;
        let names = windows_browser_names_from_tasklist(mixed);
        assert_eq!(names, vec!["Chrome", "Edge"]);
    }

    #[test]
    fn copies_profile_tree_and_local_state_atomically() {
        let tmp = std::env::temp_dir().join(format!(
            "libragent_profile_atomic_{}",
            std::process::id()
        ));
        let source_root = tmp.join("User Data");
        let source_default = source_root.join("Default");
        let dest_root = tmp.join("chrome_default");
        let _ = fs::remove_dir_all(&tmp);

        fs::create_dir_all(source_default.join("Network")).unwrap();
        fs::write(source_default.join("Preferences"), b"{}").unwrap();
        fs::write(source_default.join("Network").join("Cookies"), b"data").unwrap();
        fs::write(source_root.join("Local State"), b"{\"os_crypt\":{}}").unwrap();
        fs::write(source_default.join("SingletonLock"), b"lock").unwrap();

        let staging = dest_root.with_file_name("chrome_default.staging-test");
        let _ = fs::remove_dir_all(&staging);
        fs::create_dir_all(&staging).unwrap();
        let warnings = copy_profile_directory(&source_default, &staging.join("Default")).unwrap();
        copy_local_state(&source_root, &staging).unwrap();
        validate_imported_auth_material(&source_default, &source_root, &staging).unwrap();
        assert!(warnings.is_empty());
        assert!(staging.join("Local State").is_file());
        assert!(staging
            .join("Default")
            .join("Network")
            .join("Cookies")
            .is_file());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn validate_requires_local_state_when_cookies_present() {
        let tmp = std::env::temp_dir().join(format!(
            "libragent_profile_local_state_{}",
            std::process::id()
        ));
        let source_root = tmp.join("User Data");
        let source_default = source_root.join("Default");
        let dest = tmp.join("dest");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(source_default.join("Network")).unwrap();
        fs::write(source_default.join("Network").join("Cookies"), b"data").unwrap();
        fs::write(source_root.join("Local State"), b"{}").unwrap();
        fs::create_dir_all(dest.join("Default").join("Network")).unwrap();
        fs::write(dest.join("Default").join("Network").join("Cookies"), b"data").unwrap();

        assert!(validate_imported_auth_material(&source_default, &source_root, &dest).is_err());

        fs::write(dest.join("Local State"), b"{}").unwrap();
        assert!(validate_imported_auth_material(&source_default, &source_root, &dest).is_ok());

        let _ = fs::remove_dir_all(&tmp);
    }
}
