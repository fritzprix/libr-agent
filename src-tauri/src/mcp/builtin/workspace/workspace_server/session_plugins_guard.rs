//! File tools never write into a session's plugin store
//! (`{data_dir}/workspaces/<session_id>/.libragent/plugins` — the
//! **default** session workspace directory, #2942 D1 revised). The store is
//! written only by the
//! install/uninstall services, which keep hook approval outside the workspace;
//! a file-tool edit there could plant a plugin whose files a rewritten
//! manifest would still describe as approved. Reads are unaffected; only the
//! refusal carries the recovery guidance.
//!
//! Known caveat (#2942 worker note, kept visible for the next reader): this
//! guard covers the file tools only. `workspace__runShell` can still reach
//! the store with plain shell commands; the security boundary is that hook
//! *approval* lives outside the workspace (`session_plugin_trust`, #2942
//! G1.1), so files planted by any means stay unapproved until a user approves
//! them through the install flow.

use super::user_skills_guard::resolve_lenient;
use crate::mcp::builtin::utils::relative_path_under_base;
use std::path::Path;

/// Marker substring for session-plugin-store write rejection (recovery guidance + tests).
pub const SESSION_PLUGINS_WRITE_REJECTED_MARKER: &str = "Session plugin store is read-only";

pub fn session_plugins_write_rejected_error() -> String {
    format!(
        "{SESSION_PLUGINS_WRITE_REJECTED_MARKER} for file tools. Install or update \
         plugins through the plugin install flow (Extensions → Plugins), not by \
         editing files under `.libragent/plugins`."
    )
}

/// True when `target` is the session plugin store directory or inside it.
pub fn is_inside_session_plugins(target: &Path, plugins_dir: &Path) -> bool {
    relative_path_under_base(&resolve_lenient(target), &resolve_lenient(plugins_dir)).is_some()
}

/// `Err` with the refusal message when `target` is inside the session plugin store.
pub fn refuse_session_plugins_write(target: &Path, plugins_dir: &Path) -> Result<(), String> {
    if is_inside_session_plugins(target, plugins_dir) {
        Err(session_plugins_write_rejected_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_paths_inside_session_plugin_store() {
        let root = tempfile::tempdir().unwrap();
        let store = root.path().join(".libragent").join("plugins");
        std::fs::create_dir_all(store.join("plugin-abc")).unwrap();

        assert!(is_inside_session_plugins(&store, &store));
        assert!(is_inside_session_plugins(
            &store.join("plugin-abc/manifest.json"),
            &store
        ));
        // Not-yet-created paths inside the store count too (install targets).
        assert!(is_inside_session_plugins(
            &store.join("plugin-new/hooks/hooks.json"),
            &store
        ));
    }

    #[test]
    fn ignores_sibling_and_lookalike_paths() {
        let root = tempfile::tempdir().unwrap();
        let store = root.path().join(".libragent").join("plugins");
        std::fs::create_dir_all(&store).unwrap();

        assert!(!is_inside_session_plugins(
            &root.path().join(".libragent/plugins_backup/x"),
            &store
        ));
        assert!(!is_inside_session_plugins(
            &root.path().join(".libragent/skills/x"),
            &store
        ));
        assert!(!is_inside_session_plugins(
            &root.path().join("src/main.rs"),
            &store
        ));
    }

    #[test]
    fn dotdot_escapes_resolve_to_the_real_location() {
        let root = tempfile::tempdir().unwrap();
        let store = root.path().join(".libragent").join("plugins");
        std::fs::create_dir_all(store.join("plugin-abc")).unwrap();
        std::fs::create_dir_all(root.path().join("workspace")).unwrap();

        let dotdot = root
            .path()
            .join("workspace/../.libragent/plugins/plugin-abc/hooks/hooks.json");
        assert!(is_inside_session_plugins(&dotdot, &store));
    }

    #[test]
    fn refusal_names_the_install_flow() {
        let message = session_plugins_write_rejected_error();
        assert!(message.contains(SESSION_PLUGINS_WRITE_REJECTED_MARKER));
        assert!(message.contains("Extensions"));
    }
}
