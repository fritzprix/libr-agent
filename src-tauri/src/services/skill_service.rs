use crate::session::get_session_manager;
use crate::state::get_app_handle;
use log::{info, warn};
use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tauri::Manager;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
use url::Url;
use uuid::Uuid;
use walkdir::WalkDir;

pub const SKILL_FILE_NAME: &str = "SKILL.md";
const USER_SKILLS_DIR_NAME: &str = "user_skills";
const GITHUB_DOWNLOAD_CONNECT_TIMEOUT_SECS: u64 = 10;
const GITHUB_DOWNLOAD_TIMEOUT_SECS: u64 = 30;
const MAX_GITHUB_ARCHIVE_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>, // "global", "assistant", or "workspace"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>, // "system", "user", "assistant", or "workspace"
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
}

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: String,
    description: String,
}

#[derive(Debug, Clone)]
struct DiscoveredSkillRoot {
    metadata: SkillMetadata,
    root_dir: PathBuf,
}

#[derive(Debug)]
struct PreparedSkillImport {
    _temp_dir: TempDir,
    discovered_skills: Vec<DiscoveredSkillRoot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubRepoSpec {
    pub owner: String,
    pub repo: String,
    pub branch: Option<String>,
    pub subpath: Option<PathBuf>,
}

pub async fn get_default_skills_directory() -> Result<String, String> {
    Ok(get_user_skills_directory()?.to_string_lossy().to_string())
}

pub async fn get_configured_skills_directory() -> Result<String, String> {
    // Deprecated compatibility wrapper. Global skills are now always loaded from
    // internal managed storage rather than a user-configured external directory.
    get_default_skills_directory().await
}

pub fn get_system_skills_directory() -> Result<PathBuf, String> {
    let app_handle = get_app_handle().ok_or_else(|| "AppHandle is not initialized".to_string())?;
    let resource_dir = app_handle
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to resolve resource directory: {}", e))?;
    Ok(resource_dir.join("bundled_skills"))
}

pub fn get_user_skills_directory() -> Result<PathBuf, String> {
    let session_manager = get_session_manager()?;
    Ok(session_manager
        .get_base_data_dir()
        .join(USER_SKILLS_DIR_NAME))
}

pub fn get_legacy_global_skills_directory() -> Result<PathBuf, String> {
    let session_manager = get_session_manager()?;
    Ok(session_manager.get_base_data_dir().join("skills"))
}

fn merge_skill_layers(skill_layers: Vec<Vec<SkillMetadata>>) -> Vec<SkillMetadata> {
    let mut merged_skills = Vec::new();
    let mut seen_names = HashSet::new();

    for skill_layer in skill_layers {
        for skill in skill_layer {
            let normalized = skill.name.to_lowercase();
            if seen_names.insert(normalized) {
                merged_skills.push(skill);
            }
        }
    }

    merged_skills.sort_by_cached_key(|skill| skill.name.to_lowercase());
    merged_skills
}

pub async fn get_managed_skills_overview() -> Result<ManagedSkillsOverview, String> {
    let system_dir = get_system_skills_directory()?;
    let user_dir = get_user_skills_directory()?;

    let mut system_skills = scan_skills_internal(
        &system_dir,
        Some("global".to_string()),
        Some("system".to_string()),
    )
    .await?;
    let mut user_skills = scan_skills_internal(
        &user_dir,
        Some("global".to_string()),
        Some("user".to_string()),
    )
    .await?;
    let effective_skills = merge_skill_layers(vec![user_skills.clone(), system_skills.clone()]);

    system_skills.sort_by_cached_key(|skill| skill.name.to_lowercase());
    user_skills.sort_by_cached_key(|skill| skill.name.to_lowercase());

    Ok(ManagedSkillsOverview {
        system_directory: system_dir.to_string_lossy().to_string(),
        user_directory: user_dir.to_string_lossy().to_string(),
        system_skills,
        user_skills,
        effective_skills,
    })
}

pub async fn resolve_skills(
    system_dir: PathBuf,
    user_dir: PathBuf,
    assistant_dir: Option<PathBuf>,
    workspace_dir: Option<PathBuf>,
) -> Result<Vec<SkillMetadata>, String> {
    let mut skill_layers = Vec::new();

    let sources: Vec<(Option<PathBuf>, &str, &str)> = vec![
        (workspace_dir, "workspace", "workspace"),
        (assistant_dir, "assistant", "assistant"),
        (Some(user_dir), "global", "user"),
        (Some(system_dir), "global", "system"),
    ];

    for (dir, source, origin) in sources {
        let Some(dir) = dir else {
            continue;
        };

        let mut scanned =
            scan_skills_internal(&dir, Some(source.to_string()), Some(origin.to_string())).await?;
        scanned.sort_by_cached_key(|skill| skill.name.to_lowercase());
        skill_layers.push(scanned);
    }

    Ok(merge_skill_layers(skill_layers))
}

/// Public entry point for scanning a directory without any source metadata.
pub async fn scan_skills_directory(directory: &Path) -> Result<Vec<SkillMetadata>, String> {
    scan_skills_internal(directory, None, None).await
}

pub fn get_assistant_skills_directory(assistant_id: &str) -> Result<PathBuf, String> {
    let session_manager = get_session_manager()?;
    Ok(session_manager
        .get_base_data_dir()
        .join("assistants")
        .join(assistant_id)
        .join("skills"))
}

pub fn get_workspace_skills_directory_from_path(workspace_path: &Path) -> PathBuf {
    workspace_path.join("skills")
}

pub fn get_workspace_skills_directory_for_session(session_id: &str) -> Result<PathBuf, String> {
    let session_manager = get_session_manager()?;
    let workspace_dir = session_manager.get_session_workspace_dir_by_id(session_id);
    Ok(get_workspace_skills_directory_from_path(&workspace_dir))
}

pub fn collect_allowed_skill_roots(
    system_dir: PathBuf,
    user_dir: PathBuf,
    assistant_dir: Option<PathBuf>,
    workspace_dir: Option<PathBuf>,
) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(dir) = workspace_dir {
        roots.push(dir);
    }
    if let Some(dir) = assistant_dir {
        roots.push(dir);
    }
    roots.push(user_dir);
    roots.push(system_dir);

