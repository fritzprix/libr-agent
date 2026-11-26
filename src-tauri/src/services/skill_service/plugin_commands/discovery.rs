//! Finding the Markdown command files a plugin ships (#3043).

use super::paths::{
    collect_markdown, is_plain_dir, is_plain_file, split_frontmatter, stays_within,
};
use super::types::DiscoveredCommand;
use crate::services::skill_service::plugin_manifest::PluginManifest;
use log::warn;
use std::fs;
use std::path::Path;

/// Names of the slash commands a plugin directory provides (preview-time,
/// sorted, file stems). Used by the install preview (#2922 step 1).
pub fn plugin_command_names(root: &Path, manifest: Option<&PluginManifest>) -> Vec<String> {
    let mut names: Vec<String> = discover_command_files(root, manifest)
        .into_iter()
        .map(|command| command.name)
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Resolve every `commands/**/*.md` (default layout) or manifest-declared
/// path inside a plugin root, parsed for frontmatter metadata.
pub fn discover_command_files(
    root: &Path,
    manifest: Option<&PluginManifest>,
) -> Vec<DiscoveredCommand> {
    let files = match manifest.and_then(|m| m.commands.as_ref()) {
        Some(paths) => {
            let mut files = Vec::new();
            for raw in paths.to_vec() {
                let inner = raw.strip_prefix("./").unwrap_or(&raw);
                let target = root.join(inner);
                if is_plain_dir(&target) {
                    files.extend(collect_markdown(&target));
                } else if is_plain_file(&target)
                    && target.extension().map(|e| e == "md").unwrap_or(false)
                {
                    files.push(target);
                }
            }
            files
        }
        None => collect_markdown(&root.join("commands")),
    };

    let mut commands: Vec<DiscoveredCommand> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for path in files {
        // Plugin content is untrusted: never follow a symlink out of the
        // plugin root (a link to a private file would be sent to the LLM).
        if !is_plain_file(&path) || !stays_within(root, &path) {
            warn!(
                "Skipping plugin command outside plugin root: {}",
                path.display()
            );
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if seen.iter().any(|n| n.eq_ignore_ascii_case(name)) {
            continue;
        }
        seen.push(name.to_string());
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(error) => {
                warn!(
                    "Skipping unreadable plugin command {}: {error}",
                    path.display()
                );
                continue;
            }
        };
        let (frontmatter, _body) = split_frontmatter(&raw);
        commands.push(DiscoveredCommand {
            name: name.to_string(),
            path,
            description: frontmatter.get("description").cloned().unwrap_or_default(),
            argument_hint: frontmatter.get("argument-hint").cloned(),
        });
    }
    commands
}
