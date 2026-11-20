use super::cache::{invalidate_skill_scan_cache, scan_skills_internal_cached};
use super::contracts::{
    DiscoveredSkillRoot, HookPreviewEntry, HookPreviewExecutionStatus, PluginManifestSummary,
    PreparedSkillImport, SkillImportCandidate, SkillImportConflict, SkillImportKind,
    SkillImportPreview, SkillImportResult, SkillMetadata, GITHUB_DOWNLOAD_CONNECT_TIMEOUT_SECS,
    GITHUB_DOWNLOAD_TIMEOUT_SECS, MAX_GITHUB_ARCHIVE_BYTES, SKILL_FILE_NAME,
};
use super::directories::{
    get_assistant_skills_directory, get_system_skills_directory, get_user_skills_directory,
    resolve_skills,
};
use super::github::parse_github_repo_url;
use super::plugin_manifest::{self, PluginManifest};
use super::scanning::{copy_dir_recursive, parse_skill_metadata};
use crate::services::hooks::registry::{
    get_user_plugins_directory, save_source_in, PluginMetadata, PluginSourceManifest,
};
use crate::services::hooks::schema::{HookDispatchMode, HooksConfig};
use crate::session::get_session_manager;
use crate::state::wait_for_managed_skills_sync;
use log::warn;
use reqwest::header::USER_AGENT;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;
use walkdir::WalkDir;

pub async fn preview_user_skill_import(file_path: String) -> Result<SkillImportPreview, String> {
    let mut prepared = prepare_local_skill_import(file_path).await?;
    build_user_import_preview(&mut prepared).await
}

pub async fn import_user_skills(
    file_path: String,
    overwrite_existing: bool,
    excluded_skill_names: Option<Vec<String>>,
    approve_hooks: Option<bool>,
) -> Result<SkillImportResult, String> {
    let mut prepared = prepare_local_skill_import(file_path).await?;
    let preview = build_user_import_preview(&mut prepared).await?;
    install_user_prepared_skills(
        prepared,
        preview,
        overwrite_existing,
        excluded_skill_names.unwrap_or_default(),
        approve_hooks.unwrap_or(false),
    )
    .await
}

pub async fn preview_github_skill_install(repo_url: String) -> Result<SkillImportPreview, String> {
    let mut prepared = prepare_github_skill_import(repo_url).await?;
    build_user_import_preview(&mut prepared).await
}

pub async fn install_github_skills(
    repo_url: String,
    overwrite_existing: bool,
    excluded_skill_names: Option<Vec<String>>,
    approve_hooks: Option<bool>,
) -> Result<SkillImportResult, String> {
    let mut prepared = prepare_github_skill_import(repo_url).await?;
    let preview = build_user_import_preview(&mut prepared).await?;
    install_user_prepared_skills(
        prepared,
        preview,
        overwrite_existing,
        excluded_skill_names.unwrap_or_default(),
        approve_hooks.unwrap_or(false),
    )
    .await
}

fn invalidate_skill_cache_after_success<T>(result: Result<T, String>) -> Result<T, String> {
    if result.is_ok() {
        invalidate_skill_scan_cache();
    }
    result
}

struct SkillCacheInvalidationGuard {
    should_invalidate: bool,
}

impl SkillCacheInvalidationGuard {
    fn new() -> Self {
        Self {
            should_invalidate: false,
        }
    }

    fn mark_mutated(&mut self) {
        self.should_invalidate = true;
    }
}

impl Drop for SkillCacheInvalidationGuard {
    fn drop(&mut self) {
        if self.should_invalidate {
            invalidate_skill_scan_cache();
        }
    }
}

pub async fn delete_user_skill(skill_name: String) -> Result<String, String> {
    let user_dir = get_user_skills_directory()?;
    invalidate_skill_cache_after_success(remove_skill_name_from_directory(&user_dir, &skill_name))
        .map(|_| format!("Successfully deleted user skill '{}'", skill_name))
}

