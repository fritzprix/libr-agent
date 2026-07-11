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
pub(crate) struct BundledSkillsManifest {
    pub(crate) schema_version: u32,
    pub(crate) skills: BTreeMap<String, String>,
}

pub(crate) fn collect_skill_directory_names(skills_dir: &Path) -> Result<BTreeSet<String>, String> {
    if !skills_dir.exists() {
        return Ok(BTreeSet::new());
    }

    let mut names = BTreeSet::new();
    for entry in std::fs::read_dir(skills_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.path().is_dir() {
            continue;
        }

        names.insert(entry.file_name().to_string_lossy().to_string());
    }

    Ok(names)
}

fn normalized_relative_path(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|error| format!("Failed to strip prefix for {}: {}", path.display(), error))?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("/"))
}

pub(crate) fn hash_skill_directory(skill_dir: &Path) -> Result<String, String> {
    let mut files = walkdir::WalkDir::new(skill_dir)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to scan {}: {}", skill_dir.display(), error))?
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
        hasher.update(
            std::fs::read(&full_path)
                .map_err(|error| format!("Failed to read {}: {}", full_path.display(), error))?,
        );
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
        .map_err(|error| format!("Failed to read {}: {}", skills_dir.display(), error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read directory entry: {}", error))?;
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
        .map_err(|error| format!("Failed to read {}: {}", skills_dir.display(), error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read directory entry: {}", error))?;
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

    let payload = std::fs::read(manifest_path)
        .map_err(|error| format!("Failed to read {}: {}", manifest_path.display(), error))?;
    let manifest = match serde_json::from_slice::<BundledSkillsManifest>(&payload) {
        Ok(manifest) => manifest,
        Err(_) => return Ok(None),
    };

    if manifest.schema_version != MANAGED_SYSTEM_SKILLS_MANIFEST_SCHEMA_VERSION {
        return Ok(None);
    }

    Ok(Some(manifest))
}

pub(crate) fn write_manifest_atomically(
    manifest_path: &Path,
    manifest: &BundledSkillsManifest,
) -> Result<(), String> {
    let payload = serde_json::to_vec_pretty(manifest)
        .map_err(|error| format!("Failed to serialize manifest: {}", error))?;
    let temp_path = manifest_path.with_extension("json.tmp");
    let backup_path = manifest_path.with_extension("json.bak");

    std::fs::write(&temp_path, payload)
        .map_err(|error| format!("Failed to write {}: {}", temp_path.display(), error))?;

    if backup_path.exists() {
        std::fs::remove_file(&backup_path).map_err(|error| {
            format!(
                "Failed to clear backup manifest {}: {}",
                backup_path.display(),
                error
            )
        })?;
    }

    let moved_existing_to_backup = if manifest_path.exists() {
        std::fs::rename(manifest_path, &backup_path).map_err(|error| {
            format!(
                "Failed to move existing manifest {} aside: {}",
                manifest_path.display(),
                error
            )
        })?;
        true
    } else {
        false
    };

    match std::fs::rename(&temp_path, manifest_path) {
        Ok(()) => {
            if moved_existing_to_backup {
                std::fs::remove_file(&backup_path).map_err(|error| {
                    format!(
                        "Failed to remove backup manifest {}: {}",
                        backup_path.display(),
                        error
                    )
                })?;
            }
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::remove_file(&temp_path);
            if moved_existing_to_backup && !manifest_path.exists() {
                let _ = std::fs::rename(&backup_path, manifest_path);
            }
            Err(format!(
                "Failed to finalize manifest {}: {}",
                manifest_path.display(),
                error
            ))
        }
    }
}

pub(crate) fn replace_skill_directory_atomically(
    source_dir: &Path,
    target_dir: &Path,
) -> Result<(), String> {
    let parent = target_dir
        .parent()
        .ok_or_else(|| format!("Invalid managed skill path: {}", target_dir.display()))?;
    let skill_name = target_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            format!(
                "Invalid managed skill directory name: {}",
                target_dir.display()
            )
        })?;

    let temp_dir = parent.join(format!(".sync-tmp-{}", skill_name));
    let backup_dir = parent.join(format!(".sync-backup-{}", skill_name));

    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).map_err(|error| {
            format!("Failed to clear temp dir {}: {}", temp_dir.display(), error)
        })?;
    }
    if backup_dir.exists() {
        std::fs::remove_dir_all(&backup_dir).map_err(|error| {
            format!(
                "Failed to clear backup dir {}: {}",
                backup_dir.display(),
                error
            )
        })?;
    }

    copy_dir_recursive_path(source_dir, &temp_dir).map_err(|e| e.to_string())?;

    let moved_existing_to_backup = if target_dir.exists() {
        std::fs::rename(target_dir, &backup_dir).map_err(|error| {
            format!(
                "Failed to move existing managed skill {} aside: {}",
                target_dir.display(),
                error
            )
        })?;
        true
    } else {
        false
    };

    match std::fs::rename(&temp_dir, target_dir) {
        Ok(()) => {
            std::fs::write(target_dir.join(BUNDLED_SKILL_MARKER), b"bundled\n").map_err(
                |error| {
                    format!(
                        "Failed to write bundled marker {}: {}",
                        target_dir.join(BUNDLED_SKILL_MARKER).display(),
                        error
                    )
                },
            )?;
            if moved_existing_to_backup {
                std::fs::remove_dir_all(&backup_dir).map_err(|error| {
                    format!(
                        "Failed to remove backup dir {}: {}",
                        backup_dir.display(),
                        error
                    )
                })?;
            }
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            if moved_existing_to_backup && !target_dir.exists() {
                let _ = std::fs::rename(&backup_dir, target_dir);
            }
            Err(format!(
                "Failed to activate managed skill {}: {}",
                target_dir.display(),
                error
            ))
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
