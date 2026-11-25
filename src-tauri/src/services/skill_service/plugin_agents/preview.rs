use super::discovery::{discover_agent_files, DiscoveredAgent};
use super::ids::{plugin_agent_id, plugin_source_marker, PLUGIN_SOURCE_KEY};
use super::mapping::map_plugin_tools;
use crate::entity::assistant::Model as AssistantModel;
use crate::services::skill_service::plugin_manifest::PluginManifest;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::Path;

/// Preview entry for one plugin agent (#2923 approach 1, gatekeeper G1.2):
/// shows what the user is about to grant (`grantedServices`) and what is
/// dropped because it has no mapping (`unmappedTools`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAgentPreview {
    pub name: String,
    pub description: String,
    /// Claude Code tool names with no LibrAgent mapping; they are shown to
    /// the user and never silently granted. Restricted entries such as
    /// `Bash(git:*)` count as unmapped.
    pub unmapped_tools: Vec<String>,
    /// LibrAgent builtin service aliases the declared `tools` map to. The
    /// `workspace` alias includes file writes and shell execution — there
    /// is no narrower read-only alias, so this disclosure is the
    /// mitigation (#2923 gatekeeper G1.2).
    pub granted_services: Vec<String>,
    /// True when the definition declares no `tools` line at all: every
    /// builtin service is then granted (the LibrAgent default).
    pub grants_all_services: bool,
    /// Declared `model` frontmatter, when present and not `inherit`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// All preview entries for a plugin root, in discovery order.
pub fn plugin_agent_previews(
    root: &Path,
    manifest: Option<&PluginManifest>,
) -> Vec<PluginAgentPreview> {
    discover_agent_files(root, manifest)
        .into_iter()
        .map(|agent| {
            let (granted_services, unmapped_tools) = map_plugin_tools(&agent.tools);
            PluginAgentPreview {
                name: agent.name,
                description: agent.description,
                unmapped_tools,
                granted_services,
                grants_all_services: agent.tools.is_empty(),
                model: agent.model,
            }
        })
        .collect()
}

/// The assistant config JSON for one plugin agent definition. `scope` is
/// `"global"` for store rows; the session-scope synthesis uses `"session"`.
pub fn agent_config_json(
    agent: &DiscoveredAgent,
    source_id: &str,
    plugin_display_name: &str,
    scope: &str,
) -> Value {
    let mut config = Map::new();
    config.insert(
        "systemPrompt".into(),
        Value::String(agent.system_prompt.clone()),
    );
    config.insert(
        "description".into(),
        Value::String(agent.description.clone()),
    );
    // UI-level protection too; the hard guarantee is the backend guard.
    config.insert("deletionProtected".into(), Value::Bool(true));
    config.insert(
        PLUGIN_SOURCE_KEY.into(),
        Value::String(plugin_source_marker(source_id)),
    );
    config.insert(
        "pluginSourceName".into(),
        Value::String(plugin_display_name.to_string()),
    );
    config.insert("pluginScope".into(), Value::String(scope.to_string()));
    if let Some(model) = &agent.model {
        config.insert("pluginAgentModel".into(), Value::String(model.clone()));
    }
    // Only restrict builtin services when the definition asked for tools;
    // no `tools` frontmatter keeps the LibrAgent default (all services).
    if !agent.tools.is_empty() {
        let (mapped, _unmapped) = map_plugin_tools(&agent.tools);
        config.insert(
            "allowedBuiltInServiceAliases".into(),
            Value::Array(mapped.into_iter().map(Value::String).collect()),
        );
    }
    Value::Object(config)
}

/// Synthesize the read-only assistant row model for one definition. Used
/// for the global store rows and for session-scope spawn resolution.
pub fn synth_assistant_model(
    agent: &DiscoveredAgent,
    source_id: &str,
    plugin_display_name: &str,
    scope: &str,
) -> AssistantModel {
    AssistantModel {
        id: plugin_agent_id(source_id, &agent.name),
        name: agent.name.clone(),
        config: agent_config_json(agent, source_id, plugin_display_name, scope).to_string(),
        created_at: chrono::Utc::now().timestamp(),
        updated_at: chrono::Utc::now().timestamp(),
    }
}
