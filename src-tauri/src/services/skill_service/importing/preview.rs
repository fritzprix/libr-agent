use super::super::cache::scan_skills_internal_cached;
use super::super::contracts::{
    self, PluginManifestSummary, PreparedSkillImport, SkillImportCandidate, SkillImportConflict,
    SkillImportKind, SkillImportPreview, SkillMetadata,
};
use super::super::directories::{get_system_skills_directory, get_user_skills_directory};
use super::super::plugin_manifest;
use super::super::plugin_mcp::collect_plugin_mcp_servers;
use super::plugin_install::{
    build_hook_preview_entries, build_mcp_preview_entries, namescope_plugin_skills,
    plugin_source_id, try_load_plugin_hooks,
};
use crate::state::wait_for_managed_skills_sync;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub(super) async fn build_user_import_preview(
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
pub(in crate::services::skill_service) async fn build_import_preview(
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
                .map(|(key, spec)| contracts::UserConfigPreviewEntry {
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
        plugin_commands: super::super::plugin_commands::plugin_command_names(
            &prepared.scan_root,
            manifest.as_ref(),
        ),
        plugin_agents: super::super::plugin_agents::plugin_agent_previews(
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

pub(super) fn blocked_system_conflict_names(conflicts: &[SkillImportConflict]) -> Vec<String> {
    conflicts
        .iter()
        .filter(|conflict| conflict.existing_origin == "system")
        .map(|conflict| conflict.name.clone())
        .collect()
}
