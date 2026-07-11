use super::skills_manifest::{
    build_bundled_skills_manifest, collect_skill_directory_names, copy_dir_recursive,
    load_persisted_bundled_skills_manifest, replace_skill_directory_atomically,
    write_manifest_atomically, BUNDLED_SKILL_MARKER,
};
use crate::services::skill_service::{
    LEGACY_SYSTEM_SKILLS_DIR_NAME, MANAGED_SYSTEM_SKILLS_MANIFEST_FILE_NAME, USER_SKILLS_DIR_NAME,
};
use log::info;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, PartialEq, Eq)]
struct LegacySkillMigrationSummary {
    removed_legacy_snapshot_entries: usize,
    migrated_user_skills: usize,
    skipped_existing_user_skills: usize,
    removed_legacy_root_dir: bool,
}

impl LegacySkillMigrationSummary {
    fn is_noop(&self) -> bool {
        self.removed_legacy_snapshot_entries == 0
            && self.migrated_user_skills == 0
            && self.skipped_existing_user_skills == 0
            && !self.removed_legacy_root_dir
    }
}

/// Migration decision for a legacy skill directory from AppData/skills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacySkillMigrationAction {
    DeleteLegacyCopy,
    MigrateToUser,
}

/// Classifies a legacy AppData/skills entry for managed storage migration.
///
/// Any directory marked with `.bundled_skill` is treated as disposable snapshot
/// data from the old global bundled mirror and is never migrated into
/// `user_skills`. Only unmarked directories are considered user-managed content.
pub fn classify_legacy_skill_for_managed_storage(
    legacy_skill_dir: &Path,
) -> Result<LegacySkillMigrationAction, String> {
    if !legacy_skill_dir.is_dir() {
        return Ok(LegacySkillMigrationAction::MigrateToUser);
    }

    if legacy_skill_dir.join(BUNDLED_SKILL_MARKER).exists() {
        Ok(LegacySkillMigrationAction::DeleteLegacyCopy)
    } else {
        Ok(LegacySkillMigrationAction::MigrateToUser)
    }
}

/// Synchronize the managed system skills snapshot with the packaged bundled skills.
///
/// This keeps a runtime-owned app-data mirror instead of treating packaged resources
/// as the live source of truth, while avoiding full delete-and-recopy work when only
/// a subset of bundled skills changed.
pub fn sync_managed_system_skills_snapshot(
    bundled_skills_dir: &Path,
    system_skills_dir: &Path,
) -> Result<(), String> {
    if !bundled_skills_dir.exists() {
        return Ok(());
    }

    std::fs::create_dir_all(system_skills_dir).map_err(|e| e.to_string())?;

    let manifest_path = system_skills_dir.join(MANAGED_SYSTEM_SKILLS_MANIFEST_FILE_NAME);
    let source_manifest = build_bundled_skills_manifest(bundled_skills_dir)?;
    let source_skill_names = source_manifest
        .skills
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let installed_skill_names = collect_skill_directory_names(system_skills_dir)?;
    let persisted_manifest = load_persisted_bundled_skills_manifest(&manifest_path)?;
    let installed_manifest = match persisted_manifest.as_ref() {
        Some(manifest) => manifest.clone(),
        None => build_bundled_skills_manifest(system_skills_dir)?,
    };

    if installed_manifest == source_manifest && installed_skill_names == source_skill_names {
        if persisted_manifest.is_none() {
            write_manifest_atomically(&manifest_path, &source_manifest)?;
        }
        return Ok(());
    }

    for obsolete_skill in installed_skill_names.difference(&source_skill_names) {
        std::fs::remove_dir_all(system_skills_dir.join(obsolete_skill)).map_err(|error| {
            format!(
                "Failed to delete obsolete managed skill {}: {}",
                obsolete_skill, error
            )
        })?;
    }

    for (skill_name, source_hash) in &source_manifest.skills {
        let target_dir = system_skills_dir.join(skill_name);
        let needs_update =
            installed_manifest.skills.get(skill_name) != Some(source_hash) || !target_dir.exists();

        if needs_update {
            replace_skill_directory_atomically(&bundled_skills_dir.join(skill_name), &target_dir)?;
        }
    }

    write_manifest_atomically(&manifest_path, &source_manifest)?;
    Ok(())
}

