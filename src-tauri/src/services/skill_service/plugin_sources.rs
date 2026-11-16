use super::contracts::PluginSourceOverview;
use super::directories::get_user_skills_directory;
use super::importing::remove_skill_names_from_user_skills;
use crate::services::hooks::registry::{delete_source, get_source, list_sources};
use crate::services::hooks::schema::load_hooks_from_root;

pub fn list_plugin_source_overviews() -> Result<Vec<PluginSourceOverview>, String> {
    let records = list_sources()?;
    let mut out = records
        .into_iter()
        .map(|record| {
            let hook_events = record
                .manifest
                .hooks
                .as_ref()
                .map(|_| hook_events_for_root(&record.root_dir))
                .unwrap_or_default();
            PluginSourceOverview {
                id: record.manifest.source_id.clone(),
                source_url: record.manifest.source_url.clone(),
                skill_names: record.manifest.skill_names.clone(),
                hook_events,
                hooks_approved: record.manifest.hooks_approved,
            }
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

pub(super) fn hook_events_for_root(root: &std::path::Path) -> Vec<String> {
    load_hooks_from_root(root)
        .ok()
        .map(|cfg| {
            let mut events: Vec<String> = cfg
                .events
                .keys()
                .map(|event| event.as_str().to_string())
                .collect();
            events.sort();
            events.dedup();
            events
        })
        .unwrap_or_default()
}

pub async fn delete_plugin_source(source_id: String) -> Result<(), String> {
    let record = get_source(&source_id)?;
    let user_dir = get_user_skills_directory()?;
    remove_skill_names_from_user_skills(&user_dir, &record.manifest.skill_names)?;
    delete_source(&source_id)?;
    // #2812 criterion 4: uninstalling a source must not leave its hook
    // bootstrap in any open session's prompt slot.
    on_plugin_source_uninstalled();
    super::cache::invalidate_skill_scan_cache();
    Ok(())
}

/// Drop every session's hook-context slot after a plugin source uninstall.
/// Kept as a named step so criterion-4 tests can exercise the same path
/// `delete_plugin_source` uses without needing a live SessionManager.
pub(crate) fn on_plugin_source_uninstalled() {
    crate::services::hooks::clear_all_hook_contexts();
}

#[cfg(test)]
mod tests {
    use super::on_plugin_source_uninstalled;
    use crate::services::hooks::{clear_hook_context, get_hook_context, set_hook_context};

    #[test]
    fn uninstall_purge_clears_hook_contexts() {
        let sid = "test-plugin-uninstall-purge";
        clear_hook_context(sid);
        set_hook_context(sid, "stale bootstrap".to_string());
        on_plugin_source_uninstalled();
        assert!(
            get_hook_context(sid).is_none(),
            "delete_plugin_source must clear hook contexts via on_plugin_source_uninstalled"
        );
    }
}