pub async fn reset_user_skills() -> Result<String, String> {
    let user_dir = get_user_skills_directory()?;
    if user_dir.exists() {
        invalidate_skill_cache_after_success(
            fs::remove_dir_all(&user_dir).map_err(|error| error.to_string()),
        )?;
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
    invalidate_skill_cache_after_success(copy_dir_recursive(&source_root, &target_path))?;

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
    invalidate_skill_cache_after_success(remove_skill_name_from_directory(
        &assistant_skills_dir,
        &skill_name,
    ))?;

    Ok(format!(
        "Successfully deleted assistant skill '{}'",
        skill_name
    ))
}

pub async fn reset_assistant_skills(assistant_id: String) -> Result<String, String> {
    let assistant_skills_dir = get_assistant_skills_directory(&assistant_id)?;

    if assistant_skills_dir.exists() {
        invalidate_skill_cache_after_success(
            fs::remove_dir_all(&assistant_skills_dir).map_err(|error| error.to_string()),
        )?;
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
    let result = install_prepared_skills_to_directory(&prepared, assistant_skills_dir, true)?;
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

pub(super) fn prepare_local_skill_import_blocking(
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
        scan_root: staging_dir,
        source_key: file_path.to_string(),
        discovered_skills,
    })
}

pub(super) async fn prepare_github_skill_import(
    repo_url: String,
) -> Result<PreparedSkillImport, String> {
    let source_key = repo_url.clone();
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
            scan_root,
            source_key,
            discovered_skills,
        })
    })
    .await
    .map_err(|error| format!("Task join error: {}", error))?
}

async fn build_user_import_preview(
    prepared: &mut PreparedSkillImport,
) -> Result<SkillImportPreview, String> {
    let user_dir = get_user_skills_directory()?;
    let system_dir = get_system_skills_directory()?;
    let same_source = same_source_skill_names(&prepared.source_key);
    build_import_preview(
        prepared,
        &user_dir,
        "global",
        "user",
        &system_dir,
        &same_source,
    )
    .await
}

/// Skill names recorded in the plugin source manifest for this source key,
/// i.e. the skills this same plugin installed on a previous install. They
/// must never count as #2921 namespace collisions: a re-install or update of
/// a manifest plugin has to keep the existing overwrite-conflict flow rather
/// than renaming its own skills to `plugin:skill` and installing a duplicate
/// next to the stale copy (gatekeeper G1.1).
fn same_source_skill_names(source_key: &str) -> Vec<String> {
    crate::services::hooks::registry::get_source(&plugin_source_id(source_key))
        .map(|record| record.manifest.skill_names)
        .unwrap_or_default()
}

