//! Session-scoped plugin install / uninstall / list (#2825, storage moved by
//! #2942).
//!
//! A session install reuses the global plugin pipeline from `importing.rs`
//! with a different target: plugin sources go to the **default** session
//! workspace directory
//! (`{data_dir}/workspaces/<session_id>/.libragent/plugins`, owner decision
//! D1 as revised in #2942) and plugin skills go to the matching
//! `.libragent/skills` directory under that same default directory, which is
//! scanned as its own session-plugin layer and outranks global skills.
//! Nothing is written under the current (possibly overridden) workspace, so
//! a workspace change can never make session plugins disappear. Session load
//! reads the store manifest (`store-manifest.json`); entries whose plugin
//! directory is missing or broken are skipped with a log line and never fail
//! the load.
//!
//! Headless limit (#2942 G3.5): a headless run uses an ephemeral data dir
//! and its write-back copies DB rows only (`headless/writeback.rs`), so
//! plugins installed during an ephemeral headless run are not written back
//! to the real profile.
//!
//! Precedence: a session install wins over a global install with the same
//! `source_id` for that session only (`hooks::registry::list_effective_approved_sources_in`).

mod history;
mod install;
mod overview;
mod targets;
mod uninstall;

#[cfg(test)]
mod tests;

pub use install::{install_session_plugin, preview_session_plugin_install};
pub use overview::list_session_plugins;
pub use targets::{resolve_plugin_by_target, set_plugin_enabled_by_target};
pub use uninstall::{purge_session_plugin_state, uninstall_session_plugin};

use super::directories::get_session_plugin_skills_directory_for_session;
use super::plugin_sources::hook_events_for_record;
use crate::services::hooks::registry::{
    get_session_plugin_trust_dir, get_session_plugins_directory, list_session_store_sources,
    PluginSourceRecord,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PluginScope {
    Global,
    Session,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionPluginAction {
    Install,
    Uninstall,
}

/// One plugin active in a session (global or session-scoped).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionPluginEntry {
    pub id: String,
    pub source_url: String,
    pub scope: PluginScope,
    /// Unix seconds.
    pub installed_at: i64,
    pub skill_names: Vec<String>,
    pub hook_events: Vec<String>,
    pub hooks_approved: bool,
    /// Whether the plugin is enabled (#2927). A disabled plugin is still
    /// installed; it just contributes nothing to the session.
    pub enabled: bool,
    /// Manifest `version` from `.claude-plugin/plugin.json` (#2921).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Source commit recorded at install (#2927), when it is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

/// Append-only record of a session install/uninstall.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionPluginHistoryEntry {
    pub source_id: String,
    pub source_url: String,
    pub action: SessionPluginAction,
    /// Unix seconds.
    pub at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionPluginsOverview {
    /// Plugins in effect for the session. Session installs come first and
    /// shadow a global plugin with the same id.
    pub active: Vec<SessionPluginEntry>,
    pub history: Vec<SessionPluginHistoryEntry>,
}

/// Directories a session install writes to.
struct SessionPluginDirs {
    skills: PathBuf,
    plugins: PathBuf,
    /// Out-of-workspace trust store for this session's plugin approvals
    /// (#2942 gatekeeper G1.1): never inside the session workspace.
    trust: PathBuf,
}

fn session_dirs(session_id: &str) -> Result<SessionPluginDirs, String> {
    let dirs = SessionPluginDirs {
        // #2942 G3.2: plugin skills resolve under the default session
        // workspace directory, not the current (possibly overridden) one.
        skills: get_session_plugin_skills_directory_for_session(session_id)?,
        plugins: get_session_plugins_directory(session_id)?,
        trust: get_session_plugin_trust_dir(session_id)?,
    };
    // #2942 G3.3(b): the default directory is created at session creation,
    // but the store itself is created lazily — make sure the install targets
    // exist before writing into them.
    fs::create_dir_all(&dirs.plugins)
        .map_err(|e| format!("Failed to create session plugin store: {e}"))?;
    fs::create_dir_all(&dirs.skills)
        .map_err(|e| format!("Failed to create session plugin skills dir: {e}"))?;
    Ok(dirs)
}

/// The read API for a session's plugin components (#2942), used by session
/// load and by the command/hook/MCP loaders: resolve every store-manifest
/// entry through the plugin source store. An entry whose plugin directory was
/// deleted or whose manifest is broken is skipped with a log line; the load
/// itself never fails (#2942 D1 failure policy). Approval is overlaid from
/// the out-of-workspace trust store (`trust_dir`, #2942 gatekeeper G1.1).
pub fn load_session_plugins(session_id: &str) -> Result<Vec<PluginSourceRecord>, String> {
    let plugins_dir = get_session_plugins_directory(session_id)?;
    let trust_dir = get_session_plugin_trust_dir(session_id)?;
    Ok(list_session_store_sources(Some(&trust_dir), &plugins_dir))
}

fn entry_from_record(record: &PluginSourceRecord, scope: PluginScope) -> SessionPluginEntry {
    let manifest = &record.manifest;
    SessionPluginEntry {
        id: manifest.source_id.clone(),
        source_url: manifest.source_url.clone(),
        scope,
        installed_at: manifest.created_at,
        skill_names: manifest.skill_names.clone(),
        hook_events: if manifest.hooks.is_some() {
            hook_events_for_record(record)
        } else {
            Vec::new()
        },
        hooks_approved: manifest.hooks_approved,
        enabled: manifest.enabled,
        version: manifest.plugin.as_ref().and_then(|p| p.version.clone()),
        commit: manifest.installed_commit.clone(),
    }
}
