use super::super::cache::invalidate_skill_scan_cache;
use super::super::contracts::{
    PreparedSkillImport, SkillImportKind, SkillImportPreview, SkillImportResult,
    PLUGIN_SKILL_OWNER_FILE,
};
use super::super::directories::get_user_skills_directory;
use super::super::plugin_manifest::{self, PluginManifest};
use super::super::plugin_options;
use super::super::scanning::{
    copy_dir_recursive, discover_existing_skill_roots, find_skill_root_by_name,
    plugin_owner_marker_of_dir, remove_skill_name_from_directory, skill_storage_directory_name,
};
use super::archive::{prepare_github_skill_import, prepare_local_skill_import};
use super::plugin_install::{
    install_plugin_source, plugin_source_id, sync_installed_plugin_agents,
    sync_installed_plugin_mcp_servers,
};
use super::preview::{blocked_system_conflict_names, build_user_import_preview};
use crate::services::hooks::registry::get_user_plugins_directory;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

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
pub(in crate::services::skill_service) type ResolvedPluginOptions =
    (PluginManifest, BTreeMap<String, String>);

/// Check the `userConfig` values an install was given against the plugin
/// manifest **before** anything is written (#2926), so a missing required
/// option or a wrongly shaped value fails the install cleanly. Returns the
/// manifest and resolved values when the plugin declares options.
pub(in crate::services::skill_service) fn prepare_plugin_user_config(
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
pub(in crate::services::skill_service) async fn save_plugin_user_config(
    store_dir: &Path,
    source_id: &str,
    session_id: Option<&str>,
    manifest: &PluginManifest,
    values: &BTreeMap<String, String>,
) -> Result<(), String> {
    let saved = plugin_options::persist_options(source_id, session_id, manifest, values).await?;
    if let Err(error) = plugin_options::write_saved_options(store_dir, source_id, &saved) {
        // #2926 gatekeeper S1.2: the sidecar write failed after the secure
        // values were already stored — roll them back so a failed install
        // leaves nothing behind in secure storage.
        plugin_options::discard_persisted_options(source_id, session_id, &saved.sensitive_keys)
            .await;
        return Err(error);
    }
    Ok(())
}

pub(super) fn invalidate_skill_cache_after_success<T>(
    result: Result<T, String>,
) -> Result<T, String> {
    if result.is_ok() {
        invalidate_skill_scan_cache();
    }
    result
}

pub(super) struct SkillCacheInvalidationGuard {
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

/// Where an import lands: skills directory + plugin-source store.
pub(in crate::services::skill_service) struct ImportTarget {
    pub(in crate::services::skill_service) skills_dir: PathBuf,
    pub(in crate::services::skill_service) plugins_dir: PathBuf,
    /// Record a plugin-source manifest even for hook-less skill bundles, so
    /// the install is listable and removable (session-scoped installs).
    pub(in crate::services::skill_service) always_record_source: bool,
    /// Source id of the plugin whose skills are being installed (#2927 D2).
    /// When set, every installed skill directory gets the plugin-owner
    /// marker, and overwriting a skill owned by a **different** plugin is
    /// rejected. `None` for user imports, which can never own skills.
    pub(in crate::services::skill_service) owner_source_id: Option<String>,
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

pub(in crate::services::skill_service) fn install_prepared_import(
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
pub(super) fn install_prepared_skills_to_directory(
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
pub(in crate::services::skill_service) fn write_skill_owner_marker(
    skill_dir: &Path,
    source_id: &str,
) -> Result<(), String> {
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

pub(in crate::services::skill_service) fn remove_skill_names_from_user_skills(
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
