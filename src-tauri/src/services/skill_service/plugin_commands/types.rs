//! Slash-command types a plugin offers (#2922, split out in #3043).

use crate::services::skill_service::session_plugins::PluginScope;
use serde::Serialize;
use std::path::PathBuf;

/// Built-in command names that a plugin may never shadow.
pub(super) const BUILTIN_COMMAND_NAMES: [&str; 4] = ["clear", "reload", "permission", "plugin"];

/// One offerable slash command for the chat input dropdown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCommandInfo {
    /// The exact text to type, e.g. `/hello` or `/superpowers:hello`.
    pub label: String,
    pub name: String,
    pub plugin_name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argument_hint: Option<String>,
    pub scope: PluginScope,
}

/// A command file resolved to its name and frontmatter metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredCommand {
    pub name: String,
    pub path: PathBuf,
    pub description: String,
    pub argument_hint: Option<String>,
}
