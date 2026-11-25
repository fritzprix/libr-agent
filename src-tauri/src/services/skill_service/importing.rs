use super::cache::{invalidate_skill_scan_cache, scan_skills_internal_cached};
use super::contracts::{
    DiscoveredSkillRoot, HookPreviewEntry, HookPreviewExecutionStatus, McpServerPreviewEntry,
    PluginManifestSummary, PreparedSkillImport, SkillImportCandidate, SkillImportConflict,
    SkillImportKind, SkillImportPreview, SkillImportResult, SkillMetadata,
    GITHUB_DOWNLOAD_CONNECT_TIMEOUT_SECS, GITHUB_DOWNLOAD_TIMEOUT_SECS, MAX_GITHUB_ARCHIVE_BYTES,
    PLUGIN_SKILL_OWNER_FILE, SKILL_FILE_NAME,
};
use super::directories::{
    get_assistant_skills_directory, get_system_skills_directory, get_user_skills_directory,
    resolve_skills,
};
use super::github::parse_github_repo_url;
use super::plugin_manifest::{self, PluginManifest};
use super::plugin_mcp::collect_plugin_mcp_servers;
use super::scanning::{copy_dir_recursive, parse_skill_metadata, plugin_owner_marker_of_dir};
use crate::services::hooks::registry::{
    get_source, get_user_plugins_directory, save_source_in, PluginMetadata, PluginSourceManifest,
    PluginSourceRecord,
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
        // A local archive install collects no `userConfig` values; a plugin
        // that requires one fails with a clear message instead (#2926).
        BTreeMap::new(),
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
    plugin_options: Option<BTreeMap<String, String>>,
) -> Result<SkillImportResult, String> {
    let mut prepared = prepare_github_skill_import(repo_url).await?;
    let preview = build_user_import_preview(&mut prepared).await?;
    install_user_prepared_skills(
        prepared,
        preview,
        overwrite_existing,
        excluded_skill_names.unwrap_or_default(),
        approve_hooks.unwrap_or(false),
        plugin_options.unwrap_or_default(),
    )
    .await
}

/// A plugin manifest plus its resolved `userConfig` values (#2926).
pub(super) type ResolvedPluginOptions = (PluginManifest, BTreeMap<String, String>);

/// Check the `userConfig` values an install was given against the plugin
/// manifest **before** anything is written (#2926), so a missing required
/// option or a wrongly shaped value fails the install cleanly. Returns the
/// manifest and resolved values when the plugin declares options.
pub(super) fn prepare_plugin_user_config(
    scan_root: &Path,
    provided: &BTreeMap<String, String>,
) -> Result<Option<ResolvedPluginOptions>, String> {
    let Some(manifest) = plugin_manifest::load_plugin_manifest(scan_root)? else {
        return Ok(None);
    };
    if manifest.user_config.is_empty() {
        return Ok(None);
    }
    plugin_manifest::validate_user_config(&manifest)?;
    let values = plugin_manifest::resolve_user_config_values(&manifest, provided)?;
    Ok(Some((manifest, values)))
}

