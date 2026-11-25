use crate::services::skill_service::plugin_commands::{
    collect_markdown, is_plain_dir, is_plain_file, split_frontmatter, stays_within,
};
use crate::services::skill_service::plugin_manifest::{validate_component_path, PluginManifest};
use log::warn;
use std::fs;
use std::path::{Path, PathBuf};

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
                    files.extend(collect_markdown(&target));
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
