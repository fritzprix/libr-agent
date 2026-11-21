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

use super::cache::invalidate_skill_scan_cache;
use super::contracts::{PreparedSkillImport, SkillImportPreview, SkillImportResult};
use super::directories::{
    get_session_plugin_skills_directory_for_session, get_system_skills_directory,
};
use super::github::parse_github_repo_url;
use super::importing::{
    build_import_preview, install_prepared_import, plugin_source_id, prepare_github_skill_import,
    remove_skill_names_from_user_skills, ImportTarget,
};
use super::plugin_sources::hook_events_for_record;
use crate::services::hooks::registry::SessionPluginStoreEntry;
use crate::services::hooks::registry::{
    delete_source_in, get_session_plugin_trust_dir, get_session_plugins_directory, get_source_in,
    get_user_plugins_directory, list_session_store_sources, list_sources_in,
    move_session_approval_to_trust, remove_session_plugin_trust, remove_session_store_entry,
    upsert_session_store_entry, PluginSourceRecord,
};
use crate::services::hooks::{clear_hook_context, spawn_session_start, SessionStartSource};
use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const HISTORY_FILE: &str = "history.json";

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

/// Skill names recorded in this session's plugin-source store for the same
/// source key. Re-installs of the same manifest plugin must not namespace-
/// rename against their own previously installed skills (gatekeeper G1.1).
fn session_same_source_skill_names(plugins_dir: &Path, source_key: &str) -> Vec<String> {
    get_source_in(plugins_dir, &plugin_source_id(source_key))
        .map(|record| record.manifest.skill_names)
        .unwrap_or_default()
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

pub async fn preview_session_plugin_install(
    session_id: String,
    repo_url: String,
) -> Result<SkillImportPreview, String> {
    let dirs = session_dirs(&session_id)?;
    let mut prepared = prepare_github_skill_import(repo_url).await?;
    let same_source = session_same_source_skill_names(&dirs.plugins, &prepared.source_key);
    build_import_preview(
        &mut prepared,
        &dirs.skills,
        "workspace",
        "workspace",
        &get_system_skills_directory()?,
        &same_source,
    )
    .await
}

/// Install a plugin for one session, then fire SessionStart with
/// `source=resume` so its hooks take effect immediately.
pub async fn install_session_plugin(
    session_id: String,
    repo_url: String,
    overwrite_existing: bool,
    excluded_skill_names: Option<Vec<String>>,
    approve_hooks: Option<bool>,
    plugin_options: Option<BTreeMap<String, String>>,
) -> Result<SkillImportResult, String> {
    let plugin_options = plugin_options.unwrap_or_default();
    let dirs = session_dirs(&session_id)?;
    let mut prepared = prepare_github_skill_import(repo_url).await?;
    let same_source = session_same_source_skill_names(&dirs.plugins, &prepared.source_key);
    let preview = build_import_preview(
        &mut prepared,
        &dirs.skills,
        "workspace",
        "workspace",
        &get_system_skills_directory()?,
        &same_source,
    )
    .await?;
    // #2926: check the values the user entered before anything is written, so a
    // missing required option fails the install cleanly.
    let user_config =
        super::importing::prepare_plugin_user_config(&prepared.scan_root, &plugin_options)?;
    let source_id = plugin_source_id(&prepared.source_key);
    let result = install_prepared_with_dirs(
        &dirs,
        prepared,
        preview,
        overwrite_existing,
        excluded_skill_names.unwrap_or_default(),
        approve_hooks.unwrap_or(false),
    )?;
    let mut result = result;
    if let Some((manifest, values)) = user_config {
        if let Err(error) =
            super::importing::save_plugin_user_config(&dirs.plugins, &source_id, &manifest, &values)
                .await
        {
            log::warn!(
                "session {}: plugin {} installed but its userConfig values were not saved: {}",
                session_id,
                source_id,
                error
            );
            result.warnings.push(format!(
                "Plugin installed, but its configuration values could not be saved: {error}. Re-install the plugin and enter them again."
            ));
        }
    }
    invalidate_skill_scan_cache();
    spawn_session_start(&session_id, SessionStartSource::Resume);
    Ok(result)
}

fn install_prepared_with_dirs(
    dirs: &SessionPluginDirs,
    prepared: PreparedSkillImport,
    preview: SkillImportPreview,
    overwrite_existing: bool,
    excluded_skill_names: Vec<String>,
    approve_hooks: bool,
) -> Result<SkillImportResult, String> {
    let source_url = prepared.source_key.clone();
    let source_id = plugin_source_id(&source_url);
    let result = install_prepared_import(
        prepared,
        preview,
        overwrite_existing,
        excluded_skill_names,
        approve_hooks,
        ImportTarget {
            skills_dir: dirs.skills.clone(),
            plugins_dir: dirs.plugins.clone(),
            always_record_source: true,
        },
    )?;
    // #2942 gatekeeper G1.1: move the approval data out of the workspace
    // manifest into the out-of-workspace trust store before the plugin can
    // be loaded, so nothing inside the session workspace grants approval.
    if let Err(error) = move_session_approval_to_trust(&dirs.trust, &dirs.plugins, &source_id) {
        cleanup_failed_session_install(dirs, &source_id);
        return Err(format!(
            "Plugin files were installed but their approval could not be recorded outside the \
             session workspace, so the install was rolled back: {error}"
        ));
    }
    // #2942 gatekeeper G1.2: a plugin missing from the store manifest is
    // never loaded, so a manifest write failure is an install failure —
    // report it and remove the just-written source instead of lying.
    if let Err(error) = upsert_session_store_entry(
        &dirs.plugins,
        SessionPluginStoreEntry {
            source_id: source_id.clone(),
            source_url: source_url.clone(),
            installed_at: chrono::Utc::now().timestamp(),
        },
    ) {
        cleanup_failed_session_install(dirs, &source_id);
        return Err(format!(
            "Plugin files were installed but the session plugin store manifest could not be \
             written, so the install was rolled back: {error}"
        ));
    }
    append_history(
        &dirs.plugins,
        SessionPluginHistoryEntry {
            source_id,
            source_url,
            action: SessionPluginAction::Install,
            at: chrono::Utc::now().timestamp(),
        },
    );
    Ok(result)
}

/// Roll back a session install whose post-install bookkeeping failed (#2942
/// gatekeeper G1.2): without the manifest entry and trust record the plugin
/// would never load, so leaving its files behind only hides the failure.
fn cleanup_failed_session_install(dirs: &SessionPluginDirs, source_id: &str) {
    let _ = delete_source_in(&dirs.plugins, source_id);
    let _ = remove_session_plugin_trust(&dirs.trust, source_id);
}

/// Remove a session plugin by source URL, source id, or repository name.
///
/// The session's hook-context slot is dropped and SessionStart re-run so the
/// removed plugin's bootstrap does not linger while global plugins re-apply.
pub fn uninstall_session_plugin(
    session_id: &str,
    target: &str,
) -> Result<SessionPluginEntry, String> {
    let dirs = session_dirs(session_id)?;
    let removed = uninstall_with_dirs(&dirs, target)?;
    // #2926: the plugin's session data directory (`${CLAUDE_PLUGIN_DATA}`)
    // goes with the plugin. Its sensitive `userConfig` values are keyed by
    // source id, which the same plugin may also hold as a global install, so
    // only a global uninstall clears them; a session uninstall leaves them
    // available for a later reinstall.
    if let Ok(data_dir) =
        crate::services::hooks::registry::get_session_plugin_data_dir(session_id, &removed.id)
    {
        if let Err(error) = super::plugin_sources::remove_plugin_data_dir(&data_dir) {
            log::warn!(
                "session {}: plugin {} data dir left behind: {}",
                session_id,
                removed.id,
                error
            );
        }
    }
    // The session store's option sidecar goes with the plugin; its secure
    // values stay (see above).
    if let Err(error) = super::plugin_options::remove_saved_options(&dirs.plugins, &removed.id) {
        log::warn!(
            "session {}: plugin {} stored options left behind: {}",
            session_id,
            removed.id,
            error
        );
    }
    invalidate_skill_scan_cache();
    clear_hook_context(session_id);
    spawn_session_start(session_id, SessionStartSource::Resume);
    Ok(removed)
}

fn uninstall_with_dirs(
    dirs: &SessionPluginDirs,
    target: &str,
) -> Result<SessionPluginEntry, String> {
    let target = target.trim();
    if target.is_empty() {
        return Err("Specify a plugin git URL or name to uninstall".to_string());
    }
    let mut matches = list_sources_in(&dirs.plugins)?
        .into_iter()
        .filter(|record| matches_target(record, target))
        .collect::<Vec<_>>();
    let record = match matches.len() {
        0 => {
            return Err(format!(
                "No plugin matching '{target}' is installed in this session. \
                 Global plugins are managed under Extensions → Skills."
            ))
        }
        1 => matches.remove(0),
        _ => {
            let urls = matches
                .iter()
                .map(|r| r.manifest.source_url.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!("'{target}' is ambiguous: {urls}"));
        }
    };

    let entry = entry_from_record(&record, PluginScope::Session);
    remove_skill_names_from_user_skills(&dirs.skills, &record.manifest.skill_names)?;
    delete_source_in(&dirs.plugins, &record.manifest.source_id)?;
    // #2942: the trust record goes with the plugin, and a store-manifest
    // rewrite failure is reported, not swallowed (gatekeeper G1.2).
    remove_session_plugin_trust(&dirs.trust, &record.manifest.source_id)?;
    remove_session_store_entry(&dirs.plugins, &record.manifest.source_id)?;
    append_history(
        &dirs.plugins,
        SessionPluginHistoryEntry {
            source_id: entry.id.clone(),
            source_url: entry.source_url.clone(),
            action: SessionPluginAction::Uninstall,
            at: chrono::Utc::now().timestamp(),
        },
    );
    Ok(entry)
}

pub fn list_session_plugins(session_id: &str) -> Result<SessionPluginsOverview, String> {
    let session_dir = get_session_plugins_directory(session_id)?;
    let global_dir = get_user_plugins_directory()?;
    // #2942 G1.3: one read path for session plugins — load_session_plugins.
    let session_records = load_session_plugins(session_id)?;
    build_overview(&global_dir, &session_records, &session_dir)
}

fn build_overview(
    global_dir: &Path,
    session_records: &[PluginSourceRecord],
    session_dir: &Path,
) -> Result<SessionPluginsOverview, String> {
    // Session plugins come from the store manifest + trust overlay (#2942);
    // global stores keep directory discovery because they have no per-session
    // manifest.
    let shadowed: HashSet<String> = session_records
        .iter()
        .map(|r| r.manifest.source_id.clone())
        .collect();

    let mut active = session_records
        .iter()
        .map(|r| entry_from_record(r, PluginScope::Session))
        .collect::<Vec<_>>();
    active.extend(
        list_sources_in(global_dir)?
            .iter()
            .filter(|r| !shadowed.contains(&r.manifest.source_id))
            .map(|r| entry_from_record(r, PluginScope::Global)),
    );

    Ok(SessionPluginsOverview {
        active,
        history: read_history(session_dir),
    })
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
    }
}

