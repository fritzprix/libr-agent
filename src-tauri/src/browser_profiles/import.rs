use std::path::{Path, PathBuf};

use chrono::Utc;
use log::{info, warn};
use walkdir::WalkDir;

use super::discover::{
    browser_default_priority, discover_browser_profiles, DiscoveredBrowserProfile, ProfileEngine,
};
use super::firefox::export_firefox_cookies_to_profile;
use super::registry::{
    imported_profile_user_data_dir, load_registry, remove_imported_profile, save_registry,
    upsert_imported_profile, ImportedProfile, ImportKind,
};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
    pub warnings: Vec<String>,
}

/// One-click / Settings import: copy Default profiles from each installed browser.
///
/// - Chromium family (Chrome/Edge/Brave/…): full User Data copy (cookies + Local State).
/// - Firefox: cookie export for CDP injection (automation remains Chromium-based).
///
/// Only each browser's Default profile is imported to keep the UX simple.
pub fn import_chrome_profiles() -> Result<ImportReport, String> {
    import_browser_profiles()
}

pub fn import_browser_profiles() -> Result<ImportReport, String> {
    let mut discovered: Vec<_> = discover_browser_profiles()
        .into_iter()
        .filter(|profile| profile.name.ends_with("_default"))
        .collect();

    discovered.sort_by_key(|profile| browser_default_priority(&profile.browser_id));

    if discovered.is_empty() {
        return Ok(ImportReport {
            imported: vec![],
            skipped: vec![],
            warnings: vec![
                "No Chrome/Edge/Brave/Chromium/Firefox Default profiles were found on this machine."
                    .to_string(),
            ],
        });
    }
    import_discovered(&discovered, true)
}

/// Remove an imported profile from the registry and delete its app-local copy.
pub fn delete_imported_browser_profile(name: &str) -> Result<(), String> {
    remove_imported_profile(name)
}

fn import_discovered(
    discovered: &[DiscoveredBrowserProfile],
    set_first_default: bool,
) -> Result<ImportReport, String> {
    let mut registry = load_registry()?;
    let mut imported = Vec::new();
    let mut skipped = Vec::new();
    let mut warnings = Vec::new();
    let mut made_default = false;

    for profile in discovered {
        match import_one_profile(profile) {
            Ok((dest_user_data, kind, copy_warnings)) => {
                warnings.extend(copy_warnings);
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
                    set_first_default && !made_default,
                );
                if set_first_default && !made_default {
                    made_default = true;
                }
                imported.push(profile.name.clone());
                info!(
                    "Imported browser profile '{}' ({:?})",
                    profile.name, profile.engine
                );
            }
            Err(error) => {
                let message = format!(
                    "Failed to import '{}': {error}. Close the browser and try again.",
                    profile.label
                );
                warn!("{message}");
                warnings.push(message);
                skipped.push(profile.name.clone());
            }
        }
    }

    save_registry(&registry)?;

    Ok(ImportReport {
        imported,
        skipped,
        warnings,
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
        return Err(
            "Cookie/login data could not be copied (browser may be open). Close the browser and retry."
                .to_string(),
        );
    }

    if (source_had_auth || source_had_local_state)
        && !dest_user_data.join("Local State").is_file()
    {
        return Err(
            "Local State (encryption key for cookies) could not be copied. Close the browser and retry."
                .to_string(),
        );
    }

    Ok(())
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
