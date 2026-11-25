//! Plugin `agents/` subagent definitions (#2923).
//!
//! A Claude Code plugin may ship Markdown files under `agents/` (or the
//! manifest `agents` paths, #2921). Each file is a subagent definition:
//! the frontmatter carries `name`, `description`, `tools`, and `model`,
//! and the Markdown body is the subagent's system prompt.
//!
//! Two loading paths, per the #2920 design decisions:
//!
//! - **Global-scope plugins (D2):** install writes one assistant row per
//!   agent with the deterministic id `plugin:<source_id>:<agent>` and a
//!   `pluginSource` ownership marker in its config. The backend rejects
//!   update, delete, and batch_upsert for plugin-owned ids (the
//!   `deletionProtected` flag alone is UI-only and not sufficient), and
//!   uninstall removes the rows with the plugin.
//! - **Session-scope plugins (D1):** nothing is copied into the global
//!   store. Session load resolves agents from the plugin directory through
//!   the session plugin store manifest; a missing or broken plugin is
//!   skipped with a log line. Uninstall removes the definition with the
//!   plugin files.
//!
//! The `tools` frontmatter maps to LibrAgent builtin service aliases where
//! a mapping exists; tools with no mapping are reported in the install
//! preview and are never silently granted. The `model` frontmatter maps
//! Claude Code tiers (`haiku`/`sonnet`/`opus`) to LibrAgent complexity
//! tiers; anything else falls back to the parent session model.

use super::plugin_commands::{
    collect_markdown, is_plain_dir, is_plain_file, manifest_for_discovery, plugin_namespace,
    split_frontmatter, stays_within,
};
use super::plugin_manifest::{validate_component_path, PluginManifest};
use crate::entity::assistant::Model as AssistantModel;
use crate::services::hooks::registry::PluginSourceRecord;
use log::warn;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// The `pluginSource` config key, shared with the #2924 MCP-server marker.
pub const PLUGIN_SOURCE_KEY: &str = "pluginSource";
/// Every plugin-owned assistant id starts with this prefix (D2).
pub const PLUGIN_AGENT_ID_PREFIX: &str = "plugin:";

/// The deterministic assistant id for one plugin agent (D2).
pub fn plugin_agent_id(source_id: &str, agent_name: &str) -> String {
    format!("{PLUGIN_AGENT_ID_PREFIX}{source_id}:{agent_name}")
}

/// The `pluginSource` ownership marker value for a plugin source (D2).
pub fn plugin_source_marker(source_id: &str) -> String {
    format!("plugin:{source_id}")
}

/// True for every id this feature owns (`plugin:<source_id>:<agent>` or a
/// bare `plugin:<source_id>` — both are reserved for plugin flows).
pub fn is_plugin_agent_id(id: &str) -> bool {
    id.starts_with(PLUGIN_AGENT_ID_PREFIX)
}

/// Parse `plugin:<source_id>:<agent>` into its parts (`None` for any other
/// id shape). Source ids never contain `:` and agent names are sanitized
/// without `:`, so the first `:` after the prefix splits the two.
pub fn parse_plugin_agent_id(id: &str) -> Option<(&str, &str)> {
    let rest = id.strip_prefix(PLUGIN_AGENT_ID_PREFIX)?;
    let (source_id, agent) = rest.split_once(':')?;
    if source_id.is_empty() || agent.is_empty() {
        return None;
    }
    Some((source_id, agent))
}

/// Reject user mutations of plugin-owned assistant ids (D2). Called by
/// every assistant mutation path (Tauri commands, AssistantService, MCP
/// `agent__createAgent`/`agent__updateAgent`/`agent__deleteAgent`), and
/// again for any config that spoofs the ownership marker.
pub fn reject_plugin_agent_mutation(id: &str, new_config: Option<&Value>) -> Result<(), String> {
    if is_plugin_agent_id(id) {
        return Err(format!(
            "Assistant {id} is owned by a plugin and is read-only: it is updated \
             or removed only with its plugin."
        ));
    }
    if let Some(config) = new_config {
        if config.get(PLUGIN_SOURCE_KEY).is_some() {
            return Err(format!(
                "The `{PLUGIN_SOURCE_KEY}` config field is reserved for plugin installs \
                 and cannot be set on a user assistant."
            ));
        }
    }
    Ok(())
}

