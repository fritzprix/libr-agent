use super::contracts::{
    DiscoveredSkillRoot, PreparedSkillImport, SkillImportCandidate, SkillImportConflict,
    SkillImportPreview, SkillImportResult, SkillMetadata, GITHUB_DOWNLOAD_CONNECT_TIMEOUT_SECS,
    GITHUB_DOWNLOAD_TIMEOUT_SECS, MAX_GITHUB_ARCHIVE_BYTES, SKILL_FILE_NAME,
};
use super::directories::{
    get_assistant_skills_directory, get_system_skills_directory, get_user_skills_directory,
    resolve_skills,
};
use super::github::parse_github_repo_url;
use super::scanning::{copy_dir_recursive, parse_skill_metadata, scan_skills_internal};
use crate::session::get_session_manager;
use log::warn;
use reqwest::header::USER_AGENT;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;
use walkdir::WalkDir;

pub async fn preview_user_skill_import(file_path: String) -> Result<SkillImportPreview, String> {
    let prepared = prepare_local_skill_import(file_path).await?;
    build_user_import_preview(&prepared.discovered_skills).await
}

pub async fn import_user_skills(
    file_path: String,
    overwrite_existing: bool,
    excluded_skill_names: Option<Vec<String>>,
) -> Result<SkillImportResult, String> {
    let prepared = prepare_local_skill_import(file_path).await?;
    let preview = build_user_import_preview(&prepared.discovered_skills).await?;
    install_user_prepared_skills(
        prepared,
        preview,
        overwrite_existing,
        excluded_skill_names.unwrap_or_default(),
    )
    .await
}

pub async fn preview_github_skill_install(repo_url: String) -> Result<SkillImportPreview, String> {
    let prepared = prepare_github_skill_import(repo_url).await?;
    build_user_import_preview(&prepared.discovered_skills).await
}

pub async fn install_github_skills(
    repo_url: String,
    overwrite_existing: bool,
    excluded_skill_names: Option<Vec<String>>,
) -> Result<SkillImportResult, String> {
    let prepared = prepare_github_skill_import(repo_url).await?;
    let preview = build_user_import_preview(&prepared.discovered_skills).await?;
    install_user_prepared_skills(
        prepared,
        preview,
        overwrite_existing,
        excluded_skill_names.unwrap_or_default(),
    )
    .await
}

pub async fn delete_user_skill(skill_name: String) -> Result<String, String> {
    let user_dir = get_user_skills_directory()?;
    remove_skill_name_from_directory(&user_dir, &skill_name)?;
    Ok(format!("Successfully deleted user skill '{}'", skill_name))
}

pub async fn reset_user_skills() -> Result<String, String> {
    let user_dir = get_user_skills_directory()?;
    if user_dir.exists() {
        fs::remove_dir_all(&user_dir).map_err(|error| error.to_string())?;
        Ok("Successfully reset user skills".to_string())
    } else {
        Ok("No user skills to reset".to_string())
    }
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
        fs::remove_dir_all(&assistant_skills_dir).map_err(|error| error.to_string())?;
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
    fs::create_dir_all(&temp_root).map_err(|error| error.to_string())?;
    Ok(temp_root)
}

async fn prepare_local_skill_import(file_path: String) -> Result<PreparedSkillImport, String> {
    let temp_root = build_temp_root()?;
    tokio::task::spawn_blocking(move || prepare_local_skill_import_blocking(&temp_root, &file_path))
        .await
        .map_err(|error| format!("Task join error: {}", error))?
}

