use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tempfile::TempDir;

pub const SKILL_FILE_NAME: &str = "SKILL.md";
pub const USER_SKILLS_DIR_NAME: &str = "user_skills";
pub const SYSTEM_SKILLS_DIR_NAME: &str = "system_skills";
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ManagedSkillsOverview {
    pub system_directory: String,
    pub user_directory: String,
    pub system_skills: Vec<SkillMetadata>,
    pub user_skills: Vec<SkillMetadata>,
    pub effective_skills: Vec<SkillMetadata>,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillImportPreview {
    pub discovered_skills: Vec<SkillImportCandidate>,
    pub conflicts: Vec<SkillImportConflict>,
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
    pub(super) discovered_skills: Vec<DiscoveredSkillRoot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubRepoSpec {
    pub owner: String,
    pub repo: String,
    pub branch: Option<String>,
    pub subpath: Option<PathBuf>,
}
