use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tempfile::TempDir;

use super::plugin_agents::PluginAgentPreview;

pub const SKILL_FILE_NAME: &str = "SKILL.md";
/// Hidden file a plugin writes into each skill directory it installs
/// (#2927 D2). Its content is the owning plugin's `source_id`; its presence
/// makes the skill read-only for every non-plugin path (delete, update,
/// import overwrite), and uninstall is the only thing that removes it.
pub const PLUGIN_SKILL_OWNER_FILE: &str = ".libragent-plugin-owner";
pub const USER_SKILLS_DIR_NAME: &str = "user_skills";
pub const SYSTEM_SKILLS_DIR_NAME: &str = "system_skills";
pub const SYSTEM_SKILLS_ALIAS_PREFIX: &str = "@system-skills";
pub const USER_SKILLS_ALIAS_PREFIX: &str = "@user-skills";
pub const ASSISTANT_SKILLS_ALIAS_PREFIX: &str = "@assistant-skills";
pub const WORKSPACE_SKILLS_ALIAS_PREFIX: &str = "@workspace-skills";
pub const LEGACY_SYSTEM_SKILLS_DIR_NAME: &str = "skills";
pub const MANAGED_SYSTEM_SKILLS_MANIFEST_FILE_NAME: &str = ".bundled_manifest.json";
pub(super) const GITHUB_DOWNLOAD_CONNECT_TIMEOUT_SECS: u64 = 10;
pub(super) const GITHUB_DOWNLOAD_TIMEOUT_SECS: u64 = 30;
pub(super) const MAX_GITHUB_ARCHIVE_BYTES: u64 = 100 * 1024 * 1024;
/// Wall-clock limit for one `git clone` of a non-GitHub plugin source (#2928).
pub(super) const GIT_CLONE_TIMEOUT_SECS: u64 = 120;
/// The size cap for a `git clone` install source (#2928 gatekeeper G1.4),
/// checked on the total size of the fetched tree including the repository's
/// `.git` metadata, so a clone is bounded like the GitHub archive path
/// (G2.1).
pub(super) const MAX_CLONE_TREE_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "aliasPath")]
    pub alias_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// Source id of the plugin that owns this skill directory (#2927 D2).
    /// Set when the skill root carries the `PLUGIN_SKILL_OWNER_FILE` marker;
    /// such skills are read-only and go only with the plugin.
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "pluginOwner",
        default
    )]
    pub plugin_owner: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillAliasRoot {
    pub prefix: &'static str,
    pub root: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PluginSourceOverview {
    pub id: String,
    pub source_url: String,
    pub skill_names: Vec<String>,
    pub hook_events: Vec<String>,
    pub hooks_approved: bool,
    /// Manifest `name` from `.claude-plugin/plugin.json` (#2921), when the
    /// plugin carried one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_description: Option<String>,
    /// Whether the plugin is enabled (#2927). A disabled plugin keeps its
    /// files and contributes no skills, hooks, commands, or MCP servers.
    pub enabled: bool,
    /// Source commit recorded at install (#2927), when GitHub exposed one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_commit: Option<String>,
}

/// Whether a global plugin has a newer source on GitHub (#2927 D6). Only
/// plugins with a recorded `installed_commit` can report `update_available:
/// true`; without a recorded commit there is nothing to compare.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PluginUpdateStatus {
    pub source_id: String,
    pub update_available: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ManagedSkillsOverview {
    pub system_directory: String,
    pub user_directory: String,
    pub system_skills: Vec<SkillMetadata>,
    pub user_skills: Vec<SkillMetadata>,
    pub effective_skills: Vec<SkillMetadata>,
    pub plugin_sources: Vec<PluginSourceOverview>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillImportCandidate {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillImportConflict {
    pub name: String,
    pub existing_origin: String,
    pub existing_path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillImportKind {
    #[serde(rename = "skillBundle")]
    SkillBundle,
    #[serde(rename = "plugin")]
    Plugin,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HookPreviewExecutionStatus {
    #[serde(rename = "executed")]
    Executed,
    #[serde(rename = "stubbed")]
    Stubbed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HookPreviewEntry {
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    pub command: String,
    /// Exec-form argument vector (#3005 gatekeeper G1.2). The approval dialog
    /// must show everything that runs: `args` can carry `-c`/`-e` and
    /// arbitrary code even when `command` is just `node` or `bash`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub powershell: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_body: Option<String>,
    pub execution_status: HookPreviewExecutionStatus,
    /// HTTP hook headers as declared (name → raw `$VAR` template, never a
    /// resolved secret), so the approval dialog shows what the plugin will
    /// send (#3041 G1.1).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub http_headers: Option<BTreeMap<String, String>>,
    /// Environment variable names the plugin allows an HTTP hook to read for
    /// header interpolation — names only, values are never shown (#3041 G1.1).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub http_allowed_env_vars: Option<Vec<String>>,
}

/// Manifest fields shown in the install preview (#2921).
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifestSummary {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// One plugin-declared MCP server shown in the install preview (#2924).
/// `command` is set for stdio servers, `url` for HTTP servers. The approval
/// dialog must show everything the plugin can run (D3), so `args` (joined)
/// and the `env` / header **names** (never values) are shown as well
/// (gatekeeper G1.3).
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct McpServerPreviewEntry {
    pub name: String,
    /// "stdio" or "http".
    pub transport: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Command arguments joined with spaces, as declared (raw `${...}`
    /// placeholders included), so the user sees the full command line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,
    /// Environment variable names the server sets (values are not shown).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub env_names: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// HTTP header names the server sends (values are not shown).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub header_names: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillImportPreview {
    pub kind: SkillImportKind,
    pub discovered_skills: Vec<SkillImportCandidate>,
    pub conflicts: Vec<SkillImportConflict>,
    pub hooks: Vec<HookPreviewEntry>,
    pub hooks_require_approval: bool,
    /// MCP servers the plugin declares (`#2924`): `.mcp.json` at the plugin
    /// root plus manifest `mcpServers`, in declaration order. Starting them
    /// requires the same install-time approval as hooks (D3).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub mcp_servers: Vec<McpServerPreviewEntry>,
    #[serde(default)]
    pub mcp_require_approval: bool,
    /// Slash command names provided by the plugin's `commands/` directory
    /// (or manifest `commands` paths), offered as `/<name>` after install
    /// (#2922). Sorted, deduplicated.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub plugin_commands: Vec<String>,
    /// Subagent definitions provided by the plugin's `agents/` directory
    /// (or manifest `agents` paths) (#2923). Each entry lists the tools
    /// that have no LibrAgent mapping and will therefore NOT be granted.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub plugin_agents: Vec<PluginAgentPreview>,
    /// `.claude-plugin/plugin.json` summary, when the source carried one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_manifest: Option<PluginManifestSummary>,
    /// `userConfig` options the plugin asks for (#2926), in key order. The
    /// install dialog collects them and passes the answers back as
    /// `plugin_options`; a `sensitive` one uses masked input and is kept in
    /// secure storage, never in a plain file.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub user_config: Vec<UserConfigPreviewEntry>,
}

/// One `userConfig` option shown at install (#2926).
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserConfigPreviewEntry {
    /// Manifest key; also the `${user_config.<key>}` placeholder name.
    pub key: String,
    /// Label to show (the manifest `title`, else the key).
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// `string` | `number` | `boolean` | `directory` | `file`.
    pub value_type: String,
    /// Masked input and secure storage.
    pub sensitive: bool,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillImportResult {
    pub imported_names: Vec<String>,
    pub overwritten_names: Vec<String>,
    pub skipped_names: Vec<String>,
    /// Non-fatal problems the user must see (e.g. an approved plugin MCP
    /// server that could not be installed into the global store, #2924
    /// gatekeeper G1.2). The install succeeded; these items need attention.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SkillFrontmatter {
    pub(super) name: String,
    pub(super) description: String,
}

#[derive(Debug, Clone)]
pub(super) struct DiscoveredSkillRoot {
    pub(super) metadata: SkillMetadata,
    pub(super) root_dir: PathBuf,
}

#[derive(Debug)]
pub(super) struct PreparedSkillImport {
    pub(super) _temp_dir: TempDir,
    pub(super) scan_root: PathBuf,
    pub(super) source_key: String,
    pub(super) discovered_skills: Vec<DiscoveredSkillRoot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubRepoSpec {
    pub owner: String,
    pub repo: String,
    pub branch: Option<String>,
    pub subpath: Option<PathBuf>,
}
