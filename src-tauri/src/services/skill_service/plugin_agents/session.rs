use super::discovery::{discover_agent_files, DiscoveredAgent};
use super::ids::{parse_plugin_agent_id, plugin_agent_id};
use super::preview::{agent_config_json, synth_assistant_model};
use crate::entity::assistant::Model as AssistantModel;
use crate::services::hooks::registry::PluginSourceRecord;
use crate::services::skill_service::plugin_commands::{manifest_for_discovery, plugin_namespace};
use crate::services::skill_service::session_plugins;
use log::warn;
use serde_json::Value;
use std::path::Path;

/// The session-agent pairs for already-resolved store records. Split out so
/// the resolution is testable without session-manager state.
pub fn session_plugin_agents_from_records(
    records: &[PluginSourceRecord],
) -> Vec<(PluginSourceRecord, DiscoveredAgent)> {
    let mut out = Vec::new();
    for record in records {
        let manifest = manifest_for_discovery(record);
        for agent in discover_agent_files(&record.root_dir, manifest.as_ref()) {
            out.push((record.clone(), agent));
        }
    }
    out
}

/// Resolve one session-scope plugin agent into a synthesized read-only
/// assistant model, looking in `scope_session_id`'s session plugin store.
pub fn resolve_session_plugin_assistant(
    scope_session_id: &str,
    source_id: &str,
    agent_name: &str,
) -> Option<AssistantModel> {
    let records = session_plugins::load_session_plugins(scope_session_id).ok()?;
    resolve_session_plugin_assistant_from_records(&records, source_id, agent_name)
}

/// The pure core of [`resolve_session_plugin_assistant`] (testable).
pub fn resolve_session_plugin_assistant_from_records(
    records: &[PluginSourceRecord],
    source_id: &str,
    agent_name: &str,
) -> Option<AssistantModel> {
    let record = records.iter().find(|r| r.manifest.source_id == source_id)?;
    let manifest = manifest_for_discovery(record);
    let agent = discover_agent_files(&record.root_dir, manifest.as_ref())
        .into_iter()
        .find(|a| a.name.eq_ignore_ascii_case(agent_name))?;
    Some(synth_assistant_model(
        &agent,
        source_id,
        &plugin_namespace(record),
        "session",
    ))
}

/// Matcher scope name for one plugin source (#3003 G2.3): the plugin's
/// namespace ([`plugin_namespace`] — manifest name, else repository name,
/// else source id), looked up in the caller session's plugin store first
/// (a session install shadows a global install with the same source id,
/// #2825) and then in the global store. `None` when neither store knows it.
pub fn resolve_plugin_source_namespace(source_id: &str, session_id: &str) -> Option<String> {
    let session_records = session_plugins::load_session_plugins(session_id).unwrap_or_default();
    let global_base = crate::services::hooks::registry::get_user_plugins_directory().ok();
    resolve_plugin_source_namespace_in(source_id, &session_records, global_base.as_deref())
}

/// Path-injected core of [`resolve_plugin_source_namespace`] so tests cover
/// successful global and session lookups without the real stores (#3003
/// G2.3). `global_base` is the global plugin store directory, read with the
/// same [`crate::services::hooks::registry::get_source_in`] the production
/// path uses through `get_source`.
pub fn resolve_plugin_source_namespace_in(
    source_id: &str,
    session_records: &[PluginSourceRecord],
    global_base: Option<&Path>,
) -> Option<String> {
    if let Some(record) = session_records
        .iter()
        .find(|r| r.manifest.source_id == source_id)
    {
        return Some(plugin_namespace(record));
    }
    let base = global_base?;
    crate::services::hooks::registry::get_source_in(base, source_id)
        .ok()
        .map(|record| plugin_namespace(&record))
}

/// The declared plugin `model` for an assistant id, if any: first from the
/// global store row, then from the caller session's plugin store. The
/// spawn path maps it through [`plugin_model_to_complexity`].
pub async fn declared_plugin_agent_model(
    assistant_id: &str,
    caller_session_id: Option<&str>,
) -> Option<String> {
    let (source_id, agent) = parse_plugin_agent_id(assistant_id)?;
    use crate::repositories::AssistantRepository;
    if let Ok(Some(row)) = crate::state::get_assistant_repository()
        .get_assistant(assistant_id)
        .await
    {
        let config: Value = serde_json::from_str(&row.config).ok()?;
        return config
            .get("pluginAgentModel")
            .and_then(Value::as_str)
            .map(str::to_string);
    }
    let records =
        caller_session_id.and_then(|sid| session_plugins::load_session_plugins(sid).ok())?;
    records
        .iter()
        .find(|r| r.manifest.source_id == source_id)
        .and_then(|record| {
            let manifest = manifest_for_discovery(record);
            discover_agent_files(&record.root_dir, manifest.as_ref())
                .into_iter()
                .find(|a| a.name.eq_ignore_ascii_case(agent))
        })
        .and_then(|agent| agent.model)
}

/// Every plugin agent effective for one session, for merging into
/// `agent__listAgents(type=configs)`. Session-scoped plugin agents only —
/// global-scope plugin agents are already real assistant rows. Session
/// plugins shadow a global plugin with the same `source_id`, so the
/// caller removes ids that the session store now answers (`shadowed_ids`).
pub fn session_plugin_agent_infos(session_id: &str) -> Vec<PluginAgentListInfo> {
    let records = match session_plugins::load_session_plugins(session_id) {
        Ok(records) => records,
        Err(error) => {
            warn!("session plugin store for {session_id}: {error}");
            Vec::new()
        }
    };
    infos_from_records(&records)
}

/// One list entry for a plugin agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginAgentListInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub plugin_name: String,
    /// The same config JSON a global store row would carry (system prompt,
    /// aliases, ownership marker) so list views can show capabilities.
    pub config: Value,
}

/// The pure core of [`session_plugin_agent_infos`] (testable).
pub fn infos_from_records(records: &[PluginSourceRecord]) -> Vec<PluginAgentListInfo> {
    session_plugin_agents_from_records(records)
        .into_iter()
        .map(|(record, agent)| PluginAgentListInfo {
            id: plugin_agent_id(&record.manifest.source_id, &agent.name),
            name: agent.name.clone(),
            description: agent.description.clone(),
            plugin_name: plugin_namespace(&record),
            config: agent_config_json(
                &agent,
                &record.manifest.source_id,
                &plugin_namespace(&record),
                "session",
            ),
        })
        .collect()
}

/// The source ids of a session's plugin store, used to hide the global
/// assistant rows shadowed by a session install of the same plugin.
pub fn session_store_source_ids(session_id: &str) -> Vec<String> {
    session_plugins::load_session_plugins(session_id)
        .map(|records| records.into_iter().map(|r| r.manifest.source_id).collect())
        .unwrap_or_default()
}
