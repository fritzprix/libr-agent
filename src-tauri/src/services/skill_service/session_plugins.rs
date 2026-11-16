//! Session-scoped plugin install / uninstall / list (#2825).
//!
//! A session install reuses the global plugin pipeline from `importing.rs`
//! with a different target: plugin sources go to the session's own store
//! (`{data_dir}/session_plugins/{session_id}/`) and skills go to the session's
//! workspace skills directory, which already outranks global skills.
//!
//! Precedence: a session install wins over a global install with the same
//! `source_id` for that session only (`hooks::registry::list_effective_approved_sources_in`).

use super::cache::invalidate_skill_scan_cache;
use super::contracts::{PreparedSkillImport, SkillImportPreview, SkillImportResult};
use super::directories::get_workspace_skills_directory_for_session;
use super::github::parse_github_repo_url;
use super::importing::{
    build_import_preview, install_prepared_import, plugin_source_id, prepare_github_skill_import,
    remove_skill_names_from_user_skills, ImportTarget,
};
use super::plugin_sources::hook_events_for_root;
use crate::services::hooks::registry::{
    delete_source_in, get_session_plugins_directory, get_user_plugins_directory, list_sources_in,
    PluginSourceRecord,
};
use crate::services::hooks::{clear_hook_context, spawn_session_start, SessionStartSource};
use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
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
}

fn session_dirs(session_id: &str) -> Result<SessionPluginDirs, String> {
    Ok(SessionPluginDirs {
        skills: get_workspace_skills_directory_for_session(session_id)?,
        plugins: get_session_plugins_directory(session_id)?,
    })
}

pub async fn preview_session_plugin_install(
    session_id: String,
    repo_url: String,
) -> Result<SkillImportPreview, String> {
    let dirs = session_dirs(&session_id)?;
    let prepared = prepare_github_skill_import(repo_url).await?;
    build_import_preview(&prepared, &dirs.skills, "workspace", "workspace").await
}

/// Install a plugin for one session, then fire SessionStart with
/// `source=resume` so its hooks take effect immediately.
pub async fn install_session_plugin(
    session_id: String,
    repo_url: String,
    overwrite_existing: bool,
    excluded_skill_names: Option<Vec<String>>,
    approve_hooks: Option<bool>,
) -> Result<SkillImportResult, String> {
    let dirs = session_dirs(&session_id)?;
    let prepared = prepare_github_skill_import(repo_url).await?;
    let preview = build_import_preview(&prepared, &dirs.skills, "workspace", "workspace").await?;
    let result = install_prepared_with_dirs(
        &dirs,
        prepared,
        preview,
        overwrite_existing,
        excluded_skill_names.unwrap_or_default(),
        approve_hooks.unwrap_or(false),
    )?;
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
    build_overview(&global_dir, &session_dir)
}

fn build_overview(global_dir: &Path, session_dir: &Path) -> Result<SessionPluginsOverview, String> {
    let session_records = list_sources_in(session_dir)?;
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
            hook_events_for_root(&record.root_dir)
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
        compute_handler_approval_hashes, save_source_in, PluginSourceManifest,
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

    fn fixture() -> Fixture {
        let root = tempfile::tempdir().unwrap();
        let dirs = SessionPluginDirs {
            skills: root.path().join("workspace/.libragent/skills"),
            plugins: root.path().join("session_plugins/s1"),
        };
        let temp_imports = root.path().join("imports");
        fs::create_dir_all(&temp_imports).unwrap();
        Fixture {
            root,
            dirs,
            temp_imports,
        }
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
                source_url: url.to_string(),
                source_id: source_id.to_string(),
                skill_names: vec![],
                hooks: None,
                hooks_approved: true,
                approval_hashes: hashes,
                created_at: 1,
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
        let overview = build_overview(global.path(), &fx.dirs.plugins).unwrap();
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
        let overview = build_overview(global.path(), &fx.dirs.plugins).unwrap();
        assert_eq!(overview.active.len(), 1);
        assert!(overview.active[0].hook_events.is_empty());
        assert!(!overview.active[0].hooks_approved);

        uninstall_with_dirs(&fx.dirs, &repo.to_string_lossy()).unwrap();
        let overview = build_overview(global.path(), &fx.dirs.plugins).unwrap();
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
        let overview = build_overview(global.path(), &fx.dirs.plugins).unwrap();
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
                source_url: "https://github.com/obra/Superpowers/".to_string(),
                source_id: "plugin-0123456789abcdef".to_string(),
                skill_names: vec![],
                hooks: None,
                hooks_approved: false,
                approval_hashes: Default::default(),
                created_at: 0,
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
        let session_entry = &build_overview(Path::new("/nonexistent"), &fx.dirs.plugins)
            .unwrap()
            .active[0];

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

        let overview = build_overview(global.path(), &fx.dirs.plugins).unwrap();
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
        let a = build_overview(global.path(), &fx_a.dirs.plugins).unwrap();
        let b = build_overview(global.path(), &fx_b.dirs.plugins).unwrap();
        assert_eq!(a.active.len(), 1);
        assert!(b.active.is_empty() && b.history.is_empty());
        assert!(!fx_b.dirs.skills.exists());
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
            &sid,
            SessionStartSource::Resume,
            Some(fx.root.path().to_path_buf()),
        )
        .await;
        assert!(get_hook_context(&sid).is_none_or(|t| t.trim().is_empty()));
        clear_hook_context(&sid);
    }
}
