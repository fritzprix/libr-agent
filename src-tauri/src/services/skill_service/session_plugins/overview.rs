//! Session plugin list view: session installs shadow global installs of the
//! same source id (#2825, #2942).

use super::history::read_history;
use super::{entry_from_record, load_session_plugins, PluginScope, SessionPluginsOverview};
use crate::services::hooks::registry::{
    get_session_plugins_directory, get_user_plugins_directory, list_sources_in, PluginSourceRecord,
};
use std::collections::HashSet;
use std::path::Path;

pub fn list_session_plugins(session_id: &str) -> Result<SessionPluginsOverview, String> {
    let session_dir = get_session_plugins_directory(session_id)?;
    let global_dir = get_user_plugins_directory()?;
    // #2942 G1.3: one read path for session plugins — load_session_plugins.
    let session_records = load_session_plugins(session_id)?;
    build_overview(&global_dir, &session_records, &session_dir)
}

pub(super) fn build_overview(
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