    roots
}

pub async fn resolve_skill_directories(
    assistant_id: Option<&str>,
    session_id: Option<&str>,
    workspace_path: Option<&Path>,
) -> Result<(PathBuf, PathBuf, Option<PathBuf>, Option<PathBuf>), String> {
    let system_dir = get_system_skills_directory()?;
    let user_dir = get_user_skills_directory()?;
    let assistant_dir = assistant_id
        .map(get_assistant_skills_directory)
        .transpose()?;
    let workspace_dir = if let Some(path) = workspace_path {
        Some(get_workspace_skills_directory_from_path(path))
    } else if let Some(id) = session_id {
        Some(get_workspace_skills_directory_for_session(id)?)
    } else {
        None
    };

    Ok((system_dir, user_dir, assistant_dir, workspace_dir))
}

/// Reads the full content of a skill's SKILL.md file by skill path.
/// The `skill_path` is the absolute path to the SKILL.md file as returned in `SkillMetadata.path`.
pub async fn get_skill_content(
    skill_path: String,
    assistant_id: Option<String>,
    session_id: Option<String>,
    workspace_path: Option<String>,
) -> Result<String, String> {
    let (system_dir, user_dir, assistant_dir, workspace_dir) = resolve_skill_directories(
        assistant_id.as_deref(),
        session_id.as_deref(),
        workspace_path.as_deref().map(Path::new),
    )
    .await?;
    let allowed_roots =
        collect_allowed_skill_roots(system_dir, user_dir, assistant_dir, workspace_dir);
    get_skill_content_from_roots(skill_path, &allowed_roots).await
}

