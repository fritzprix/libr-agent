//! Session plugin uninstall, runtime-state purge, and the removal helpers
//! shared with the install rollback path (#2825, #2926, #2942).

use super::history::append_history;
use super::targets::matches_target;
use super::{
    entry_from_record, session_dirs, PluginScope, SessionPluginAction, SessionPluginDirs,
    SessionPluginEntry, SessionPluginHistoryEntry,
};
use crate::services::hooks::registry::{
    delete_source_in, list_sources_in, remove_session_plugin_trust, remove_session_store_entry,
};
use crate::services::hooks::{clear_hook_context, spawn_session_start, SessionStartSource};
use crate::services::skill_service::cache::invalidate_skill_scan_cache;
use crate::services::skill_service::importing::remove_skill_names_from_user_skills;
use crate::services::skill_service::{plugin_options, plugin_sources};
use std::path::Path;

/// Remove a session plugin by source URL, source id, or repository name.
///
/// The session's hook-context slot is dropped and SessionStart re-run so the
/// removed plugin's bootstrap does not linger while global plugins re-apply.
pub async fn uninstall_session_plugin(
    session_id: &str,
    target: &str,
) -> Result<SessionPluginEntry, String> {
    let dirs = session_dirs(session_id)?;
    let removed = uninstall_with_dirs(&dirs, target)?;
    // `get_session_plugin_data_dir` needs the session manager, which unit
    // tests do not have, so the resolved location (possibly absent) is
    // injected into the cleanup body — tests run the same body with a temp
    // directory (#2926 gatekeeper G1.4 AC5).
    let data_dir =
        crate::services::hooks::registry::get_session_plugin_data_dir(session_id, &removed.id).ok();
    cleanup_removed_session_plugin(&dirs.trust, data_dir.as_deref(), session_id, &removed.id).await;
    invalidate_skill_scan_cache();
    clear_hook_context(session_id);
    spawn_session_start(session_id, SessionStartSource::Resume);
    Ok(removed)
}

/// Delete everything a session plugin keeps outside the plugin source itself:
/// its `${CLAUDE_PLUGIN_DATA}` directory, its option sidecar in the trust
/// directory, and — scoped to this session — its sensitive `userConfig`
/// values (#2926 gatekeeper G1.2: a session uninstall removes exactly its
/// own secrets; a global install of the same source is untouched).
pub(super) async fn cleanup_removed_session_plugin(
    trust_dir: &Path,
    data_dir: Option<&Path>,
    session_id: &str,
    source_id: &str,
) {
    if let Some(dir) = data_dir {
        if let Err(error) = plugin_sources::remove_plugin_data_dir(dir) {
            log::warn!(
                "session {}: plugin {} data dir left behind: {}",
                session_id,
                source_id,
                error
            );
        }
    }
    let saved = plugin_options::read_saved_options(trust_dir, source_id);
    if let Err(error) = plugin_options::remove_saved_options(trust_dir, source_id) {
        log::warn!(
            "session {}: plugin {} stored options left behind: {}",
            session_id,
            source_id,
            error
        );
    }
    if let Err(error) =
        plugin_options::delete_sensitive_options(source_id, Some(session_id), &saved.sensitive_keys)
            .await
    {
        log::warn!(
            "session {}: plugin {} secure option values left behind: {}",
            session_id,
            source_id,
            error
        );
    }
}

/// Remove every trace of one session's plugin runtime state from the app
/// data directory (#2926 gatekeeper G1.3), called by
/// `SessionDirectoryService::remove_workspace` **before** it deletes the
/// trust directory: the session-scoped sensitive option values (whose key
/// names are read from each sidecar in the trust directory), the option
/// cache entries for that store (S1.3), the in-memory `CLAUDE_ENV_FILE`
/// variables, the `CLAUDE_ENV_FILE` directory itself, and the
/// `${CLAUDE_PLUGIN_DATA}` tree. A missing directory is fine; real failures
/// are collected and returned so nothing silently lingers behind a deleted
/// session.
pub async fn purge_session_plugin_state(
    base_data_dir: &Path,
    session_id: &str,
) -> Result<(), String> {
    use crate::services::hooks::registry::{
        PLUGIN_DATA_DIR_NAME, PLUGIN_ENV_DIR_NAME, SESSION_PLUGIN_TRUST_DIR_NAME,
    };
    crate::services::hooks::plugin_env::clear_session_plugin_env(session_id);
    let trust_dir = base_data_dir
        .join(SESSION_PLUGIN_TRUST_DIR_NAME)
        .join(session_id);
    let mut failures: Vec<String> = Vec::new();
    for source_id in plugin_options::list_sidecar_source_ids(&trust_dir) {
        let saved = plugin_options::read_saved_options(&trust_dir, &source_id);
        if let Err(error) = plugin_options::delete_sensitive_options(
            &source_id,
            Some(session_id),
            &saved.sensitive_keys,
        )
        .await
        {
            failures.push(format!(
                "session {session_id}: plugin {source_id} secure option values: {error}"
            ));
        }
    }
    plugin_options::forget_cached_store(&trust_dir);
    for dir in [
        base_data_dir.join(PLUGIN_ENV_DIR_NAME).join(session_id),
        base_data_dir
            .join(PLUGIN_DATA_DIR_NAME)
            .join("sessions")
            .join(session_id),
    ] {
        match tokio::fs::remove_dir_all(&dir).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => failures.push(format!("remove {}: {error}", dir.display())),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

pub(super) fn uninstall_with_dirs(
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
    remove_skill_names_from_user_skills(
        &dirs.skills,
        &record.manifest.skill_names,
        &record.manifest.source_id,
    )?;
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
