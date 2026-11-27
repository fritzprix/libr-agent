//! Listing the commands offered to a session (#3043).

use super::discovery::discover_command_files;
use super::paths::plugin_namespace;
use super::types::{DiscoveredCommand, PluginCommandInfo, BUILTIN_COMMAND_NAMES};
use crate::services::hooks::registry::{
    get_user_plugins_directory, list_sources_in, PluginSourceRecord,
};
use crate::services::skill_service::plugin_manifest::{load_plugin_manifest, PluginManifest};
use crate::services::skill_service::session_plugins::PluginScope;
use log::warn;
use std::path::Path;

/// List every slash command the chat input should offer for a session.
///
/// Session-scoped plugin stores shadow a global plugin with the same
/// `source_id`. Each command always yields the namespaced `/<plugin>:<name>`
/// label; the bare `/<name>` is added only when the name is unique across
/// effective plugins and does not collide with a built-in.
pub fn list_plugin_commands(session_id: &str) -> Result<Vec<PluginCommandInfo>, String> {
    let global_dir = get_user_plugins_directory()?;
    // #2942 gatekeeper G1.3: one session plugin read path — load_session_plugins
    // resolves the store manifest with the out-of-workspace trust overlay, so
    // command discovery sees exactly the records the hook/skill loaders see.
    let session_records =
        crate::services::skill_service::session_plugins::load_session_plugins(session_id)?;
    Ok(list_in_dirs(&global_dir, session_records))
}

pub(super) fn list_in_dirs(
    global_dir: &Path,
    session_records: Vec<PluginSourceRecord>,
) -> Vec<PluginCommandInfo> {
    let mut records: Vec<(PluginSourceRecord, PluginScope)> = Vec::new();
    // Session plugins load through the store manifest (#2942); broken or
    // deleted entries are skipped with a log line, never an error.
    let shadowed: Vec<String> = session_records
        .iter()
        .map(|r| r.manifest.source_id.clone())
        .collect();
    for record in session_records {
        // #2927: a disabled plugin offers no commands, in any scope.
        if !record.manifest.enabled {
            continue;
        }
        records.push((record, PluginScope::Session));
    }
    let global_records = list_sources_in(global_dir).unwrap_or_else(|error| {
        warn!("global plugin store {}: {error}", global_dir.display());
        Vec::new()
    });
    for record in global_records {
        if shadowed.contains(&record.manifest.source_id) || !record.manifest.enabled {
            continue;
        }
        records.push((record, PluginScope::Global));
    }

    // Per-plugin command files (session plugins first for deterministic
    // bare-name resolution when counts tie is impossible: duplicates are
    // rejected below unless the count is exactly one).
    let mut per_plugin: Vec<(String, PluginScope, Vec<DiscoveredCommand>)> = Vec::new();
    for (record, scope) in &records {
        let manifest = manifest_for_discovery(record);
        let commands = discover_command_files(&record.root_dir, manifest.as_ref());
        if commands.is_empty() {
            continue;
        }
        per_plugin.push((plugin_namespace(record), *scope, commands));
    }

    let mut namespaced_count: Vec<(String, usize)> = Vec::new();
    for (_, _, commands) in &per_plugin {
        for command in commands {
            match namespaced_count
                .iter_mut()
                .find(|(name, _)| name.eq_ignore_ascii_case(&command.name))
            {
                Some((_, count)) => *count += 1,
                None => namespaced_count.push((command.name.clone(), 1)),
            }
        }
    }

    let mut out: Vec<PluginCommandInfo> = Vec::new();
    for (plugin_name, scope, commands) in &per_plugin {
        for command in commands {
            let info = PluginCommandInfo {
                label: format!("/{plugin_name}:{}", command.name),
                name: command.name.clone(),
                plugin_name: plugin_name.clone(),
                description: command.description.clone(),
                argument_hint: command.argument_hint.clone(),
                scope: *scope,
            };
            let unique = namespaced_count
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&command.name))
                .map(|(_, count)| *count == 1)
                .unwrap_or(false);
            let builtin_collision = BUILTIN_COMMAND_NAMES
                .iter()
                .any(|b| b.eq_ignore_ascii_case(&command.name));
            if unique && !builtin_collision {
                let mut bare = info.clone();
                bare.label = format!("/{}", command.name);
                out.push(bare);
            }
            out.push(info);
        }
    }
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

/// Re-resolve custom `commands` paths from the on-disk
/// `.claude-plugin/plugin.json` when one survived install; otherwise default
/// `commands/` discovery. The install record persists only display
/// metadata (`PluginMetadata`), so discovery reads the manifest from disk.
pub(crate) fn manifest_for_discovery(record: &PluginSourceRecord) -> Option<PluginManifest> {
    load_plugin_manifest(&record.root_dir).ok().flatten()
}
