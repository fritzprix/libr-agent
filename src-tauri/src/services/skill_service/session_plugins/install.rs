//! Session plugin install: preview and install a plugin into one session's
//! skill/plugin stores (#2825, storage revised by #2942).

use super::history::append_history;
use super::{session_dirs, SessionPluginAction, SessionPluginDirs, SessionPluginHistoryEntry};
use crate::services::hooks::registry::{
    delete_source_in, get_source_in, move_session_approval_to_trust, remove_session_plugin_trust,
    upsert_session_store_entry, SessionPluginStoreEntry,
};
use crate::services::hooks::{spawn_session_start, SessionStartSource};
use crate::services::skill_service::cache::invalidate_skill_scan_cache;
use crate::services::skill_service::contracts::{
    PreparedSkillImport, SkillImportPreview, SkillImportResult,
};
use crate::services::skill_service::directories::get_system_skills_directory;
use crate::services::skill_service::importing::{
    build_import_preview, install_prepared_import, plugin_source_id, prepare_github_skill_import,
    prepare_plugin_user_config, save_plugin_user_config, ImportTarget,
};
use std::collections::BTreeMap;
use std::path::Path;

/// Skill names recorded in this session's plugin-source store for the same
/// source key. Re-installs of the same manifest plugin must not namespace-
/// rename against their own previously installed skills (gatekeeper G1.1).
fn session_same_source_skill_names(plugins_dir: &Path, source_key: &str) -> Vec<String> {
    get_source_in(plugins_dir, &plugin_source_id(source_key))
        .map(|record| record.manifest.skill_names)
        .unwrap_or_default()
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
        // Session plugin skills carry no owner marker (D2), so there is no
        // owning source id to exempt here.
        None,
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
        // Session plugin skills carry no owner marker (D2).
        None,
    )
    .await?;
    // #2926: check the values the user entered before anything is written, so a
    // missing required option fails the install cleanly.
    let user_config = prepare_plugin_user_config(&prepared.scan_root, &plugin_options)?;
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
        // #2926 gatekeeper G1.1: a session's option sidecar lives in that
        // session's out-of-workspace trust directory, never in the
        // agent-writable plugin store; G1.2: the secure namespace is scoped
        // to this session.
        if let Err(error) = save_plugin_user_config(
            &dirs.trust,
            &source_id,
            Some(&session_id),
            &manifest,
            &values,
        )
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

pub(super) fn install_prepared_with_dirs(
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
            // Session plugin skills live in the session's own skill layer,
            // not in shared user storage; the #2927 D2 read-only marker is
            // only for user-owned global skills.
            owner_source_id: None,
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
