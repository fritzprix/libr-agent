use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tempfile::TempDir;

pub const SKILL_FILE_NAME: &str = "SKILL.md";
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub powershell: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_body: Option<String>,
    pub execution_status: HookPreviewExecutionStatus,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillImportPreview {
    pub kind: SkillImportKind,
    pub discovered_skills: Vec<SkillImportCandidate>,
    pub conflicts: Vec<SkillImportConflict>,
    pub hooks: Vec<HookPreviewEntry>,
    pub hooks_require_approval: bool,
    /// `.claude-plugin/plugin.json` summary, when the source carried one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_manifest: Option<PluginManifestSummary>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillImportResult {
    pub imported_names: Vec<String>,
    pub overwritten_names: Vec<String>,
    pub skipped_names: Vec<String>,
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
