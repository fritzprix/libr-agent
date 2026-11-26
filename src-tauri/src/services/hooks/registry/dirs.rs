use super::sources::validate_source_id;
use crate::session::get_session_manager;
use std::fs;
use std::path::{Path, PathBuf};

pub const USER_PLUGINS_DIR_NAME: &str = "user_plugins";
/// Legacy location of per-session plugin stores
/// (`{data_dir}/session_plugins/{session_id}/`, #2825). #2942 moved the store
/// into the **default** session workspace directory (see
/// `SESSION_WORKSPACE_PLUGINS_DIR`); this constant now only drives cleanup of
/// leftover legacy stores on session deletion. Existing legacy entries are
/// ignored, not migrated (documented in the #2942 PR).
pub const LEGACY_SESSION_PLUGINS_DIR_NAME: &str = "session_plugins";
/// Hidden-directory name holding the session-scoped plugin store under the
/// **default** session workspace directory
/// (`{data_dir}/workspaces/<session_id>/.libragent/plugins`, #2942 D1
/// revised). Unlike tool-result spillover (which follows the current
/// workspace), the store never moves with a workspace override.
pub const SESSION_WORKSPACE_PLUGINS_DIR: &str = "plugins";

pub fn get_user_plugins_directory() -> Result<PathBuf, String> {
    let session_manager = get_session_manager()?;
    Ok(session_manager
        .get_base_data_dir()
        .join(USER_PLUGINS_DIR_NAME))
}

/// Directory holding the plugins installed for one session only (#2825).
/// Per the revised owner decision D1 (#2942), the store lives under the
/// **default** session workspace directory
/// (`{data_dir}/workspaces/<session_id>/.libragent/plugins`), never under
/// the current (possibly overridden) workspace — a workspace change can
/// therefore never make session plugins disappear. Session deletion removes
/// the store together with the default directory
/// (`SessionDirectoryService::remove_workspace`).
/// Headless limit (#2942 G3.5): a headless run uses an ephemeral data dir
/// and its write-back copies DB rows only
/// (`headless/writeback.rs::write_back_*`), so plugins installed during an
/// ephemeral headless run are not written back to the real profile.
pub fn get_session_plugins_directory(session_id: &str) -> Result<PathBuf, String> {
    validate_session_id(session_id)?;
    let session_manager = get_session_manager()?;
    // #2942 G3.1: resolve from the session id only. The previous
    // `get_session_workspace_dir_by_id` returned the override once hydrated,
    // so the same session could resolve to two different stores.
    Ok(session_store_dir_in(
        &session_manager.get_session_default_workspace_dir_unverified(session_id),
    ))
}

/// Pure location of one session's plugin store
/// (`<default_workspace_root>/.libragent/plugins`), the join used by
/// [`get_session_plugins_directory`] (extracted for direct testing, #2942
/// gatekeeper G1.4). `default_workspace_root` must be the **default**
/// session workspace directory (`workspaces/<session_id>`), not the
/// current/override workspace (#2942 D1 revised).
pub fn session_store_dir_in(default_workspace_root: &Path) -> PathBuf {
    default_workspace_root
        .join(".libragent")
        .join(SESSION_WORKSPACE_PLUGINS_DIR)
}

fn validate_session_id(session_id: &str) -> Result<(), String> {
    if session_id.is_empty()
        || session_id == "."
        || session_id == ".."
        || session_id.contains(['/', '\\'])
    {
        return Err(format!("invalid session id: {:?}", session_id));
    }
    Ok(())
}

/// Directory name under the app data dir holding per-session plugin trust
/// records (#2942 gatekeeper G1.1). Plugin files and the store manifest live
/// in the session workspace (owner D1), but approval data never does: the
/// workspace is agent-writable, so a rewritten hook script plus a rewritten
/// workspace manifest must not be able to make a hook run.
pub const SESSION_PLUGIN_TRUST_DIR_NAME: &str = "session_plugin_trust";

/// `{data_dir}/session_plugin_trust/{session_id}` — the out-of-workspace
/// trust records for one session's plugins (see [`super::SessionPluginTrust`]).
pub fn get_session_plugin_trust_dir(session_id: &str) -> Result<PathBuf, String> {
    validate_session_id(session_id)?;
    let session_manager = get_session_manager()?;
    Ok(session_manager
        .get_base_data_dir()
        .join(SESSION_PLUGIN_TRUST_DIR_NAME)
        .join(session_id))
}

/// Directory name under the app data dir holding per-plugin data
/// (`${CLAUDE_PLUGIN_DATA}`, #2926). It lives in the app data dir, never in
/// the plugin store or the agent-writable workspace, so plugin data survives
/// both a plugin update and a reinstall of the same source (#2926 keeps it
/// when the plugin updates) and uninstall is the only thing that removes it.
pub const PLUGIN_DATA_DIR_NAME: &str = "plugin_data";

/// `{data_dir}/plugin_data/user/{source_id}` — `${CLAUDE_PLUGIN_DATA}` for a
/// globally installed plugin.
pub fn get_user_plugin_data_dir(source_id: &str) -> Result<PathBuf, String> {
    validate_source_id(source_id)?;
    let session_manager = get_session_manager()?;
    Ok(session_manager
        .get_base_data_dir()
        .join(PLUGIN_DATA_DIR_NAME)
        .join("user")
        .join(source_id))
}

/// `{data_dir}/plugin_data/sessions/{session_id}/{source_id}` —
/// `${CLAUDE_PLUGIN_DATA}` for a session-scope plugin. A session's plugin
/// data goes away with the plugin's uninstall in that session.
pub fn get_session_plugin_data_dir(session_id: &str, source_id: &str) -> Result<PathBuf, String> {
    validate_session_id(session_id)?;
    validate_source_id(source_id)?;
    let session_manager = get_session_manager()?;
    Ok(session_manager
        .get_base_data_dir()
        .join(PLUGIN_DATA_DIR_NAME)
        .join("sessions")
        .join(session_id)
        .join(source_id))
}

/// Create a plugin data directory (and its parents) and return it.
pub fn ensure_plugin_data_dir(dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir.to_path_buf())
}

/// `${CLAUDE_ENV_FILE}` for one session's `SessionStart` hooks (#2926): a
/// file the hook writes `NAME=value` lines to, which later shell tool calls
/// in that session then see. It lives in the app data dir, out of the agent
/// workspace, and is rewritten for every `SessionStart` run.
pub const PLUGIN_ENV_FILE_NAME: &str = "session-start.env";

pub fn get_session_plugin_env_file(session_id: &str) -> Result<PathBuf, String> {
    validate_session_id(session_id)?;
    match get_session_manager() {
        Ok(session_manager) => Ok(session_manager
            .get_base_data_dir()
            .join(PLUGIN_ENV_DIR_NAME)
            .join(session_id)
            .join(PLUGIN_ENV_FILE_NAME)),
        // No global session manager (unit tests, or a very early startup
        // state): keep the file out of the agent workspace all the same.
        Err(_) => Ok(std::env::temp_dir()
            .join("libragent-plugin-env")
            .join(session_id)
            .join(PLUGIN_ENV_FILE_NAME)),
    }
}

/// Directory name under the app data dir holding per-session
/// `CLAUDE_ENV_FILE` files.
pub const PLUGIN_ENV_DIR_NAME: &str = "plugin_env";