/// Persist resolved `userConfig` values for an installed plugin (#2926): the
/// non-sensitive ones in the store's option sidecar, the sensitive ones in
/// secure storage. The sidecar sits next to the store entry so a plugin update
/// (which replaces the installed directory) keeps its options; for a session
/// install that "store entry" lives in the session's out-of-workspace trust
/// directory (#2926 gatekeeper G1.1), and `session_id` scopes the secure
/// namespace (G1.2). `None` is the global install.
pub(super) async fn save_plugin_user_config(
    store_dir: &Path,
    source_id: &str,
    session_id: Option<&str>,
    manifest: &PluginManifest,
    values: &BTreeMap<String, String>,
) -> Result<(), String> {
    let saved =
        super::plugin_options::persist_options(source_id, session_id, manifest, values).await?;
    if let Err(error) = super::plugin_options::write_saved_options(store_dir, source_id, &saved) {
        // #2926 gatekeeper S1.2: the sidecar write failed after the secure
        // values were already stored — roll them back so a failed install
        // leaves nothing behind in secure storage.
        super::plugin_options::discard_persisted_options(
            source_id,
            session_id,
            &saved.sensitive_keys,
        )
        .await;
        return Err(error);
    }
    Ok(())
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
    // #2927 D2: a plugin-owned skill is read-only for the user. It goes only
    // with its plugin, so a lone delete is rejected with the owner named.
    if let Some(root) = find_skill_root_by_name(&user_dir, &skill_name)? {
        if let Some(owner) = plugin_owner_marker_of_dir(&root) {
            return Err(format!(
                "Skill '{skill_name}' is owned by plugin {owner}. Disable or uninstall the \
                 plugin to remove it."
            ));
        }
    }
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
    let global_skills = resolve_skills(system_dir, user_dir, None, None, None).await?;
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
    // #2927 D2: a copy into an assistant is the user's own skill; the
    // plugin-owner marker must not travel with it, or the assistant copy
    // would inherit the plugin's read-only rule. A missing marker (plain
    // skill) is fine; any other failure would leave an undeletable
    // assistant copy, so it is reported and the copy is rolled back
    // (#2954 G1.4).
    if let Err(error) = fs::remove_file(target_path.join(PLUGIN_SKILL_OWNER_FILE)) {
        if error.kind() != std::io::ErrorKind::NotFound {
            let _ = fs::remove_dir_all(&target_path);
            invalidate_skill_scan_cache();
            return Err(format!(
                "strip plugin-owner marker from assistant copy of '{skill_name}': {error}"
            ));
        }
    }

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
    let result = install_prepared_skills_to_directory(&prepared, assistant_skills_dir, true, None)?;
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
    // #2927 D2/D6: global plugin installs mark their skills with this source
    // id, so the preview must treat the plugin's own skills as replaceable,
    // not as user-data conflicts (#2954 G1.2). A skill bundle without the
    // marker never matches, so passing the id unconditionally is safe.
    let owner_source_id = plugin_source_id(&prepared.source_key);
    build_import_preview(
        prepared,
        &user_dir,
        "global",
        "user",
        &system_dir,
        &same_source,
        Some(&owner_source_id),
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
pub(super) async fn build_import_preview(
    prepared: &mut PreparedSkillImport,
    target_skills_dir: &Path,
    source_tag: &str,
    origin_tag: &str,
    system_skills_dir: &Path,
    same_source_skills: &[String],
    // #2927 D2/D6: source id of the plugin being installed/previewed. An
    // existing user skill whose owner marker equals it is the plugin's own
    // skill — an update replaces it, so it is not a conflict (#2954 G1.2).
    owner_source_id: Option<&str>,
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
    let conflicts = build_skill_import_conflicts(
        &discovered_candidates,
        &system_skills,
        &user_skills,
        owner_source_id,
    );

    let hooks_config = try_load_plugin_hooks(&prepared.scan_root, manifest.as_ref())?;
    let hooks = hooks_config
        .as_ref()
        .map(build_hook_preview_entries)
        .unwrap_or_default();
    // #2924 (D3, gatekeeper G1.3): preview everything the plugin can run —
    // command with its full argument line, env variable names, and HTTP
    // header names (values never) — so the one-time approval is informed.
    let declared_servers = collect_plugin_mcp_servers(&prepared.scan_root, manifest.as_ref())?;
    let mcp_servers = build_mcp_preview_entries(&declared_servers);
    let kind = if hooks.is_empty() && mcp_servers.is_empty() {
        SkillImportKind::SkillBundle
    } else {
        SkillImportKind::Plugin
    };
    let hooks_require_approval = !hooks.is_empty();
    let mcp_require_approval = !mcp_servers.is_empty();
    // #2926: the values the plugin asks for. The install dialog collects them;
    // a sensitive one is masked there and stored in secure storage.
    let user_config = manifest
        .as_ref()
        .map(|manifest| {
            manifest
                .user_config
                .iter()
                .map(|(key, spec)| super::contracts::UserConfigPreviewEntry {
                    key: key.clone(),
                    title: spec.display_title(key).to_string(),
                    description: spec.description.clone(),
                    value_type: spec
                        .value_type
                        .clone()
                        .unwrap_or_else(|| "string".to_string()),
                    sensitive: spec.sensitive,
                    required: spec.required,
                    default: spec.default.clone(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(SkillImportPreview {
        kind,
        discovered_skills: discovered_candidates,
        conflicts,
        hooks,
        hooks_require_approval,
        mcp_servers,
        mcp_require_approval,
        user_config,
        plugin_commands: super::plugin_commands::plugin_command_names(
            &prepared.scan_root,
            manifest.as_ref(),
        ),
        plugin_agents: super::plugin_agents::plugin_agent_previews(
            &prepared.scan_root,
            manifest.as_ref(),
        ),
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
    owner_source_id: Option<&str>,
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
                // #2927 D2/D6: a skill this same plugin owns (marker ==
                // owner_source_id) is not a conflict — an update replaces
                // its own skills (#2954 G1.2). User imports and other
                // plugins' skills still conflict. The exemption applies
                // only when both ids are known: a `None` owner id (session
                // previews) never exempts, and an unmarked existing skill
                // is always ordinary user data.
                .filter(|existing| {
                    !matches!(
                        (existing.plugin_owner.as_deref(), owner_source_id),
                        (Some(existing_owner), Some(pending_owner))
                            if existing_owner == pending_owner
                    )
                })
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
    /// Source id of the plugin whose skills are being installed (#2927 D2).
    /// When set, every installed skill directory gets the plugin-owner
    /// marker, and overwriting a skill owned by a **different** plugin is
    /// rejected. `None` for user imports, which can never own skills.
    pub(super) owner_source_id: Option<String>,
}

async fn install_user_prepared_skills(
    prepared: PreparedSkillImport,
    preview: SkillImportPreview,
    overwrite_existing: bool,
    excluded_skill_names: Vec<String>,
    approve_hooks: bool,
    plugin_options: BTreeMap<String, String>,
) -> Result<SkillImportResult, String> {
    // #2927 D2: only a plugin install owns its skills; a plain skill-bundle
    // import into user storage stays user-owned.
    let owner_source_id =
        (preview.kind == SkillImportKind::Plugin).then(|| plugin_source_id(&prepared.source_key));
    let target = ImportTarget {
        skills_dir: get_user_skills_directory()?,
        plugins_dir: get_user_plugins_directory()?,
        always_record_source: false,
        owner_source_id,
    };
    // #2926: check the values the user entered before anything is written, so a
    // missing required option does not leave a half-installed plugin behind.
    let user_config = prepare_plugin_user_config(&prepared.scan_root, &plugin_options)?;
    let store_dir = target.plugins_dir.clone();
    let source_key = prepared.source_key.clone();
    let result = install_prepared_import(
        prepared,
        preview,
        overwrite_existing,
        excluded_skill_names,
        approve_hooks,
        target,
    );
    // #2924 (D1/D2): a global install puts the plugin's approved servers into
    // the global MCP server store as plugin-owned rows. Session-scoped
    // installs never reach this function, so they never touch the global
    // store. A store failure must not report the file install itself as
    // failed, but it must be reported to the user (gatekeeper G1.2): the
    // servers they approved are not running, so the result carries warnings.
    match result {
        Ok(mut result) => {
            // #2926: options are saved before the MCP sync below, because a
            // plugin server may reference those values.
            if let Some((manifest, values)) = user_config {
                match save_plugin_user_config(
                    &store_dir,
                    &plugin_source_id(&source_key),
                    None,
                    &manifest,
                    &values,
                )
                .await
                {
                    Ok(()) => {}
                    Err(error) => {
                        log::warn!(
                            "Installed plugin {} but could not save its userConfig values: {}",
                            source_key,
                            error
                        );
                        result.warnings.push(format!(
                            "Plugin installed, but its configuration values could not be saved: {error}. Re-install the plugin and enter them again."
                        ));
                    }
                }
            }
            let mcp_warnings = match sync_installed_plugin_mcp_servers(&source_key).await {
                Ok(warnings) => warnings,
                Err(error) => {
                    log::warn!(
                        "Installed plugin {} but could not sync its MCP servers to the global store: {}",
                        source_key,
                        error
                    );
                    // The sync may have stopped after removing some rows
                    // (gatekeeper S2.1): say so, and that a re-install heals it.
                    vec![format!(
                        "Plugin installed, but its MCP servers could not be synced to the global store: {error}. Re-installing the plugin re-syncs them."
                    )]
                }
            };
            result.warnings.extend(mcp_warnings);
            // #2923 (D2): a global install also syncs the plugin's `agents/`
            // definitions into the assistant store as plugin-owned rows.
            // Same reporting rule as the MCP sync above: the file install
            // succeeded, so the failure is a warning, not an error.
            if let Err(error) = sync_installed_plugin_agents(&source_key).await {
                log::warn!(
                    "Installed plugin {} but could not sync its agents to the assistant store: {}",
                    source_key,
                    error
                );
                result.warnings.push(format!(
                    "Plugin installed, but its agents could not be synced to the assistant store: {error}. Re-installing the plugin re-syncs them."
                ));
            }
            Ok(result)
        }
        error => error,
    }
}

/// Re-sync one plugin source's rows in the global MCP server store after an
/// install/update (D2). Reads back the persisted source record, so it must
/// run after `install_plugin_source`.
///
/// Existing rows owned by this source are ALWAYS removed first — even when
/// the new snapshot is empty or unapproved — so an update that drops servers
/// or a re-install without approval cannot leave stale rows running
/// (gatekeeper G1.1). Returns per-server warnings for approved servers that
/// could not be installed (gatekeeper G1.2).
async fn sync_installed_plugin_mcp_servers(source_key: &str) -> Result<Vec<String>, String> {
    let source_id = plugin_source_id(source_key);
    let record = get_source(&source_id)?;
    sync_plugin_mcp_rows_from_record(&record).await
}

/// Re-sync one plugin source's plugin-agent rows in the assistant store
/// after a global install/update (#2923 D2). Reads back the persisted
/// source record, so it must run after `install_prepared_import`.
async fn sync_installed_plugin_agents(source_key: &str) -> Result<(), String> {
    let source_id = plugin_source_id(source_key);
    let record = get_source(&source_id)?;
    super::plugin_agents::sync_global_plugin_agents(&record).await
}

/// The sync body once the persisted source record is read (see
/// [`sync_installed_plugin_mcp_servers`]). Extracted so the remove-before-
/// every-return rule (gatekeeper G1.1) is testable through the record
/// itself, without a global plugin directory (gatekeeper G2.1).
pub(super) async fn sync_plugin_mcp_rows_from_record(
    record: &PluginSourceRecord,
) -> Result<Vec<String>, String> {
    let source_id = &record.manifest.source_id;
    super::plugin_mcp::remove_global_plugin_mcp_servers(source_id).await?;
    if record.manifest.mcp_servers.is_empty() {
        return Ok(Vec::new());
    }
    if !record.manifest.mcp_approved {
        log::info!(
            "Plugin {} declares MCP servers but they were not approved at install; not starting them",
            source_id
        );
        return Ok(Vec::new());
    }
    // #2926: a plugin-declared server may reference its data directory and its
    // `userConfig` values, exactly like its hooks may.
    let data_dir = crate::services::hooks::registry::get_user_plugin_data_dir(source_id).ok();
    let (options, warning) = match crate::services::hooks::registry::get_user_plugins_directory() {
        Ok(store_dir) => {
            super::plugin_options::load_option_values(&store_dir, source_id, None).await
        }
        Err(_) => (BTreeMap::new(), None),
    };
    if let Some(warning) = warning {
        log::warn!("{warning}");
    }
    super::plugin_mcp::create_plugin_mcp_server_rows_in(
        source_id,
        &super::plugin_mcp::PluginExpansion {
            root: &record.root_dir,
            data_dir: data_dir.as_deref(),
            options: &options,
        },
        &record.manifest.mcp_servers,
    )
    .await
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
    let mut result = install_prepared_skills_to_directory(
        &prepared,
        target.skills_dir,
        overwrite_existing,
        target.owner_source_id.as_deref(),
    )?;

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
///
/// `owner` (#2927 D2) is the source id of a plugin whose skills are being
/// installed. Every copied skill directory then carries the plugin-owner
/// marker, overwriting a skill owned by a **different** plugin is rejected,
/// and re-installing the same plugin (an update) may replace its own skills.
/// `None` means a user import: plugin-owned skills are never overwritten.
fn install_prepared_skills_to_directory(
    prepared: &PreparedSkillImport,
    target_dir: PathBuf,
    overwrite_existing: bool,
    owner: Option<&str>,
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
            check_overwrite_allowed(&existing_root, &skill.metadata.name, owner)?;
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
            check_overwrite_allowed(&target_path, &skill.metadata.name, owner)?;
            fs::remove_dir_all(&target_path).map_err(|error| error.to_string())?;
            cache_invalidation_guard.mark_mutated();
        }

        copy_dir_recursive(&skill.root_dir, &target_path)?;
        if let Some(owner) = owner {
            write_skill_owner_marker(&target_path, owner)?;
        }
        cache_invalidation_guard.mark_mutated();
        imported_names.push(skill.metadata.name.clone());
    }

    Ok(SkillImportResult {
        imported_names,
        overwritten_names,
        skipped_names: Vec::new(),
        warnings: Vec::new(),
    })
}

/// Write the plugin-owner marker (#2927 D2) into a freshly installed skill
/// directory. A failure is fatal for the install: without the marker the
/// skill is unprotected, and uninstall would fall back to name matching.
pub(super) fn write_skill_owner_marker(skill_dir: &Path, source_id: &str) -> Result<(), String> {
    fs::write(skill_dir.join(PLUGIN_SKILL_OWNER_FILE), source_id).map_err(|error| {
        format!(
            "write plugin-owner marker in {}: {error}",
            skill_dir.display()
        )
    })
}

/// Reject overwriting a skill directory another plugin owns (#2927 D2). The
/// owning plugin itself may re-install over its own skills (that is exactly
/// what an update is), and a user-owned directory is always replaceable.
fn check_overwrite_allowed(
    existing_root: &Path,
    skill_name: &str,
    owner: Option<&str>,
) -> Result<(), String> {
    if let Some(existing_owner) = plugin_owner_marker_of_dir(existing_root) {
        if Some(existing_owner.as_str()) != owner {
            return Err(format!(
                "Skill '{skill_name}' is owned by plugin {existing_owner}. Update or uninstall \
                 that plugin instead of overwriting this skill."
            ));
        }
    }
    Ok(())
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

/// Hook config files for a plugin root, in load order: the auto-loaded
/// `hooks/hooks.json` / `hooks-cursor.json` plus the files the manifest's
/// `hooks` field declares (spec plugins-reference add-semantics). A manifest
/// entry duplicating the default or missing on disk is an error (S1.1).
/// Install persists these paths (relative to the root) so dispatch and the
/// Skills overview load exactly what the user approved (gatekeeper G2.2).
fn collect_plugin_hook_files(
    scan_root: &Path,
    manifest: Option<&PluginManifest>,
) -> Result<Vec<PathBuf>, String> {
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

    Ok(files)
}

fn try_load_plugin_hooks(
    scan_root: &Path,
    manifest: Option<&PluginManifest>,
) -> Result<Option<HooksConfig>, String> {
    let files = collect_plugin_hook_files(scan_root, manifest)?;
    let inline = plugin_manifest::hook_inline_values(manifest);
    if files.is_empty() && inline.is_empty() {
        return Ok(None);
    }
    let mut config = if files.is_empty() {
        HooksConfig::empty()
    } else {
        crate::services::hooks::schema::load_hooks_from_config_files(&files)?
    };
    // Inline manifest hooks (#3005) are add-semantics like the file entries,
    // and are validated here so a bad object fails the install preview.
    crate::services::hooks::schema::merge_hooks_inline(
        &mut config,
        &inline,
        "plugin manifest hooks",
    )?;
    Ok(Some(config))
}

/// #2924 (D3, gatekeeper G1.3): preview everything the plugin can run —
/// command with its full argument line, env variable names, and HTTP header
/// names (values never) — so the one-time approval is informed.
fn build_mcp_preview_entries(
    servers: &[crate::services::hooks::registry::PluginMcpServerSnapshot],
) -> Vec<McpServerPreviewEntry> {
    servers
        .iter()
        .map(|server| {
            let entry = &server.entry;
            let declared_names = |field: &str| -> Option<Vec<String>> {
                let mut names: Vec<String> = entry
                    .get(field)
                    .and_then(|v| v.as_object())
                    .map(|map| map.keys().cloned().collect())
                    .unwrap_or_default();
                (!names.is_empty()).then(|| {
                    names.sort();
                    names
                })
            };
            let args = entry
                .get("args")
                .and_then(|v| v.as_array())
                .map(|items| {
                    // Quote arguments containing whitespace so the joined
                    // line stays unambiguous (gatekeeper S2.2).
                    items
                        .iter()
                        .filter_map(|item| item.as_str())
                        .map(|arg| {
                            if arg.contains(char::is_whitespace) {
                                format!("\"{arg}\"")
                            } else {
                                arg.to_string()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .filter(|joined| !joined.is_empty());
            McpServerPreviewEntry {
                name: server.name.clone(),
                transport: if entry.get("url").and_then(|v| v.as_str()).is_some() {
                    "http".to_string()
                } else {
                    "stdio".to_string()
                },
                command: entry
                    .get("command")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string()),
                args,
                env_names: declared_names("env"),
                url: entry
                    .get("url")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string()),
                header_names: declared_names("headers"),
            }
        })
        .collect()
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
                // HTTP handlers run too; show their POST target where a
                // command hook shows its command (#3009). The header names
                // with their raw `$VAR` templates and the allowed env-var
                // names must also be visible — the allowlist is written by
                // the plugin, so the user approves exactly which variables
                // may leave the machine (#3041 G1.1).
                let (command, http_headers, http_allowed_env_vars) = match &handler.http {
                    Some(http) => (
                        http.url.clone(),
                        (!http.headers.is_empty()).then(|| http.headers.clone()),
                        (!http.allowed_env_vars.is_empty()).then(|| http.allowed_env_vars.clone()),
                    ),
                    None => (handler.command.clone(), None, None),
                };
                entries.push(HookPreviewEntry {
                    event: event.as_str().to_string(),
                    matcher: group.matcher.clone(),
                    command,
                    powershell: handler.powershell.clone(),
                    script_body: None,
                    execution_status,
                    http_headers,
                    http_allowed_env_vars,
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
                http_headers: None,
                http_allowed_env_vars: None,
            });
        }
    }
    // Handler types LibrAgent does not run yet are marked unsupported in the
    // preview instead of failing the install (#3009).
    for (event_name, unsupported) in &config.unsupported_handlers {
        for item in unsupported {
            entries.push(HookPreviewEntry {
                event: event_name.clone(),
                matcher: item.matcher.clone(),
                command: item.command.clone(),
                powershell: None,
                script_body: None,
                execution_status: HookPreviewExecutionStatus::Stubbed,
                http_headers: None,
                http_allowed_env_vars: None,
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
    let hook_files = collect_plugin_hook_files(scan_root, manifest.as_ref())?;
    let config = if hook_files.is_empty() {
        None
    } else {
        Some(crate::services::hooks::schema::load_hooks_from_config_files(&hook_files)?)
    };
    // Relative, slash-separated for the source store: dispatch loads exactly
    // these files from the installed root (gatekeeper G2.2).
    let hook_file_paths: Vec<String> = hook_files
        .iter()
        .filter_map(|file| file.strip_prefix(scan_root).ok())
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        .collect();

    let source_id = plugin_source_id(source_key);
    // #2927: an update keeps the state the user chose; a fresh install starts
    // in the manifest's `defaultEnabled` state (enabled when it says nothing).
    let previous_enabled = crate::services::hooks::registry::get_source_in(plugins_dir, &source_id)
        .ok()
        .map(|record| record.manifest.enabled);
    let enabled = previous_enabled.unwrap_or_else(|| {
        manifest
            .as_ref()
            .map(PluginManifest::starts_enabled)
            .unwrap_or(true)
    });
    let installed_commit = commit_from_archive_root(scan_root);
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

    // #2924 (D3): one install-time approval covers hooks and MCP servers
    // alike — the dialog shows both before the user agrees.
    let mcp_servers = collect_plugin_mcp_servers(scan_root, manifest.as_ref())?;
    let mcp_approved = approve_hooks && !mcp_servers.is_empty();

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
            hook_files: hook_file_paths,
            inline_hooks: plugin_manifest::hook_inline_values(manifest.as_ref()),
            mcp_servers,
            mcp_approved,
            enabled,
            installed_commit,
        },
    )?;

    Ok(())
}

/// The commit GitHub put in a zipball root name (`<owner>-<repo>-<sha>`),
/// when the archive carries one (#2927). A branch archive is named
/// `<repo>-<branch>` and yields `None`. The plugin root is often the zipball
/// root itself, so its own name is checked before its ancestors.
fn commit_from_archive_root(scan_root: &Path) -> Option<String> {
    for ancestor in scan_root.ancestors().take(3) {
        let Some(name) = ancestor.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some((_, sha)) = name.rsplit_once('-') else {
            continue;
        };
        if sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(sha.to_ascii_lowercase());
        }
    }
    None
}

pub(super) fn remove_skill_names_from_user_skills(
    user_dir: &Path,
    skill_names: &[String],
    plugin_id: &str,
) -> Result<(), String> {
    let mut cache_invalidation_guard = SkillCacheInvalidationGuard::new();
    for name in skill_names {
        // #2927 D2: a skill whose directory carries a marker for a *different*
        // plugin must survive this uninstall. A markerless directory is a
        // pre-#2927 plugin skill (sanctioned imports can no longer overwrite
        // a plugin-owned skill), so it still goes by name — which is exactly
        // what those installs could only ever have had.
        if let Some(root) = find_skill_root_by_name(user_dir, name)? {
            if let Some(owner) = plugin_owner_marker_of_dir(&root) {
                if owner != plugin_id {
                    continue;
                }
            }
        }
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

    // #2927 D2: a plugin install marks every skill directory it writes, the
    // owning plugin (an update) may replace its own, and neither another
    // plugin nor a user import may overwrite a marked skill.
    #[test]
    fn plugin_install_marks_skills_and_only_the_owner_may_replace() {
        let temp = tempfile::tempdir().unwrap();
        let staging = temp.path().join("staging");
        let staged = staging.join("alpha");
        write_valid_skill(&staged, "alpha");
        let prepared = PreparedSkillImport {
            _temp_dir: temp,
            scan_root: staging.clone(),
            source_key: "https://github.com/acme/plugin".to_string(),
            discovered_skills: vec![DiscoveredSkillRoot {
                metadata: parse_skill_metadata(&staged.join(SKILL_FILE_NAME)).unwrap(),
                root_dir: staged,
            }],
        };
        let target = tempfile::tempdir().unwrap();

        install_prepared_skills_to_directory(
            &prepared,
            target.path().to_path_buf(),
            false,
            Some("plugin-acme"),
        )
        .unwrap();
        // Skills install under their storage name (`<slug>-<digest>`), not
        // the bare skill name.
        let installed = target
            .path()
            .join(skill_storage_directory_name("alpha").unwrap());
        assert_eq!(
            plugin_owner_marker_of_dir(&installed).as_deref(),
            Some("plugin-acme"),
            "every skill the plugin installs carries its owner marker"
        );

        install_prepared_skills_to_directory(
            &prepared,
            target.path().to_path_buf(),
            true,
            Some("plugin-acme"),
        )
        .unwrap();
        assert_eq!(
            plugin_owner_marker_of_dir(&installed).as_deref(),
            Some("plugin-acme"),
            "an update replaces the plugin's own skill and re-marks it"
        );

        for intruder in [Some("plugin-other"), None] {
            let error = install_prepared_skills_to_directory(
                &prepared,
                target.path().to_path_buf(),
                true,
                intruder,
            )
            .unwrap_err();
            assert!(error.contains("owned by plugin plugin-acme"), "{error}");
        }
        assert_eq!(
            plugin_owner_marker_of_dir(&installed).as_deref(),
            Some("plugin-acme"),
            "rejected installs must leave the owned skill intact"
        );
    }

    // #2927 D2: uninstall removes the plugin's own skills (and pre-#2927
    // markerless ones) but never a skill marked for another plugin.
    #[test]
    fn uninstall_removes_own_skills_but_not_other_owners() {
        let user = tempfile::tempdir().unwrap();

        let mine = user.path().join("mine");
        write_valid_skill(&mine, "mine");
        write_skill_owner_marker(&mine, "plugin-mine").unwrap();

        let theirs = user.path().join("theirs");
        write_valid_skill(&theirs, "theirs");
        write_skill_owner_marker(&theirs, "plugin-theirs").unwrap();

        let legacy = user.path().join("legacy");
        write_valid_skill(&legacy, "legacy");

        remove_skill_names_from_user_skills(
            user.path(),
            &[
                "mine".to_string(),
                "theirs".to_string(),
                "legacy".to_string(),
            ],
            "plugin-mine",
        )
        .unwrap();

        assert!(!mine.exists(), "the plugin's own skills go with it");
        assert!(
            theirs.exists(),
            "a skill another plugin owns must survive this uninstall"
        );
        assert!(
            !legacy.exists(),
            "a markerless skill is a pre-#2927 install and goes by name"
        );
    }

    // #2927: GitHub serves an API zipball as `<owner>-<repo>-<sha>`, so the
    // installed commit is readable from the archive root without a git call.
    #[test]
    fn installed_commit_reads_zipball_root_name() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let dir = tempfile::tempdir().unwrap();
        let zipball = dir.path().join(format!("acme-myrepo-{sha}"));
        fs::create_dir_all(zipball.join("plugin")).unwrap();

        assert_eq!(
            commit_from_archive_root(&zipball).as_deref(),
            Some(sha),
            "the archive root is usually the plugin root itself"
        );
        assert_eq!(
            commit_from_archive_root(&zipball.join("plugin")).as_deref(),
            Some(sha),
            "a nested plugin root still sees the archive root"
        );
        // Branch archives carry no commit, and neither does a plain checkout.
        let branch = dir.path().join("myrepo-main");
        fs::create_dir_all(&branch).unwrap();
        assert_eq!(commit_from_archive_root(&branch), None);
        assert_eq!(commit_from_archive_root(dir.path()), None);
    }

    #[test]
    fn skill_bundle_when_no_hooks_present() {
        let dir = tempfile::tempdir().unwrap();
        write_valid_skill(dir.path(), "alpha");
        let hooks = try_load_plugin_hooks(dir.path(), None).unwrap();
        assert!(hooks.is_none());
    }

    // #2924 (D3): install persists the declared MCP servers, and the
    // install-time approval flag covers them alongside hooks.
    #[test]
    fn install_persists_mcp_snapshot_and_approval() {
        let dir = tempfile::tempdir().unwrap();
        write_valid_skill(dir.path(), "alpha");
        fs::write(
            dir.path().join(".mcp.json"),
            r#"{"mcpServers":{"fetch":{"command":"uvx","args":["srv"]}}}"#,
        )
        .unwrap();

        let plugins = tempfile::tempdir().unwrap();
        let source_key = "https://example.com/mcp-plugin";
        install_plugin_source(
            dir.path(),
            source_key,
            &["alpha".to_string()],
            true,
            plugins.path(),
        )
        .unwrap();
        let record = crate::services::hooks::registry::get_source_in(
            plugins.path(),
            &plugin_source_id(source_key),
        )
        .unwrap();
        assert_eq!(record.manifest.mcp_servers.len(), 1);
        assert_eq!(record.manifest.mcp_servers[0].name, "fetch");
        assert_eq!(record.manifest.mcp_servers[0].entry["command"], "uvx");
        assert!(record.manifest.mcp_approved);

        // Without approval the snapshot is still recorded (so the install is
        // inspectable) but the servers are never started.
        let plugins2 = tempfile::tempdir().unwrap();
        install_plugin_source(
            dir.path(),
            source_key,
            &["alpha".to_string()],
            false,
            plugins2.path(),
        )
        .unwrap();
        let record = crate::services::hooks::registry::get_source_in(
            plugins2.path(),
            &plugin_source_id(source_key),
        )
        .unwrap();
        assert_eq!(record.manifest.mcp_servers.len(), 1);
        assert!(!record.manifest.mcp_approved);
    }

    // #2924 (gatekeeper G1.3): the install preview shows the full command
    // line and declared env/header NAMES — never their values.
    #[test]
    fn mcp_preview_shows_args_and_names_not_values() {
        use crate::services::hooks::registry::PluginMcpServerSnapshot;
        let servers = vec![
            PluginMcpServerSnapshot {
                name: "local".to_string(),
                entry: serde_json::json!({
                    "command": "uvx",
                    "args": ["mcp-fetch", "--max", "10", "--path", "/tmp/my dir"],
                    "env": { "Z_TOKEN": "super-secret", "A_KEY": "another-secret" }
                }),
            },
            PluginMcpServerSnapshot {
                name: "remote".to_string(),
                entry: serde_json::json!({
                    "url": "https://x.example/mcp",
                    "headers": { "Authorization": "Bearer tok" }
                }),
            },
        ];
        let entries = build_mcp_preview_entries(&servers);
        assert_eq!(entries.len(), 2);

        let local = &entries[0];
        assert_eq!(local.transport, "stdio");
        assert_eq!(local.command.as_deref(), Some("uvx"));
        assert_eq!(
            local.args.as_deref(),
            Some("mcp-fetch --max 10 --path \"/tmp/my dir\""),
            "arguments with whitespace are quoted (S2.2)"
        );
        assert_eq!(
            local.env_names.as_deref(),
            Some(&["A_KEY".to_string(), "Z_TOKEN".to_string()][..]),
            "names only, sorted"
        );
        assert!(local.url.is_none());

        let remote = &entries[1];
        assert_eq!(remote.transport, "http");
        assert_eq!(remote.url.as_deref(), Some("https://x.example/mcp"));
        assert_eq!(
            remote.header_names.as_deref(),
            Some(&["Authorization".to_string()][..])
        );
        assert!(remote.args.is_none());

        // The preview struct itself never carries the declared values.
        let serialized = serde_json::to_string(&entries).unwrap();
        assert!(!serialized.contains("super-secret"), "{serialized}");
        assert!(!serialized.contains("another-secret"), "{serialized}");
        assert!(!serialized.contains("Bearer tok"), "{serialized}");
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

    /// #3009: `http` handlers install and preview as executed (their POST
    /// target is shown); other handler types preview as not supported
    /// instead of failing the load.
    #[test]
    fn plugin_preview_shows_http_executed_and_other_types_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        write_valid_skill(dir.path(), "alpha");
        let hooks_dir = dir.path().join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        fs::write(
            hooks_dir.join("hooks.json"),
            r#"{"hooks":{"PreToolUse":[{"matcher":"workspace__runShell","hooks":[
                {"type":"http","url":"http://127.0.0.1:9/hook","headers":{"Authorization":"Bearer $API_KEY"},"allowedEnvVars":["API_KEY"]},
                {"type":"prompt","prompt":"check"}
            ]}]}}"#,
        )
        .unwrap();
        let cfg = try_load_plugin_hooks(dir.path(), None)
            .unwrap()
            .expect("hooks");
        let entries = build_hook_preview_entries(&cfg);
        assert_eq!(entries.len(), 2);
        let http = entries
            .iter()
            .find(|e| e.command.contains("127.0.0.1:9"))
            .expect("http entry shows its url");
        assert_eq!(http.execution_status, HookPreviewExecutionStatus::Executed);
        // #3041 G1.1: the preview must show the declared header names with
        // their raw templates and the env-var names the plugin may read —
        // the allowlist is written by the plugin itself, so the user has to
        // see it before approving.
        assert_eq!(
            http.http_headers
                .as_ref()
                .and_then(|h| h.get("Authorization"))
                .map(String::as_str),
            Some("Bearer $API_KEY"),
            "header name with its raw $VAR template must be visible"
        );
        assert_eq!(
            http.http_allowed_env_vars.as_deref(),
            Some(&["API_KEY".to_string()][..]),
            "allowed env-var names must be visible"
        );
        let prompt = entries
            .iter()
            .find(|e| e.command.contains("prompt"))
            .expect("prompt entry is recorded");
        assert_eq!(prompt.execution_status, HookPreviewExecutionStatus::Stubbed);
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
                plugin_owner: None,
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
                    plugin_owner: None,
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
            None,
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
            None,
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
            None,
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
            None,
        )
        .await
        .unwrap_err();
        assert!(err.contains("skills"), "error names the field: {err}");
    }

    /// Gatekeeper G2.2: a real install of a plugin whose hooks are declared
    /// only in `.claude-plugin/plugin.json` must persist those files in the
    /// source manifest, so the record-aware loader (dispatch and overview)
    /// sees exactly the config the user approved.
    #[test]
    fn install_persists_manifest_only_hook_files() {
        use crate::services::hooks::registry::{get_source_in, hook_config_for_record};
        use crate::services::hooks::schema::HookEvent;

        let temp = tempfile::tempdir().unwrap();
        let scan_root = temp.path().join("plugin");
        let plugins_dir = temp.path().join("store");
        fs::create_dir_all(scan_root.join("custom-hooks")).unwrap();
        fs::write(
            scan_root.join("custom-hooks").join("extra.json"),
            r#"{"hooks":{"SessionStart":[{"command":"echo manifest only"}]}}"#,
        )
        .unwrap();
        write_manifest(
            &scan_root,
            r#"{ "name": "p", "hooks": "./custom-hooks/extra.json" }"#,
        );

        install_plugin_source(
            &scan_root,
            "test://manifest-only-hooks",
            &[],
            true,
            &plugins_dir,
        )
        .unwrap();

        let record = get_source_in(
            &plugins_dir,
            &plugin_source_id("test://manifest-only-hooks"),
        )
        .unwrap();
        assert_eq!(
            record.manifest.hook_files,
            vec!["custom-hooks/extra.json".to_string()],
            "install persists the manifest hook file relative to the root"
        );
        assert!(record.manifest.hooks_approved);
        assert!(
            !record.manifest.approval_hashes.is_empty(),
            "manifest-only handlers get approval hashes"
        );

        let cfg = hook_config_for_record(&record).unwrap();
        assert!(cfg.events.contains_key(&HookEvent::SessionStart));
        assert_eq!(
            super::super::plugin_sources::hook_events_for_record(&record),
            vec!["SessionStart".to_string()],
            "overview lists manifest-declared events"
        );
    }

    // #2927 gatekeeper G1.2: a fresh install starts in the manifest's
    // `defaultEnabled` state, and an update keeps the state the user chose
    // instead of falling back to the manifest default.
    #[test]
    fn install_honours_default_enabled_and_keeps_the_user_choice() {
        use crate::services::hooks::registry::{
            get_source_in, list_approved_sources_in, set_source_enabled_in,
        };

        let temp = tempfile::tempdir().unwrap();
        let scan_root = temp.path().join("plugin");
        let plugins_dir = temp.path().join("store");
        fs::create_dir_all(&scan_root).unwrap();
        write_valid_skill(&scan_root, "alpha");
        write_manifest(
            &scan_root,
            r#"{ "name": "opt-in", "defaultEnabled": false }"#,
        );
        let source_id = plugin_source_id("test://opt-in");
        let skills = ["alpha".to_string()];

        // Fresh install: the manifest asks to start disabled, and the loaders
        // skip it at once.
        install_plugin_source(&scan_root, "test://opt-in", &skills, false, &plugins_dir).unwrap();
        assert!(
            !get_source_in(&plugins_dir, &source_id)
                .unwrap()
                .manifest
                .enabled,
            "defaultEnabled: false must install disabled"
        );
        assert!(list_approved_sources_in(&plugins_dir).is_empty());

        // The user turns it on; a later update of the same source keeps that.
        set_source_enabled_in(&plugins_dir, &source_id, true).unwrap();
        install_plugin_source(&scan_root, "test://opt-in", &skills, false, &plugins_dir).unwrap();
        assert!(
            get_source_in(&plugins_dir, &source_id)
                .unwrap()
                .manifest
                .enabled,
            "an update must keep the state the user chose"
        );

        // The same holds for a plugin the user turned off.
        set_source_enabled_in(&plugins_dir, &source_id, false).unwrap();
        install_plugin_source(&scan_root, "test://opt-in", &skills, false, &plugins_dir).unwrap();
        assert!(
            !get_source_in(&plugins_dir, &source_id)
                .unwrap()
                .manifest
                .enabled,
            "an update must not re-enable a plugin the user disabled"
        );
    }

    // #2927: `defaultEnabled: true` and an absent field both install enabled,
    // so every plugin written before this spec keeps working.
    #[test]
    fn absent_default_enabled_installs_enabled() {
        use crate::services::hooks::registry::get_source_in;

        let temp = tempfile::tempdir().unwrap();
        let scan_root = temp.path().join("plugin");
        let plugins_dir = temp.path().join("store");
        fs::create_dir_all(&scan_root).unwrap();
        write_valid_skill(&scan_root, "alpha");
        write_manifest(&scan_root, r#"{ "name": "plain" }"#);

        install_plugin_source(&scan_root, "test://plain", &[], false, &plugins_dir).unwrap();
        assert!(
            get_source_in(&plugins_dir, &plugin_source_id("test://plain"))
                .unwrap()
                .manifest
                .enabled
        );
    }
}
