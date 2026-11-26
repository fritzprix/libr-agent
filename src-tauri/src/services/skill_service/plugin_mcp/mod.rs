//! Plugin-declared MCP servers (#2924).
//!
//! A plugin declares servers in `.mcp.json` at its root and/or through the
//! `.claude-plugin/plugin.json` `mcpServers` field (path file(s) or an inline
//! map). Files load in that order and a server name declared later replaces
//! an earlier one (spec: plugins-reference). The raw declared entry is kept
//! in the plugin source manifest so `${CLAUDE_PLUGIN_ROOT}` can be expanded
//! against the installed root.
//!
//! Scope rules (owner decisions D1/D2): a **global** plugin's approved
//! servers are installed into the global MCP server store as rows marked
//! `pluginSource: "plugin:<source_id>"` (read-only, removed only with the
//! plugin); a **session-scoped** plugin's servers are never copied there —
//! session load reads them straight from the session's plugin store.

mod collect;
mod config;
mod resolve;
mod rows;
mod session;

#[cfg(test)]
mod tests_collect;
#[cfg(test)]
mod tests_runtime;

pub use collect::collect_plugin_mcp_servers;
#[cfg(test)]
pub use config::{entry_to_mcp_config_json, plugin_source_marker};
#[cfg(test)]
pub use resolve::resolve_plugin_root;
#[cfg(test)]
pub use rows::create_plugin_mcp_server_rows;
pub use rows::{create_plugin_mcp_server_rows_in, remove_global_plugin_mcp_servers};
#[cfg(test)]
pub use session::session_plugin_mcp_records;
pub use session::{build_session_plugin_server_configs, SessionPluginServerConfig};

use std::collections::BTreeMap;
use std::path::Path;

/// Default server-declaration file at the plugin root (Claude Code spec).
pub const PLUGIN_MCP_FILE: &str = ".mcp.json";

/// Placeholder names expanded to the plugin's installed root. Same alias set
/// the hook command resolver uses (`hooks/registry.rs`).
const PLUGIN_ROOT_PLACEHOLDERS: [&str; 6] = [
    "${CLAUDE_PLUGIN_ROOT}",
    "${LIBRAGENT_PLUGIN_ROOT}",
    "${CURSOR_PLUGIN_ROOT}",
    "$CLAUDE_PLUGIN_ROOT",
    "$LIBRAGENT_PLUGIN_ROOT",
    "$CURSOR_PLUGIN_ROOT",
];

/// Placeholder names expanded to the plugin's `${CLAUDE_PLUGIN_DATA}`
/// directory (#2926).
const PLUGIN_DATA_PLACEHOLDERS: [&str; 4] = [
    "${CLAUDE_PLUGIN_DATA}",
    "$CLAUDE_PLUGIN_DATA",
    "${LIBRAGENT_PLUGIN_DATA}",
    "$LIBRAGENT_PLUGIN_DATA",
];

/// Shared empty option map for expansions that carry no `userConfig` values.
#[cfg(test)]
pub(super) fn no_options() -> &'static BTreeMap<String, String> {
    static EMPTY: once_cell::sync::Lazy<BTreeMap<String, String>> =
        once_cell::sync::Lazy::new(BTreeMap::new);
    &EMPTY
}

/// What a plugin-declared MCP server entry may reference (#2926): the
/// installed root, the plugin data directory, and the resolved `userConfig`
/// values (`${user_config.KEY}` and `CLAUDE_PLUGIN_OPTION_*`).
///
/// `CLAUDE_PROJECT_DIR` is deliberately absent: an MCP server config is
/// loaded per server (global store) as well as per session, so it has no
/// single project directory. The spec defines it for hooks, where it is set.
pub struct PluginExpansion<'a> {
    pub root: &'a Path,
    pub data_dir: Option<&'a Path>,
    pub options: &'a BTreeMap<String, String>,
}

impl<'a> PluginExpansion<'a> {
    /// Root-only expansion (no plugin data, no options).
    #[cfg(test)]
    pub fn root_only(root: &'a Path) -> Self {
        Self {
            root,
            data_dir: None,
            options: no_options(),
        }
    }

    pub(super) fn replacements(&self) -> Vec<(String, String)> {
        let root = self.root.to_string_lossy().to_string();
        let mut pairs: Vec<(String, String)> = PLUGIN_ROOT_PLACEHOLDERS
            .iter()
            .map(|placeholder| ((*placeholder).to_string(), root.clone()))
            .collect();
        if let Some(data_dir) = self.data_dir {
            let data = data_dir.to_string_lossy().to_string();
            pairs.extend(
                PLUGIN_DATA_PLACEHOLDERS
                    .iter()
                    .map(|placeholder| ((*placeholder).to_string(), data.clone())),
            );
        }
        pairs.extend(self.options.iter().map(|(key, value)| {
            (
                super::plugin_manifest::user_config_placeholder(key),
                value.clone(),
            )
        }));
        pairs
    }
}
