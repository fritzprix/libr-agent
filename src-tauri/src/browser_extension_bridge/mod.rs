//! Loopback WebSocket bridge for the LibrAgent Chrome MV3 extension (Load unpacked / Store).
//!
//! Session routing is explicit at `browser__createSession` via `browser=sidecar|userChrome`.
//! There is no silent auto-fallback between everyday Chrome and the sticky sidecar.

mod bridge;
mod messages;

pub use bridge::{page_state_from_extension_tab, ExtensionBridge};
pub use messages::{
    decode_reply, decode_request, resolve_backend_mode, resolve_bridge_port, resolve_bridge_token,
    ExtensionBridgeStatus, ExtensionMethod, ExtensionReply, ExtensionRequest, ExtensionTabState,
    DEFAULT_BRIDGE_PORT, DEV_BRIDGE_TOKEN,
};

use std::path::{Path, PathBuf};

/// Start this process's extension bridge (idempotent).
///
/// Each backend binds its own loopback port, falling back from
/// [`DEFAULT_BRIDGE_PORT`] when the preferred port is taken (#2858).
pub fn ensure_started() {
    ExtensionBridge::global().ensure_started();
}

pub fn status() -> ExtensionBridgeStatus {
    ExtensionBridge::global().status()
}

pub fn is_connected() -> bool {
    ExtensionBridge::global().is_connected()
}

/// Absolute path to the Load unpacked extension folder, when resolvable.
///
/// Packaged builds carry the folder as a bundle resource whose mount path
/// changes on every run (AppImage), so the folder is staged into the stable
/// app data directory and that copy is returned (#3109).
pub fn extension_unpacked_path() -> Result<String, String> {
    for candidate in extension_path_candidates() {
        if candidate.join("manifest.json").is_file() {
            let absolute = candidate
                .canonicalize()
                .unwrap_or_else(|_| candidate.clone());
            if is_bundled_resource(&absolute) {
                let base_data_dir = crate::session::get_session_manager()?
                    .get_base_data_dir()
                    .clone();
                return match stage_bundled_extension(&absolute, &base_data_dir) {
                    Ok(staged) => Ok(staged.display().to_string()),
                    Err(error) => {
                        log::warn!(
                            "Could not stage the bundled chrome-extension/ folder ({error}); returning the bundle path instead."
                        );
                        Ok(absolute.display().to_string())
                    }
                };
            }
            return Ok(absolute.display().to_string());
        }
    }
    Err(
        "Could not locate chrome-extension/ (expected next to the repo or app resources)"
            .to_string(),
    )
}

/// Where Tauri places the bundled extension folder.
///
/// `tauri.conf.json` bundles `../chrome-extension/**/*`; resources above the
/// config folder land under `_up_` in the resource directory, but the direct
/// layout is kept as a fallback candidate so either layout resolves.
fn bundled_resource_candidates(resource_dir: &Path) -> Vec<PathBuf> {
    vec![
        resource_dir.join("_up_").join("chrome-extension"),
        resource_dir.join("chrome-extension"),
    ]
}

fn is_bundled_resource(candidate: &Path) -> bool {
    let Some(app) = crate::state::get_app_handle() else {
        return false;
    };
    use tauri::Manager;
    let Ok(resource_dir) = app.path().resource_dir() else {
        return false;
    };
    let resource_dir = resource_dir.canonicalize().unwrap_or(resource_dir);
    candidate.starts_with(&resource_dir)
}

/// Copy the bundled extension folder into `<base_data_dir>/chrome-extension`
/// so "Load unpacked" receives a path that survives app restarts. The folder
/// is refreshed wholesale so a previous version's leftover files cannot
/// linger.
fn stage_bundled_extension(source: &Path, base_data_dir: &Path) -> Result<PathBuf, String> {
    let target = base_data_dir.join("chrome-extension");
    if target.exists() {
        std::fs::remove_dir_all(&target)
            .map_err(|error| format!("Could not refresh {}: {error}", target.display()))?;
    }
    crate::services::skill_service::copy_dir_recursive(source, &target)?;
    Ok(target)
}

fn extension_path_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();

    // Compile-time workspace layout: src-tauri/../chrome-extension
    out.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../chrome-extension"));

    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd.join("chrome-extension"));
        out.push(cwd.join("../chrome-extension"));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            out.push(dir.join("chrome-extension"));
            out.push(dir.join("../chrome-extension"));
            out.push(dir.join("../../chrome-extension"));
            out.push(dir.join("../../../chrome-extension"));
        }
    }

    if let Some(app) = crate::state::get_app_handle() {
        use tauri::Manager;
        if let Ok(resource_dir) = app.path().resource_dir() {
            out.extend(bundled_resource_candidates(&resource_dir));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn bundled_resource_candidates_cover_the_up_prefix() {
        let resource_dir = Path::new("/tmp/resources");
        let candidates = bundled_resource_candidates(resource_dir);
        assert_eq!(
            candidates[0],
            resource_dir.join("_up_").join("chrome-extension")
        );
        assert_eq!(candidates[1], resource_dir.join("chrome-extension"));
    }

    #[test]
    fn stage_bundled_extension_copies_tree_and_drops_stale_files() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("bundle/chrome-extension");
        write_file(&source.join("manifest.json"), "{\"manifest_version\":3}");
        write_file(&source.join("scripts/content.js"), "console.log(1)");

        let base = temp.path().join("data");
        let staged = stage_bundled_extension(&source, &base).unwrap();
        assert_eq!(staged, base.join("chrome-extension"));
        assert!(staged.join("manifest.json").is_file());
        assert!(staged.join("scripts/content.js").is_file());

        // A stale file from a previous version must not survive a re-stage.
        write_file(&staged.join("scripts/old_leftover.js"), "stale");
        let staged_again = stage_bundled_extension(&source, &base).unwrap();
        assert_eq!(staged, staged_again);
        assert!(staged.join("manifest.json").is_file());
        assert!(staged.join("scripts/content.js").is_file());
        assert!(
            !staged.join("scripts/old_leftover.js").exists(),
            "stale files must be removed on refresh"
        );
    }
}
