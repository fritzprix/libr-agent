use crate::services::skill_service::SKILL_FILE_NAME;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Marker file written into every bundled skill directory in AppData.
/// Used to distinguish bundled skills from user-created ones so that skills
/// removed from the bundle can be cleaned up automatically on the next launch.
pub(crate) const BUNDLED_SKILL_MARKER: &str = ".bundled_skill";
pub(crate) const ASSISTANT_BUNDLED_SKILLS_MANIFEST_FILE_NAME: &str =
    ".bundled_skills_manifest.json";
const MANAGED_SYSTEM_SKILLS_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundledSkillsManifest {
    pub schema_version: u32,
    pub skills: BTreeMap<String, String>,
}

pub(crate) fn collect_skill_directory_names(skills_dir: &Path) -> Result<BTreeSet<String>, String> {
    if !skills_dir.exists() {
        return Ok(BTreeSet::new());
    }

    let mut names = BTreeSet::new();
    for entry in std::fs::read_dir(skills_dir).map_err(|e| {
        log::warn!(
            "Failed to read skills directory {}: {}",
            skills_dir.display(),
            e
        );
        "Failed to read skills directory".to_string()
    })? {
        let entry = entry.map_err(|e| {
            log::warn!("Failed to read directory entry: {}", e);
            "Failed to read directory entry".to_string()
        })?;
        if !entry.path().is_dir() {
            continue;
        }

        names.insert(entry.file_name().to_string_lossy().to_string());
    }

    Ok(names)
}

fn normalized_relative_path(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(root).map_err(|error| {
        log::warn!("Failed to strip prefix for {}: {}", path.display(), error);
        "Failed to resolve relative path".to_string()
    })?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("/"))
}

