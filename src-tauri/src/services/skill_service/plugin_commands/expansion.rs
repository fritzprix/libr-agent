//! Expanding `/plugin:name args` into the command body (#3043).

use super::discovery::discover_command_files;
use super::listing::manifest_for_discovery;
use super::paths::{is_plain_file, plugin_namespace, split_frontmatter};
use super::types::{DiscoveredCommand, BUILTIN_COMMAND_NAMES};
use crate::services::hooks::registry::{
    get_user_plugins_directory, list_sources_in, PluginSourceRecord,
};
use log::warn;
use std::fs;
use std::path::Path;

/// Expand `/name [args]` or `/plugin:name [args]` into the command body with
/// `$ARGUMENTS` and `$1..$9` substituted. `Ok(None)` means no plugin command
/// matches (the caller keeps its "unknown command" behavior).
pub fn expand_plugin_command(
    session_id: &str,
    command_text: &str,
) -> Result<Option<ExpandedCommand>, String> {
    let global_dir = get_user_plugins_directory()?;
    // #2942 gatekeeper G1.3: one session plugin read path (see list_plugin_commands).
    let session_records =
        crate::services::skill_service::session_plugins::load_session_plugins(session_id)?;
    Ok(expand_named_in_dirs(
        &global_dir,
        session_records,
        command_text,
    ))
}

/// A typed plugin command and the prompt it expands into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedCommand {
    /// `plugin:command`, whichever form was typed.
    pub name: String,
    pub args: String,
    pub body: String,
}

pub(super) fn expand_named_in_dirs(
    global_dir: &Path,
    session_records: Vec<PluginSourceRecord>,
    command_text: &str,
) -> Option<ExpandedCommand> {
    let trimmed = command_text.trim();
    let head = trimmed.strip_prefix('/')?;
    if head.is_empty()
        || head
            .chars()
            .next()
            .map(|c| c.is_whitespace())
            .unwrap_or(true)
    {
        return None;
    }
    let (token, args) = match head.split_once(char::is_whitespace) {
        Some((token, args)) => (token, args.trim()),
        None => (head, ""),
    };
    if BUILTIN_COMMAND_NAMES.iter().any(|b| {
        token.eq_ignore_ascii_case(b)
            || token
                .split(':')
                .next()
                .unwrap_or(token)
                .eq_ignore_ascii_case(b)
    }) {
        return None;
    }

    let records = effective_records(global_dir, session_records);
    let (plugin_filter, command_name) = match token.split_once(':') {
        Some((plugin, name)) => (Some(plugin), name),
        None => (None, token),
    };

    let mut candidate_bodies: Vec<(String, DiscoveredCommand)> = Vec::new();
    for record in &records {
        let namespace = plugin_namespace(record);
        if let Some(plugin) = plugin_filter {
            if !namespace.eq_ignore_ascii_case(plugin) {
                continue;
            }
        }
        let manifest = manifest_for_discovery(record);
        for command in discover_command_files(&record.root_dir, manifest.as_ref()) {
            if command.name.eq_ignore_ascii_case(command_name) {
                candidate_bodies.push((namespace.clone(), command));
            }
        }
    }

    match candidate_bodies.len() {
        0 => None,
        1 => {
            let (namespace, command) = candidate_bodies.pop().unwrap();
            if !is_plain_file(&command.path) {
                return None;
            }
            let raw = fs::read_to_string(&command.path).ok()?;
            let (_frontmatter, body) = split_frontmatter(&raw);
            Some(ExpandedCommand {
                name: format!("{namespace}:{}", command.name),
                args: args.to_string(),
                body: substitute_arguments(body.trim(), args),
            })
        }
        _ => {
            // Bare-name ambiguity across distinct plugins: namespaced form
            // is the disambiguator. Same-plugin duplicates cannot happen
            // (discovery dedups by name).
            let plugins: Vec<String> = candidate_bodies
                .iter()
                .map(|(namespace, _)| namespace.clone())
                .collect();
            warn!(
                "Slash command /{command_name} is ambiguous across plugins {:?}",
                plugins
            );
            None
        }
    }
}

fn effective_records(
    global_dir: &Path,
    mut records: Vec<PluginSourceRecord>,
) -> Vec<PluginSourceRecord> {
    let shadowed: Vec<String> = records
        .iter()
        .map(|r| r.manifest.source_id.clone())
        .collect();
    records.extend(
        list_sources_in(global_dir)
            .unwrap_or_default()
            .into_iter()
            .filter(|r| !shadowed.contains(&r.manifest.source_id)),
    );
    records
}

/// Replace `$ARGUMENTS` with the full argument string and `$1..$9` with
/// positional arguments (missing positions expand to empty). A single scan
/// keeps `$10` from being eaten as `$1` + `0`.
pub(super) fn substitute_arguments(body: &str, args: &str) -> String {
    let positional: Vec<&str> = args.split_whitespace().collect();
    let mut out = String::with_capacity(body.len() + args.len());
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let rest = &body[i + 1..];
            if rest.starts_with("ARGUMENTS") {
                out.push_str(args);
                i += "$ARGUMENTS".len();
                continue;
            }
            if let Some(index) = rest.chars().next().and_then(|c| c.to_digit(10)) {
                if (1..=9).contains(&index) {
                    out.push_str(positional.get(index as usize - 1).unwrap_or(&""));
                    i += 2;
                    continue;
                }
            }
        }
        let ch = body[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}