/// Read the `pluginSource` marker out of a stored assistant config string.
pub fn plugin_source_of_config(config_json: &str) -> Option<String> {
    let value: Value = serde_json::from_str(config_json).ok()?;
    value
        .get(PLUGIN_SOURCE_KEY)
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// One discovered `agents/*.md` definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredAgent {
    /// Definition name: frontmatter `name` when present, else the file stem.
    pub name: String,
    pub path: PathBuf,
    pub description: String,
    /// Raw frontmatter `tools` entries (as written), in order.
    pub tools: Vec<String>,
    /// Raw frontmatter `model` value, when present.
    pub model: Option<String>,
    /// Markdown body: the subagent system prompt.
    pub system_prompt: String,
}

/// Resolve every `agents/**/*.md` (default layout) or manifest-declared
/// path inside a plugin root. A manifest entry that is a directory collects
/// its Markdown files recursively; a `.md` file entry is taken directly.
pub fn discover_agent_files(
    root: &Path,
    manifest: Option<&PluginManifest>,
) -> Vec<DiscoveredAgent> {
    let files = match manifest.and_then(|m| m.agents.as_ref()) {
        Some(paths) => {
            let mut files = Vec::new();
            for raw in paths.to_vec() {
                // Same component-path rule as the other plugin loaders
                // (#2921, #2924): a manifest path may never traverse out of
                // the plugin root, so it is not even probed here.
                if let Err(error) = validate_component_path(&raw) {
                    warn!("Skipping plugin agents manifest path {raw:?}: {error}");
                    continue;
                }
                let inner = raw.strip_prefix("./").unwrap_or(&raw);
                let target = root.join(inner);
                if is_plain_dir(&target) {
                    files.extend(super::plugin_commands::collect_markdown(&target));
                } else if is_plain_file(&target)
                    && target.extension().map(|e| e == "md").unwrap_or(false)
                {
                    files.push(target);
                }
            }
            files
        }
        None => collect_markdown(&root.join("agents")),
    };

    let mut agents: Vec<DiscoveredAgent> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for path in files {
        // Plugin content is untrusted: never follow a symlink out of the
        // plugin root (a link to a private file would be sent to the LLM).
        if !is_plain_file(&path) || !stays_within(root, &path) {
            warn!(
                "Skipping plugin agent outside plugin root: {}",
                path.display()
            );
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(error) => {
                warn!(
                    "Skipping unreadable plugin agent {}: {error}",
                    path.display()
                );
                continue;
            }
        };
        let (frontmatter, body) = split_frontmatter(&raw);
        let name = frontmatter
            .get("name")
            .map(|n| sanitize_agent_name(n))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| sanitize_agent_name(stem));
        if name.is_empty() {
            continue;
        }
        if seen.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            continue;
        }
        seen.push(name.clone());
        let tools_raw = frontmatter.get("tools").cloned().unwrap_or_default();
        agents.push(DiscoveredAgent {
            name,
            path,
            description: frontmatter.get("description").cloned().unwrap_or_default(),
            tools: tools_raw
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect(),
            model: frontmatter
                .get("model")
                .map(|m| m.trim().to_string())
                .filter(|m| !m.is_empty() && !m.eq_ignore_ascii_case("inherit")),
            system_prompt: body.trim().to_string(),
        });
    }
    agents
}

/// Keep agent names safe for ids and display: drop `:` `/` `\` and control
/// characters, collapse whitespace, trim.
fn sanitize_agent_name(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, ':' | '/' | '\\') && !c.is_control())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

/// Claude Code tool name → LibrAgent builtin service alias. Conservative:
/// only tools with a clear LibrAgent counterpart are mapped; everything
/// else is reported unmapped and never granted (#2923 approach 3).
///
/// Restricted Claude Code entries such as `Bash(git:*)` do not match any
/// name here and fall to "unmapped" (never granted). There is no narrower
/// read-only workspace alias in LibrAgent: `Read`/`Glob`/`Grep`/`LS` map to
/// the same `workspace` service that also carries file writes and shell,
/// and the install preview discloses every granted service as the
/// mitigation (#2923 gatekeeper G1.2).
fn map_plugin_tool(tool: &str) -> Option<&'static str> {
    Some(match tool.to_ascii_lowercase().as_str() {
        "bash" | "read" | "write" | "edit" | "multiedit" | "notebookedit" | "notebookread"
        | "glob" | "grep" | "ls" | "search" => "workspace",
        "webfetch" | "websearch" => "browser",
        "task" | "subagent" => "agent",
        "todowrite" | "plan" => "planning",
        "askuserquestion" => "ui",
        _ => return None,
    })
}

