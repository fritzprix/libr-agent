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

mod discovery;
mod ids;
mod mapping;
mod preview;
mod session;
mod sync;

#[cfg(test)]
mod tests;

pub use ids::{
    is_plugin_agent_id, parse_plugin_agent_id, plugin_agent_id, plugin_source_of_config,
    reject_plugin_agent_mutation,
};
pub use mapping::plugin_model_to_complexity;
pub use preview::{plugin_agent_previews, PluginAgentPreview};
pub use session::{
    declared_plugin_agent_model, resolve_plugin_source_namespace,
    resolve_plugin_source_namespace_in, resolve_session_plugin_assistant,
    session_plugin_agent_infos, session_store_source_ids, PluginAgentListInfo,
};
pub use sync::{remove_global_plugin_agents, sync_global_plugin_agents};