pub fn hash_skill_directory(skill_dir: &Path) -> Result<String, String> {
    let mut files = walkdir::WalkDir::new(skill_dir)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            log::warn!(
                "Failed to scan skill directory {}: {}",
                skill_dir.display(),
                error
            );
            "Failed to scan skill directory".to_string()
        })?
        .into_iter()
        .filter(|entry| {
            entry.file_type().is_file()
                && entry.file_name().to_string_lossy().as_ref() != BUNDLED_SKILL_MARKER
        })
        .map(|entry| {
            let path = entry.into_path();
            let normalized = normalized_relative_path(skill_dir, &path)?;
            Ok((normalized, path))
        })
        .collect::<Result<Vec<_>, String>>()?;

    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut hasher = Sha256::new();
    for (relative_path, full_path) in files {
        hasher.update(relative_path.as_bytes());
        hasher.update([0]);
        hasher.update(std::fs::read(&full_path).map_err(|error| {
            log::warn!("Failed to read file {}: {}", full_path.display(), error);
            "Failed to read skill file content".to_string()
        })?);
        hasher.update([0]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn build_bundled_skills_manifest(
    skills_dir: &Path,
) -> Result<BundledSkillsManifest, String> {
    let mut manifest = BundledSkillsManifest {
        schema_version: MANAGED_SYSTEM_SKILLS_MANIFEST_SCHEMA_VERSION,
        skills: BTreeMap::new(),
    };

    if !skills_dir.exists() {
        return Ok(manifest);
    }

    let mut entries = std::fs::read_dir(skills_dir)
        .map_err(|error| {
            log::warn!(
                "Failed to read skills directory {}: {}",
                skills_dir.display(),
                error
            );
            "Failed to read skills directory".to_string()
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            log::warn!("Failed to read directory entry: {}", error);
            "Failed to read directory entry".to_string()
        })?;
    entries.sort_by_key(|entry| entry.file_name().to_string_lossy().to_string());

    for entry in entries {
        let skill_path = entry.path();
        if !skill_path.is_dir() {
            continue;
        }

        if !skill_path.join(SKILL_FILE_NAME).is_file() {
            continue;
        }

        let skill_dir_name = entry.file_name().to_string_lossy().to_string();
        manifest
            .skills
            .insert(skill_dir_name, hash_skill_directory(&skill_path)?);
    }

    Ok(manifest)
}

pub(crate) fn build_marked_bundled_skills_manifest(
    skills_dir: &Path,
) -> Result<BundledSkillsManifest, String> {
    let mut manifest = BundledSkillsManifest {
        schema_version: MANAGED_SYSTEM_SKILLS_MANIFEST_SCHEMA_VERSION,
        skills: BTreeMap::new(),
    };

    if !skills_dir.exists() {
        return Ok(manifest);
    }

    let mut entries = std::fs::read_dir(skills_dir)
        .map_err(|error| {
            log::warn!(
                "Failed to read skills directory {}: {}",
                skills_dir.display(),
                error
            );
            "Failed to read skills directory".to_string()
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            log::warn!("Failed to read directory entry: {}", error);
            "Failed to read directory entry".to_string()
        })?;
    entries.sort_by_key(|entry| entry.file_name().to_string_lossy().to_string());

    for entry in entries {
        let skill_path = entry.path();
        if !skill_path.is_dir() {
            continue;
        }

        if !skill_path.join(BUNDLED_SKILL_MARKER).is_file()
            || !skill_path.join(SKILL_FILE_NAME).is_file()
        {
            continue;
        }

        let skill_dir_name = entry.file_name().to_string_lossy().to_string();
        manifest
            .skills
            .insert(skill_dir_name, hash_skill_directory(&skill_path)?);
    }

    Ok(manifest)
}

pub(crate) fn load_persisted_bundled_skills_manifest(
    manifest_path: &Path,
) -> Result<Option<BundledSkillsManifest>, String> {
    if !manifest_path.exists() {
        return Ok(None);
    }

    let payload = std::fs::read(manifest_path).map_err(|error| {
        log::warn!(
            "Failed to read manifest {}: {}",
            manifest_path.display(),
            error
        );
        "Failed to read persisted skills manifest".to_string()
    })?;
    let manifest = match serde_json::from_slice::<BundledSkillsManifest>(&payload) {
        Ok(manifest) => manifest,
        Err(_) => return Ok(None),
    };

    if manifest.schema_version != MANAGED_SYSTEM_SKILLS_MANIFEST_SCHEMA_VERSION {
        return Ok(None);
    }

    Ok(Some(manifest))
}

pub fn write_manifest_atomically(
    manifest_path: &Path,
    manifest: &BundledSkillsManifest,
) -> Result<(), String> {
    let payload = serde_json::to_vec_pretty(manifest)
        .map_err(|error| format!("Failed to serialize manifest: {}", error))?;
    let temp_path = manifest_path.with_extension("json.tmp");
    let backup_path = manifest_path.with_extension("json.bak");

    std::fs::write(&temp_path, payload).map_err(|error| {
        log::warn!(
            "Failed to write temp manifest {}: {}",
            temp_path.display(),
            error
        );
        "Failed to write temporary manifest file".to_string()
    })?;

    if backup_path.exists() {
        std::fs::remove_file(&backup_path).map_err(|error| {
            log::warn!(
                "Failed to clear backup manifest {}: {}",
                backup_path.display(),
                error
            );
            "Failed to clear existing backup manifest".to_string()
        })?;
    }

    let moved_existing_to_backup = if manifest_path.exists() {
        std::fs::rename(manifest_path, &backup_path).map_err(|error| {
            log::warn!(
                "Failed to move existing manifest aside from {} to {}: {}",
                manifest_path.display(),
                backup_path.display(),
                error
            );
            "Failed to move existing manifest aside".to_string()
        })?;
        true
    } else {
        false
    };

    match std::fs::rename(&temp_path, manifest_path) {
        Ok(()) => {
            if moved_existing_to_backup {
                std::fs::remove_file(&backup_path).map_err(|error| {
                    log::warn!(
                        "Failed to remove backup manifest {}: {}",
                        backup_path.display(),
                        error
                    );
                    "Failed to remove backup manifest".to_string()
                })?;
            }
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::remove_file(&temp_path);
            if moved_existing_to_backup && !manifest_path.exists() {
                let _ = std::fs::rename(&backup_path, manifest_path);
            }
            log::warn!(
                "Failed to finalize manifest from {} to {}: {}",
                temp_path.display(),
                manifest_path.display(),
                error
            );
            Err("Failed to finalize manifest file".to_string())
        }
    }
}

pub fn replace_skill_directory_atomically(
    source_dir: &Path,
    target_dir: &Path,
) -> Result<(), String> {
    let parent = target_dir
        .parent()
        .ok_or_else(|| "Invalid managed skill path".to_string())?;
    let skill_name = target_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Invalid managed skill directory name".to_string())?;

    let temp_dir = parent.join(format!(".sync-tmp-{}", skill_name));
    let backup_dir = parent.join(format!(".sync-backup-{}", skill_name));

    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).map_err(|error| {
            log::warn!("Failed to clear temp dir {}: {}", temp_dir.display(), error);
            "Failed to clear temporary sync directory".to_string()
        })?;
    }
    if backup_dir.exists() {
        std::fs::remove_dir_all(&backup_dir).map_err(|error| {
            log::warn!(
                "Failed to clear backup dir {}: {}",
                backup_dir.display(),
                error
            );
            "Failed to clear existing backup directory".to_string()
        })?;
    }

    copy_dir_recursive_path(source_dir, &temp_dir).map_err(|e| {
        log::warn!(
            "Failed to copy dir from {} to {}: {}",
            source_dir.display(),
            temp_dir.display(),
            e
        );
        "Failed to copy skill directory".to_string()
    })?;

    let moved_existing_to_backup = if target_dir.exists() {
        std::fs::rename(target_dir, &backup_dir).map_err(|error| {
            log::warn!(
                "Failed to move existing managed skill aside from {} to {}: {}",
                target_dir.display(),
                backup_dir.display(),
                error
            );
            "Failed to move existing managed skill aside".to_string()
        })?;
        true
    } else {
        false
    };

    match std::fs::rename(&temp_dir, target_dir) {
        Ok(()) => {
            let marker_path = target_dir.join(BUNDLED_SKILL_MARKER);
            std::fs::write(&marker_path, b"bundled\n").map_err(|error| {
                log::warn!(
                    "Failed to write bundled marker at {}: {}",
                    marker_path.display(),
                    error
                );
                "Failed to write bundled skill marker".to_string()
            })?;
            if moved_existing_to_backup {
                std::fs::remove_dir_all(&backup_dir).map_err(|error| {
                    log::warn!(
                        "Failed to remove backup dir {}: {}",
                        backup_dir.display(),
                        error
                    );
                    "Failed to remove backup directory".to_string()
                })?;
            }
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            if moved_existing_to_backup && !target_dir.exists() {
                let _ = std::fs::rename(&backup_dir, target_dir);
            }
            log::warn!(
                "Failed to activate managed skill from {} to {}: {}",
                temp_dir.display(),
                target_dir.display(),
                error
            );
            Err("Failed to activate managed skill".to_string())
        }
    }
}

pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    copy_dir_recursive_path(src, dst)
}

fn copy_dir_recursive_path(src: &Path, dst: &Path) -> std::io::Result<()> {
    use std::fs;

    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}