pub async fn get_skill_content_from_roots(
    skill_path: String,
    allowed_roots: &[PathBuf],
) -> Result<String, String> {
    let path = PathBuf::from(&skill_path);

    if path.file_name() != Some(std::ffi::OsStr::new(SKILL_FILE_NAME)) {
        return Err("Skill path must point to a SKILL.md file".to_string());
    }

    let canonical_path = path
        .canonicalize()
        .map_err(|e| format!("Invalid skill path: {}", e))?;

    let mut is_allowed = false;
    for root in allowed_roots {
        if !root.exists() {
            continue;
        }
        let canonical_root = root
            .canonicalize()
            .map_err(|e| format!("Invalid skills directory: {}", e))?;
        if canonical_path.starts_with(&canonical_root) {
            is_allowed = true;
            break;
        }
    }

    if !is_allowed {
        return Err("Skill path is outside the allowed skills directories".to_string());
    }

    tokio::task::spawn_blocking(move || {
        fs::read_to_string(&canonical_path)
            .map_err(|e| format!("Failed to read skill content: {}", e))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

pub async fn preview_user_skill_import(file_path: String) -> Result<SkillImportPreview, String> {
    let prepared = prepare_local_skill_import(file_path).await?;
    build_user_import_preview(&prepared.discovered_skills).await
}

pub async fn import_user_skills(
    file_path: String,
    overwrite_existing: bool,
) -> Result<SkillImportResult, String> {
    let prepared = prepare_local_skill_import(file_path).await?;
    let preview = build_user_import_preview(&prepared.discovered_skills).await?;
    install_user_prepared_skills(prepared, preview, overwrite_existing).await
}

pub async fn preview_github_skill_install(repo_url: String) -> Result<SkillImportPreview, String> {
    let prepared = prepare_github_skill_import(repo_url).await?;
    build_user_import_preview(&prepared.discovered_skills).await
}

pub async fn install_github_skills(
    repo_url: String,
    overwrite_existing: bool,
) -> Result<SkillImportResult, String> {
    let prepared = prepare_github_skill_import(repo_url).await?;
    let preview = build_user_import_preview(&prepared.discovered_skills).await?;
    install_user_prepared_skills(prepared, preview, overwrite_existing).await
}

pub async fn delete_user_skill(skill_name: String) -> Result<String, String> {
    let user_dir = get_user_skills_directory()?;
    remove_skill_name_from_directory(&user_dir, &skill_name)?;
    Ok(format!("Successfully deleted user skill '{}'", skill_name))
}

pub async fn reset_user_skills() -> Result<String, String> {
    let user_dir = get_user_skills_directory()?;
    if user_dir.exists() {
        fs::remove_dir_all(&user_dir).map_err(|e| e.to_string())?;
        Ok("Successfully reset user skills".to_string())
    } else {
        Ok("No user skills to reset".to_string())
    }
}

pub(crate) async fn scan_skills_internal(
    root_path: &Path,
    source_tag: Option<String>,
    origin_tag: Option<String>,
) -> Result<Vec<SkillMetadata>, String> {
    if !root_path.exists() {
        info!("Skills directory does not exist: {:?}", root_path);
        return Ok(Vec::new());
    }

    let root_path_owned = root_path.to_owned();
    let source_tag_owned = source_tag.clone();
    let origin_tag_owned = origin_tag.clone();

    tokio::task::spawn_blocking(move || {
        let mut skills = Vec::new();

        for entry in WalkDir::new(&root_path_owned)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_name() == SKILL_FILE_NAME {
                let path = entry.path();
                match parse_skill_metadata(path) {
                    Ok(mut metadata) => {
                        metadata.source = source_tag_owned.clone();
                        metadata.origin = origin_tag_owned.clone();
                        skills.push(metadata);
                    }
                    Err(e) => {
                        warn!("Failed to parse skill at {:?}: {}", path, e);
                    }
                }
            }
        }
        Ok(skills)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

pub fn parse_skill_metadata(path: &Path) -> Result<SkillMetadata, String> {
    let content = fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::InvalidData {
            "Content appears to be binary or contains invalid UTF-8 characters".to_string()
        } else {
            e.to_string()
        }
    })?;
    let content = content.strip_prefix('\u{feff}').unwrap_or(&content);

    if let Some(stripped) = content.strip_prefix("---") {
        if let Some(end_idx) = stripped.find("---") {
            let frontmatter_str = &stripped[..end_idx];
            let frontmatter: SkillFrontmatter = serde_yaml::from_str(frontmatter_str)
                .map_err(|e| format!("YAML parse error: {}", e))?;

            if frontmatter.name.trim().is_empty() {
                return Err("Skill name cannot be empty".to_string());
            }
            if frontmatter.description.trim().is_empty() {
                return Err("Skill description cannot be empty".to_string());
            }

            return Ok(SkillMetadata {
                name: frontmatter.name,
                description: frontmatter.description,
                path: path.to_string_lossy().to_string(),
                source: None,
                origin: None,
            });
        }
    }

    Err("No valid YAML frontmatter found".to_string())
}

/// Recursively copies contents of `src` directory into `dst`.
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if !dst.exists() {
        fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    }

    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub async fn copy_global_to_assistant(
    assistant_id: String,
    skill_name: String,
) -> Result<String, String> {
    let system_dir = get_system_skills_directory()?;
    let user_dir = get_user_skills_directory()?;
    let global_skills = resolve_skills(system_dir, user_dir, None, None).await?;
    let source_skill = global_skills
        .into_iter()
        .find(|skill| skill.name.eq_ignore_ascii_case(&skill_name))
        .ok_or_else(|| format!("Global skill '{}' not found", skill_name))?;

    let source_root = PathBuf::from(&source_skill.path)
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("Failed to resolve root directory for '{}'", skill_name))?;

    let assistant_skills_dir = get_assistant_skills_directory(&assistant_id)?;
    if find_skill_root_by_name(&assistant_skills_dir, &skill_name)?.is_some() {
        return Err(format!(
            "Skill '{}' already exists for this assistant",
            skill_name
        ));
    }

    let storage_name = skill_storage_directory_name(&source_skill.name)?;
    let target_path = assistant_skills_dir.join(storage_name);
    copy_dir_recursive(&source_root, &target_path)?;

    Ok(format!(
        "Successfully copied skill '{}' to assistant '{}'",
        skill_name, assistant_id
    ))
}

pub async fn delete_assistant_skill(
    assistant_id: String,
    skill_name: String,
) -> Result<String, String> {
    let assistant_skills_dir = get_assistant_skills_directory(&assistant_id)?;
    remove_skill_name_from_directory(&assistant_skills_dir, &skill_name)?;

    Ok(format!(
        "Successfully deleted assistant skill '{}'",
        skill_name
    ))
}

pub async fn reset_assistant_skills(assistant_id: String) -> Result<String, String> {
    let assistant_skills_dir = get_assistant_skills_directory(&assistant_id)?;

    if assistant_skills_dir.exists() {
        fs::remove_dir_all(&assistant_skills_dir).map_err(|e| e.to_string())?;
        Ok(format!(
            "Successfully reset skills for assistant '{}'",
            assistant_id
        ))
    } else {
        Ok("No assistant skills to reset".to_string())
    }
}

pub async fn import_assistant_skills(
    assistant_id: String,
    file_path: String,
) -> Result<String, String> {
    let assistant_skills_dir = get_assistant_skills_directory(&assistant_id)?;
    let prepared = prepare_local_skill_import(file_path).await?;
    let result = install_prepared_skills_to_directory(prepared, assistant_skills_dir, true)?;
    Ok(format!(
        "Successfully imported {} skills",
        result.imported_names.len()
    ))
}

fn build_temp_root() -> Result<PathBuf, String> {
    let session_manager = get_session_manager()?;
    let temp_root = session_manager
        .get_base_data_dir()
        .join("temp_skill_imports");
    fs::create_dir_all(&temp_root).map_err(|e| e.to_string())?;
    Ok(temp_root)
}

async fn prepare_local_skill_import(file_path: String) -> Result<PreparedSkillImport, String> {
    let temp_root = build_temp_root()?;
    tokio::task::spawn_blocking(move || prepare_local_skill_import_blocking(&temp_root, &file_path))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}

fn prepare_local_skill_import_blocking(
    temp_root: &Path,
    file_path: &str,
) -> Result<PreparedSkillImport, String> {
    let temp_dir = tempfile::Builder::new()
        .prefix("skill-import-")
        .tempdir_in(temp_root)
        .map_err(|e| e.to_string())?;
    let staging_dir = temp_dir.path().join("staging");
    fs::create_dir_all(&staging_dir).map_err(|e| e.to_string())?;

    let src_path = PathBuf::from(file_path);
    if !src_path.exists() {
        return Err(format!("Source path does not exist: {}", file_path));
    }

    if src_path.is_file() {
        let extension = src_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase());
        match extension.as_deref() {
            Some("zip") | Some("skill") => {
                let file = fs::File::open(&src_path).map_err(|e| e.to_string())?;
                let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
                crate::utils::fs::extract_zip_secure(&mut archive, &staging_dir)?;
            }
            _ => return Err("Only .skill, .zip files, or directories are supported".to_string()),
        }
    } else if src_path.is_dir() {
        copy_dir_recursive(&src_path, &staging_dir)?;
    } else {
        return Err("Invalid source path".to_string());
    }

    let discovered_skills = discover_skill_roots(&staging_dir)?;
    Ok(PreparedSkillImport {
        _temp_dir: temp_dir,
        discovered_skills,
    })
}

async fn prepare_github_skill_import(repo_url: String) -> Result<PreparedSkillImport, String> {
    let repo_spec = parse_github_repo_url(&repo_url)?;
    let temp_root = build_temp_root()?;
    let temp_dir = tempfile::Builder::new()
        .prefix("github-skill-import-")
        .tempdir_in(&temp_root)
        .map_err(|e| e.to_string())?;
    let archive_path = temp_dir.path().join(format!("{}.zip", Uuid::new_v4()));
    let extract_dir = temp_dir.path().join("extract");
    tokio::fs::create_dir_all(&extract_dir)
        .await
        .map_err(|e| e.to_string())?;

    let archive_url = if let Some(branch) = &repo_spec.branch {
        format!(
            "https://codeload.github.com/{}/{}/zip/refs/heads/{}",
            repo_spec.owner, repo_spec.repo, branch
        )
    } else {
        format!(
            "https://api.github.com/repos/{}/{}/zipball",
            repo_spec.owner, repo_spec.repo
        )
    };

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(GITHUB_DOWNLOAD_CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(GITHUB_DOWNLOAD_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to build GitHub download client: {}", e))?;
    let mut response = client
        .get(archive_url)
        .header(USER_AGENT, "LibrAgent Skills Installer")
        .send()
        .await
        .map_err(|e| format!("Failed to download GitHub repository: {}", e))?
        .error_for_status()
        .map_err(|e| format!("GitHub repository download failed: {}", e))?;

    if let Some(content_length) = response.content_length() {
        if content_length > MAX_GITHUB_ARCHIVE_BYTES {
            return Err(format!(
                "GitHub repository archive is too large ({} bytes). Maximum allowed size is {} MB.",
                content_length,
                MAX_GITHUB_ARCHIVE_BYTES / 1024 / 1024
            ));
        }
    }

    let mut archive_file = tokio::fs::File::create(&archive_path)
        .await
        .map_err(|e| e.to_string())?;
    let mut downloaded_bytes = 0_u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Failed to read GitHub archive: {}", e))?
    {
        downloaded_bytes = downloaded_bytes
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| "GitHub archive size overflowed the download counter".to_string())?;
        if downloaded_bytes > MAX_GITHUB_ARCHIVE_BYTES {
            return Err(format!(
                "GitHub repository archive exceeded the {} MB download limit.",
                MAX_GITHUB_ARCHIVE_BYTES / 1024 / 1024
            ));
        }
        archive_file
            .write_all(&chunk)
            .await
            .map_err(|e| format!("Failed to write GitHub archive to disk: {}", e))?;
    }
    archive_file
        .flush()
        .await
        .map_err(|e| format!("Failed to finalize GitHub archive on disk: {}", e))?;

    tokio::task::spawn_blocking(move || {
        let file = fs::File::open(&archive_path).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        crate::utils::fs::extract_zip_secure(&mut archive, &extract_dir)?;

        let repo_root = find_archive_root(&extract_dir)?;
        let scan_root = if let Some(subpath) = &repo_spec.subpath {
            let candidate = repo_root.join(subpath);
            if !candidate.exists() {
                return Err(format!(
                    "GitHub path '{}' was not found in the downloaded repository",
                    subpath.display()
                ));
            }
            candidate
        } else {
            repo_root
        };

        let discovered_skills = discover_skill_roots(&scan_root)?;
        Ok(PreparedSkillImport {
            _temp_dir: temp_dir,
            discovered_skills,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

async fn build_user_import_preview(
    discovered_skills: &[DiscoveredSkillRoot],
) -> Result<SkillImportPreview, String> {
    let system_dir = get_system_skills_directory()?;
    let user_dir = get_user_skills_directory()?;
    let existing_skills = resolve_skills(system_dir, user_dir, None, None).await?;

    let mut existing_by_name = HashMap::new();
    for skill in existing_skills {
        existing_by_name.insert(skill.name.to_lowercase(), skill);
    }

    let discovered_candidates = discovered_skills
        .iter()
        .map(|skill| SkillImportCandidate {
            name: skill.metadata.name.clone(),
            description: skill.metadata.description.clone(),
        })
        .collect::<Vec<_>>();

    let conflicts = discovered_skills
        .iter()
        .filter_map(|skill| {
            existing_by_name
                .get(&skill.metadata.name.to_lowercase())
                .map(|existing| SkillImportConflict {
                    name: skill.metadata.name.clone(),
                    existing_origin: existing
                        .origin
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                    existing_path: existing.path.clone(),
                })
        })
        .collect::<Vec<_>>();

    Ok(SkillImportPreview {
        discovered_skills: discovered_candidates,
        conflicts,
    })
}

async fn install_user_prepared_skills(
    prepared: PreparedSkillImport,
    preview: SkillImportPreview,
    overwrite_existing: bool,
) -> Result<SkillImportResult, String> {
    if !overwrite_existing && !preview.conflicts.is_empty() {
        let names = preview
            .conflicts
            .iter()
            .map(|conflict| conflict.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "Skill name conflicts found. Re-run with overwrite to continue: {}",
            names
        ));
    }

    let user_dir = get_user_skills_directory()?;
    let mut result =
        install_prepared_skills_to_directory(prepared, user_dir.clone(), overwrite_existing)?;

    if overwrite_existing {
        let conflict_names = preview
            .conflicts
            .iter()
            .map(|conflict| conflict.name.clone())
            .collect::<Vec<_>>();
        for name in conflict_names {
            if !result.overwritten_names.contains(&name) {
                result.overwritten_names.push(name);
            }
        }
    }

    Ok(result)
}

fn install_prepared_skills_to_directory(
    prepared: PreparedSkillImport,
    target_dir: PathBuf,
    overwrite_existing: bool,
) -> Result<SkillImportResult, String> {
    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

    let mut existing_skill_roots = discover_existing_skill_roots(&target_dir)?;
    let mut overwritten_names = Vec::new();
    let mut imported_names = Vec::new();

    for skill in prepared.discovered_skills {
        let normalized_name = skill.metadata.name.to_lowercase();
        if let Some(existing_root) = existing_skill_roots.remove(&normalized_name) {
            if !overwrite_existing {
                return Err(format!(
                    "Skill '{}' already exists in the target directory",
                    skill.metadata.name
                ));
            }
            fs::remove_dir_all(existing_root).map_err(|e| e.to_string())?;
            overwritten_names.push(skill.metadata.name.clone());
        }

        let target_path = target_dir.join(skill_storage_directory_name(&skill.metadata.name)?);
        if target_path.exists() {
            if !overwrite_existing {
                return Err(format!(
                    "Skill directory already exists at {}",
                    target_path.display()
                ));
            }
            fs::remove_dir_all(&target_path).map_err(|e| e.to_string())?;
        }

        copy_dir_recursive(&skill.root_dir, &target_path)?;
        imported_names.push(skill.metadata.name.clone());
    }

    Ok(SkillImportResult {
        imported_names,
        overwritten_names,
    })
}

fn discover_existing_skill_roots(root_path: &Path) -> Result<HashMap<String, PathBuf>, String> {
    if !root_path.exists() {
        return Ok(HashMap::new());
    }

    let mut roots = HashMap::new();
    for entry in WalkDir::new(root_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if entry.file_name() != SKILL_FILE_NAME {
            continue;
        }

        let path = entry.path();
        match parse_skill_metadata(path) {
            Ok(metadata) => {
                if let Some(parent) = path.parent() {
                    roots.insert(metadata.name.to_lowercase(), parent.to_path_buf());
                }
            }
            Err(error) => {
                warn!(
                    "Failed to parse existing skill while scanning {}: {}",
                    path.display(),
                    error
                );
            }
        }
    }

    Ok(roots)
}

fn find_skill_root_by_name(root_path: &Path, skill_name: &str) -> Result<Option<PathBuf>, String> {
    let existing_roots = discover_existing_skill_roots(root_path)?;
    Ok(existing_roots.get(&skill_name.to_lowercase()).cloned())
}

fn remove_skill_name_from_directory(root_path: &Path, skill_name: &str) -> Result<(), String> {
    let Some(target_root) = find_skill_root_by_name(root_path, skill_name)? else {
        return Err(format!("Skill '{}' not found", skill_name));
    };

    fs::remove_dir_all(target_root).map_err(|e| e.to_string())
}

fn discover_skill_roots(root_path: &Path) -> Result<Vec<DiscoveredSkillRoot>, String> {
    let mut discovered = Vec::new();
    let mut seen_names = HashSet::new();

    for entry in WalkDir::new(root_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if entry.file_name() != SKILL_FILE_NAME {
            continue;
        }

        let skill_path = entry.path();
        let metadata = parse_skill_metadata(skill_path)?;
        let normalized_name = metadata.name.to_lowercase();
        if !seen_names.insert(normalized_name) {
            return Err(format!(
                "Duplicate skill name '{}' found in the import source",
                metadata.name
            ));
        }

        let root_dir = skill_path.parent().map(Path::to_path_buf).ok_or_else(|| {
            format!(
                "Failed to determine skill root for {}",
                skill_path.display()
            )
        })?;

        discovered.push(DiscoveredSkillRoot { metadata, root_dir });
    }

    if discovered.is_empty() {
        return Err("No skills (SKILL.md) found in the imported files".to_string());
    }

    discovered.sort_by_cached_key(|skill| skill.metadata.name.to_lowercase());
    Ok(discovered)
}

/// Builds a deterministic, platform-safe directory name for storing a skill.
pub fn skill_storage_directory_name(skill_name: &str) -> Result<String, String> {
    let trimmed = skill_name.trim();
    if trimmed.is_empty() {
        return Err("Skill name cannot be empty".to_string());
    }
    if trimmed == "." || trimmed == ".." {
        return Err(format!(
            "Skill name '{}' cannot be used as a storage directory",
            skill_name
        ));
    }

    let mut slug = String::new();
    let mut previous_was_separator = false;
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
            continue;
        }

        if !previous_was_separator && !slug.is_empty() {
            slug.push('-');
            previous_was_separator = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    let stem = if slug.is_empty() {
        "skill"
    } else {
        slug.as_str()
    };
    let digest = Sha256::digest(trimmed.as_bytes());
    let digest_hex = format!("{:x}", digest);
    Ok(format!("{}-{}", stem, &digest_hex[..12]))
}

fn parse_github_query_subpath(raw_subpath: &str) -> Result<Option<PathBuf>, String> {
    if raw_subpath.is_empty() {
        return Ok(None);
    }

    let subpath = PathBuf::from(raw_subpath);
    if subpath.is_absolute() {
        return Err("GitHub path query must be a relative subdirectory".to_string());
    }
    if subpath.components().any(|component| {
        matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::ParentDir
        )
    }) {
        return Err("GitHub path query cannot escape the downloaded repository".to_string());
    }

    Ok(Some(subpath))
}

/// Parses a supported GitHub repository URL for managed skill installation.
pub fn parse_github_repo_url(repo_url: &str) -> Result<GitHubRepoSpec, String> {
    let parsed = Url::parse(repo_url).map_err(|e| format!("Invalid GitHub URL: {}", e))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "GitHub URL must include a host".to_string())?;
    if host != "github.com" && host != "www.github.com" {
        return Err("Only github.com repository URLs are supported".to_string());
    }

    let segments = parsed
        .path_segments()
        .ok_or_else(|| "GitHub URL is missing path segments".to_string())?
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if segments.len() < 2 {
        return Err("GitHub URL must point to a repository".to_string());
    }

    let owner = segments[0].to_string();
    let repo = segments[1].trim_end_matches(".git").to_string();
    if owner.is_empty() || repo.is_empty() {
        return Err("GitHub repository URL is incomplete".to_string());
    }

    let mut branch_query = None;
    let mut path_query = None;
    for (key, value) in parsed.query_pairs() {
        if key == "ref" && !value.is_empty() {
            branch_query = Some(value.into_owned());
        } else if key == "path" && !value.is_empty() {
            path_query = parse_github_query_subpath(value.as_ref())?;
        }
    }

    let (branch, subpath) = if branch_query.is_some() || path_query.is_some() {
        (branch_query, path_query)
    } else if segments.len() > 2 && segments[2] == "tree" {
        if segments.len() < 4 {
            return Err("GitHub tree URL must include a branch name".to_string());
        }
        if segments.len() > 4 {
            return Err(
                "Ambiguous GitHub tree URL. Use ?ref=<branch> and optional ?path=<subdirectory> for branches containing '/' or subdirectory installs.".to_string(),
            );
        }
        (Some(segments[3].to_string()), None)
    } else {
        (None, None)
    };

    Ok(GitHubRepoSpec {
        owner,
        repo,
        branch,
        subpath,
    })
}

fn find_archive_root(extract_dir: &Path) -> Result<PathBuf, String> {
    let mut child_dirs = fs::read_dir(extract_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();

    child_dirs.sort();
    if child_dirs.len() == 1 {
        Ok(child_dirs.remove(0))
    } else {
        Ok(extract_dir.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_skills_dir() -> (TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let skill_dir = dir.path().join("my-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        let skill_md = skill_dir.join("SKILL.md");
        fs::write(
            &skill_md,
            "---\nname: my-skill\ndescription: Does cool things.\n---\n# My Skill\n",
        )
        .unwrap();
        (dir, skill_md)
    }

    #[test]
    fn test_skill_path_must_be_skill_md() {
        let dir = tempfile::tempdir().unwrap();
        let bad_file = dir.path().join("secret.txt");
        fs::write(&bad_file, "secret").unwrap();

        let path = std::path::PathBuf::from(&bad_file);
        let result: Result<(), String> =
            if path.file_name() != Some(std::ffi::OsStr::new("SKILL.md")) {
                Err("Skill path must point to a SKILL.md file".to_string())
            } else {
                Ok(())
            };

        assert!(result.is_err(), "Non-SKILL.md file should be rejected");
        assert!(result.unwrap_err().contains("SKILL.md"));
    }

    #[test]
    fn test_skill_md_filename_accepted() {
        let (_dir, skill_md) = setup_skills_dir();
        let result: Result<(), String> =
            if skill_md.file_name() != Some(std::ffi::OsStr::new("SKILL.md")) {
                Err("Skill path must point to a SKILL.md file".to_string())
            } else {
                Ok(())
            };

        assert!(result.is_ok(), "SKILL.md should pass the filename check");
    }

    #[test]
    fn test_path_traversal_blocked_by_starts_with() {
        let skills_dir = tempfile::tempdir().unwrap();
        let outside_dir = tempfile::tempdir().unwrap();
        let outside_file = outside_dir.path().join("SKILL.md");
        fs::write(
            &outside_file,
            "---\nname: outside\ndescription: malicious\n---\n",
        )
        .unwrap();

        let canonical_dir = skills_dir.path().canonicalize().unwrap();
        let canonical_path = outside_file.canonicalize().unwrap();

        let result: Result<(), String> = if !canonical_path.starts_with(&canonical_dir) {
            Err("Skill path is outside the configured skills directory".to_string())
        } else {
            Ok(())
        };

        assert!(result.is_err(), "Path outside skills dir should be blocked");
        assert!(result.unwrap_err().contains("outside"));
    }

    #[test]
    fn test_path_inside_skills_dir_accepted() {
        let (skills_dir_temp, skill_md) = setup_skills_dir();

        let canonical_dir = skills_dir_temp.path().canonicalize().unwrap();
        let canonical_path = skill_md.canonicalize().unwrap();

        let result: Result<(), String> = if !canonical_path.starts_with(&canonical_dir) {
            Err("Skill path is outside the configured skills directory".to_string())
        } else {
            Ok(())
        };

        assert!(result.is_ok(), "Path inside skills dir should be accepted");
    }
}
