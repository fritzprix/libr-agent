//! Target resolution for `/plugin enable|disable|update`: a session install is
//! matched first, then a global install (#2927).

use super::{
    entry_from_record, load_session_plugins, session_dirs, PluginScope, SessionPluginDirs,
    SessionPluginEntry,
};
use crate::services::hooks::clear_hook_context;
use crate::services::hooks::registry::{
    get_source_in, get_user_plugins_directory, list_sources_in, set_session_source_enabled_in,
    PluginSourceRecord,
};
use crate::services::skill_service::github::parse_github_repo_url;
use std::path::Path;

fn normalize_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_ascii_lowercase()
}

/// Turn one plugin on or off by name, URL, or source id (#2927).
///
/// A session install is matched first (it is what `/plugin` manages), then a
/// global install, so one command covers both scopes. The files stay on disk
/// either way; the change applies when a session opens or on `/reload`
/// (owner decision D5).
pub fn set_plugin_enabled_by_target(
    session_id: &str,
    target: &str,
    enabled: bool,
) -> Result<SessionPluginEntry, String> {
    let target = target.trim();
    if target.is_empty() {
        return Err("Specify a plugin name, git URL, or id".to_string());
    }
    let dirs = session_dirs(session_id)?;
    let global_dir = get_user_plugins_directory()?;
    let (scope, source_id) = resolve_plugin_target(&dirs, &global_dir, target)?;
    match scope {
        PluginScope::Session => {
            // #2927 gatekeeper G1.1: a session plugin's state is trust data,
            // never the agent-writable workspace manifest.
            let saved =
                set_session_source_enabled_in(&dirs.trust, &dirs.plugins, &source_id, enabled)?;
            clear_hook_context(session_id);
            Ok(entry_from_record(&saved, PluginScope::Session))
        }
        PluginScope::Global => {
            crate::services::skill_service::set_plugin_enabled(&source_id, enabled)?;
            let saved = get_source_in(&global_dir, &source_id)?;
            Ok(entry_from_record(&saved, PluginScope::Global))
        }
    }
}

/// Which installed plugin a `/plugin enable|disable` target means: a session
/// install first (it is what `/plugin` manages), then a global install. Split
/// from the command so the match order and its errors are testable without an
/// app data directory (gatekeeper G1.2).
pub(super) fn resolve_plugin_target(
    dirs: &SessionPluginDirs,
    global_dir: &Path,
    target: &str,
) -> Result<(PluginScope, String), String> {
    let session_match = matching_records(&dirs.plugins, target)?;
    if !session_match.is_empty() {
        return Ok((
            PluginScope::Session,
            single_match(session_match, target)?.manifest.source_id,
        ));
    }
    let global_match = matching_records(global_dir, target)?;
    if global_match.is_empty() {
        return Err(format!(
            "No plugin matching '{target}' is installed. Global plugins are managed under \
             Extensions → Skills."
        ));
    }
    Ok((
        PluginScope::Global,
        single_match(global_match, target)?.manifest.source_id,
    ))
}

fn matching_records(base: &Path, target: &str) -> Result<Vec<PluginSourceRecord>, String> {
    Ok(list_sources_in(base)?
        .into_iter()
        .filter(|record| matches_target(record, target))
        .collect())
}

/// Resolve a `/plugin update` target to its installed plugin (#2927 item 4).
/// The chat input needs the source URL and the scope before it can run the
/// update flow, and the match order is exactly what enable and disable use:
/// session install first, then global.
pub fn resolve_plugin_by_target(
    session_id: &str,
    target: &str,
) -> Result<SessionPluginEntry, String> {
    let target = target.trim();
    if target.is_empty() {
        return Err("Specify a plugin name, git URL, or id".to_string());
    }
    let dirs = session_dirs(session_id)?;
    let global_dir = get_user_plugins_directory()?;
    let (scope, source_id) = resolve_plugin_target(&dirs, &global_dir, target)?;
    let record = match scope {
        PluginScope::Session => load_session_plugins(session_id)?
            .into_iter()
            .find(|record| record.manifest.source_id == source_id)
            .ok_or_else(|| {
                format!("Plugin {source_id} matched but is missing from the session store")
            })?,
        PluginScope::Global => get_source_in(&global_dir, &source_id)?,
    };
    Ok(entry_from_record(&record, scope))
}

fn single_match(
    mut matches: Vec<PluginSourceRecord>,
    target: &str,
) -> Result<PluginSourceRecord, String> {
    match matches.len() {
        1 => Ok(matches.remove(0)),
        _ => {
            let urls = matches
                .iter()
                .map(|r| r.manifest.source_url.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            Err(format!("'{target}' is ambiguous: {urls}"))
        }
    }
}

pub(super) fn matches_target(record: &PluginSourceRecord, target: &str) -> bool {
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
