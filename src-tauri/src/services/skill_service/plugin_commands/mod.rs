//! Plugin slash commands (#2922), split by responsibility in #3043.
//!
//! A Claude Code plugin may ship Markdown files under `commands/` (or the
//! manifest `commands` paths, #2921). Each file becomes a slash command: the
//! file stem is the command name, the frontmatter `description` /
//! `argument-hint` describe it, and the Markdown body is the prompt template
//! ($ARGUMENTS / $1..$9 substitution).
//!
//! Commands live with the plugin source (global or session store), so they
//! are read-only by construction and an uninstall removes them with the
//! plugin. Session-scoped plugins shadow a global plugin with the same
//! `source_id` for that session only (same rule as #2825 hooks/skills).
//!
//! Names: every command is offered as `/<plugin>:<name>`; the bare
//! `/<name>` is offered only when exactly one effective plugin provides it.
//! A plugin command can never replace a built-in command.

mod discovery;
mod expansion;
mod listing;
mod paths;
mod types;

pub use discovery::plugin_command_names;
pub use expansion::expand_plugin_command;
pub use listing::list_plugin_commands;
pub use types::PluginCommandInfo;

// The `plugin_agents` loaders share the frontmatter and path-safety helpers,
// so they keep one implementation rather than a copy each.
pub(crate) use listing::manifest_for_discovery;
pub(crate) use paths::{
    collect_markdown, is_plain_dir, is_plain_file, plugin_namespace, split_frontmatter,
    stays_within,
};

#[cfg(test)]
mod tests;
