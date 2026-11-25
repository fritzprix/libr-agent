//! Session `CLAUDE_ENV_FILE` and `${CLAUDE_PLUGIN_DATA}` directories
//! (#2926): the filesystem side of the hook environment.

use std::path::{Path, PathBuf};

/// Create (or truncate) the session's `CLAUDE_ENV_FILE` and return it. A
/// failure only costs the plugin-variable feature, so it is logged, not fatal.
pub(super) fn prepare_session_env_file(session_id: &str) -> Option<PathBuf> {
    let path = crate::services::hooks::registry::get_session_plugin_env_file(session_id).ok()?;
    if let Some(parent) = path.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            log::warn!(
                "CLAUDE_ENV_FILE unavailable for session {}: create {}: {}",
                session_id,
                parent.display(),
                error
            );
            return None;
        }
    }
    match std::fs::write(&path, "") {
        Ok(()) => Some(path),
        Err(error) => {
            log::warn!(
                "CLAUDE_ENV_FILE unavailable for session {}: truncate {}: {}",
                session_id,
                path.display(),
                error
            );
            None
        }
    }
}

/// `${CLAUDE_PLUGIN_DATA}` of one source record (#2926): a session-store
/// record gets the session's data dir, anything else the global one. A path
/// that cannot be resolved falls back to the plugin root's sibling
/// `.plugin-data`, so a hook still gets a writable directory instead of none.
pub(super) fn plugin_data_dir_for(
    record: &crate::services::hooks::registry::PluginSourceRecord,
    session_id: &str,
    session_plugins_dir: Option<&Path>,
) -> PathBuf {
    use crate::services::hooks::registry::{get_session_plugin_data_dir, get_user_plugin_data_dir};
    let session_scope = session_plugins_dir.is_some_and(|base| record.root_dir.starts_with(base));
    let resolved = if session_scope {
        get_session_plugin_data_dir(session_id, &record.manifest.source_id)
    } else {
        get_user_plugin_data_dir(&record.manifest.source_id)
    };
    resolved.unwrap_or_else(|_| fallback_plugin_data_dir(record))
}

/// Data directory used when the data-dir root is unavailable (unit tests with
/// no session manager): keep the location stable per plugin source.
pub(super) fn fallback_plugin_data_dir(
    record: &crate::services::hooks::registry::PluginSourceRecord,
) -> PathBuf {
    record
        .root_dir
        .parent()
        .map(|base| base.join(format!("{}-data", record.manifest.source_id)))
        .unwrap_or_else(|| record.root_dir.join(".plugin-data"))
}
