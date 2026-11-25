use super::discovery::discover_agent_files;
use super::ids::{is_plugin_agent_id, plugin_source_marker, plugin_source_of_config};
use super::preview::synth_assistant_model;
use crate::services::hooks::registry::PluginSourceRecord;
use crate::services::skill_service::plugin_commands::{manifest_for_discovery, plugin_namespace};

/// Delete every global assistant row owned by `source_id` (plugin
/// uninstall, and the remove-first step of every re-sync).
pub async fn remove_global_plugin_agents(source_id: &str) -> Result<(), String> {
    use crate::repositories::AssistantRepository;
    let repo = crate::state::get_assistant_repository();
    let marker = plugin_source_marker(source_id);
    let rows = repo
        .list_assistants()
        .await
        .map_err(|e| format!("list assistants: {e}"))?;
    for row in rows {
        if !is_plugin_agent_id(&row.id) {
            continue;
        }
        if plugin_source_of_config(&row.config).as_deref() != Some(marker.as_str()) {
            continue;
        }
        repo.delete_assistant(&row.id)
            .await
            .map_err(|e| format!("delete plugin agent {}: {e}", row.id))?;
    }
    Ok(())
}

/// Re-sync one installed global plugin source's assistant rows (D2).
/// Existing rows owned by this source are ALWAYS removed first so an
/// update that drops agents converges exactly on the current definitions
/// (same remove-before-every-return rule as #2924 G1.1).
pub async fn sync_global_plugin_agents(record: &PluginSourceRecord) -> Result<(), String> {
    use crate::repositories::AssistantRepository;
    let source_id = record.manifest.source_id.as_str();
    remove_global_plugin_agents(source_id).await?;
    let manifest = manifest_for_discovery(record);
    let agents = discover_agent_files(&record.root_dir, manifest.as_ref());
    let display_name = plugin_namespace(record);
    let repo = crate::state::get_assistant_repository();
    for agent in &agents {
        let model = synth_assistant_model(agent, source_id, &display_name, "global");
        repo.create_assistant(model.id, model.name, model.config)
            .await
            .map_err(|e| format!("create plugin agent row for {}: {e}", agent.name))?;
    }
    Ok(())
}
