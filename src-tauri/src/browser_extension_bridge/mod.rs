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

use std::path::PathBuf;

/// Start the bridge server if it is not already running.
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
pub fn extension_unpacked_path() -> Result<String, String> {
    let candidates = extension_path_candidates();
    for candidate in candidates {
        if candidate.join("manifest.json").is_file() {
            let absolute = candidate
                .canonicalize()
                .unwrap_or_else(|_| candidate.clone());
            return Ok(absolute.display().to_string());
        }
    }
    Err(
        "Could not locate chrome-extension/ (expected next to the repo or app resources)"
            .to_string(),
    )
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
            out.push(resource_dir.join("chrome-extension"));
        }
    }

    out
}