/// Build an import preview whose conflicts are computed against the bundled
/// system skills (`system_skills_dir`) plus the target skills directory
/// (`source_tag`/`origin_tag` label that directory's existing skills in the
/// reported conflicts).
///
/// When the source carries a `.claude-plugin/plugin.json` (#2921), its
/// component paths are validated here (preview-time validation errors), its
/// `hooks` files are added to the default `hooks/hooks.json` discovery, and
/// plugin skills whose names collide with other skills are renamed to
/// `plugin:skill` — in the staging copy, so the same names carry into the
/// install. `same_source_skills` (from the plugin's own source manifest) are
/// excluded from the collision set so re-installs keep overwrite semantics.
#[allow(clippy::too_many_arguments)]
pub(super) async fn build_import_preview(
    prepared: &mut PreparedSkillImport,
    target_skills_dir: &Path,
    source_tag: &str,
    origin_tag: &str,
    system_skills_dir: &Path,
    same_source_skills: &[String],
) -> Result<SkillImportPreview, String> {
    wait_for_managed_skills_sync().await;

    let manifest = plugin_manifest::load_plugin_manifest(&prepared.scan_root)?;
    if let Some(manifest) = &manifest {
        plugin_manifest::validate_manifest_paths(manifest)?;
    }

    let system_skills = scan_skills_internal_cached(
        system_skills_dir,
        Some("global".to_string()),
        Some("system".to_string()),
    )
    .await?;
    let user_skills = scan_skills_internal_cached(
        target_skills_dir,
        Some(source_tag.to_string()),
        Some(origin_tag.to_string()),
    )
    .await?;

    if let Some(manifest) = &manifest {
        let same_source_lower: HashSet<String> = same_source_skills
            .iter()
            .map(|name| name.to_lowercase())
            .collect();
        let existing_lower: HashSet<String> = system_skills
            .iter()
            .chain(user_skills.iter())
            .map(|skill| skill.name.to_lowercase())
            .filter(|name| !same_source_lower.contains(name))
            .collect();
        namescope_plugin_skills(prepared, &manifest.name, &existing_lower)?;
    }

    let discovered_candidates = prepared
        .discovered_skills
        .iter()
        .map(|skill| SkillImportCandidate {
            name: skill.metadata.name.clone(),
            description: skill.metadata.description.clone(),
        })
        .collect::<Vec<_>>();
    let conflicts =
        build_skill_import_conflicts(&discovered_candidates, &system_skills, &user_skills);

    let hooks_config = try_load_plugin_hooks(&prepared.scan_root, manifest.as_ref())?;
    let hooks = hooks_config
        .as_ref()
        .map(build_hook_preview_entries)
        .unwrap_or_default();
    let kind = if hooks.is_empty() {
        SkillImportKind::SkillBundle
    } else {
        SkillImportKind::Plugin
    };
    let hooks_require_approval = !hooks.is_empty();

    Ok(SkillImportPreview {
        kind,
        discovered_skills: discovered_candidates,
        conflicts,
        hooks,
        hooks_require_approval,
        plugin_manifest: manifest.as_ref().map(|manifest| PluginManifestSummary {
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            description: manifest.description.clone(),
        }),
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

/// Where an import lands: skills directory + plugin-source store.
pub(super) struct ImportTarget {
    pub(super) skills_dir: PathBuf,
    pub(super) plugins_dir: PathBuf,
    /// Record a plugin-source manifest even for hook-less skill bundles, so
    /// the install is listable and removable (session-scoped installs).
    pub(super) always_record_source: bool,
}

async fn install_user_prepared_skills(
    prepared: PreparedSkillImport,
    preview: SkillImportPreview,
    overwrite_existing: bool,
    excluded_skill_names: Vec<String>,
    approve_hooks: bool,
) -> Result<SkillImportResult, String> {
    let target = ImportTarget {
        skills_dir: get_user_skills_directory()?,
        plugins_dir: get_user_plugins_directory()?,
        always_record_source: false,
    };
    install_prepared_import(
        prepared,
        preview,
        overwrite_existing,
        excluded_skill_names,
        approve_hooks,
        target,
    )
}

pub(super) fn install_prepared_import(
    prepared: PreparedSkillImport,
    preview: SkillImportPreview,
    overwrite_existing: bool,
    excluded_skill_names: Vec<String>,
    approve_hooks: bool,
    target: ImportTarget,
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
            "Skill name conflicts found. Re-run with overwrite to continue, or provide excluded_skill_names to skip specific conflicting skills: {}",
            names
        ));
    }

    let PreparedSkillImport {
        _temp_dir,
        scan_root,
        source_key,
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

    let prepared = PreparedSkillImport {
        _temp_dir,
        scan_root: scan_root.clone(),
        source_key: source_key.clone(),
        discovered_skills: selected_skills,
    };
    let mut result =
        install_prepared_skills_to_directory(&prepared, target.skills_dir, overwrite_existing)?;

    if preview.kind == SkillImportKind::Plugin || target.always_record_source {
        install_plugin_source(
            &scan_root,
            &source_key,
            &result.imported_names,
            approve_hooks,
            &target.plugins_dir,
        )?;
    }

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

/// Borrows `prepared` so the caller keeps its staging temp dir alive until
/// follow-up steps (e.g. copying the plugin source) are done.
fn install_prepared_skills_to_directory(
    prepared: &PreparedSkillImport,
    target_dir: PathBuf,
    overwrite_existing: bool,
) -> Result<SkillImportResult, String> {
    fs::create_dir_all(&target_dir).map_err(|error| error.to_string())?;

    let mut cache_invalidation_guard = SkillCacheInvalidationGuard::new();
    let mut existing_skill_roots = discover_existing_skill_roots(&target_dir)?;
    let mut overwritten_names = Vec::new();
    let mut imported_names = Vec::new();

    for skill in &prepared.discovered_skills {
        let normalized_name = skill.metadata.name.to_lowercase();
        if let Some(existing_root) = existing_skill_roots.remove(&normalized_name) {
            if !overwrite_existing {
                return Err(format!(
                    "Skill '{}' already exists in the target directory",
                    skill.metadata.name
                ));
            }
            fs::remove_dir_all(existing_root).map_err(|error| error.to_string())?;
            cache_invalidation_guard.mark_mutated();
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
            cache_invalidation_guard.mark_mutated();
        }

        copy_dir_recursive(&skill.root_dir, &target_path)?;
        cache_invalidation_guard.mark_mutated();
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

fn try_load_plugin_hooks(
    scan_root: &Path,
    manifest: Option<&PluginManifest>,
) -> Result<Option<HooksConfig>, String> {
    let hooks_dir = scan_root.join("hooks");
    let default_main = hooks_dir.join("hooks.json");
    let mut files: Vec<PathBuf> = Vec::new();
    for candidate in [
        default_main.as_path(),
        hooks_dir.join("hooks-cursor.json").as_path(),
    ] {
        if candidate.is_file() {
            files.push(candidate.to_path_buf());
        }
    }

    // Spec (plugins-reference): `hooks/hooks.json` is auto-loaded and the
    // manifest `hooks` field lists ADDITIONAL hook files; a manifest entry
    // that duplicates the default is rejected (gatekeeper S1.1).
    if let Some(paths) = plugin_manifest::hook_config_paths(manifest) {
        for raw in paths {
            let resolved = scan_root.join(&raw);
            if resolved == default_main {
                return Err(format!(
                    "plugin manifest hooks path {raw:?} duplicates the default hooks/hooks.json"
                ));
            }
            if !resolved.is_file() {
                return Err(format!(
                    "plugin manifest hooks path {raw:?} not found in the plugin"
                ));
            }
            files.push(resolved);
        }
    }

    if files.is_empty() {
        return Ok(None);
    }
    crate::services::hooks::schema::load_hooks_from_config_files(&files).map(Some)
}

fn build_hook_preview_entries(config: &HooksConfig) -> Vec<HookPreviewEntry> {
    let mut entries = Vec::new();
    for (event, groups) in &config.events {
        let execution_status = match event.dispatch_mode() {
            HookDispatchMode::Execute => HookPreviewExecutionStatus::Executed,
            HookDispatchMode::Stub => HookPreviewExecutionStatus::Stubbed,
        };
        for group in groups {
            for handler in &group.handlers {
                entries.push(HookPreviewEntry {
                    event: event.as_str().to_string(),
                    matcher: group.matcher.clone(),
                    command: handler.command.clone(),
                    powershell: handler.powershell.clone(),
                    script_body: None,
                    execution_status,
                });
            }
        }
    }
    for (raw_event, unsupported) in &config.unsupported_events {
        for item in unsupported {
            entries.push(HookPreviewEntry {
                event: raw_event.clone(),
                matcher: item.matcher.clone(),
                command: item.command.clone(),
                powershell: None,
                script_body: None,
                execution_status: HookPreviewExecutionStatus::Stubbed,
            });
        }
    }
    entries.sort_by(|a, b| {
        a.event
            .cmp(&b.event)
            .then_with(|| a.matcher.cmp(&b.matcher))
            .then_with(|| a.command.cmp(&b.command))
    });
    entries
}

pub(super) fn plugin_source_id(source_key: &str) -> String {
    let digest = Sha256::digest(source_key.as_bytes());
    format!("plugin-{}", &format!("{:x}", digest)[..16])
}

/// Rename plugin skills whose names collide with existing skills to
/// `<plugin-name>:<skill-name>` (Claude Code's namespace convention, #2921),
/// rewriting the staged `SKILL.md` so the installed skill carries the
/// namespaced name. Neither side hides the other afterwards.
fn namescope_plugin_skills(
    prepared: &mut PreparedSkillImport,
    plugin_name: &str,
    existing_lower: &HashSet<String>,
) -> Result<(), String> {
    for skill in &mut prepared.discovered_skills {
        if !existing_lower.contains(&skill.metadata.name.to_lowercase()) {
            continue;
        }
        let renamed = format!("{}:{}", plugin_name, skill.metadata.name);
        rewrite_skill_frontmatter_name(&skill.root_dir.join(SKILL_FILE_NAME), &renamed)?;
        skill.metadata.name = renamed;
    }
    Ok(())
}

/// Replace the top-level `name:` line inside a skill's YAML frontmatter with
/// `new_name`, keeping every other line — and the original line endings —
/// byte-identical where possible. Only a non-indented `name:` matches: an
/// indented one belongs to a nested map, not the skill name (gatekeeper S1.2).
fn rewrite_skill_frontmatter_name(skill_file: &Path, new_name: &str) -> Result<(), String> {
    let raw = fs::read_to_string(skill_file)
        .map_err(|error| format!("read {}: {error}", skill_file.display()))?;
    let content = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    let rest = content
        .strip_prefix("---")
        .ok_or_else(|| format!("{}: missing YAML frontmatter", skill_file.display()))?;
    let end = rest
        .find("\n---")
        .ok_or_else(|| format!("{}: missing YAML frontmatter end", skill_file.display()))?;
    let (frontmatter, tail) = (&rest[..end], &rest[end..]);
    // A JSON string is a valid YAML double-quoted scalar for any name.
    let quoted = serde_json::to_string(new_name).map_err(|error| error.to_string())?;
    let mut replaced = false;
    let rewritten = frontmatter
        .split('\n')
        .map(|line| {
            let (body, eol) = match line.strip_suffix('\r') {
                Some(body) => (body, "\r"),
                None => (line, ""),
            };
            if !replaced && body.starts_with("name:") {
                replaced = true;
                format!("name: {quoted}{eol}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if !replaced {
        return Err(format!(
            "{}: no name field in frontmatter",
            skill_file.display()
        ));
    }
    fs::write(skill_file, format!("---{rewritten}{tail}")).map_err(|error| error.to_string())
}

fn plugin_metadata_from_manifest(manifest: Option<&PluginManifest>) -> Option<PluginMetadata> {
    let manifest = manifest?;
    Some(PluginMetadata {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        description: manifest.description.clone(),
        author: manifest
            .author
            .as_ref()
            .map(|author| author.display_name().to_string()),
        homepage: manifest.homepage.clone(),
        repository: manifest.repository.clone(),
        license: manifest.license.clone(),
        keywords: manifest.keywords.clone().unwrap_or_default(),
    })
}

fn install_plugin_source(
    scan_root: &Path,
    source_key: &str,
    skill_names: &[String],
    approve_hooks: bool,
    plugins_dir: &Path,
) -> Result<(), String> {
    let manifest = plugin_manifest::load_plugin_manifest(scan_root)?;
    let config = try_load_plugin_hooks(scan_root, manifest.as_ref())?;

    let source_id = plugin_source_id(source_key);
    let target = plugins_dir.join(&source_id);
    if target.exists() {
        fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
    }
    copy_dir_recursive(scan_root, &target)?;

    let hooks_approved = approve_hooks && config.is_some();
    let approval_hashes = match &config {
        Some(config) if hooks_approved => {
            crate::services::hooks::registry::compute_handler_approval_hashes(&target, config)
        }
        _ => BTreeMap::new(),
    };

    save_source_in(
        plugins_dir,
        PluginSourceManifest {
            source_url: source_key.to_string(),
            source_id,
            skill_names: skill_names.to_vec(),
            hooks: config.as_ref().map(|config| {
                crate::services::hooks::registry::HooksConfigSnapshot {
                    version: config.version,
                    validated: true,
                }
            }),
            hooks_approved,
            approval_hashes,
            created_at: chrono::Utc::now().timestamp(),
            plugin: plugin_metadata_from_manifest(manifest.as_ref()),
        },
    )?;

    Ok(())
}

pub(super) fn remove_skill_names_from_user_skills(
    user_dir: &Path,
    skill_names: &[String],
) -> Result<(), String> {
    let mut cache_invalidation_guard = SkillCacheInvalidationGuard::new();
    for name in skill_names {
        if remove_skill_name_from_directory(user_dir, name).is_ok() {
            cache_invalidation_guard.mark_mutated();
        }
    }
    Ok(())
}

#[cfg(test)]
mod hook_preview_tests {
    use super::*;
    use std::fs;

    fn write_valid_skill(dir: &Path, name: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: test\n---\n"),
        )
        .unwrap();
    }

    #[test]
    fn skill_bundle_when_no_hooks_present() {
        let dir = tempfile::tempdir().unwrap();
        write_valid_skill(dir.path(), "alpha");
        let hooks = try_load_plugin_hooks(dir.path(), None).unwrap();
        assert!(hooks.is_none());
    }

    #[test]
    fn plugin_preview_when_hooks_valid() {
        let dir = tempfile::tempdir().unwrap();
        write_valid_skill(dir.path(), "alpha");
        let hooks_dir = dir.path().join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        fs::write(
            hooks_dir.join("hooks.json"),
            r#"{"hooks":{"SessionStart":[{"command":"echo hi"}]}}"#,
        )
        .unwrap();
        let cfg = try_load_plugin_hooks(dir.path(), None)
            .unwrap()
            .expect("hooks");
        let entries = build_hook_preview_entries(&cfg);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].execution_status,
            HookPreviewExecutionStatus::Executed
        );
    }

    #[test]
    fn invalid_hooks_fail_preview_load() {
        let dir = tempfile::tempdir().unwrap();
        write_valid_skill(dir.path(), "alpha");
        let hooks_dir = dir.path().join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        fs::write(
            hooks_dir.join("hooks.json"),
            r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"x","timeout":0}]}]}}"#,
        )
        .unwrap();
        let err = try_load_plugin_hooks(dir.path(), None).unwrap_err();
        assert!(err.contains("timeout"));
    }

    fn write_manifest(root: &Path, json: &str) {
        let dir = root.join(".claude-plugin");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("plugin.json"), json).unwrap();
    }

    #[test]
    fn manifest_hooks_paths_add_to_the_default_discovery() {
        let dir = tempfile::tempdir().unwrap();
        write_valid_skill(dir.path(), "alpha");
        let hooks_dir = dir.path().join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        fs::write(
            hooks_dir.join("hooks.json"),
            r#"{"hooks":{"SessionStart":[{"command":"echo default"}]}}"#,
        )
        .unwrap();
        fs::write(
            hooks_dir.join("custom.json"),
            r#"{"hooks":{"SessionStart":[{"command":"echo custom"}]}}"#,
        )
        .unwrap();
        write_manifest(
            dir.path(),
            r#"{ "name": "p", "hooks": "./hooks/custom.json" }"#,
        );
        let manifest = plugin_manifest::load_plugin_manifest(dir.path())
            .unwrap()
            .expect("manifest");
        let cfg = try_load_plugin_hooks(dir.path(), Some(&manifest))
            .unwrap()
            .expect("hooks");
        let entries = build_hook_preview_entries(&cfg);
        assert_eq!(entries.len(), 2, "manifest hooks add to the default");
        let commands: Vec<&str> = entries.iter().map(|e| e.command.as_str()).collect();
        assert_eq!(commands, ["echo custom", "echo default"]);
    }

    #[test]
    fn manifest_hooks_path_duplicating_the_default_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        write_valid_skill(dir.path(), "alpha");
        let hooks_dir = dir.path().join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        fs::write(
            hooks_dir.join("hooks.json"),
            r#"{"hooks":{"SessionStart":[{"command":"echo default"}]}}"#,
        )
        .unwrap();
        write_manifest(
            dir.path(),
            r#"{ "name": "p", "hooks": "./hooks/hooks.json" }"#,
        );
        let manifest = plugin_manifest::load_plugin_manifest(dir.path())
            .unwrap()
            .expect("manifest");
        let err = try_load_plugin_hooks(dir.path(), Some(&manifest)).unwrap_err();
        assert!(err.contains("duplicates"), "error explains the rule: {err}");
    }

    #[test]
    fn manifest_hooks_path_missing_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        write_valid_skill(dir.path(), "alpha");
        write_manifest(
            dir.path(),
            r#"{ "name": "p", "hooks": "./hooks/missing.json" }"#,
        );
        let manifest = plugin_manifest::load_plugin_manifest(dir.path())
            .unwrap()
            .expect("manifest");
        let err = try_load_plugin_hooks(dir.path(), Some(&manifest)).unwrap_err();
        assert!(err.contains("missing.json"), "error names the path: {err}");
    }

    fn staged_skill(dir: &std::path::Path, name: &str) -> DiscoveredSkillRoot {
        let root_dir = dir.join(name);
        write_valid_skill(&root_dir, name);
        DiscoveredSkillRoot {
            metadata: SkillMetadata {
                name: name.to_string(),
                description: "test".to_string(),
                path: root_dir.join(SKILL_FILE_NAME).to_string_lossy().to_string(),
                alias_path: None,
                source: None,
                origin: None,
            },
            root_dir,
        }
    }

    #[test]
    fn namescope_plugin_skills_renames_only_collisions() {
        let temp = tempfile::tempdir().unwrap();
        let staging = temp.path().join("staging");
        fs::create_dir_all(&staging).unwrap();

        let mut prepared = PreparedSkillImport {
            _temp_dir: temp,
            scan_root: staging.clone(),
            source_key: "test://plugin".to_string(),
            discovered_skills: vec![
                staged_skill(&staging, "alpha"),
                staged_skill(&staging, "beta"),
            ],
        };
        let existing: HashSet<String> = ["ALPHA".to_string().to_lowercase()].into();

        namescope_plugin_skills(&mut prepared, "superpowers", &existing).unwrap();

        assert_eq!(
            prepared.discovered_skills[0].metadata.name,
            "superpowers:alpha"
        );
        assert_eq!(prepared.discovered_skills[1].metadata.name, "beta");
        // The rename must be on disk so the install carries it.
        let rewritten =
            parse_skill_metadata(&prepared.discovered_skills[0].root_dir.join(SKILL_FILE_NAME))
                .unwrap();
        assert_eq!(rewritten.name, "superpowers:alpha");
        assert_eq!(rewritten.description, "test");
    }

    #[test]
    fn namescope_rewrite_preserves_crlf_and_ignores_nested_name() {
        let temp = tempfile::tempdir().unwrap();
        let staging = temp.path().join("staging");
        fs::create_dir_all(&staging).unwrap();
        let root_dir = staging.join("alpha");
        fs::create_dir_all(&root_dir).unwrap();
        let skill_file = root_dir.join(SKILL_FILE_NAME);
        fs::write(
            &skill_file,
            "---\r\nname: alpha\r\ndescription: test\r\nmetadata:\r\n  name: nested\r\n---\r\nbody\r\n",
        )
        .unwrap();

        let mut prepared = PreparedSkillImport {
            _temp_dir: temp,
            scan_root: staging.clone(),
            source_key: "test://plugin".to_string(),
            discovered_skills: vec![DiscoveredSkillRoot {
                metadata: SkillMetadata {
                    name: "alpha".to_string(),
                    description: "test".to_string(),
                    path: skill_file.to_string_lossy().to_string(),
                    alias_path: None,
                    source: None,
                    origin: None,
                },
                root_dir,
            }],
        };
        let existing: HashSet<String> = ["alpha".to_string()].into();

        namescope_plugin_skills(&mut prepared, "p", &existing).unwrap();

        let raw = fs::read_to_string(&skill_file).unwrap();
        assert!(
            raw.contains("name: \"p:alpha\"\r\n"),
            "top-level name rewritten with CRLF kept: {raw:?}"
        );
        assert!(
            raw.contains("  name: nested"),
            "nested name must not be rewritten: {raw:?}"
        );
        assert!(raw.ends_with("body\r\n"), "trailing CRLF kept: {raw:?}");
        let rewritten = parse_skill_metadata(&skill_file).unwrap();
        assert_eq!(rewritten.name, "p:alpha");
    }

    fn staged_prepared(temp: tempfile::TempDir, skill_names: &[&str]) -> PreparedSkillImport {
        let scan_root = temp.path().join("staging");
        fs::create_dir_all(&scan_root).unwrap();
        let discovered_skills = skill_names
            .iter()
            .map(|name| staged_skill(&scan_root, name))
            .collect();
        PreparedSkillImport {
            _temp_dir: temp,
            scan_root,
            source_key: "test://plugin".to_string(),
            discovered_skills,
        }
    }

    // Gatekeeper G1.2: preview-level coverage through the real
    // build_import_preview with injected directories.

    #[tokio::test]
    async fn preview_reinstall_same_source_keeps_overwrite_conflict() {
        let mut prepared = staged_prepared(tempfile::tempdir().unwrap(), &["alpha"]);
        write_manifest(&prepared.scan_root, r#"{ "name": "p" }"#);
        let target = tempfile::tempdir().unwrap();
        let system = tempfile::tempdir().unwrap();
        // The same plugin's own skill is already installed in the target dir.
        write_valid_skill(&target.path().join("alpha"), "alpha");

        let preview = build_import_preview(
            &mut prepared,
            target.path(),
            "global",
            "user",
            system.path(),
            &["alpha".to_string()],
        )
        .await
        .unwrap();

        assert_eq!(
            preview.discovered_skills[0].name, "alpha",
            "own previous install must not trigger the plugin: rename"
        );
        assert_eq!(preview.conflicts.len(), 1);
        assert_eq!(preview.conflicts[0].name, "alpha");
    }

    #[tokio::test]
    async fn preview_bundled_collision_renames_without_conflict() {
        let mut prepared = staged_prepared(tempfile::tempdir().unwrap(), &["alpha"]);
        write_manifest(&prepared.scan_root, r#"{ "name": "p" }"#);
        let target = tempfile::tempdir().unwrap();
        let system = tempfile::tempdir().unwrap();
        write_valid_skill(&system.path().join("alpha"), "alpha");

        let preview = build_import_preview(
            &mut prepared,
            target.path(),
            "global",
            "user",
            system.path(),
            &[],
        )
        .await
        .unwrap();

        assert_eq!(preview.discovered_skills[0].name, "p:alpha");
        assert!(
            preview.conflicts.is_empty(),
            "rename resolves the bundled collision: {:?}",
            preview.conflicts
        );
    }

    #[tokio::test]
    async fn preview_reports_plugin_manifest_summary() {
        let mut prepared = staged_prepared(tempfile::tempdir().unwrap(), &["alpha"]);
        write_manifest(
            &prepared.scan_root,
            r#"{ "name": "p", "version": "1.2.3", "description": "A plugin" }"#,
        );
        let target = tempfile::tempdir().unwrap();
        let system = tempfile::tempdir().unwrap();

        let preview = build_import_preview(
            &mut prepared,
            target.path(),
            "global",
            "user",
            system.path(),
            &[],
        )
        .await
        .unwrap();

        let summary = preview.plugin_manifest.expect("manifest summary");
        assert_eq!(summary.name, "p");
        assert_eq!(summary.version.as_deref(), Some("1.2.3"));
        assert_eq!(summary.description.as_deref(), Some("A plugin"));
    }

    #[tokio::test]
    async fn preview_fails_on_manifest_path_outside_root() {
        let mut prepared = staged_prepared(tempfile::tempdir().unwrap(), &["alpha"]);
        write_manifest(
            &prepared.scan_root,
            r#"{ "name": "p", "skills": "../evil" }"#,
        );
        let target = tempfile::tempdir().unwrap();
        let system = tempfile::tempdir().unwrap();

        let err = build_import_preview(
            &mut prepared,
            target.path(),
            "global",
            "user",
            system.path(),
            &[],
        )
        .await
        .unwrap_err();
        assert!(err.contains("skills"), "error names the field: {err}");
    }
}