fn normalize_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_ascii_lowercase()
}

fn matches_target(record: &PluginSourceRecord, target: &str) -> bool {
    let manifest = &record.manifest;
    if manifest.source_id == target || normalize_url(&manifest.source_url) == normalize_url(target)
    {
        return true;
    }
    // Repository name, e.g. `superpowers` for https://github.com/obra/superpowers.
    parse_github_repo_url(&manifest.source_url)
        .map(|spec| spec.repo.eq_ignore_ascii_case(target))
        .unwrap_or(false)
}

fn read_history(plugins_dir: &Path) -> Vec<SessionPluginHistoryEntry> {
    let path = plugins_dir.join(HISTORY_FILE);
    let Ok(raw) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str(&raw).unwrap_or_else(|error| {
        warn!("Ignoring unreadable {}: {}", path.display(), error);
        Vec::new()
    })
}

/// Best-effort: a failed history write must not fail the install itself.
fn append_history(plugins_dir: &Path, entry: SessionPluginHistoryEntry) {
    let mut history = read_history(plugins_dir);
    history.push(entry);
    let write = fs::create_dir_all(plugins_dir)
        .map_err(|e| e.to_string())
        .and_then(|_| serde_json::to_string_pretty(&history).map_err(|e| e.to_string()))
        .and_then(|json| {
            fs::write(plugins_dir.join(HISTORY_FILE), json).map_err(|e| e.to_string())
        });
    if let Err(error) = write {
        warn!("Failed to record session plugin history: {}", error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::hooks::context::{clear_hook_context, get_hook_context};
    use crate::services::hooks::execute::run_session_start_with_sources;
    use crate::services::hooks::registry::{
        compute_handler_approval_hashes, read_session_plugin_trust, read_store_manifest,
        save_source_in, PluginSourceManifest, SESSION_STORE_MANIFEST_FILE,
    };
    use crate::services::hooks::schema::load_hooks_from_root;
    use crate::services::skill_service::contracts::SkillImportKind;
    use crate::services::skill_service::importing::prepare_local_skill_import_blocking;
    use tempfile::TempDir;

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
                format!(
                    r#"{{ "hooks": {{ "SessionStart": [ {{ "command": "echo {echo}" }} ] }} }}"#
                ),
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
        let prepared =
            prepare_local_skill_import_blocking(&fx.temp_imports, &repo.to_string_lossy())?;
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
            },
        )
        .unwrap();
    }

    #[test]
    fn install_records_scope_url_time_hooks_and_skills() {
        let fx = fixture();
        let repo = write_plugin_repo(fx.root.path(), "alpha", Some("hi"));
        install_from_dir(&fx, &repo, true).unwrap();

        let global = tempfile::tempdir().unwrap();
        let overview = overview_for(&fx, global.path());
        assert_eq!(overview.active.len(), 1);
        let entry = &overview.active[0];
        assert_eq!(entry.scope, PluginScope::Session);
        assert_eq!(entry.source_url, repo.to_string_lossy());
        assert!(entry.installed_at > 0);
        assert_eq!(entry.skill_names, vec!["alpha-skill".to_string()]);
        assert_eq!(entry.hook_events, vec!["SessionStart".to_string()]);
        assert!(entry.hooks_approved);
        assert_eq!(overview.history.len(), 1);
        assert_eq!(overview.history[0].action, SessionPluginAction::Install);
        // Skills land in the session workspace skills dir, not a global dir.
        assert!(fx.dirs.skills.is_dir());
    }

    #[test]
    fn hook_less_skill_bundle_is_still_listable_and_removable() {
        let fx = fixture();
        let repo = write_plugin_repo(fx.root.path(), "bundle", None);
        install_from_dir(&fx, &repo, false).unwrap();

        let global = tempfile::tempdir().unwrap();
        let overview = overview_for(&fx, global.path());
        assert_eq!(overview.active.len(), 1);
        assert!(overview.active[0].hook_events.is_empty());
        assert!(!overview.active[0].hooks_approved);

        uninstall_with_dirs(&fx.dirs, &repo.to_string_lossy()).unwrap();
        let overview = overview_for(&fx, global.path());
        assert!(overview.active.is_empty());
    }

    #[test]
    fn uninstall_by_url_removes_source_skills_and_logs_history() {
        let fx = fixture();
        let repo = write_plugin_repo(fx.root.path(), "alpha", Some("hi"));
        install_from_dir(&fx, &repo, true).unwrap();
        let skills_before = fs::read_dir(&fx.dirs.skills).unwrap().count();
        assert_eq!(skills_before, 1);

        let removed = uninstall_with_dirs(&fx.dirs, &repo.to_string_lossy()).unwrap();
        assert_eq!(removed.skill_names, vec!["alpha-skill".to_string()]);
        assert_eq!(fs::read_dir(&fx.dirs.skills).unwrap().count(), 0);

        let global = tempfile::tempdir().unwrap();
        let overview = overview_for(&fx, global.path());
        assert!(overview.active.is_empty());
        let actions: Vec<_> = overview.history.iter().map(|h| h.action).collect();
        assert_eq!(
            actions,
            vec![SessionPluginAction::Install, SessionPluginAction::Uninstall]
        );
    }

    #[test]
    fn matches_target_by_id_url_or_repo_name() {
        let record = PluginSourceRecord {
            manifest: PluginSourceManifest {
                mcp_servers: Vec::new(),
                mcp_approved: false,
                source_url: "https://github.com/obra/Superpowers/".to_string(),
                source_id: "plugin-0123456789abcdef".to_string(),
                skill_names: vec![],
                hooks: None,
                hooks_approved: false,
                approval_hashes: Default::default(),
                created_at: 0,
                plugin: None,
                hook_files: vec![],
            },
            root_dir: PathBuf::new(),
        };
        assert!(matches_target(&record, "plugin-0123456789abcdef"));
        assert!(matches_target(
            &record,
            "https://github.com/obra/superpowers"
        ));
        assert!(matches_target(&record, "superpowers"));
        assert!(!matches_target(&record, "other"));
    }

    #[test]
    fn uninstall_unknown_target_errors() {
        let fx = fixture();
        let err = uninstall_with_dirs(&fx.dirs, "nope").unwrap_err();
        assert!(err.contains("No plugin matching"));
        assert!(uninstall_with_dirs(&fx.dirs, "  ").is_err());
    }

    #[test]
    fn overview_dedups_global_and_session_with_session_winning() {
        let fx = fixture();
        let repo = write_plugin_repo(fx.root.path(), "alpha", Some("session-hook"));
        install_from_dir(&fx, &repo, true).unwrap();
        let session_entry = &overview_for(&fx, Path::new("/nonexistent")).active[0];

        let global = tempfile::tempdir().unwrap();
        // Same source id as the session install => shadowed.
        write_global_source(
            global.path(),
            &session_entry.id,
            &session_entry.source_url,
            "global-hook",
        );
        // A different global plugin stays visible.
        write_global_source(
            global.path(),
            "plugin-0000000000000001",
            "https://github.com/o/other",
            "other",
        );

        let overview = overview_for(&fx, global.path());
        let scopes: Vec<_> = overview
            .active
            .iter()
            .map(|e| (e.id.as_str(), e.scope))
            .collect();
        assert_eq!(overview.active.len(), 2, "no duplicate entry: {scopes:?}");
        assert_eq!(overview.active[0].scope, PluginScope::Session);
        assert_eq!(overview.active[1].id, "plugin-0000000000000001");
        assert_eq!(overview.active[1].scope, PluginScope::Global);
    }

    #[test]
    fn sessions_are_isolated_from_each_other() {
        let fx_a = fixture();
        let fx_b = fixture();
        let repo = write_plugin_repo(fx_a.root.path(), "alpha", Some("hi"));
        install_from_dir(&fx_a, &repo, true).unwrap();

        let global = tempfile::tempdir().unwrap();
        let a = overview_for(&fx_a, global.path());
        let b = overview_for(&fx_b, global.path());
        assert_eq!(a.active.len(), 1);
        assert!(b.active.is_empty() && b.history.is_empty());
        assert!(!fx_b.dirs.skills.exists());
    }

    #[test]
    fn install_writes_store_manifest_entry() {
        let fx = fixture();
        let repo = write_plugin_repo(fx.root.path(), "alpha", None);
        install_from_dir(&fx, &repo, false).unwrap();

        let entries = read_store_manifest(&fx.dirs.plugins);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source_url, repo.to_string_lossy());
        assert!(entries[0].installed_at > 0);
        assert!(fx.dirs.plugins.join(&entries[0].source_id).exists());
    }

    #[test]
    fn manifest_entry_with_missing_dir_is_skipped_without_failing() {
        let fx = fixture();
        let repo = write_plugin_repo(fx.root.path(), "alpha", None);
        install_from_dir(&fx, &repo, false).unwrap();
        let source_id = read_store_manifest(&fx.dirs.plugins)[0].source_id.clone();

        // Simulate the user deleting the plugin directory: the entry is
        // skipped with a log line, the overview still loads (D1 policy).
        fs::remove_dir_all(fx.dirs.plugins.join(&source_id)).unwrap();
        let global = tempfile::tempdir().unwrap();
        let overview = overview_for(&fx, global.path());
        assert!(overview.active.is_empty());
        assert_eq!(overview.history.len(), 1, "history still readable");
    }

    #[test]
    fn uninstall_removes_manifest_entry_and_files() {
        let fx = fixture();
        let repo = write_plugin_repo(fx.root.path(), "alpha", None);
        install_from_dir(&fx, &repo, false).unwrap();
        let source_id = read_store_manifest(&fx.dirs.plugins)[0].source_id.clone();

        uninstall_with_dirs(&fx.dirs, &repo.to_string_lossy()).unwrap();

        assert!(read_store_manifest(&fx.dirs.plugins).is_empty());
        assert!(!fx.dirs.plugins.join(&source_id).exists());
    }

    #[test]
    fn directory_without_manifest_entry_is_not_loaded() {
        let fx = fixture();
        let repo = write_plugin_repo(fx.root.path(), "alpha", None);
        install_from_dir(&fx, &repo, false).unwrap();
        let source_id = read_store_manifest(&fx.dirs.plugins)[0].source_id.clone();

        // Simulate removing only the manifest entry: the plugin must not load
        // even though its files are still on disk (#2942 load-from-manifest).
        remove_session_store_entry(&fx.dirs.plugins, &source_id).unwrap();
        assert!(fx.dirs.plugins.join(&source_id).is_dir());
        let global = tempfile::tempdir().unwrap();
        let overview = overview_for(&fx, global.path());
        assert!(overview.active.is_empty());
    }

    #[tokio::test]
    async fn session_install_survives_resume_and_runs_hooks() {
        let fx = fixture();
        let repo = write_plugin_repo(fx.root.path(), "alpha", Some("restored-bootstrap"));
        install_from_dir(&fx, &repo, true).unwrap();

        // "Resume": a later SessionStart(resume) reads only what is on disk.
        let global = tempfile::tempdir().unwrap();
        let sid = format!("resume-{}", uuid::Uuid::new_v4());
        clear_hook_context(&sid);
        run_session_start_with_sources(
            global.path(),
            Some(&fx.dirs.plugins),
            Some(&fx.dirs.trust),
            &sid,
            SessionStartSource::Resume,
            Some(fx.root.path().to_path_buf()),
        )
        .await;
        assert_eq!(
            get_hook_context(&sid).as_deref(),
            Some("restored-bootstrap")
        );
        clear_hook_context(&sid);
    }

    #[tokio::test]
    async fn unapproved_session_install_runs_no_hooks() {
        let fx = fixture();
        let repo = write_plugin_repo(fx.root.path(), "alpha", Some("must-not-run"));
        install_from_dir(&fx, &repo, false).unwrap();

        let global = tempfile::tempdir().unwrap();
        let sid = format!("unapproved-{}", uuid::Uuid::new_v4());
        clear_hook_context(&sid);
        run_session_start_with_sources(
            global.path(),
            Some(&fx.dirs.plugins),
            Some(&fx.dirs.trust),
            &sid,
            SessionStartSource::Resume,
            Some(fx.root.path().to_path_buf()),
        )
        .await;
        assert!(get_hook_context(&sid).is_none_or(|t| t.trim().is_empty()));
        clear_hook_context(&sid);
    }

    // --- #2942 gatekeeper G1.1 / G1.2 -----------------------------------------

    #[test]
    fn install_moves_approval_out_of_the_workspace_manifest() {
        let fx = fixture();
        let repo = write_plugin_repo(fx.root.path(), "alpha", Some("hi"));
        install_from_dir(&fx, &repo, true).unwrap();
        let source_id = read_store_manifest(&fx.dirs.plugins)[0].source_id.clone();

        // The workspace manifest on disk grants nothing: approval fields are
        // scrubbed at install, so a rewritten workspace file alone can never
        // re-approve hooks (#2942 gatekeeper G1.1).
        let on_disk = get_source_in(&fx.dirs.plugins, &source_id).unwrap();
        assert!(!on_disk.manifest.hooks_approved);
        assert!(on_disk.manifest.approval_hashes.is_empty());

        // Approval lives only in the out-of-workspace trust record, and its
        // hashes still match the installed handlers.
        let trust = read_session_plugin_trust(&fx.dirs.trust, &source_id);
        assert!(trust.hooks_approved);
        let root = fx.dirs.plugins.join(&source_id);
        let cfg = load_hooks_from_root(&root).unwrap();
        assert_eq!(
            trust.approval_hashes,
            compute_handler_approval_hashes(&root, &cfg)
        );

        // The single reader overlays the trust back onto the record.
        let global = tempfile::tempdir().unwrap();
        let overview = overview_for(&fx, global.path());
        assert!(overview.active[0].hooks_approved);
    }

    #[test]
    fn blocked_store_manifest_fails_install_and_cleans_up() {
        let fx = fixture();
        // A directory sitting where store-manifest.json must be written makes
        // every manifest write fail. The install must report failure and roll
        // back the just-written source instead of claiming success (#2942
        // gatekeeper G1.2).
        fs::create_dir_all(fx.dirs.plugins.join(SESSION_STORE_MANIFEST_FILE)).unwrap();
        let repo = write_plugin_repo(fx.root.path(), "alpha", None);
        let err = install_from_dir(&fx, &repo, false).unwrap_err();
        assert!(err.contains("store manifest"), "{err}");
        assert!(
            list_sources_in(&fx.dirs.plugins).unwrap().is_empty(),
            "a rolled-back install must not leave the plugin source behind"
        );
        if fx.dirs.trust.exists() {
            assert_eq!(fs::read_dir(&fx.dirs.trust).unwrap().count(), 0);
        }
    }
}