/// Split raw `tools` entries into mapped canonical aliases and unmapped
/// names. Aliases are deduplicated and sorted for deterministic configs.
pub fn map_plugin_tools(tools: &[String]) -> (Vec<String>, Vec<String>) {
    let mut mapped: Vec<String> = Vec::new();
    let mut unmapped: Vec<String> = Vec::new();
    for tool in tools {
        match map_plugin_tool(tool) {
            Some(alias) => {
                if !mapped.iter().any(|a| a == alias) {
                    mapped.push(alias.to_string());
                }
            }
            None => {
                if !unmapped.iter().any(|t| t == tool) {
                    unmapped.push(tool.clone());
                }
            }
        }
    }
    mapped.sort();
    (mapped, unmapped)
}

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

/// Claude Code `model` tier → LibrAgent complexity tier name, as written
/// by `read_complexity` (`low`/`normal`/`high`). `None` means "no
/// mapping": the parent session model is used (#2923 approach 4).
pub fn plugin_model_to_complexity(declared: &str) -> Option<&'static str> {
    match declared.to_ascii_lowercase().as_str() {
        "haiku" => Some("low"),
        "sonnet" => Some("normal"),
        "opus" => Some("high"),
        _ => None,
    }
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
    let records = super::session_plugins::load_session_plugins(scope_session_id).ok()?;
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
        caller_session_id.and_then(|sid| super::session_plugins::load_session_plugins(sid).ok())?;
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
    let records = match super::session_plugins::load_session_plugins(session_id) {
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
    super::session_plugins::load_session_plugins(session_id)
        .map(|records| records.into_iter().map(|r| r.manifest.source_id).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::skill_service::plugin_manifest::PLUGIN_MANIFEST_FILE;
    use std::fs;

    fn write_agent(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
    }

    const REVIEWER: &str = "---\nname: reviewer\ndescription: Reviews code\nmodel: sonnet\ntools: Read, Grep, CustomLint\n---\nYou review code carefully.\n";

    #[test]
    fn discovers_agents_dir_with_frontmatter_and_body() {
        let dir = tempfile::tempdir().unwrap();
        write_agent(dir.path(), "agents/reviewer.md", REVIEWER);
        let agents = discover_agent_files(dir.path(), None);
        assert_eq!(agents.len(), 1);
        let agent = &agents[0];
        assert_eq!(agent.name, "reviewer");
        assert_eq!(agent.description, "Reviews code");
        assert_eq!(agent.model.as_deref(), Some("sonnet"));
        assert_eq!(agent.tools, vec!["Read", "Grep", "CustomLint"]);
        assert_eq!(agent.system_prompt, "You review code carefully.");
    }

    #[test]
    fn falls_back_to_file_stem_and_treats_inherit_as_none() {
        let dir = tempfile::tempdir().unwrap();
        write_agent(
            dir.path(),
            "agents/writer.md",
            "---\nmodel: inherit\n---\nWrite docs.\n",
        );
        let agents = discover_agent_files(dir.path(), None);
        assert_eq!(agents[0].name, "writer");
        assert_eq!(agents[0].model, None);
        assert!(agents[0].tools.is_empty());
    }

    #[test]
    fn manifest_agents_paths_are_resolved() {
        let dir = tempfile::tempdir().unwrap();
        write_agent(
            dir.path(),
            &format!(
                "{}/{PLUGIN_MANIFEST_FILE}",
                super::super::plugin_manifest::PLUGIN_MANIFEST_DIR
            ),
            r#"{ "name": "p", "agents": ["./custom/sub.md", "./extra/"] }"#,
        );
        write_agent(
            dir.path(),
            "custom/sub.md",
            "---\ndescription: s\n---\nbody",
        );
        write_agent(dir.path(), "extra/one.md", "---\ndescription: e\n---\nbody");
        write_agent(dir.path(), "agents/ignored.md", "not loaded");
        let manifest = super::super::plugin_manifest::load_plugin_manifest(dir.path())
            .unwrap()
            .expect("manifest");
        let mut names: Vec<String> = discover_agent_files(dir.path(), Some(&manifest))
            .into_iter()
            .map(|a| a.name)
            .collect();
        names.sort();
        assert_eq!(names, vec!["one", "sub"]);
    }

    #[test]
    fn tool_mapping_maps_known_and_reports_unmapped() {
        let tools: Vec<String> = [
            "Read",
            "Bash",
            "WebSearch",
            "Task",
            "TodoWrite",
            "AskUserQuestion",
            "McpCustom",
        ]
        .iter()
        .map(|t| t.to_string())
        .collect();
        let (mapped, unmapped) = map_plugin_tools(&tools);
        assert_eq!(
            mapped,
            vec!["agent", "browser", "planning", "ui", "workspace"]
        );
        assert_eq!(unmapped, vec!["McpCustom"]);
    }

    #[test]
    fn preview_lists_agents_their_granted_and_unmapped_tools() {
        let dir = tempfile::tempdir().unwrap();
        write_agent(dir.path(), "agents/reviewer.md", REVIEWER);
        let previews = plugin_agent_previews(dir.path(), None);
        assert_eq!(previews.len(), 1);
        assert_eq!(previews[0].name, "reviewer");
        assert_eq!(previews[0].unmapped_tools, vec!["CustomLint"]);
        // G1.2: Read/Grep map to `workspace`; the user sees the grant too.
        assert_eq!(previews[0].granted_services, vec!["workspace"]);
        assert_eq!(previews[0].grants_all_services, false);
        assert_eq!(previews[0].model.as_deref(), Some("sonnet"));
    }

    #[test]
    fn preview_without_tools_declares_all_services_granted() {
        // G1.2: an absent `tools` line grants everything; the preview must
        // say so instead of rendering an empty grant.
        let dir = tempfile::tempdir().unwrap();
        write_agent(
            dir.path(),
            "agents/plain.md",
            "---\ndescription: d\n---\nbody",
        );
        let previews = plugin_agent_previews(dir.path(), None);
        assert_eq!(previews[0].granted_services, Vec::<String>::new());
        assert_eq!(previews[0].grants_all_services, true);
        assert_eq!(previews[0].unmapped_tools, Vec::<String>::new());
    }

    #[test]
    fn preview_of_read_only_tools_lists_workspace_grant() {
        // G1.2 test pair: `tools: Read` grants exactly `workspace`.
        let dir = tempfile::tempdir().unwrap();
        write_agent(
            dir.path(),
            "agents/reader.md",
            "---\ndescription: d\ntools: Read\n---\nbody",
        );
        let previews = plugin_agent_previews(dir.path(), None);
        assert_eq!(previews[0].granted_services, vec!["workspace"]);
        assert_eq!(previews[0].grants_all_services, false);
    }

    #[test]
    fn manifest_paths_traversing_out_of_the_root_are_never_probed() {
        // S1.1: manifest `agents` paths follow the same component-path
        // rule as the other plugin loaders; `../` entries are skipped.
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        write_agent(
            outside.path(),
            "outside/evil.md",
            "---\ndescription: e\n---\nsecret",
        );
        write_agent(
            dir.path(),
            &format!(
                "{}/{PLUGIN_MANIFEST_FILE}",
                super::super::plugin_manifest::PLUGIN_MANIFEST_DIR
            ),
            r#"{ "name": "p", "agents": ["./../outside"] }"#,
        );
        let manifest = super::super::plugin_manifest::load_plugin_manifest(dir.path())
            .unwrap()
            .expect("manifest");
        assert!(discover_agent_files(dir.path(), Some(&manifest)).is_empty());
    }

    #[test]
    fn ids_roundtrip_and_reject_other_shapes() {
        let id = plugin_agent_id("plugin-0123456789abcdef", "reviewer");
        assert_eq!(id, "plugin:plugin-0123456789abcdef:reviewer");
        assert_eq!(
            parse_plugin_agent_id(&id),
            Some(("plugin-0123456789abcdef", "reviewer"))
        );
        assert!(is_plugin_agent_id(&id));
        assert!(!is_plugin_agent_id("b15fdf3e-6192-453d"));
        assert_eq!(parse_plugin_agent_id("plugin:only-source"), None);
        assert_eq!(parse_plugin_agent_id("plugin::agent"), None);
    }

    #[test]
    fn mutation_guard_rejects_plugin_ids_and_spoofed_configs() {
        assert!(reject_plugin_agent_mutation(&plugin_agent_id("s", "a"), None).is_err());
        assert!(reject_plugin_agent_mutation("user-assistant", None).is_ok());
        let spoofed = serde_json::json!({ "pluginSource": "plugin:s" });
        assert!(reject_plugin_agent_mutation("user-assistant", Some(&spoofed)).is_err());
    }

    #[test]
    fn synth_config_carries_ownership_prompt_and_aliases() {
        let dir = tempfile::tempdir().unwrap();
        write_agent(dir.path(), "agents/reviewer.md", REVIEWER);
        let agent = discover_agent_files(dir.path(), None).remove(0);
        let config = agent_config_json(&agent, "plugin-aa", "myp", "global");
        assert_eq!(config["systemPrompt"], "You review code carefully.");
        assert_eq!(config[PLUGIN_SOURCE_KEY], "plugin:plugin-aa");
        assert_eq!(config["pluginSourceName"], "myp");
        assert_eq!(config["pluginScope"], "global");
        assert_eq!(config["deletionProtected"], true);
        assert_eq!(config["pluginAgentModel"], "sonnet");
        // Read/Grep map to workspace; CustomLint is never granted.
        assert_eq!(
            config["allowedBuiltInServiceAliases"],
            serde_json::json!(["workspace"])
        );

        let model = synth_assistant_model(&agent, "plugin-aa", "myp", "global");
        assert_eq!(model.id, "plugin:plugin-aa:reviewer");
        assert_eq!(model.name, "reviewer");
    }

    #[test]
    fn no_tools_frontmatter_keeps_default_services() {
        let dir = tempfile::tempdir().unwrap();
        write_agent(
            dir.path(),
            "agents/plain.md",
            "---\ndescription: d\n---\nbody",
        );
        let agent = discover_agent_files(dir.path(), None).remove(0);
        let config = agent_config_json(&agent, "s", "p", "global");
        assert!(config.get("allowedBuiltInServiceAliases").is_none());
    }

    #[test]
    fn model_tiers_map_only_known_names() {
        assert_eq!(plugin_model_to_complexity("Haiku"), Some("low"));
        assert_eq!(plugin_model_to_complexity("sonnet"), Some("normal"));
        assert_eq!(plugin_model_to_complexity("opus"), Some("high"));
        assert_eq!(plugin_model_to_complexity("gpt-5"), None);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_agent_outside_root_is_skipped() {
        // `std::os::unix::fs::symlink` does not compile on Windows, so the
        // whole test is unix-only (same as the plugin_commands symlink
        // discovery test).
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        write_agent(
            outside.path(),
            "evil.md",
            "---\ndescription: e\n---\nsecret",
        );
        let link = dir.path().join("agents");
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();
        assert!(discover_agent_files(dir.path(), None).is_empty());
    }

    #[test]
    fn session_resolution_finds_agent_by_name_and_synthesizes() {
        let dir = tempfile::tempdir().unwrap();
        write_agent(dir.path(), "agents/reviewer.md", REVIEWER);
        let record = record_for(dir.path(), "plugin-aa", "https://github.com/acme/p");
        let model =
            resolve_session_plugin_assistant_from_records(&[record], "plugin-aa", "Reviewer")
                .expect("agent resolves case-insensitively");
        assert_eq!(model.id, "plugin:plugin-aa:reviewer");
        assert_eq!(model.config.contains("\"pluginScope\":\"session\""), true);

        assert!(
            resolve_session_plugin_assistant_from_records(&[], "plugin-aa", "reviewer").is_none()
        );
        assert!(resolve_session_plugin_assistant_from_records(
            std::slice::from_ref(&record_for(dir.path(), "plugin-aa", "u")),
            "plugin-bb",
            "reviewer"
        )
        .is_none());
    }

    #[test]
    fn infos_list_session_agents_with_deterministic_ids() {
        let dir = tempfile::tempdir().unwrap();
        write_agent(dir.path(), "agents/reviewer.md", REVIEWER);
        let record = record_for(dir.path(), "plugin-aa", "https://github.com/acme/p");
        let infos = infos_from_records(&[record]);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].id, "plugin:plugin-aa:reviewer");
        assert_eq!(infos[0].name, "reviewer");
        assert_eq!(infos[0].description, "Reviews code");
    }

    fn record_for(root: &Path, source_id: &str, source_url: &str) -> PluginSourceRecord {
        PluginSourceRecord {
            manifest: crate::services::hooks::registry::PluginSourceManifest {
                source_url: source_url.to_string(),
                source_id: source_id.to_string(),
                skill_names: Vec::new(),
                hooks: None,
                hooks_approved: false,
                approval_hashes: Default::default(),
                created_at: 0,
                plugin: None,
                hook_files: Vec::new(),
                mcp_servers: Vec::new(),
                mcp_approved: false,
                enabled: true,
                installed_commit: None,
            },
            root_dir: root.to_path_buf(),
        }
    }

    async fn memory_db() -> sea_orm::DatabaseConnection {
        use sea_orm_migration::MigratorTrait;
        crate::lifecycle::database::register_sqlite_vec();
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("memory db");
        crate::migration::Migrator::up(&db, None)
            .await
            .expect("migrations");
        db
    }

    // G1.3(b): the store-facing lifecycle paths, not just the pure helpers.

    #[tokio::test]
    async fn sync_converges_on_current_definitions_across_updates() {
        let _guard = crate::state::lock_test_global_state().await;
        crate::state::set_assistant_repository(
            crate::repositories::assistant_repository::SqliteAssistantRepository::new(
                memory_db().await,
            ),
        );
        use crate::repositories::AssistantRepository;
        let repo = crate::state::get_assistant_repository();
        // A user row and another plugin's row must never be touched.
        repo.create_assistant(
            "user-assistant".to_string(),
            "user".to_string(),
            "{}".to_string(),
        )
        .await
        .unwrap();

        let dir = tempfile::tempdir().unwrap();
        write_agent(dir.path(), "agents/reviewer.md", REVIEWER);
        let record = record_for(
            dir.path(),
            "plugin-cc00000000000001",
            "https://github.com/acme/p",
        );

        // Install.
        sync_global_plugin_agents(&record).await.unwrap();
        assert!(repo
            .get_assistant("plugin:plugin-cc00000000000001:reviewer")
            .await
            .unwrap()
            .is_some());

        // Update: the plugin dropped reviewer and added writer — the rows
        // must converge exactly on the current definitions (remove-first).
        fs::remove_file(dir.path().join("agents/reviewer.md")).unwrap();
        write_agent(
            dir.path(),
            "agents/writer.md",
            "---\ndescription: w\n---\nbody",
        );
        sync_global_plugin_agents(&record).await.unwrap();
        assert!(repo
            .get_assistant("plugin:plugin-cc00000000000001:reviewer")
            .await
            .unwrap()
            .is_none());
        assert!(repo
            .get_assistant("plugin:plugin-cc00000000000001:writer")
            .await
            .unwrap()
            .is_some());

        // Uninstall (AC4): this source's rows go, everything else stays.
        remove_global_plugin_agents("plugin-cc00000000000001")
            .await
            .unwrap();
        assert!(repo
            .get_assistant("plugin:plugin-cc00000000000001:writer")
            .await
            .unwrap()
            .is_none());
        assert!(repo
            .get_assistant("user-assistant")
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn declared_model_reads_store_row_and_survives_broken_config() {
        let _guard = crate::state::lock_test_global_state().await;
        crate::state::set_assistant_repository(
            crate::repositories::assistant_repository::SqliteAssistantRepository::new(
                memory_db().await,
            ),
        );
        use crate::repositories::AssistantRepository;
        let repo = crate::state::get_assistant_repository();
        repo.create_assistant(
            "plugin:plugin-dd00000000000001:reviewer".to_string(),
            "reviewer".to_string(),
            serde_json::json!({ "pluginAgentModel": "sonnet" }).to_string(),
        )
        .await
        .unwrap();
        repo.create_assistant(
            "plugin:plugin-dd00000000000002:broken".to_string(),
            "broken".to_string(),
            "not json".to_string(),
        )
        .await
        .unwrap();

        assert_eq!(
            declared_plugin_agent_model("plugin:plugin-dd00000000000001:reviewer", None)
                .await
                .as_deref(),
            Some("sonnet")
        );
        // A broken row must not fail the spawn: None means "inherit the
        // parent model" (spawn.rs keeps the caller's complexity tier).
        assert_eq!(
            declared_plugin_agent_model("plugin:plugin-dd00000000000002:broken", None).await,
            None
        );
        // A missing row and a non-plugin id resolve the same way.
        assert_eq!(
            declared_plugin_agent_model("plugin:plugin-dd00000000000003:ghost", None).await,
            None
        );
        assert_eq!(
            declared_plugin_agent_model("user-assistant", None).await,
            None
        );
    }

    #[test]
    fn session_resolution_skips_missing_or_broken_plugin_root() {
        // A store entry whose plugin directory was deleted resolves to
        // "nothing" (spawn/resume then fall back), never a panic.
        let record = record_for(Path::new("Z:/definitely-not-installed"), "plugin-aa", "u");
        assert!(resolve_session_plugin_assistant_from_records(
            &[record.clone()],
            "plugin-aa",
            "reviewer"
        )
        .is_none());
        assert!(infos_from_records(&[record]).is_empty());
    }
}
