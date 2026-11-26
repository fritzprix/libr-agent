//! Tests for hook execution, split by area over shared fixtures.

use super::*;
use crate::services::hooks::context::{
    await_hook_run, begin_hook_run, clear_hook_context, end_hook_run, get_hook_context,
};
use crate::services::hooks::envelope::{HookEnvelopeParams, SessionStartSource};
use crate::services::hooks::output::{HookDecision, HookOutput};
use crate::services::hooks::registry::{
    compute_handler_approval_hashes, record_session_plugin_trust, save_source_in,
    upsert_session_store_entry, PluginSourceManifest, SessionPluginStoreEntry, SessionPluginTrust,
};
use crate::services::hooks::schema::{load_hooks_from_root, HookDispatchMode, HookEvent};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

mod context_replace;
mod dispatch;
mod events;
mod http;
mod http_limits;
mod matcher;
mod session_plugins;

/// Rewrite a source's hooks.json and re-approve it, simulating a plugin
/// update between two SessionStart runs of the same session.
fn reapprove_source(temp: &Path, root: &Path, hooks_json: &str) {
    fs::write(root.join("hooks").join("hooks.json"), hooks_json).unwrap();
    let source_id = root.file_name().unwrap().to_string_lossy().to_string();
    let cfg = load_hooks_from_root(root).unwrap();
    let hashes = compute_handler_approval_hashes(root, &cfg);
    save_source_in(
        temp,
        PluginSourceManifest {
            mcp_servers: Vec::new(),
            mcp_approved: false,
            source_url: "test://local".to_string(),
            source_id,
            skill_names: vec![],
            hooks: None,
            hooks_approved: true,
            approval_hashes: hashes,
            created_at: 0,
            plugin: None,
            hook_files: vec![],
            inline_hooks: vec![],
            enabled: true,
            installed_commit: None,
        },
    )
    .unwrap();
}

fn setup_approved_plugin(hooks_json: &str) -> (TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("tempdir");
    let source_id = "plugin-0123456789abcdef".to_string();
    let root = temp.path().join(&source_id);
    fs::create_dir_all(root.join("hooks")).unwrap();
    fs::write(root.join("hooks").join("hooks.json"), hooks_json).unwrap();
    let cfg = load_hooks_from_root(&root).unwrap();
    let hashes = compute_handler_approval_hashes(&root, &cfg);
    save_source_in(
        temp.path(),
        PluginSourceManifest {
            mcp_servers: Vec::new(),
            mcp_approved: false,
            source_url: "test://local".to_string(),
            source_id,
            skill_names: vec![],
            hooks: None,
            hooks_approved: true,
            approval_hashes: hashes,
            created_at: 0,
            plugin: None,
            hook_files: vec![],
            inline_hooks: vec![],
            enabled: true,
            installed_commit: None,
        },
    )
    .unwrap();
    (temp, root)
}
