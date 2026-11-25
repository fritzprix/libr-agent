//! Unit tests for the session plugin install/uninstall/list surface.

use super::install::install_prepared_with_dirs;
use super::overview::build_overview;
use super::targets::{matches_target, resolve_plugin_target};
use super::uninstall::{cleanup_removed_session_plugin, uninstall_with_dirs};
use super::*;
use crate::services::hooks::context::{clear_hook_context, get_hook_context};
use crate::services::hooks::execute::run_session_start_with_sources;
use crate::services::hooks::registry::{
    compute_handler_approval_hashes, get_source_in, list_session_store_sources, list_sources_in,
    read_session_plugin_trust, read_store_manifest, remove_session_store_entry, save_source_in,
    PluginSourceManifest, SESSION_STORE_MANIFEST_FILE,
};
use crate::services::hooks::schema::load_hooks_from_root;
use crate::services::hooks::SessionStartSource;
use crate::services::skill_service::contracts::{
    SkillImportKind, SkillImportPreview, SkillImportResult,
};
use crate::services::skill_service::importing::{
    plugin_source_id, prepare_local_skill_import_blocking,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

mod install;
mod lookup;
mod uninstall;

struct Fixture {
    root: TempDir,
    dirs: SessionPluginDirs,
    temp_imports: PathBuf,
}

// #2942 G3.6/S3.1: the resolver's override-independence (the property the
// guard and the installer both rely on) is pinned by
// `session::manager::tests::default_workspace_dir_never_follows_workspace_override`,
// which drives the real SessionManager across register/hydrate/cancel —
// replacing the former path-identity assertion.

fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let dirs = SessionPluginDirs {
        skills: root.path().join("workspace/.libragent/skills"),
        plugins: root.path().join("session_plugins/s1"),
        // Stands in for `{data_dir}/session_plugin_trust/s1` (G1.1):
        // deliberately NOT under the plugin/workspace dirs.
        trust: root.path().join("data/session_plugin_trust/s1"),
    };
    let temp_imports = root.path().join("imports");
    fs::create_dir_all(&temp_imports).unwrap();
    Fixture {
        root,
        dirs,
        temp_imports,
    }
}

/// The overview the way the production reader builds it: store-manifest
/// records with the trust overlay (G1.1), then the global merge.
fn overview_for(fx: &Fixture, global_dir: &Path) -> SessionPluginsOverview {
    let records = list_session_store_sources(Some(&fx.dirs.trust), &fx.dirs.plugins);
    build_overview(global_dir, &records, &fx.dirs.plugins).unwrap()
}

/// Write a plugin repo with one skill and an optional SessionStart hook.
fn write_plugin_repo(parent: &Path, name: &str, hook_echo: Option<&str>) -> PathBuf {
    let repo = parent.join(name);
    let skill_dir = repo.join("skills").join(format!("{name}-skill"));
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {name}-skill\ndescription: test skill\n---\n"),
    )
    .unwrap();
    if let Some(echo) = hook_echo {
        fs::create_dir_all(repo.join("hooks")).unwrap();
        fs::write(
            repo.join("hooks").join("hooks.json"),
            format!(r#"{{ "hooks": {{ "SessionStart": [ {{ "command": "echo {echo}" }} ] }} }}"#),
        )
        .unwrap();
    }
    repo
}

fn install_from_dir(
    fx: &Fixture,
    repo: &Path,
    approve_hooks: bool,
) -> Result<SkillImportResult, String> {
    let prepared = prepare_local_skill_import_blocking(&fx.temp_imports, &repo.to_string_lossy())?;
    let has_hooks = repo.join("hooks").is_dir();
    let preview = SkillImportPreview {
        kind: if has_hooks {
            SkillImportKind::Plugin
        } else {
            SkillImportKind::SkillBundle
        },
        discovered_skills: vec![],
        conflicts: vec![],
        hooks: vec![],
        hooks_require_approval: has_hooks,
        mcp_servers: vec![],
        mcp_require_approval: false,
        plugin_commands: vec![],
        plugin_agents: vec![],
        plugin_manifest: None,
        user_config: vec![],
    };
    install_prepared_with_dirs(&fx.dirs, prepared, preview, false, vec![], approve_hooks)
}

fn write_global_source(global: &Path, source_id: &str, url: &str, echo: &str) {
    let root = global.join(source_id);
    fs::create_dir_all(root.join("hooks")).unwrap();
    fs::write(
        root.join("hooks").join("hooks.json"),
        format!(r#"{{ "hooks": {{ "SessionStart": [ {{ "command": "echo {echo}" }} ] }} }}"#),
    )
    .unwrap();
    let cfg = load_hooks_from_root(&root).unwrap();
    let hashes = compute_handler_approval_hashes(&root, &cfg);
    save_source_in(
        global,
        PluginSourceManifest {
            mcp_servers: Vec::new(),
            mcp_approved: false,
            source_url: url.to_string(),
            source_id: source_id.to_string(),
            skill_names: vec![],
            hooks: None,
            hooks_approved: true,
            approval_hashes: hashes,
            created_at: 1,
            plugin: None,
            hook_files: vec![],
            inline_hooks: vec![],
            enabled: true,
            installed_commit: None,
        },
    )
    .unwrap();
}

/// One installed source manifest in `base`, for target-resolution tests
/// that only need names and ids on disk.
fn write_source_manifest(base: &Path, source_id: &str, url: &str, enabled: bool) {
    fs::create_dir_all(base).unwrap();
    save_source_in(
        base,
        PluginSourceManifest {
            source_url: url.to_string(),
            source_id: source_id.to_string(),
            skill_names: vec![],
            hooks: None,
            hooks_approved: false,
            approval_hashes: Default::default(),
            created_at: 0,
            hook_files: vec![],
            inline_hooks: vec![],
            plugin: None,
            mcp_servers: vec![],
            mcp_approved: false,
            enabled,
            installed_commit: None,
        },
    )
    .unwrap();
}

// #2927 gatekeeper G1.2: `/plugin enable|disable` resolves a session
// install first (what the session shows), then a global install, and
// refuses to guess between two matches.

fn sensitive_manifest() -> crate::services::skill_service::plugin_manifest::PluginManifest {
    let mut user_config = BTreeMap::new();
    user_config.insert(
        "token".to_string(),
        crate::services::skill_service::plugin_manifest::UserConfigSpec {
            sensitive: true,
            required: true,
            ..Default::default()
        },
    );
    crate::services::skill_service::plugin_manifest::PluginManifest {
        name: "p".into(),
        version: None,
        description: None,
        author: None,
        homepage: None,
        repository: None,
        license: None,
        keywords: None,
        skills: None,
        commands: None,
        agents: None,
        hooks: None,
        mcp_servers: None,
        user_config,
        default_enabled: None,
    }
}