fn prepare_local_skill_import_blocking(
    temp_root: &Path,
    file_path: &str,
) -> Result<PreparedSkillImport, String> {
    let temp_dir = tempfile::Builder::new()
        .prefix("skill-import-")
        .tempdir_in(temp_root)
        .map_err(|error| error.to_string())?;
    let staging_dir = temp_dir.path().join("staging");
    fs::create_dir_all(&staging_dir).map_err(|error| error.to_string())?;

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
                let file = fs::File::open(&src_path).map_err(|error| error.to_string())?;
                let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
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
        .map_err(|error| error.to_string())?;
    let archive_path = temp_dir.path().join(format!("{}.zip", Uuid::new_v4()));
    let extract_dir = temp_dir.path().join("extract");
    tokio::fs::create_dir_all(&extract_dir)
        .await
        .map_err(|error| error.to_string())?;

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
        .map_err(|error| format!("Failed to build GitHub download client: {}", error))?;
    let mut response = client
        .get(archive_url)
        .header(USER_AGENT, "LibrAgent Skills Installer")
        .send()
        .await
        .map_err(|error| format!("Failed to download GitHub repository: {}", error))?
        .error_for_status()
        .map_err(|error| format!("GitHub repository download failed: {}", error))?;

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
        .map_err(|error| error.to_string())?;
    let mut downloaded_bytes = 0_u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("Failed to read GitHub archive: {}", error))?
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
            .map_err(|error| format!("Failed to write GitHub archive to disk: {}", error))?;
    }
    archive_file
        .flush()
        .await
        .map_err(|error| format!("Failed to finalize GitHub archive on disk: {}", error))?;

    tokio::task::spawn_blocking(move || {
        let file = fs::File::open(&archive_path).map_err(|error| error.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
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
    .map_err(|error| format!("Task join error: {}", error))?
}

async fn build_user_import_preview(
    discovered_skills: &[DiscoveredSkillRoot],
) -> Result<SkillImportPreview, String> {
    let system_dir = get_system_skills_directory()?;
    let user_dir = get_user_skills_directory()?;
    let system_skills = scan_skills_internal(
        &system_dir,
        Some("global".to_string()),
        Some("system".to_string()),
    )
    .await?;
    let user_skills = scan_skills_internal(
        &user_dir,
        Some("global".to_string()),
        Some("user".to_string()),
    )
    .await?;

    let discovered_candidates = discovered_skills
        .iter()
        .map(|skill| SkillImportCandidate {
            name: skill.metadata.name.clone(),
            description: skill.metadata.description.clone(),
        })
        .collect::<Vec<_>>();
    let conflicts =
        build_skill_import_conflicts(&discovered_candidates, &system_skills, &user_skills);

    Ok(SkillImportPreview {
        discovered_skills: discovered_candidates,
        conflicts,
    })
}

pub fn build_skill_import_conflicts(
    discovered_skills: &[SkillImportCandidate],
    system_skills: &[SkillMetadata],
    user_skills: &[SkillMetadata],
) -> Vec<SkillImportConflict> {
    let system_by_name = system_skills
        .iter()
        .map(|skill| (skill.name.to_lowercase(), skill))
        .collect::<HashMap<_, _>>();
    let user_by_name = user_skills
        .iter()
        .map(|skill| (skill.name.to_lowercase(), skill))
        .collect::<HashMap<_, _>>();

    discovered_skills
        .iter()
        .filter_map(|skill| {
            if let Some(existing) = system_by_name.get(&skill.name.to_lowercase()) {
                return Some(SkillImportConflict {
                    name: skill.name.clone(),
                    existing_origin: existing
                        .origin
                        .clone()
                        .unwrap_or_else(|| "system".to_string()),
                    existing_path: existing.path.clone(),
                });
            }

            user_by_name
                .get(&skill.name.to_lowercase())
                .map(|existing| SkillImportConflict {
                    name: skill.name.clone(),
                    existing_origin: existing
                        .origin
                        .clone()
                        .unwrap_or_else(|| "user".to_string()),
                    existing_path: existing.path.clone(),
                })
        })
        .collect()
}

fn blocked_system_conflict_names(conflicts: &[SkillImportConflict]) -> Vec<String> {
    conflicts
        .iter()
        .filter(|conflict| conflict.existing_origin == "system")
        .map(|conflict| conflict.name.clone())
        .collect()
}

async fn install_user_prepared_skills(
    prepared: PreparedSkillImport,
    preview: SkillImportPreview,
    overwrite_existing: bool,
    excluded_skill_names: Vec<String>,
) -> Result<SkillImportResult, String> {
    let mut excluded_names = excluded_skill_names
        .into_iter()
        .map(|name| (name.to_lowercase(), name))
        .collect::<HashMap<_, _>>();
    for name in blocked_system_conflict_names(&preview.conflicts) {
        excluded_names.entry(name.to_lowercase()).or_insert(name);
    }

    let retained_conflicts = preview
        .conflicts
        .iter()
        .filter(|conflict| {
            conflict.existing_origin != "system"
                && !excluded_names.contains_key(&conflict.name.to_lowercase())
        })
        .cloned()
        .collect::<Vec<_>>();

    if !overwrite_existing && !retained_conflicts.is_empty() {
        let names = retained_conflicts
            .iter()
            .map(|conflict| conflict.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "Skill name conflicts found. Re-run with overwrite to continue: {}",
            names
        ));
    }

    let PreparedSkillImport {
        _temp_dir,
        discovered_skills,
    } = prepared;
    let mut skipped_names = Vec::new();
    let selected_skills = discovered_skills
        .into_iter()
        .filter_map(|skill| {
            if excluded_names.contains_key(&skill.metadata.name.to_lowercase()) {
                skipped_names.push(skill.metadata.name.clone());
                None
            } else {
                Some(skill)
            }
        })
        .collect::<Vec<_>>();

    let user_dir = get_user_skills_directory()?;
    let prepared = PreparedSkillImport {
        _temp_dir,
        discovered_skills: selected_skills,
    };
    let mut result =
        install_prepared_skills_to_directory(prepared, user_dir.clone(), overwrite_existing)?;

    if overwrite_existing {
        let conflict_names = retained_conflicts
            .iter()
            .map(|conflict| conflict.name.clone())
            .collect::<Vec<_>>();
        for name in conflict_names {
            if !result.overwritten_names.contains(&name) {
                result.overwritten_names.push(name);
            }
        }
    }

    result.skipped_names = skipped_names;
    Ok(result)
}

fn install_prepared_skills_to_directory(
    prepared: PreparedSkillImport,
    target_dir: PathBuf,
    overwrite_existing: bool,
) -> Result<SkillImportResult, String> {
    fs::create_dir_all(&target_dir).map_err(|error| error.to_string())?;

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
            fs::remove_dir_all(existing_root).map_err(|error| error.to_string())?;
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
            fs::remove_dir_all(&target_path).map_err(|error| error.to_string())?;
        }

        copy_dir_recursive(&skill.root_dir, &target_path)?;
        imported_names.push(skill.metadata.name.clone());
    }

    Ok(SkillImportResult {
        imported_names,
        overwritten_names,
        skipped_names: Vec::new(),
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

    fs::remove_dir_all(target_root).map_err(|error| error.to_string())
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

fn find_archive_root(extract_dir: &Path) -> Result<PathBuf, String> {
    let mut child_dirs = fs::read_dir(extract_dir)
        .map_err(|error| error.to_string())?
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