/// Move legacy user-managed skills from AppData/skills into AppData/user_skills.
/// Old `.bundled_skill` snapshot entries are always discarded so the bundled
/// system snapshot can be rebuilt as an exact copy of the current bundle.
pub fn remove_legacy_skills_dir_if_empty(
    legacy_skills_dir: &Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    if !legacy_skills_dir.exists() {
        return Ok(false);
    }

    let mut entries = std::fs::read_dir(legacy_skills_dir)?;
    if entries.next().is_some() {
        return Ok(false);
    }

    std::fs::remove_dir(legacy_skills_dir)?;
    Ok(true)
}

async fn migrate_legacy_skills_to_managed_storage(
) -> Result<LegacySkillMigrationSummary, Box<dyn std::error::Error>> {
    use std::fs;

    let base_data_dir = crate::session::get_session_manager()
        .map_err(std::io::Error::other)?
        .get_base_data_dir()
        .clone();
    let legacy_skills_dir = base_data_dir.join(LEGACY_SYSTEM_SKILLS_DIR_NAME);
    let user_skills_dir = base_data_dir.join(USER_SKILLS_DIR_NAME);

    if !legacy_skills_dir.exists() {
        return Ok(LegacySkillMigrationSummary::default());
    }

    fs::create_dir_all(&user_skills_dir)?;
    let mut summary = LegacySkillMigrationSummary::default();

    for entry in fs::read_dir(&legacy_skills_dir)? {
        let entry = entry?;
        let skill_name = entry.file_name();
        let legacy_skill_dir = entry.path();
        if !legacy_skill_dir.is_dir() {
            continue;
        }

        let migration_action = classify_legacy_skill_for_managed_storage(&legacy_skill_dir)?;

        let target_skill_dir = user_skills_dir.join(&skill_name);
        if migration_action == LegacySkillMigrationAction::DeleteLegacyCopy {
            log::info!(
                "🧹 Removing legacy bundled skill snapshot entry: {:?}",
                skill_name
            );
            fs::remove_dir_all(&legacy_skill_dir)?;
            summary.removed_legacy_snapshot_entries += 1;
            continue;
        }

        if target_skill_dir.exists() {
            log::info!(
                "⏭️  Managed user skill already exists, leaving legacy copy untouched: {:?}",
                skill_name
            );
            summary.skipped_existing_user_skills += 1;
            continue;
        }

        match fs::rename(&legacy_skill_dir, &target_skill_dir) {
            Ok(_) => {
                log::info!(
                    "📦 Migrated legacy skill into managed storage: {:?}",
                    skill_name
                );
                summary.migrated_user_skills += 1;
            }
            Err(error) => {
                log::warn!(
                    "Failed to move legacy skill {:?} directly ({}), copying instead",
                    skill_name,
                    error
                );
                copy_dir_recursive(&legacy_skill_dir, &target_skill_dir)?;
                fs::remove_dir_all(&legacy_skill_dir)?;
                summary.migrated_user_skills += 1;
            }
        }
    }

    summary.removed_legacy_root_dir = remove_legacy_skills_dir_if_empty(&legacy_skills_dir)?;

    Ok(summary)
}

pub(crate) fn spawn_managed_skills_startup_work(
    bundled_skills_dir: PathBuf,
    system_skills_dir: PathBuf,
) {
    crate::state::begin_managed_skills_sync();

    tauri::async_runtime::spawn(async move {
        if let Err(e) = async {
            match migrate_legacy_skills_to_managed_storage().await {
                Ok(summary) if summary.is_noop() => {
                    log::debug!("No legacy skills migration work was needed");
                }
                Ok(summary) => {
                    info!(
                        "✅ Legacy skills migration completed (removed snapshots: {}, migrated user skills: {}, skipped existing user skills: {}, removed legacy root: {})",
                        summary.removed_legacy_snapshot_entries,
                        summary.migrated_user_skills,
                        summary.skipped_existing_user_skills,
                        summary.removed_legacy_root_dir
                    );
                }
                Err(error) => {
                    log::warn!("⚠️  Failed to migrate legacy skills: {}", error);
                }
            }

            if let Err(error) =
                sync_managed_system_skills_snapshot(&bundled_skills_dir, &system_skills_dir)
            {
                log::warn!("⚠️  Failed to sync managed system skills snapshot: {}", error);
            } else {
                info!("✅ Managed system skills snapshot synchronized");
            }

            crate::services::skill_service::invalidate_skill_scan_cache();

            if let Err(error) = crate::services::skill_service::prewarm_managed_skill_scans().await
            {
                log::warn!("⚠️  Failed to prewarm managed skill cache: {}", error);
            }

            Ok::<(), String>(())
        }
        .await
        {
            log::warn!("⚠️  Managed skills startup preparation finished with warnings: {}", e);
        }

        crate::state::complete_managed_skills_sync();
    });
}
